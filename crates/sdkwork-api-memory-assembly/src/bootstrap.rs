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
    extract::Extension,
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
    healthz_handler, livez_handler, readyz_handler, ReadinessCheck, ReadinessFuture, WebModule,
};
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

/// Default maximum concurrent in-flight requests.
const DEFAULT_MAX_CONCURRENCY: usize = 256;

/// Minimum process pool budget the Memory assembly admits in a production-like
/// environment. The retention and provider-health workers each hold one
/// dedicated pool connection for the whole duration of a lease (session-scoped
/// PostgreSQL advisory locks), so a pool below
/// retention + provider-health + business concurrency headroom (4) lets those
/// lease sessions starve every business acquire until the acquire timeout.
const MIN_DATABASE_POOL_MAX_CONNECTIONS: u32 = 4;

/// Resolves the process pool budget through the exact resolver pool creation
/// uses (`DatabaseConfig::from_env("MEMORY")`): process override, else the
/// workspace database-config profile, else the default. Mirroring only the
/// env override here would let a profile-configured undersized pool pass this
/// admission and starve the lease workers anyway.
fn resolved_database_pool_max_connections() -> Result<u32, String> {
    sdkwork_database_config::DatabaseConfig::from_env("MEMORY")
        .map(|config| config.max_connections)
        .map_err(|error| format!("database pool configuration failed: {error}"))
}

/// Startup admission on the process pool budget: the retention and
/// provider-health lease sessions each hold one dedicated connection while
/// business traffic keeps acquiring from the same pool, so a production-like
/// environment configured below the lease floor is rejected with a named
/// startup error instead of idling through acquire timeouts at runtime.
/// Development environments only warn with the same diagnostic.
fn enforce_minimum_database_pool_capacity_from_env() -> Result<(), String> {
    let max_connections = resolved_database_pool_max_connections()?;
    enforce_minimum_database_pool_capacity(
        max_connections,
        platform::is_production_like_environment(),
    )
}

fn enforce_minimum_database_pool_capacity(
    max_connections: u32,
    production_like: bool,
) -> Result<(), String> {
    if max_connections >= MIN_DATABASE_POOL_MAX_CONNECTIONS {
        return Ok(());
    }
    let diagnostic = format!(
        "memory database pool is too small for production-like startup: \
         SDKWORK_DATABASE_MAX_CONNECTIONS resolves to {max_connections}, but the minimum \
         admitted budget is {MIN_DATABASE_POOL_MAX_CONNECTIONS}. The retention and \
         provider-health workers each hold one dedicated pool connection for the whole \
         duration of a lease (session-scoped advisory locks), so the budget needs room for \
         retention + provider-health + business concurrency headroom (>= \
         {MIN_DATABASE_POOL_MAX_CONNECTIONS}); below that the lease sessions starve every \
         business acquire until the 10s acquire timeout. Raise SDKWORK_DATABASE_MAX_CONNECTIONS \
         to {MIN_DATABASE_POOL_MAX_CONNECTIONS} or more."
    );
    if production_like {
        return Err(diagnostic);
    }
    tracing::warn!("{diagnostic}");
    Ok(())
}

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

    // A zero permit count would park every in-flight request forever while
    // readiness still reports healthy, so the ceiling is bounded below by 1.
    let max_concurrency = platform::read_env_usize(
        "SDKWORK_MEMORY_MAX_CONCURRENCY",
        DEFAULT_MAX_CONCURRENCY,
    )
    .clamp(1, 4096);

    // The infra probes (/healthz /livez /readyz /metrics) are deliberately
    // OUTSIDE the concurrency-limit layer: a saturated admission ceiling must
    // never make health probes queue behind business traffic, or the
    // orchestrator would start killing an otherwise healthy process exactly
    // when it needs the probe's verdict. The ceiling applies to business
    // routers only; the probes keep the panic shield and the product
    // extension (the metrics renderer reads it).
    let probe_router = Router::new()
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
        .layer(sdkwork_routes_memory_support::MemoryPanicShieldLayer)
        .layer(Extension(product.clone()));

    let business_router = Router::new()
        .merge(open_router)
        .merge(app_router)
        .merge(backend_router)
        .layer(sdkwork_routes_memory_support::MemoryPanicShieldLayer)
        .layer(Extension(product))
        .layer(ConcurrencyLimitLayer::new(max_concurrency));

    let router = probe_router.merge(business_router);

    // The request body limit is deliberately NOT layered here. Each surface
    // applies `memory_request_body_limit_bytes()` as the innermost
    // DefaultBodyLimit after its framework layer, which is the only position
    // that wins over the framework's own 16 MiB default — an outer layer here
    // was silently overridden. Single enforcement point:
    // `SDKWORK_MEMORY_MAX_BODY_BYTES` read in sdkwork-routes-memory-support.
    let max_body_bytes = sdkwork_routes_memory_support::memory_request_body_limit_bytes();

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
            if !platform::numeric_id_generator_healthy() {
                memory_domain_metrics().set_serving(false);
                return Err("memory numeric id generator lease unhealthy".to_owned());
            }
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
        "{}{}{}",
        memory_http_metrics().render_prometheus(),
        sdkwork_routes_memory_support::render_memory_web_prometheus(),
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
    let product = open_memory_service_from_env().await?;
    let readiness: Arc<dyn ReadinessCheck> = Arc::new(MemoryReadinessCheck::new(product.clone()));
    let contribution = assemble_api_router(product.clone(), readiness).await?;

    Ok((contribution, product))
}

/// Boots the Memory product service from `SDKWORK_DATABASE_*` environment.
///
/// Shared by the whole-module factory and the app-api-only factory so every
/// entrypoint boots the identical product: runtime profile, data plane, Drive
/// export uploader, and the optional OpenAI-compatible embedding/chat
/// providers.
async fn open_memory_service_from_env() -> Result<Arc<OpenMemoryService>, String> {
    refresh_memory_http_metric_dimensions();
    sdkwork_intelligence_memory_service::platform::validate_runtime_secrets_for_environment()?;
    validate_outbox_runtime_config().await?;
    // Fail fast on a pool budget the lease workers can starve, before any
    // connection is opened (see `enforce_minimum_database_pool_capacity`).
    enforce_minimum_database_pool_capacity_from_env()?;
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
    // Batch 8b supply side: binding an embedding provider turns on the vector
    // similarity signal for profiles that grant `vector` a positive weight.
    // Without SDKWORK_MEMORY_OPENAI_API_KEY the deployment stays purely
    // lexical, exactly as before.
    if let Some(config) = sdkwork_memory_provider_openai::OpenAiProviderConfig::from_env() {
        // The provider receives the same SSRF-hardened outbound client the
        // outbox uses: resolved addresses validated, non-public/mixed DNS
        // rejected, validated addresses pinned, redirects disabled. This is
        // what makes the architecture document's outbound-egress claim true
        // for provider traffic, not only for outbox delivery.
        let pinned_config = match sdkwork_intelligence_memory_service::endpoint_validation::build_pinned_http_client(
            &config.base_url,
            std::time::Duration::from_secs(config.timeout_secs),
            16,
        )
        .await
        {
            Ok((_, client)) => config.clone().with_http_client(client),
            Err(error) => {
                return Err(format!(
                    "provider endpoint failed outbound-egress validation: {error}"
                ));
            }
        };
        let embedder = sdkwork_memory_provider_openai::OpenAiEmbeddings::new(pinned_config.clone());
        product = product.with_embedder(Arc::new(embedder));
        let llm = sdkwork_memory_provider_openai::OpenAiLlm::new(pinned_config);
        product = product.with_llm(Arc::new(llm));
        info!("embedding + chat providers bound (openai-compatible, pinned egress client)");
    }
    product
        .ready_check()
        .await
        .map_err(|_| "memory database schema preflight failed".to_owned())?;
    Ok(Arc::new(product))
}

/// The Memory owner's app-api route manifest (API_ASSEMBLY_SPEC §4 "Export
/// Completeness").
///
/// An embedding host composes its app-surface route manifest from every mounted
/// capability's own manifest — the web-framework auth pipeline decides public
/// versus protected from that composed manifest — so the app surface MUST be
/// reachable through an assembly entrypoint instead of a direct
/// `sdkwork-routes-memory-app-api` import (API_ASSEMBLY_SPEC §3).
pub fn app_api_route_manifest() -> HttpRouteManifest {
    sdkwork_routes_memory_app_api::app_route_manifest()
}

/// Assembles the **app-api** Memory contribution for an embedding host
/// (API_ASSEMBLY_SPEC §6.1, §6.2.1).
///
/// The embedding host — the Cloud Router unified runtime, for example — owns
/// the process and serves the Memory app surface under the canonical
/// `/app/v3/api/memory*` paths, so this factory publishes **only** that
/// surface. Publishing the open-api and backend-api surfaces here would collide
/// with the capability mounts the host already owns, and a host MUST NOT apply
/// a second Web Framework layer on top of the returned contribution: every
/// surface selects its own auth profile through `memory_web_auth_mode_from_env`.
///
/// The dependency keeps its own background plane. `MemoryBackgroundWorkers`
/// carries no `Drop` that stops the workers, so dropping the handle leaves them
/// running until process exit — the dependency-owned shape an embedded host
/// expects — and every claimed job/outbox row is lease-fenced, so a rollout
/// that kills them mid-batch loses no work.
pub async fn assemble_app_api_contribution_from_env() -> Result<ApiAssembly, String> {
    let product = open_memory_service_from_env().await?;
    let _workers = OpenMemoryService::spawn_background_workers(&product);
    let business_router = build_app_router_with_product(product.clone());
    let router = wrap_app_router(business_router).await;

    ApiAssemblyContribution::from_manifest(
        "sdkwork-memory",
        "SDKWork Memory App API",
        router,
        sdkwork_routes_memory_app_api::app_route_manifest(),
        Vec::new(),
        Arc::new(MemoryReadinessCheck::new(product)),
    )
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
///
/// The division is deliberate but must never be silent: without the background
/// plane nothing drains `ai_outbox_event`, runs learning/eval jobs, or sweeps
/// retention, and nothing else reports that gap. The one-time warning below is
/// the operator's signal to either use the retaining-background factory or
/// start the workers host-side.
pub async fn web_module() -> Result<WebModule, String> {
    tracing::warn!(
        "memory web_module installed WITHOUT background workers: the outbox, learning, \
         evaluation, retention, and provider-health planes are idle. Use \
         assemble_api_router_retaining_background_from_env (and keep the returned handle \
         alive) if this process should own them."
    );
    Ok(WebModule::from_contribution(
        assemble_api_router_from_env().await?,
    ))
}

#[cfg(test)]
mod database_pool_admission_tests {
    use super::*;

    /// Serializes process-environment mutations across the test binary's
    /// threads; a leaked `SDKWORK_*` value would otherwise flip sibling tests
    /// between the production-like and development admission paths.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Panic-safe environment override that restores every touched key on
    /// drop — the same isolation pattern as `sdkwork-memory-contract`'s
    /// `MemoryEnvScope` paired with `env_test_lock`, localized because the
    /// assembly does not depend on that crate.
    struct EnvScope(Vec<(String, Option<std::ffi::OsString>)>);

    impl EnvScope {
        fn new(vars: &[(&str, Option<&str>)]) -> Self {
            let previous = vars
                .iter()
                .map(|(key, _)| ((*key).to_string(), std::env::var_os(key)))
                .collect();
            for (key, value) in vars {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            Self(previous)
        }
    }

    impl Drop for EnvScope {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    fn production_like_environment_rejects_a_pool_below_the_lease_floor() {
        for max_connections in [0, 1, 2, 3] {
            let error = enforce_minimum_database_pool_capacity(max_connections, true)
                .expect_err("below the lease floor, production-like startup must be rejected");
            assert!(
                error.contains("SDKWORK_DATABASE_MAX_CONNECTIONS"),
                "diagnostic must name the operator knob, got: {error}"
            );
            assert!(
                error.contains("retention"),
                "diagnostic must explain the retention lease connection, got: {error}"
            );
            assert!(
                error.contains("provider-health"),
                "diagnostic must explain the provider-health lease connection, got: {error}"
            );
            let lease_floor = MIN_DATABASE_POOL_MAX_CONNECTIONS.to_string();
            assert!(
                error.contains(&lease_floor),
                "diagnostic must state the minimum admitted budget, got: {error}"
            );
        }
    }

    #[test]
    fn pool_at_or_above_the_lease_floor_is_admitted_in_every_environment() {
        for max_connections in [MIN_DATABASE_POOL_MAX_CONNECTIONS, 5, 16] {
            enforce_minimum_database_pool_capacity(max_connections, true)
                .expect("the lease floor plus headroom must be admitted");
            enforce_minimum_database_pool_capacity(max_connections, false)
                .expect("development startup must be admitted too");
        }
    }

    #[test]
    fn development_environment_only_warns_below_the_lease_floor() {
        // The warn path is the Ok(()) arm: development startup proceeds, and
        // the operator sees the same diagnostic in the log.
        enforce_minimum_database_pool_capacity(2, false)
            .expect("development startup must not be rejected below the lease floor");
    }

    #[test]
    fn env_resolver_delegates_to_the_pool_creation_resolver() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _scope = EnvScope::new(&[("SDKWORK_DATABASE_MAX_CONNECTIONS", None)]);
        // Unset: whatever the canonical resolver resolves (profile or
        // default) must equal what pool creation will use — the admission
        // never pins a second default.
        let unset = resolved_database_pool_max_connections().expect("unset resolves");
        let canonical = sdkwork_database_config::DatabaseConfig::from_env("MEMORY")
            .expect("canonical resolver")
            .max_connections;
        assert_eq!(unset, canonical);

        {
            let _scope = EnvScope::new(&[("SDKWORK_DATABASE_MAX_CONNECTIONS", Some("6"))]);
            assert_eq!(resolved_database_pool_max_connections().ok(), Some(6));
        }
        {
            // An unparseable override is a loud configuration error from the
            // canonical resolver, not a silent fallback.
            let _scope = EnvScope::new(&[("SDKWORK_DATABASE_MAX_CONNECTIONS", Some("not-a-number"))]);
            let error = resolved_database_pool_max_connections()
                .expect_err("unparseable override must fail loudly");
            assert!(
                error.contains("SDKWORK_DATABASE_MAX_CONNECTIONS"),
                "the resolver error must name the operator knob, got: {error}"
            );
        }
        {
            let _scope = EnvScope::new(&[("SDKWORK_DATABASE_MAX_CONNECTIONS", Some(" 3 "))]);
            assert_eq!(
                resolved_database_pool_max_connections().ok(),
                Some(3),
                "surrounding whitespace is trimmed like the pool read"
            );
        }
    }

    #[test]
    fn env_admission_rejects_production_startup_when_the_override_is_too_small() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _scope = EnvScope::new(&[
            ("SDKWORK_DATABASE_MAX_CONNECTIONS", Some("2")),
            ("SDKWORK_MEMORY_ENVIRONMENT", Some("production")),
            ("SDKWORK_MEMORY_CONFIG_PROFILE", None),
        ]);
        let error = enforce_minimum_database_pool_capacity_from_env()
            .expect_err("production-like startup with a 2-connection pool must be rejected");
        assert!(
            error.contains("SDKWORK_DATABASE_MAX_CONNECTIONS"),
            "startup diagnostic must name the operator knob, got: {error}"
        );
    }

    #[test]
    fn env_admission_allows_development_startup_when_the_override_is_too_small() {
        let _lock = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let _scope = EnvScope::new(&[
            ("SDKWORK_DATABASE_MAX_CONNECTIONS", Some("2")),
            ("SDKWORK_MEMORY_ENVIRONMENT", Some("development")),
            ("SDKWORK_MEMORY_CONFIG_PROFILE", None),
        ]);
        enforce_minimum_database_pool_capacity_from_env()
            .expect("development startup only warns below the lease floor");
    }
}
