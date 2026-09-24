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

pub fn memory_open_api_prefixes() -> Vec<String> {
    vec![paths::PREFIX.to_owned()]
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

    let layer = WebFrameworkLayer::new(resolver)
        .with_profile(WebRequestContextProfile {
            open_api_prefixes: memory_open_api_prefixes(),
            public_path_prefixes: memory_open_api_public_path_prefixes(),
            ..WebRequestContextProfile::default()
        })
        .with_route_manifest(route_manifest.clone())
        .with_domain_injector(Arc::new(MemoryOpenApiContextInjector))
        .with_metrics(memory_http_metrics());
    harden_memory_web_framework_layer(layer, route_manifest)
}

/// Wrap router using the dev-inline web framework.
pub fn wrap_router_with_web_framework<S>(
    resolver: DefaultWebRequestContextResolver,
    router: Router<S>,
) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    with_web_request_context(
        with_problem_correlation(router),
        build_open_api_framework_layer(resolver),
    )
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
    with_web_request_context(
        with_problem_correlation(router),
        build_open_api_framework_layer(resolver),
    )
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
        MemoryWebAuthMode::ProductionFailClosed => with_web_request_context(
            with_problem_correlation(router),
            build_open_api_framework_layer(ProductionFailClosedResolver),
        ),
        MemoryWebAuthMode::IamDatabase(resolver) => {
            wrap_router_with_iam_database_web_framework(*resolver, router)
        }
    }
}
