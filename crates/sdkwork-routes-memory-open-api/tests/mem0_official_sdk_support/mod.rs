//! Everything the official-SDK acceptance drivers have in common.
//!
//! Two official clients speak the mem0 platform wire — the Python `mem0ai` and
//! the JavaScript `mem0ai`. They are separate codebases with separate request
//! shaping, separate option surfaces and separate error taxonomies, so each needs
//! its own driver; but wherever they overlap they must satisfy **one** set of
//! expectations. Keeping that set here, rather than in either driver's test, is
//! what makes "both clients agree" a structural property instead of a claim that
//! two files happen to be kept in sync by hand.
//!
//! Where the clients genuinely differ, the difference is *declared* by the caller
//! and asserted in that client's own test — never smoothed over with a tolerant
//! assertion that would hide a real divergence.
//!
//! The drivers report what they observed as JSON (prefixed with `RESULT_PREFIX`);
//! [`assert_wire_observations`] re-derives every claim from that raw observation
//! rather than trusting the driver's own verdict. Both must agree.
//!
//! This directory has no `main.rs`, so cargo does not build it as a test target;
//! each driver test pulls it in by `#[path]`.

use axum::Router;
use serde_json::Value;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::web_auth::memory_dev_api_key;
use sdkwork_routes_memory_open_api::{build_router_with_open_api, wrap_router_with_web_framework};
use sdkwork_web_core::DefaultWebRequestContextResolver;

pub const CONVERSATION: &str = "The operator prefers concise bullet-point summaries over prose.";
pub const UPDATED_TEXT: &str = "The operator prefers concise bullet-point summaries, not prose.";

/// Two subjects for the batch path. The updated forms differ from the originals,
/// which is what makes "the batch really wrote" observable after the fact — and
/// what lets the assertions assert the *expected* text rather than merely echoing
/// what the driver claims it sent.
pub const BATCH_FIRST: &str = "The staging database password rotates every ninety days.";
pub const BATCH_SECOND: &str = "The design review happens on the first Wednesday of the month.";
pub const BATCH_FIRST_UPDATED: &str = "The staging database password rotates every sixty days.";
pub const BATCH_SECOND_UPDATED: &str = "The design review happens on the first Thursday of the month.";

/// A `memory_id` that names nothing. A valid unsigned integer, so it is rejected
/// for being *absent* rather than for being malformed — the two failures are
/// different code paths and only the first is being exercised here.
pub const UNKNOWN_ID: &str = "999999999999999999";

pub const EXPECTED_TENANT: &str = "100001";
pub const RESULT_PREFIX: &str = "MEM0_E2E_RESULT ";

/// What one client's run is expected to have attempted.
///
/// The counts exist because a run classifies probes that are recorded in several
/// different blocks — the filter/paging refusals, the feedback withdrawal, the
/// post-delete read-backs. Taking a summary from one list's length under-reports
/// the coverage, and a later regression pass reads that number as ground truth.
/// Declaring the expected totals here forces each driver to account for every
/// probe it performs.
pub struct ClientExpectations<'a> {
    /// Refusal labels recorded in the filter/paging block, in call order.
    pub refusals: &'a [&'a str],
    /// Every classified 501 the run performs, including the ones recorded in
    /// their own blocks (such as the feedback withdrawal).
    pub refusals_total: u64,
    /// The boundary rejections, in call order: `(call, status, client type)`.
    pub rejections: &'a [(&'a str, &'a str, &'a str)],
    /// Every classified 4xx the run performs, including the post-delete
    /// read-backs.
    pub rejections_total: u64,
}

/// The raw claim-form credential — **without** an authorization scheme.
///
/// `MemoryClient` composes the header itself (`Authorization: Token {api_key}`),
/// so passing a value that already carries the scheme sends `Token Token
/// <claims>`. The credential bridge splits on the first space, so that arrives
/// at the claim parser as a key literally named `Token api_key_id` and fails as
/// a *missing claim* — a 401 whose message names the wrong thing. The published
/// contract is a bare key; give the client that.
pub fn mem0_api_key() -> String {
    memory_dev_api_key("2001", "mem0-compat-key")
}

pub async fn build_app() -> Router {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    wrap_router_with_web_framework(
        DefaultWebRequestContextResolver::default(),
        build_router_with_open_api(OpenMemoryService::new(store)),
    )
}

pub fn parse_result(stdout: &str) -> Value {
    let line = stdout
        .lines()
        .rev()
        .find(|line| line.starts_with(RESULT_PREFIX))
        .unwrap_or_else(|| {
            panic!("the driver printed no `{RESULT_PREFIX}` line; stdout was:\n{stdout}")
        });
    serde_json::from_str(line.trim_start_matches(RESULT_PREFIX))
        .unwrap_or_else(|error| panic!("the driver's result line is not JSON ({error}): {line}"))
}

/// Every expectation the two official clients share, re-derived from the raw
/// observation. Returns the observed SDK version so the caller can print it.
///
/// `label` names the client in failure messages, so a red assertion immediately
/// says *which* SDK disagreed with the surface.
pub fn assert_wire_observations(
    result: &Value,
    stderr: &str,
    label: &str,
    expect: &ClientExpectations<'_>,
) -> String {
    let observed = &result["observed"];

    assert_eq!(
        result["ok"], true,
        "[{label}] the driver reported failures: {}\n{}",
        result["failures"], stderr
    );
    assert_eq!(result["failures"], serde_json::json!([]), "[{label}] {stderr}");

    // Provenance: a run against a vendored or stale copy must not be mistakable
    // for a run against the published package.
    let sdk = &result["sdk"];
    assert_eq!(sdk["package"], "mem0ai", "[{label}] unexpected SDK package: {sdk}");
    let version = sdk["version"].as_str().unwrap_or_default();
    assert!(
        !version.is_empty(),
        "[{label}] the driver could not establish the SDK version: {sdk}"
    );
    let module = sdk["module"].as_str().unwrap_or_default();
    assert!(
        !module.contains("external") && !module.contains("mem0-ts"),
        "[{label}] the driver imported an in-repo copy, not the installed package: {module}"
    );

    // 1. Both clients resolve their identity through `GET /v1/ping/`, so a
    //    failure here is visible in the tenant they report.
    assert_eq!(observed["ping"]["org_id"], EXPECTED_TENANT, "[{label}] {observed}");
    assert_eq!(observed["ping"]["project_id"], EXPECTED_TENANT, "[{label}] {observed}");
    // Both drivers also capture the raw body, because only one of the two
    // clients looks at this field at all: the JavaScript `ping()` throws unless
    // it reads exactly `"ok"`, while the Python validator never reads it. Only
    // one client can observe the requirement, so it is asserted from the raw
    // body for both rather than left to depend on which run was written first.
    assert_eq!(
        observed["ping"]["status"], "ok",
        "[{label}] `status` is part of this surface's contract with the \
         JavaScript client: {observed}"
    );

    // 2. `POST /v3/memories/add/`, with `user_id` at the **top level** of the
    //    body — the exact shape the framework's context-selector guard rejects
    //    unless the external protocol prefix is declared.
    let memory_id = observed["add"]["id"].as_str().expect("add returned an id");
    assert!(!memory_id.is_empty(), "[{label}] {observed}");
    assert_eq!(observed["add"]["event"], "ADD", "[{label}] {observed}");
    assert_eq!(observed["add"]["memory"], CONVERSATION, "[{label}] {observed}");
    assert_eq!(observed["add"]["user_id"], "alice", "[{label}] {observed}");
    assert_eq!(observed["add"]["agent_id"], "planner", "[{label}] {observed}");
    assert_eq!(
        observed["add"]["metadata"]["topic"], "formatting",
        "[{label}] {observed}"
    );
    let hash = observed["add"]["hash"].as_str().unwrap_or_default();
    assert_eq!(hash.len(), 64, "[{label}] `hash` must be a hex digest: {observed}");

    // 3. `GET /v1/memories/{memory_id}/`
    assert_eq!(observed["get"]["id"], memory_id, "[{label}] {observed}");
    assert_eq!(observed["get"]["memory"], CONVERSATION, "[{label}] {observed}");

    // 4. `POST /v3/memories/search/` with `filters` nested — which the guard must
    //    *not* object to, because it only inspects top-level keys.
    let ids = observed["search"]["ids"]
        .as_array()
        .expect("search reports ids");
    assert!(
        ids.iter().any(|id| id.as_str() == Some(memory_id)),
        "[{label}] the memory must be retrievable by its own text: {observed}"
    );

    // 5. `PUT /v1/memories/{memory_id}/`
    assert_eq!(observed["update"]["id"], memory_id, "[{label}] {observed}");
    assert_eq!(observed["update"]["memory"], UPDATED_TEXT, "[{label}] {observed}");

    // 6. `GET /v1/memories/{memory_id}/history/` — a bare JSON array, which is
    //    what both clients' `history` is typed to return.
    let events = observed["history"]["events"]
        .as_array()
        .expect("history reports events");
    let events: Vec<&str> = events.iter().filter_map(Value::as_str).collect();
    assert!(events.contains(&"ADD"), "[{label}] history: {events:?}");
    assert!(events.contains(&"UPDATE"), "[{label}] history: {events:?}");
    assert!(
        events
            .iter()
            .all(|event| ["ADD", "UPDATE", "DELETE"].contains(event)),
        "[{label}] mem0 declares `event` a closed enum: {events:?}"
    );

    // 7. `GET /v1/entities/`
    let scopes = observed["users"]["scopes"]
        .as_array()
        .expect("users reports scopes");
    assert!(
        scopes.iter().any(|scope| {
            scope["type"] == "user" && scope["name"] == "alice" && scope["owner"] == EXPECTED_TENANT
        }),
        "[{label}] the addressed user scope must be listed: {scopes:?}"
    );

    // 8. `DELETE /v1/memories/{memory_id}/`, then the read that proves it took.
    assert_eq!(
        observed["delete"]["message"], "Memory deleted successfully",
        "[{label}] {observed}"
    );
    let after_delete = &observed["get_after_delete"]["error"];
    assert_eq!(
        after_delete["type"], "MemoryNotFoundError",
        "[{label}] the client maps our 404 onto its own type: {after_delete}"
    );
    assert_eq!(after_delete["error_code"], "HTTP_404", "[{label}] {after_delete}");
    // The surface's own wording must reach the client. *How* each client hands it
    // over is the client's business, and the two genuinely differ — measured, not
    // assumed: the Python client parses the body's `detail` and passes it through
    // bare, while the JavaScript client keeps the raw body as its `message`. What
    // this surface owes both of them is that its wording survives the framework's
    // response interceptor; the framework's status-derived "Not found" would mean
    // the media-type bridge has regressed. The Python client's exact bare form is
    // pinned in the Python test, so nothing is lost by asserting containment here.
    assert!(
        after_delete["message"]
            .as_str()
            .is_some_and(|message| message.contains("memory not found")),
        "[{label}] the failure must reach the client in this surface's own words: {after_delete}"
    );

    // 9. Every refusal must arrive as a readable, typed failure — never as a 2xx.
    let refusals = observed["refusals"]
        .as_array()
        .expect("refusals are recorded");
    assert_eq!(
        refusals.len(),
        expect.refusals.len(),
        "[{label}] expected refusals {expect:?} but observed {refusals:?}",
        expect = expect.refusals
    );
    for (refusal, call) in refusals.iter().zip(expect.refusals) {
        assert_eq!(refusal["call"], *call, "[{label}] {refusals:?}");
        assert!(
            refusal.get("accepted").is_none(),
            "[{label}] `{call}` was answered instead of refused: {refusal}"
        );
        let error = &refusal["error"];
        assert_eq!(error["error_code"], "HTTP_501", "[{label}] `{call}`: {error}");
        let message = error["message"].as_str().unwrap_or_default();
        assert!(
            message.len() > 20,
            "[{label}] `{call}` must explain itself, not just fail: {error}"
        );
    }

    // 10. The bulk sweep really empties the space.
    let sweep = observed["delete_all"]["message"]
        .as_str()
        .expect("delete_all is acknowledged");
    assert!(
        sweep.ends_with("memories deleted successfully"),
        "[{label}] unexpected sweep acknowledgement: {sweep}"
    );
    assert_eq!(
        observed["list_after_sweep"]["count"], 0,
        "[{label}] the space must be empty after the sweep: {observed}"
    );

    // --- the operations added for the second compatibility pass ---------------

    // 11. `POST /v1/feedback/`. The canonical record accepts a comment but does
    //     not read it back, so `feedback_reason` must come home null. Echoing the
    //     request would claim a round trip this surface cannot prove.
    let feedback_id = observed["feedback"]["id"].as_str().unwrap_or_default();
    assert!(!feedback_id.is_empty(), "[{label}] {observed}");
    assert_eq!(observed["feedback"]["feedback"], "POSITIVE", "[{label}] {observed}");
    assert!(
        observed["feedback"]["feedback_reason"].is_null(),
        "[{label}] feedback_reason is not persisted or read back, so reporting a \
         value would be an unprovable echo: {observed}"
    );

    // 12. A withdrawal is refused, not silently answered. Upstream reads an
    //     omitted `feedback` as *clearing* the feedback; the canonical record is
    //     only ever written, never cleared, so a 2xx would report a mutation that
    //     did not happen.
    let withdrawal = &observed["feedback_withdrawal"];
    assert!(
        withdrawal.get("accepted").is_none(),
        "[{label}] a withdrawal must not be answered: {withdrawal}"
    );
    assert_eq!(withdrawal["error"]["error_code"], "HTTP_501", "[{label}] {withdrawal}");
    // 501 is outside either client's status table, so it lands on the base class
    // — still a readable, catchable failure rather than a raw transport error.
    assert_eq!(withdrawal["error"]["type"], "MemoryError", "[{label}] {withdrawal}");
    assert!(
        withdrawal["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("feedback withdrawal")),
        "[{label}] the refusal must name the operation: {withdrawal}"
    );

    // 13. `PUT /v1/batch/`. Upstream's 200 carries only a count in prose and no
    //     per-item channel, so the count is the *only* thing the response can say
    //     — the writes themselves have to be proved by reading both records back.
    assert_eq!(
        observed["batch_update"]["message"], "Successfully updated 2 memories",
        "[{label}] {observed}"
    );
    assert_eq!(
        observed["batch_update"]["first"], BATCH_FIRST_UPDATED,
        "[{label}] {observed}"
    );
    assert_eq!(
        observed["batch_update"]["second"], BATCH_SECOND_UPDATED,
        "[{label}] {observed}"
    );

    // 14. Each boundary must be refused as a whole, for the stated reason. The
    //     status is not incidental: 404 is what both clients turn into
    //     `MemoryNotFoundError`, and 400 into `ValidationError`.
    let rejections = observed["rejections"]
        .as_array()
        .expect("rejections are recorded");
    assert_eq!(rejections.len(), expect.rejections.len(), "[{label}] {rejections:?}");
    for (rejection, (call, code, kind)) in rejections.iter().zip(expect.rejections) {
        assert_eq!(rejection["call"], *call, "[{label}] {rejections:?}");
        assert!(
            rejection.get("accepted").is_none(),
            "[{label}] `{call}` was answered instead of refused: {rejection}"
        );
        let error = &rejection["error"];
        assert_eq!(error["error_code"], *code, "[{label}] `{call}`: {error}");
        assert_eq!(error["type"], *kind, "[{label}] `{call}`: {error}");
    }
    // The pre-flight is only worth anything if it stopped the whole batch: the
    // resolvable entry beside the unknown id must still hold its previous text.
    // Containment rather than equality for the same measured reason as above.
    assert!(
        rejections[0]["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains(&format!("memory {UNKNOWN_ID} not found"))),
        "[{label}] the 404 must name the entry it could not resolve, and must \
         arrive in this surface's own words: {rejections:?}"
    );

    // 15. `DELETE /v1/batch/`, then the reads that prove both removals took.
    assert_eq!(
        observed["batch_delete"]["message"], "Successfully deleted 2 memories",
        "[{label}] {observed}"
    );
    let proof = observed["batch_delete_proof"]
        .as_array()
        .expect("the post-delete reads are recorded");
    assert_eq!(proof.len(), 2, "[{label}] {proof:?}");
    for entry in proof {
        assert!(
            entry.get("accepted").is_none(),
            "[{label}] the record must be gone: {entry}"
        );
        assert_eq!(entry["error"]["error_code"], "HTTP_404", "[{label}] {entry}");
        assert_eq!(entry["error"]["type"], "MemoryNotFoundError", "[{label}] {entry}");
        assert!(
            entry["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("memory not found")),
            "[{label}] the surface's wording must reach the client: {entry}"
        );
    }

    // 16. The classified totals must describe what the run actually performed,
    //     not the length of one named list. The named lists are grouped by *why*
    //     a call cannot be honoured (filters, batch payload), so the
    //     feedback-withdrawal refusal and the two post-delete read-backs sit in
    //     their own blocks. A summary built from a list length under-reports the
    //     coverage — and a later regression pass reads that number as ground
    //     truth, so it must be the whole count.
    assert_eq!(
        result["refusals_total"].as_u64(),
        Some(expect.refusals_total),
        "[{label}] the 501 probes performed must equal the declared total {}",
        expect.refusals_total
    );
    assert_eq!(
        result["rejections_total"].as_u64(),
        Some(expect.rejections_total),
        "[{label}] the 4xx probes performed must equal the declared total {}",
        expect.rejections_total
    );

    version.to_owned()
}
