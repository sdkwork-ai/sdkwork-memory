//! Gateway bootstrap for sdkwork-memory.
//!
//! The assembly owns Memory service construction (runtime bootstrap, product
//! service, Drive export uploader), business route composition, the readiness
//! check, and the metrics endpoint (API_ASSEMBLY_SPEC §6.1). It publishes one
//! host-neutral contribution through [`assemble_api_router_from_env`], and the
//! paired factory [`assemble_api_router_retaining_background_from_env`] hands
//! the background-worker shutdown trigger back to the process that owns it.

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
use sdkwork_web_bootstrap::{healthz_handler, livez_handler, ReadinessCheck, ReadinessFuture, readyz_handler, WebModule};
use sdkwork_web_core::HttpRouteManifest;
use tower::limit::ConcurrencyLimitLayer;
use tracing::info;

// Exactly one import binding for the contribution type: the `pub use` re-export below is the
// module's public contract (API_ASSEMBLY_SPEC section 4 "Export Completeness"), so it must not be
// shadowed by a second private `use` of the same name.
pub use sdkwork_web_bootstrap::ApiAssemblyContribution;

/// Indivisible host-neutral API assembly contribution (web-bootstrap contract,
/// API_ASSEMBLY_SPEC.md section 4).
pub type ApiAssembly = ApiAssemblyContribution;

/// The owner's complete route manifest: every surface this module owns.
fn memory_route_manifest() -> HttpRouteManifest {
    let routes = [
        sdkwork_routes_memory_open_api::gateway_route_manifest(),
        sdkwork_routes_memory_app_api::gateway_route_manifest(),
        sdkwork_routes_memory_backend_api::gateway_route_manifest(),
    ]
    .into_iter()
    .flat_map(|manifest| manifest.routes().to_vec())
    .collect();
    HttpRouteManifest::from_owned_routes(routes)
}

/// Default maximum request body size: 1 MiB.
const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;
/// Default maximum concurrent in-flight requests.
const DEFAULT_MAX_CONCURRENCY: usize = 256;

/// Builds the complete Memory contribution for `product`, pinned to `readiness`:
/// every business route, the owner's route manifest, the process endpoints, the
/// request-body ceiling, and the admission ceiling.
///
/// Each surface keeps its own Web Framework layer: `app-api`, `backend-api`, and
/// `open-api` select distinct auth profiles through `memory_web_auth_mode_from_env`
/// (dev-inline, production fail-closed, or the IAM database resolver), so a single
/// host-side layer cannot replace them. The contribution is therefore already
/// framework-wrapped and a host MUST NOT apply a second layer on top of it
/// (API_ASSEMBLY_SPEC §6.1).
///
/// Process endpoints are mounted exactly once here, after all three surfaces have
/// been merged, so `/healthz`, `/livez`, `/readyz`, and `/metrics` are never
/// duplicated per surface (APPLICATION_GATEWAY_SPEC §5.7.1, HEALTH_CHECK_SPEC).
/// `/metrics` keeps the Memory domain renderer instead of the generic
/// registry-only handler.
pub async fn assemble_api_router(
    product: Arc<OpenMemoryService>,
    readiness: Arc<dyn ReadinessCheck>,
) -> Result<ApiAssembly, String> {
    let open_business_router = build_open_router_with_product(product.clone());
    let app_business_router = build_app_router_with_product(product.clone());
    let backend_business_router = build_backend_router_with_product(product.clone());

    let open_router = wrap_open_router(open_business_router).await;
    let app_router = wrap_app_router(app_business_router).await;
    let backend_router = wrap_backend_router(backend_business_router).await;

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
        .merge(open_router)
        .merge(app_router)
        .merge(backend_router)
        .layer(Extension(product))
        .layer(DefaultBodyLimit::max(max_body_bytes))
        .layer(ConcurrencyLimitLayer::new(max_concurrency));

    info!(
        max_body_bytes,
        max_concurrency, "memory standalone-gateway rate limits configured"
    );

    ApiAssemblyContribution::from_manifest(
        "sdkwork-memory",
        "SDKWork Memory API",
        router,
        memory_route_manifest(),
        Vec::new(),
        readiness,
    )
}

// ---------------------------------------------------------------------------
// Standalone application construction (API_ASSEMBLY_SPEC §6.1)
// ---------------------------------------------------------------------------

/// Background workers owned by the process that hosts this assembly.
///
/// Process-owner handle for the Memory background plane.
///
/// Re-exported from the service layer: [`ApiAssembly`] is an indivisible
/// host-neutral contribution, so it MUST NOT carry process-lifecycle handles
/// (API_ASSEMBLY_SPEC §4). The shutdown trigger therefore travels separately,
/// through the paired factory
/// [`assemble_api_router_retaining_background_from_env`] (API_ASSEMBLY_SPEC
/// §6.2.1 "retaining background" rule). Dropping this value leaves the workers
/// running until process exit.
pub use sdkwork_intelligence_memory_service::MemoryBackgroundWorkers;

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

/// Boots the memory product service from `SDKWORK_DATABASE_*` environment and
/// builds the complete Memory HTTP surface as a host-neutral
/// `ApiAssemblyContribution`: every business route, the owner's route manifest,
/// the infra probes, the request-body ceiling, and the admission ceiling.
///
/// This is the contribution the canonical [`web_module`] publishes
/// (API_ASSEMBLY_SPEC §4.1.1). It performs no process-lifecycle side effects:
/// background workers are started only by
/// [`assemble_api_router_retaining_background_from_env`], so a host that
/// installs the module owns worker lifecycle and cannot be left with a dropped
/// shutdown handle.
async fn assemble_contribution_with_product_from_env(
) -> Result<(ApiAssemblyContribution, Arc<OpenMemoryService>), String> {
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

    let readiness: Arc<dyn ReadinessCheck> = Arc::new(MemoryReadinessCheck::new(product.clone()));
    let contribution = assemble_api_router(product.clone(), readiness).await?;

    Ok((contribution, product))
}

/// Assembles the complete Memory API contribution from `SDKWORK_DATABASE_*`
/// environment (API_ASSEMBLY_SPEC §6.1).
///
/// The returned contribution is already framework-wrapped per surface, so the
/// host composes it with [`ApiModuleRegistry`](sdkwork_web_bootstrap::ApiModuleRegistry)
/// and serves `ComposedApiAssembly::router` without applying a second Web
/// Framework layer. Background workers are not started: use
/// [`assemble_api_router_retaining_background_from_env`] when the caller owns
/// worker lifecycle.
pub async fn assemble_api_router_from_env() -> Result<ApiAssembly, String> {
    Ok(assemble_contribution_with_product_from_env().await?.0)
}

/// Paired factory for the process that hosts Memory: the complete API
/// contribution **plus** the background-worker handle that process must own
/// (API_ASSEMBLY_SPEC §6.2.1 "retaining background" rule).
///
/// The module still owns the complete route definition; only task shutdown
/// moves to the host, which MUST keep the returned
/// [`MemoryBackgroundWorkers`] alive, call
/// [`MemoryBackgroundWorkers::shutdown`], and then await
/// [`MemoryBackgroundWorkers::drain`] so the drain is bounded by a real
/// completion signal rather than a guessed sleep duration.
pub async fn assemble_api_router_retaining_background_from_env(
) -> Result<(ApiAssembly, MemoryBackgroundWorkers), String> {
    let (contribution, product) = assemble_contribution_with_product_from_env().await?;
    let workers = OpenMemoryService::spawn_background_workers(&product);
    Ok((contribution, workers))
}

/// Database migration-only lifecycle for the `db-migrate` CLI mode of the thin
/// standalone gateway. The assembly owns database bootstrap concerns
/// (API_ASSEMBLY_SPEC §6.1); the gateway must not import implementation crates
/// such as `sdkwork-memory-database-host`.
///
/// The server-role engine admission rule runs first so `db-migrate` reports the
/// same actionable "PostgreSQL is required" diagnostic as startup, instead of
/// failing later inside the migration history table.
pub async fn run_database_migrate_only() -> Result<(), String> {
    sdkwork_intelligence_memory_repository_sqlx::ensure_server_role_database_engine_from_env()?;
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

/// Canonical Web Module definition for this application
/// (API_ASSEMBLY_SPEC §4.1.1): the complete HTTP surface — every route,
/// manifest, and OpenAPI document of this owner — as one installable module.
///
/// The module is built from a real `ApiAssemblyContribution`, never from a bare
/// router struct, so the published contract keeps its owner identity, route
/// manifest, OpenAPI document, permission catalog, and readiness check.
///
/// Background workers stay with the process owner: installing this module does
/// not start them, and the worker handle is never dropped out-of-band. A host
/// that must own worker lifecycle uses the paired factory
/// [`assemble_api_router_retaining_background_from_env`] (API_ASSEMBLY_SPEC
/// §6.2.1 "retaining background" rule).
pub async fn web_module() -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(
        assemble_api_router_from_env().await?,
    ))
}
