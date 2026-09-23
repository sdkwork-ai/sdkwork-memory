#![allow(clippy::await_holding_lock)] // Process-wide test environment must remain serialized.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::runtime_env::env_test_lock;
use sdkwork_memory_test_support::web_auth::{
    memory_access_token, memory_auth_token_bearer, memory_dev_api_key,
};
use sdkwork_routes_memory_app_api::build_router_with_app_api;
use sdkwork_routes_memory_open_api::build_router_with_open_api;
use sdkwork_web_bootstrap::AlwaysReady;
use tower::util::ServiceExt;

const DEV_API_KEY: &str = "dev-key";

fn restore_optional_env(key: &str, value: Option<String>) {
    match value {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
}

#[tokio::test]
async fn api_server_bootstrap_auth_and_healthz_contracts() {
    let _guard = env_test_lock();
    let previous_environment = std::env::var("SDKWORK_MEMORY_ENVIRONMENT").ok();
    let previous_profile = std::env::var("SDKWORK_MEMORY_CONFIG_PROFILE").ok();
    let previous_bypass = std::env::var("SDKWORK_MEMORY_DEV_AUTH_BYPASS").ok();
    let previous_database_url = std::env::var("SDKWORK_DATABASE_URL").ok();
    let previous_auto_migrate = std::env::var("SDKWORK_DATABASE_AUTO_MIGRATE").ok();
    let previous_outbox_mode = std::env::var("SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE").ok();
    let previous_outbox_url = std::env::var("SDKWORK_MEMORY_OUTBOX_DELIVERY_URL").ok();

    std::env::set_var("SDKWORK_MEMORY_ENVIRONMENT", "development");
    std::env::set_var("SDKWORK_MEMORY_DEV_AUTH_BYPASS", "true");

    // Process endpoints are part of the host-neutral contribution, so they are verified against a
    // store-backed product service rather than through the environment bootstrap: this
    // authoritative server does not admit a SQLite URL (`authoritative_server_rejects_sqlite_engine`
    // owns that admission rule) and no external PostgreSQL is required for route-shape coverage.
    let dev_product = Arc::new(OpenMemoryService::new(
        sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await,
    ));
    let dev_contribution =
        sdkwork_api_memory_assembly::assemble_api_router(dev_product, Arc::new(AlwaysReady))
            .await
            .expect("memory contribution must assemble from a store-backed product service");
    let dev_router = dev_contribution.router;

    let healthz = dev_router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(healthz.status(), StatusCode::OK);

    let readyz = dev_router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(readyz.status(), StatusCode::OK);

    let metrics = dev_router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    let metrics_body = axum::body::to_bytes(metrics.into_body(), usize::MAX)
        .await
        .unwrap();
    let metrics_text = String::from_utf8_lossy(&metrics_body);
    assert!(
        metrics_text.contains("http_requests_total") || metrics_text.contains("http_request"),
        "metrics endpoint must expose HTTP request counters"
    );

    // Production must fail closed on an unsafe outbox configuration. Outbox validation runs before
    // any database work, so leaving `SDKWORK_DATABASE_URL` unset does not weaken the assertion.
    std::env::set_var("SDKWORK_MEMORY_ENVIRONMENT", "production");
    std::env::set_var("SDKWORK_MEMORY_CONFIG_PROFILE", "production");
    std::env::remove_var("SDKWORK_MEMORY_DEV_AUTH_BYPASS");
    std::env::remove_var("SDKWORK_DATABASE_URL");
    std::env::set_var("SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE", "disabled");
    std::env::remove_var("SDKWORK_MEMORY_OUTBOX_DELIVERY_URL");

    let production_bootstrap = sdkwork_api_memory_assembly::assemble_api_router_from_env().await;
    let Err(error) = production_bootstrap else {
        panic!("production bootstrap must reject disabled outbox delivery");
    };
    assert!(
        error.contains("SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE=http is required"),
        "production startup must fail closed when durable outbox delivery is disabled"
    );

    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let production_open_router = build_router_with_open_api(OpenMemoryService::new(store.clone()));
    let production_app_router = build_router_with_app_api(OpenMemoryService::new(store));

    let protected = production_open_router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mem/v3/api/memory/capabilities")
                .header("x-api-key", memory_dev_api_key("2001", DEV_API_KEY))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(protected.status(), StatusCode::UNAUTHORIZED);

    let protected_app = production_app_router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/app/v3/api/memory/learning_settings")
                .header("Authorization", memory_auth_token_bearer("2001"))
                .header("Access-Token", memory_access_token("2001"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(protected_app.status(), StatusCode::UNAUTHORIZED);

    restore_optional_env("SDKWORK_MEMORY_ENVIRONMENT", previous_environment);
    restore_optional_env("SDKWORK_MEMORY_CONFIG_PROFILE", previous_profile);
    restore_optional_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", previous_bypass);
    restore_optional_env("SDKWORK_DATABASE_URL", previous_database_url);
    restore_optional_env("SDKWORK_DATABASE_AUTO_MIGRATE", previous_auto_migrate);
    restore_optional_env("SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE", previous_outbox_mode);
    restore_optional_env("SDKWORK_MEMORY_OUTBOX_DELIVERY_URL", previous_outbox_url);
}

/// `database/database.manifest.json` declares this module `databaseRole: authoritative-server`
/// with `engines: ["postgres"]`, so ENVIRONMENT_SPEC section 7.2 applies: a server-authoritative
/// module must not silently accept SQLite — it resolves the server role and rejects a
/// non-PostgreSQL pool with an actionable diagnostic.
///
/// Every server entrypoint is covered, and each must reject before spawning a worker or touching
/// a migration history table, so the operator sees the configuration error and not a downstream
/// symptom. The explicit `test-runner` escape hatch is exercised by
/// `sdkwork-intelligence-memory-repository-sqlx`'s own `bootstrap_memory_runtime_from_env_with_sqlite`.
#[tokio::test]
async fn authoritative_server_rejects_sqlite_engine() {
    let _guard = env_test_lock();
    let previous_environment = std::env::var("SDKWORK_MEMORY_ENVIRONMENT").ok();
    let previous_target = std::env::var("SDKWORK_MEMORY_RUNTIME_TARGET").ok();
    let previous_bypass = std::env::var("SDKWORK_MEMORY_DEV_AUTH_BYPASS").ok();
    let previous_database_url = std::env::var("SDKWORK_DATABASE_URL").ok();

    std::env::set_var("SDKWORK_MEMORY_ENVIRONMENT", "development");
    std::env::set_var("SDKWORK_MEMORY_DEV_AUTH_BYPASS", "true");
    std::env::set_var("SDKWORK_DATABASE_URL", "sqlite::memory:");
    std::env::remove_var("SDKWORK_MEMORY_RUNTIME_TARGET");

    let outcomes: [(&str, Result<(), String>); 3] = [
        (
            "assemble_api_router_from_env",
            sdkwork_api_memory_assembly::assemble_api_router_from_env()
                .await
                .map(|_| ()),
        ),
        (
            "assemble_api_router_retaining_background_from_env",
            sdkwork_api_memory_assembly::assemble_api_router_retaining_background_from_env()
                .await
                .map(|_| ()),
        ),
        (
            "run_database_migrate_only",
            sdkwork_api_memory_assembly::run_database_migrate_only().await,
        ),
    ];

    for (entrypoint, outcome) in outcomes {
        let error = outcome.err().unwrap_or_else(|| {
            panic!("{entrypoint} must reject a SQLite URL for the authoritative server")
        });
        assert!(
            error.contains("authoritative-server Memory rejects the SQLite engine"),
            "{entrypoint} must reject SQLite as a server-role violation, got: {error}"
        );
        assert!(
            error.contains("PostgreSQL is required"),
            "{entrypoint} must name the required engine, got: {error}"
        );
        assert!(
            error.contains("SDKWORK_DATABASE_URL"),
            "{entrypoint} must name the actionable PostgreSQL key, got: {error}"
        );
        assert!(
            error.contains("SDKWORK_MEMORY_RUNTIME_TARGET=test-runner"),
            "{entrypoint} must name the explicit test-runner escape hatch, got: {error}"
        );
    }

    restore_optional_env("SDKWORK_MEMORY_ENVIRONMENT", previous_environment);
    restore_optional_env("SDKWORK_MEMORY_RUNTIME_TARGET", previous_target);
    restore_optional_env("SDKWORK_MEMORY_DEV_AUTH_BYPASS", previous_bypass);
    restore_optional_env("SDKWORK_DATABASE_URL", previous_database_url);
}
