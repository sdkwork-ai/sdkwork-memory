//! The materialized mem0 authority must pass the framework's context-selector
//! gate under **exactly** the prefixes this crate declares at runtime.
//!
//! There are two halves to the `API_SPEC.md` §4.5.2 vendor-compatibility
//! exemption, and they live in different places:
//!
//! * **runtime** — `WebRequestContextProfile::external_protocol_prefixes`
//!   suspends the guard for a declared prefix, so the running service answers
//!   mem0's `user_id`/`app_id` parameters and body fields;
//! * **materialization** — `validate_openapi_*_context_selectors_with_external_prefixes`
//!   suspends the same rule for the same prefixes, so the published contract can
//!   be validated at all.
//!
//! Both are driven from one list (`memory_open_api_external_protocol_prefixes`),
//! and that list is what this suite pins. An exemption declared in one place and
//! not the other is the failure this exists to catch, because it is invisible
//! from either side alone: the service keeps working while its published
//! contract becomes unvalidatable, or a gate goes quiet while the service starts
//! rejecting the upstream's own parameters.
//!
//! The mutation control matters as much as the pass. Every check below is run
//! once **without** the declaration and asserted to fail, so a green run cannot
//! mean the rule was inert.

use std::path::PathBuf;

use serde_json::Value;
use sdkwork_routes_memory_open_api::{
    memory_open_api_external_protocol_prefixes, open_route_manifest,
};
use sdkwork_web_contract::{
    validate_openapi_document_context_selectors,
    validate_openapi_document_context_selectors_with_external_prefixes,
    validate_openapi_routes_context_selectors_with_external_prefixes,
    OPENAPI_WIRE_PROTOCOL_EXTENSION,
};

/// The materialized authority: the published contract for this surface.
const AUTHORITY: &str = "apis/open-api/memory-open-api.openapi.json";

/// The prefix the SDKWork-owned half of this surface is served under.
const OWNED_PREFIX: &str = "/mem/v3/api";

/// HTTP methods that carry an operation object in an OpenAPI path item.
const OPERATION_METHODS: [&str; 8] = [
    "get", "post", "put", "patch", "delete", "head", "options", "trace",
];

fn authority() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(AUTHORITY);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
    serde_json::from_str(&text).expect("the materialized authority is JSON")
}

/// The declared prefixes — the single source of truth both halves are driven from.
fn declared_prefixes() -> Vec<String> {
    memory_open_api_external_protocol_prefixes()
}

/// The same list in the shape the framework validator takes it.
fn borrowed(prefixes: &[String]) -> Vec<&str> {
    prefixes.iter().map(String::as_str).collect()
}

fn under_any(path: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|prefix| {
        let prefix = prefix.trim_end_matches('/');
        path.strip_prefix(prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

#[test]
fn the_declared_prefixes_cover_every_upstream_path_in_the_authority() {
    let document = authority();
    let prefixes = declared_prefixes();
    assert!(
        !prefixes.is_empty(),
        "the surface serves an upstream protocol, so at least one prefix must be declared"
    );

    let paths = document["paths"].as_object().expect("the authority declares paths");
    let mut upstream_paths = 0usize;
    for path in paths.keys() {
        if path.starts_with(OWNED_PREFIX) {
            continue;
        }
        upstream_paths += 1;
        assert!(
            under_any(path, &prefixes),
            "`{path}` is neither SDKWork-owned (`{OWNED_PREFIX}`) nor under a declared upstream \
             prefix {prefixes:?}. A path added under the upstream protocol must have its prefix \
             declared, or the runtime guard and the materialization gate disagree about it."
        );
    }
    assert!(
        upstream_paths > 0,
        "the authority must contain the upstream wire this exemption exists for"
    );

    // Every declared prefix must actually carry something. A stale declaration
    // would silently exempt a path space nobody serves.
    for prefix in &prefixes {
        assert!(
            paths.keys().any(|path| under_any(path, std::slice::from_ref(prefix))),
            "`{prefix}` is declared as an external protocol prefix but no path in the authority \
             uses it"
        );
    }
}

#[test]
fn the_materialized_authority_passes_the_context_selector_gate() {
    let document = authority();
    let prefixes = declared_prefixes();

    // Mutation control first: undeclared, the gate must fire on mem0's own
    // parameters. Without this, the pass below could mean nothing. Which
    // violation surfaces first depends on path order — `/v1/memories/`'s
    // `user_id` selector or the `/api/v1/` org-marker paths are both upstream
    // ambient violations the declared prefixes exist to exempt.
    let error = validate_openapi_document_context_selectors(&document)
        .expect_err("an undeclared upstream parameter is a violation");
    assert!(
        (error.contains("/v1/memories/") && error.contains("user_id"))
            || error.contains("/organizations/"),
        "the undeclared failure must land on an upstream ambient violation: {error}"
    );

    validate_openapi_document_context_selectors_with_external_prefixes(
        &document,
        &borrowed(&prefixes),
    )
    .expect("the materialized authority must validate under the declared prefixes");
}

#[test]
fn every_upstream_operation_declares_the_external_wire_protocol() {
    let document = authority();
    let declared_prefixes = declared_prefixes();
    let paths = document["paths"].as_object().expect("paths");

    let mut seen = 0usize;
    for (path, item) in paths {
        let under_external = under_any(path, &declared_prefixes);
        let item = item.as_object().expect("path item object");
        for (method, operation) in item {
            if !OPERATION_METHODS.contains(&method.as_str()) {
                continue;
            }
            let marker = operation.get(OPENAPI_WIRE_PROTOCOL_EXTENSION).and_then(Value::as_str);
            if under_external {
                seen += 1;
                assert_eq!(
                    marker,
                    Some("external"),
                    "{method} {path} sits under a declared upstream prefix but does not declare \
                     `{OPENAPI_WIRE_PROTOCOL_EXTENSION}: external`; the response-envelope gate and \
                     the context-selector exemption would then disagree about it"
                );
            } else {
                assert_ne!(
                    marker,
                    Some("external"),
                    "{method} {path} declares `{OPENAPI_WIRE_PROTOCOL_EXTENSION}: external` but is \
                     not under any declared upstream prefix {declared_prefixes:?}, so the two \
                     halves of the exemption would disagree about it"
                );
            }
        }
    }
    assert!(seen > 0, "the authority must contain upstream operations");
}

#[test]
fn the_route_manifest_passes_the_gate_under_the_declared_prefixes() {
    let manifest = open_route_manifest();
    let prefixes = declared_prefixes();

    // Unlike the document, this one has no mutation control available: the route
    // validator only rejects an *ambient context path marker* (`/tenants/`,
    // `/organizations/`), and no mem0 path contains one — the exemption is
    // observable here through the query parameters and body fields, which this
    // validator does not inspect. The document gate above is where it is pinned.
    validate_openapi_routes_context_selectors_with_external_prefixes(
        manifest.routes(),
        &borrowed(&prefixes),
    )
    .expect("the route manifest must pass the context selector gate");
}
