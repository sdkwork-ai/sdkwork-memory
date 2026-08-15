//! Gateway bootstrap for sdkwork-memory.
//!
//! The assembly owns Memory service construction (runtime bootstrap, product
//! service, Drive export uploader, background workers), business route
//! composition, the readiness check, and the metrics endpoint
//! (API_ASSEMBLY_SPEC §6.1). The thin standalone gateway calls
//! `assemble_api_router_from_env` and projects `.router` /
//! `.worker_shutdown_tx`.

use std::sync::Arc;

use axum::{
    extract::{DefaultBodyLimit, Extension},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use sdkwork_intelligence_memory_repository_sqlx::bootstrap_memory_runtime_from_env;
use sdkwork_intelligence_memory_service::{
    memory_domain_metrics, platform, render_memory_domain_prometheus,
    validate_outbox_runtime_config, OpenMemoryService,
};
use sdkwork_memory_database_host::bootstrap_memory_database_from_env;
use sdkwork_routes_memory_app_api::{
    build_router_with_open_memory_service as build_app_router_with_product,
    wrap_router_with_web_framework_from_env as wrap_app_router,
};
use sdkwork_routes_memory_backend_api::{
    build_router_with_open_memory_service as build_backend_router_with_product,
    wrap_router_with_web_framework_from_env as wrap_backend_router,
};
use sdkwork_routes_memory_open_api::{
    build_router_with_open_memory_service as build_open_router_with_product,
    wrap_router_with_web_framework_from_env as wrap_open_router,
};
use sdkwork_routes_memory_support::{
    memory_dependency_ready_check, memory_http_metrics, memory_metric_environment_label,
    refresh_memory_http_metric_dimensions,
};
use sdkwork_web_bootstrap::{
    healthz_handler, livez_handler, readyz_handler, ApiAssemblyContribution, ReadinessCheck,
    ReadinessFuture,
};
use sdkwork_web_core::HttpRouteManifest;
use tower::limit::ConcurrencyLimitLayer;
use tracing::info;

pub use sdkwork_web_bootstrap::ApiAssemblyContribution;

/// Indivisible host-neutral API assembly contribution (web-bootstrap contract,
/// API_ASSEMBLY_SPEC.md section 4).
pub type ApiAssembly = ApiAssemblyContribution;

pub async fn assemble_api_router(
    product: Arc<OpenMemoryService>,
) -> ApiAssembly {
    let open_business_router = build_open_router_with_product(product.clone());
    let app_business_router = build_app_router_with_product(product.clone());
    let backend_business_router = build_backend_router_with_product(product.clone());

    let open_router = wrap_open_router(open_business_router).await;
    let app_router = wrap_app_router(app_business_router).await;
    let backend_router = wrap_backend_router(backend_business_router).await;

    let router = Router::new()
        .merge(open_router)
        .merge(app_router)
        .merge(backend_router)
        .layer(axum::Extension(product));

    let routes = [
        sdkwork_routes_memory_open_api::gateway_route_manifest(),
        sdkwork_routes_memory_app_api::gateway_route_manifest(),
        sdkwork_routes_memory_backend_api::gateway_route_manifest(),
    ]
    .into_iter()
    .flat_map(|manifest| manifest.routes().to_vec())
    .collect();

    ApiAssemblyContribution::from_manifest(
        "sdkwork-memory",
        "SDKWork Memory API",
        router,
        HttpRouteManifest::from_owned_routes(routes),
        Vec::new(),
        Arc::new(sdkwork_web_bootstrap::AlwaysReady),
    )
    .expect("memory assembly contribution contract is valid")
}

// ---------------------------------------------------------------------------
// Standalone application construction (API_ASSEMBLY_SPEC §6.1)
// ---------------------------------------------------------------------------

/// Default maximum request body size: 1 MiB.
const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;
/// Default maximum concurrent in-flight requests.
const DEFAULT_MAX_CONCURRENCY: usize = 256;

/// Assembled standalone application: business + infra router and the
/// background-worker shutdown handle.
///
/// The caller MUST keep `worker_shutdown_tx` alive and call `send(true)`
/// during graceful shutdown so that background workers (outbox publisher,
/// learning job worker, eval run worker, provider health probe) can drain
/// in-flight work and exit cleanly.
pub struct MemoryStandaloneApplication {
    pub router: Router,
    pub worker_shutdown_tx: tokio::sync::watch::Sender<bool>,
}

/// Readiness probe over the memory product service and its dependencies.
pub struct MemoryReadinessCheck {
    service: Arc<OpenMemoryService>,
}

impl MemoryReadinessCheck {
    pub fn new(service: Arc<OpenMemoryService>) -> Self {
        Self { service }
    }
}

impl ReadinessCheck for MemoryReadinessCheck {
    fn check(&self) -> ReadinessFuture<'_> {
        let service = self.service.clone();
        Box::pin(async move {
            if service.ready_check().await.is_err() {
                memory_domain_metrics().set_serving(false);
                return Err("memory store not ready".to_owned());
            }
            if !memory_dependency_ready_check().await {
                memory_domain_metrics().set_serving(false);
                return Err("memory dependencies not ready".to_owned());
            }
            memory_domain_metrics().set_serving(true);
            Ok(())
        })
    }
}

async fn metrics(Extension(product): Extension<Arc<OpenMemoryService>>) -> impl IntoResponse {
    let environment = memory_metric_environment_label();
    let deployment_profile = std::env::var("SDKWORK_MEMORY_DEPLOYMENT_PROFILE")
        .unwrap_or_else(|_| "standalone".to_owned());
    let runtime_target =
        std::env::var("SDKWORK_MEMORY_RUNTIME_TARGET").unwrap_or_else(|_| "server".to_owned());
    let runtime_profile = product.runtime_profile_label();
    let body = format!(
        "{}{}",
        memory_http_metrics().render_prometheus(),
        render_memory_domain_prometheus(
            "sdkwork-api-memory-standalone-gateway",
            &environment,
            &deployment_profile,
            &runtime_target,
            runtime_profile,
        )
    );
    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}

/// Boots the memory product service from `SDKWORK_DATABASE_*` environment,
/// assembles the business router contribution, and returns the complete
/// standalone application router with infrastructure routes, readiness,
/// metrics, and the background-worker shutdown handle. The thin standalone
/// gateway projects `.router` / `.worker_shutdown_tx`.
pub async fn assemble_api_router_from_env() -> Result<MemoryStandaloneApplication, String> {
    refresh_memory_http_metric_dimensions();
    validate_outbox_runtime_config().await?;
    let runtime = bootstrap_memory_runtime_from_env().await?;
    info!(
        profile_id = %runtime.core_runtime.profile().profile_id,
        primary_plugin_id = %runtime.core_runtime.profile().primary_plugin_id,
        dialect = ?runtime.data_plane.store().dialect(),
        postgres_host_pool = runtime.data_plane.host_pool.is_some(),
        "memory runtime ready"
    );
    let mut product = OpenMemoryService::try_from_core_runtime_with_retrieval_strategy(
        runtime.data_plane.phase1,
        runtime.core_runtime,
        runtime.retrieval_strategy,
    )?;
    if let Some(uploader) =
        sdkwork_memory_drive::bootstrap_memory_drive_export_uploader_from_env().await?
    {
        product = product.with_drive_export_uploader(uploader);
    }
    product
        .ready_check()
        .await
        .map_err(|_| "memory database schema preflight failed".to_owned())?;
    let product = Arc::new(product);
    let worker_shutdown_tx = OpenMemoryService::spawn_background_workers(&product);

    let business_router = assemble_api_router(product.clone()).await.router;

    let readiness = Arc::new(MemoryReadinessCheck::new(product.clone()));

    let max_body_bytes =
        platform::read_env_usize("SDKWORK_MEMORY_MAX_BODY_BYTES", DEFAULT_MAX_BODY_BYTES);
    let max_concurrency =
        platform::read_env_usize("SDKWORK_MEMORY_MAX_CONCURRENCY", DEFAULT_MAX_CONCURRENCY);

    let router = Router::new()
        .route("/metrics", get(metrics))
        .route("/healthz", get(healthz_handler))
        .route("/livez", get(livez_handler))
        .route(
            "/readyz",
            get({
                let readiness = readiness.clone();
                move || async move { readyz_handler(Some(readiness)).await }
            }),
        )
        .merge(business_router)
        .layer(Extension(product))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(ConcurrencyLimitLayer::new(max_concurrency));

    info!(
        max_body_bytes,
        max_concurrency, "memory standalone-gateway rate limits configured"
    );

    Ok(MemoryStandaloneApplication {
        router,
        worker_shutdown_tx,
    })
}

/// Database migration-only lifecycle for the `db-migrate` CLI mode of the thin
/// standalone gateway. The assembly owns database bootstrap concerns
/// (API_ASSEMBLY_SPEC §6.1); the gateway must not import implementation crates
/// such as `sdkwork-memory-database-host`.
pub async fn run_database_migrate_only() -> Result<(), String> {
    let previous_auto_migrate = std::env::var_os("SDKWORK_DATABASE_AUTO_MIGRATE");
    std::env::set_var("SDKWORK_DATABASE_AUTO_MIGRATE", "true");
    let result = bootstrap_memory_database_from_env().await;
    match previous_auto_migrate {
        Some(value) => std::env::set_var("SDKWORK_DATABASE_AUTO_MIGRATE", value),
        None => std::env::remove_var("SDKWORK_DATABASE_AUTO_MIGRATE"),
    }
    result?;
    info!("memory database migration completed");
    Ok(())
}
