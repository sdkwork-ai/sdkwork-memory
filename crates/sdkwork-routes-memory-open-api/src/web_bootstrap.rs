use std::sync::Arc;

use axum::Router;
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_memory_contract::MemoryOpenApiRequestContext;
use sdkwork_routes_memory_support::{
    harden_memory_web_framework_layer, memory_http_metrics, memory_web_auth_mode_from_env,
    parse_principal_u64, with_problem_correlation, MemoryWebAuthMode, ProductionFailClosedResolver,
};
use sdkwork_web_axum::{with_web_request_context, WebFrameworkLayer};
use sdkwork_web_core::{DefaultWebRequestContextResolver, WebRequestContextProfile};

use crate::http_route_manifest::open_route_manifest;
use crate::paths;

pub fn memory_open_api_public_path_prefixes() -> Vec<String> {
    vec![paths::HEALTHZ.to_owned()]
}

/// Every path prefix this crate serves as an `open-api` surface.
///
/// The crate serves two open-api dialects on one router: the SDKWork Memory
/// authority under `/mem/v3/api`, and the vendor-compatibility `mem0-platform`
/// authority under `/v1`/`/v3` (`API_SPEC.md` section 4.5.2). Both are declared
/// `apiSurface: open-api` in the route manifest, so both must be classified that
/// way at runtime or `validate_route_auth_for_surfaces` rejects their `api-key`
/// auth profile.
pub fn memory_open_api_prefixes() -> Vec<String> {
    let mut prefixes = vec![paths::PREFIX.to_owned()];
    prefixes.extend(memory_open_api_external_protocol_prefixes());
    prefixes
}

/// The prefixes hosting the `mem0-platform` upstream protocol.
///
/// Declared to the framework as `external_protocol_prefixes`, which suspends the
/// client context-selector guard for them. mem0's request vocabulary includes
/// `user_id` and `app_id` as top-level body keys and query parameters — the
/// entity a memory is filed under, not a tenant selector — and the official
/// clients cannot be changed. See
/// `sdkwork_web_core::WebRequestContextProfile::external_protocol_prefixes`.
pub fn memory_open_api_external_protocol_prefixes() -> Vec<String> {
    paths::MEM0_PATH_PREFIXES
        .iter()
        .map(|prefix| prefix.trim_end_matches('/').to_owned())
        .collect()
}

#[derive(Clone, Default)]
struct MemoryOpenApiContextInjector;

impl sdkwork_web_core::DomainContextInjector for MemoryOpenApiContextInjector {
    fn inject(
        &self,
        request: &mut axum::extract::Request,
        context: &sdkwork_web_core::WebRequestContext,
    ) {
        if let Some(open_context) = memory_open_api_context_from_web_request(context) {
            request.extensions_mut().insert(open_context);
        }
    }
}

fn memory_open_api_context_from_web_request(
    context: &sdkwork_web_core::WebRequestContext,
) -> Option<MemoryOpenApiRequestContext> {
    let principal = context.principal.as_ref()?;
    let tenant_id = parse_principal_u64(principal.tenant_id())?;
    let actor_id = parse_principal_u64(principal.user_id());
    let credential_id = principal
        .api_key_id()
        .map(str::to_owned)
        .or_else(|| principal.session_id().map(str::to_owned))
        .unwrap_or_else(|| principal.user_id().to_owned());
    Some(MemoryOpenApiRequestContext {
        api_key_id: credential_id,
        tenant_id,
        actor_id,
        elevated_tenant_access: false,
    })
}

/// Build the framework layer for open-api routes.
/// Each route crate provides its own closure that configures the layer
/// with route-specific settings (context injector, manifest, profile).
fn build_open_api_framework_layer<R>(resolver: R) -> WebFrameworkLayer<R>
where
    R: sdkwork_web_core::WebRequestContextResolver,
{
    let route_manifest = open_route_manifest();
    route_manifest
        .validate_public_path_prefixes(&memory_open_api_public_path_prefixes())
        .expect("memory open-api public prefixes must not cover protected manifest routes");

    // `classify_api_surface` tests the gateway prefixes before the open-api
    // ones, and the framework default claims `/v1` for the gateway surface. That
    // default is a property of a *gateway* host, not of this service: every
    // route this crate serves under `/v1` is the mem0 compatibility authority,
    // declared `api-key` in the manifest. Left in place, `/v1/ping/` would
    // classify as gateway-api and the manifest validator would reject its
    // credential profile — so the list is emptied here rather than in the shared
    // default, which other services still need.
    let layer = WebFrameworkLayer::new(resolver)
        .with_profile(WebRequestContextProfile {
            open_api_prefixes: memory_open_api_prefixes(),
            public_path_prefixes: memory_open_api_public_path_prefixes(),
            gateway_api_prefixes: Vec::new(),
            external_protocol_prefixes: memory_open_api_external_protocol_prefixes(),
            ..WebRequestContextProfile::default()
        })
        .with_route_manifest(route_manifest.clone())
        .with_domain_injector(Arc::new(MemoryOpenApiContextInjector))
        .with_metrics(memory_http_metrics());
    harden_memory_web_framework_layer(layer, route_manifest)
}

/// Installs the mem0 wire adapters outside the web framework layer.
///
/// Both adapters have to straddle the framework layer — the credential bridge on
/// the way in, the media-type bridge on the way out — and `Router::layer` wraps,
/// so the layer applied last runs outermost. Order here is therefore:
///
/// ```text
/// request  → credential bridge → problem bridge → [ framework layer → routes ]
/// response ← credential bridge ← problem bridge ← [ framework layer ← routes ]
/// ```
///
/// * The framework validates credential headers during surface classification
///   (`SecurityPolicy::validate_route_auth_credentials`), so the credential
///   rewrite has to be in front of it — hence applied last.
/// * The framework's `ResponseIdentity` interceptor normalizes every 4xx/5xx
///   body it sees, so the media-type rewrite has to be behind it — hence applied
///   first, but still outside `with_web_request_context`.
///
/// The two are independent: neither order between *them* matters, only their
/// position relative to the framework layer.
fn with_mem0_wire_adapters<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(axum::middleware::from_fn(
            crate::mem0::mem0_problem_document_bridge,
        ))
        .layer(axum::middleware::from_fn(
            crate::mem0::mem0_credential_bridge,
        ))
}

/// Wrap router using the dev-inline web framework.
pub fn wrap_router_with_web_framework<S>(
    resolver: DefaultWebRequestContextResolver,
    router: Router<S>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    with_mem0_wire_adapters(with_web_request_context(
        with_problem_correlation(router),
        build_open_api_framework_layer(resolver),
    ))
    // Innermost body limit: this wins over the framework's own default and is
    // the single enforced bound (`SDKWORK_MEMORY_MAX_BODY_BYTES`).
    .layer(axum::extract::DefaultBodyLimit::max(
        sdkwork_routes_memory_support::memory_request_body_limit_bytes(),
    ))
}

/// Wrap router using the IAM database web framework.
pub fn wrap_router_with_iam_database_web_framework<S>(
    resolver: IamWebRequestContextResolver,
    router: Router<S>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    with_mem0_wire_adapters(with_web_request_context(
        with_problem_correlation(router),
        build_open_api_framework_layer(resolver),
    ))
    // Innermost body limit: this wins over the framework's own default and is
    // the single enforced bound (`SDKWORK_MEMORY_MAX_BODY_BYTES`).
    .layer(axum::extract::DefaultBodyLimit::max(
        sdkwork_routes_memory_support::memory_request_body_limit_bytes(),
    ))
}

/// Dispatch router wrapping based on configured auth mode.
pub async fn wrap_router_with_web_framework_from_env<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    match memory_web_auth_mode_from_env().await {
        MemoryWebAuthMode::DevInline => {
            wrap_router_with_web_framework(DefaultWebRequestContextResolver::default(), router)
        }
        MemoryWebAuthMode::ProductionFailClosed => {
            with_mem0_wire_adapters(with_web_request_context(
                with_problem_correlation(router),
                build_open_api_framework_layer(ProductionFailClosedResolver),
            ))
            // Same innermost body limit as the other auth modes: every mode
            // must enforce the single configured bound
            // (`SDKWORK_MEMORY_MAX_BODY_BYTES`), not the framework default.
            .layer(axum::extract::DefaultBodyLimit::max(
                sdkwork_routes_memory_support::memory_request_body_limit_bytes(),
            ))
        }
        MemoryWebAuthMode::IamDatabase(resolver) => {
            wrap_router_with_iam_database_web_framework(*resolver, router)
        }
    }
}
