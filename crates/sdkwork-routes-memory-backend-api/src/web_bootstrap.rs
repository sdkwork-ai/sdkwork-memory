use std::sync::Arc;

use axum::Router;
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_memory_contract::MemoryBackendRequestContext;
use sdkwork_routes_memory_support::{
    harden_memory_web_framework_layer, memory_http_metrics, memory_web_auth_mode_from_env,
    parse_principal_u64, with_problem_correlation, MemoryWebAuthMode, ProductionFailClosedResolver,
};
use sdkwork_web_axum::{with_web_request_context, WebFrameworkLayer};
use sdkwork_web_core::{
    DefaultWebRequestContextResolver, DomainContextInjector, WebRequestContext,
    WebRequestContextProfile,
};

use crate::http_route_manifest::backend_route_manifest;
use crate::paths;

pub fn memory_backend_api_public_path_prefixes() -> Vec<String> {
    vec![paths::HEALTHZ.to_owned()]
}

pub fn memory_backend_api_prefixes() -> Vec<String> {
    vec![paths::PREFIX.to_owned()]
}

#[derive(Clone, Default)]
struct MemoryBackendContextInjector;

impl DomainContextInjector for MemoryBackendContextInjector {
    fn inject(&self, request: &mut axum::extract::Request, context: &WebRequestContext) {
        if let Some(backend_context) = memory_backend_context_from_web_request(context) {
            request.extensions_mut().insert(backend_context);
        }
    }
}

fn memory_backend_context_from_web_request(
    context: &WebRequestContext,
) -> Option<MemoryBackendRequestContext> {
    let principal = context.principal.as_ref()?;
    let tenant_id = parse_principal_u64(principal.tenant_id())?;
    let operator_id = parse_principal_u64(principal.user_id());
    Some(MemoryBackendRequestContext {
        tenant_id,
        operator_id,
    })
}

pub fn wrap_router_with_web_framework<S>(
    resolver: DefaultWebRequestContextResolver,
    router: Router<S>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    with_web_request_context(
        with_problem_correlation(router),
        build_memory_backend_api_framework_layer(resolver),
    )
    // Innermost body limit: this wins over the framework's own default and is
    // the single enforced bound (`SDKWORK_MEMORY_MAX_BODY_BYTES`).
    .layer(axum::extract::DefaultBodyLimit::max(
        sdkwork_routes_memory_support::memory_request_body_limit_bytes(),
    ))
}

pub fn wrap_router_with_iam_database_web_framework<S>(
    resolver: IamWebRequestContextResolver,
    router: Router<S>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    with_web_request_context(
        with_problem_correlation(router),
        build_memory_backend_api_framework_layer(resolver),
    )
    // Innermost body limit: this wins over the framework's own default and is
    // the single enforced bound (`SDKWORK_MEMORY_MAX_BODY_BYTES`).
    .layer(axum::extract::DefaultBodyLimit::max(
        sdkwork_routes_memory_support::memory_request_body_limit_bytes(),
    ))
}

fn build_memory_backend_api_framework_layer<R>(resolver: R) -> WebFrameworkLayer<R>
where
    R: sdkwork_web_core::WebRequestContextResolver + Clone,
{
    let route_manifest = backend_route_manifest();
    route_manifest
        .validate_public_path_prefixes(&memory_backend_api_public_path_prefixes())
        .expect("memory backend-api public prefixes must not cover protected manifest routes");

    let layer = WebFrameworkLayer::new(resolver)
        .with_profile(WebRequestContextProfile {
            backend_api_prefix: paths::PREFIX.to_owned(),
            public_path_prefixes: memory_backend_api_public_path_prefixes(),
            ..WebRequestContextProfile::default()
        })
        .with_route_manifest(route_manifest.clone())
        .with_domain_injector(Arc::new(MemoryBackendContextInjector))
        .with_metrics(memory_http_metrics());
    harden_memory_web_framework_layer(layer, route_manifest)
}

pub async fn wrap_router_with_web_framework_from_env<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    match memory_web_auth_mode_from_env().await {
        MemoryWebAuthMode::DevInline => {
            wrap_router_with_web_framework(DefaultWebRequestContextResolver::default(), router)
        }
        MemoryWebAuthMode::ProductionFailClosed => with_web_request_context(
            with_problem_correlation(router),
            build_memory_backend_api_framework_layer(ProductionFailClosedResolver),
        ),
        MemoryWebAuthMode::IamDatabase(resolver) => {
            wrap_router_with_iam_database_web_framework(*resolver, router)
        }
    }
}
