use sdkwork_intelligence_memory_repository_sqlx::{
    bootstrap_memory_plugin_registry, bootstrap_memory_runtime_from_env,
    resolve_memory_deployment_mode_from_env, resolve_memory_profile_for_runtime,
    resolve_memory_retrieval_strategy_from_env, resolve_native_sql_profile_for_dialect,
    resolve_native_sql_profile_for_runtime, MemoryImplementationProfileSelection,
};
use sdkwork_memory_contract::runtime_env::{env_test_lock, MemoryEnvScope};
use sdkwork_memory_plugin_native_sql::MemorySqlDialect;
use sdkwork_memory_retrieval::MemoryRetrievalStrategy;
use sdkwork_memory_spi::{MemoryDeploymentMode, MemoryImplementationKind};

#[test]
fn native_sql_profile_selection_matches_database_dialect() {
    let registry = bootstrap_memory_plugin_registry();

    let postgres = resolve_native_sql_profile_for_dialect(&registry, MemorySqlDialect::Postgres)
        .expect("postgres profile must resolve");
    assert_eq!(postgres.profile_id, "native-sql-phase1");
    assert_eq!(
        postgres.implementation_kind,
        MemoryImplementationKind::NativeSql
    );
    assert_eq!(postgres.deployment_mode, MemoryDeploymentMode::Server);

    let sqlite = resolve_native_sql_profile_for_dialect(&registry, MemorySqlDialect::Sqlite)
        .expect("sqlite profile must resolve");
    assert_eq!(sqlite.profile_id, "local-embedded-phase1");
    assert_eq!(
        sqlite.implementation_kind,
        MemoryImplementationKind::LocalEmbedded
    );
    assert_eq!(sqlite.deployment_mode, MemoryDeploymentMode::Local);
}

#[test]
fn native_sql_profile_selection_separates_dialect_from_runtime_target() {
    let registry = bootstrap_memory_plugin_registry();

    let container = resolve_native_sql_profile_for_runtime(
        &registry,
        MemorySqlDialect::Postgres,
        MemoryDeploymentMode::Container,
    )
    .expect("postgres container profile must resolve");
    assert_eq!(container.profile_id, "native-sql-phase1");
    assert_eq!(container.deployment_mode, MemoryDeploymentMode::Container);

    let test_runner = resolve_native_sql_profile_for_runtime(
        &registry,
        MemorySqlDialect::Sqlite,
        MemoryDeploymentMode::Test,
    )
    .expect("sqlite test-runner profile must resolve");
    assert_eq!(test_runner.profile_id, "local-embedded-phase1");
    assert_eq!(test_runner.deployment_mode, MemoryDeploymentMode::Test);
}

#[test]
fn explicit_implementation_selection_is_typed_and_dialect_safe() {
    let registry = bootstrap_memory_plugin_registry();
    let native = resolve_memory_profile_for_runtime(
        &registry,
        MemorySqlDialect::Postgres,
        MemoryDeploymentMode::Container,
        MemoryImplementationProfileSelection::NativeSql,
    )
    .expect("explicit native SQL profile must resolve on PostgreSQL");
    assert_eq!(
        native.implementation_kind,
        MemoryImplementationKind::NativeSql
    );

    assert!(resolve_memory_profile_for_runtime(
        &registry,
        MemorySqlDialect::Sqlite,
        MemoryDeploymentMode::Local,
        MemoryImplementationProfileSelection::NativeSql,
    )
    .is_err());
    assert!(resolve_memory_profile_for_runtime(
        &registry,
        MemorySqlDialect::Postgres,
        MemoryDeploymentMode::Server,
        MemoryImplementationProfileSelection::LocalEmbedded,
    )
    .is_err());
}

#[test]
fn runtime_target_env_is_validated_separately_from_database_dialect() {
    let _guard = env_test_lock();
    {
        let _scope = MemoryEnvScope::new(&[("SDKWORK_MEMORY_RUNTIME_TARGET", Some("container"))]);
        assert_eq!(
            resolve_memory_deployment_mode_from_env(MemorySqlDialect::Postgres)
                .expect("container target must resolve"),
            MemoryDeploymentMode::Container
        );
    }
    {
        let _scope = MemoryEnvScope::new(&[("SDKWORK_MEMORY_RUNTIME_TARGET", Some("desktop"))]);
        assert!(resolve_memory_deployment_mode_from_env(MemorySqlDialect::Sqlite).is_err());
    }
}

#[test]
fn retrieval_strategy_env_rejects_unimplemented_schemes() {
    let _guard = env_test_lock();
    {
        let _scope =
            MemoryEnvScope::new(&[("SDKWORK_MEMORY_RETRIEVAL_STRATEGY", Some("event_aware"))]);
        assert_eq!(
            resolve_memory_retrieval_strategy_from_env().unwrap(),
            MemoryRetrievalStrategy::EventAware
        );
    }
    {
        let _scope =
            MemoryEnvScope::new(&[("SDKWORK_MEMORY_RETRIEVAL_STRATEGY", Some("graph_magic"))]);
        assert!(resolve_memory_retrieval_strategy_from_env().is_err());
    }
}

/// The local-embedded profile (`SQLite` + explicit test runner) must bootstrap end to end.
///
/// This is the one plane that legitimately has no shared node registry to allocate a snowflake
/// `node_id` from, so the bootstrap must install the env/random node id explicitly rather than
/// depending on `id_generator`'s deliberately strict lazy path.
#[tokio::test]
#[allow(clippy::await_holding_lock)] // Serializes process-wide environment mutation for the full bootstrap.
async fn bootstrap_memory_runtime_from_env_with_sqlite() {
    let _guard = env_test_lock();
    let _env = MemoryEnvScope::new(&[
        ("SDKWORK_DATABASE_URL", Some("sqlite::memory:")),
        ("SDKWORK_MEMORY_RUNTIME_TARGET", Some("test-runner")),
        ("SDKWORK_MEMORY_IMPLEMENTATION_PROFILE", Some("local_embedded")),
        ("SDKWORK_MEMORY_RETRIEVAL_STRATEGY", Some("search_first")),
    ]);

    let runtime = bootstrap_memory_runtime_from_env()
        .await
        .expect("runtime bootstrap must succeed with in-memory sqlite");

    assert_eq!(
        runtime.profile.primary_plugin_id,
        sdkwork_memory_plugin_native_sql::NATIVE_SQL_PLUGIN_ID
    );
    assert_eq!(runtime.profile.profile_id, "local-embedded-phase1");
    assert_eq!(runtime.profile.deployment_mode, MemoryDeploymentMode::Test);
    assert_eq!(
        runtime.retrieval_strategy,
        MemoryRetrievalStrategy::SearchFirst
    );
    assert_eq!(
        runtime.core_runtime.profile().profile_id,
        runtime.profile.profile_id
    );
    for port in [
        "MemoryRecordStorePort",
        "MemoryEventStorePort",
        "MemoryAuditStorePort",
        "MemoryOutboxStorePort",
        "MemoryCandidateStorePort",
        "MemoryHabitStorePort",
        "MemoryRetrievalTraceStorePort",
        "MemoryGovernanceAccessPort",
        "MemorySpaceStorePort",
        "MemoryRetrieverPort",
    ] {
        assert!(runtime.core_runtime.has_port(port));
        assert_eq!(
            runtime.core_runtime.port_owner(port),
            Some(sdkwork_memory_plugin_native_sql::NATIVE_SQL_PLUGIN_ID)
        );
    }
    assert!(runtime
        .registry
        .get(&runtime.profile.primary_plugin_id)
        .is_some());
    assert!(
        runtime.data_plane.host_pool.is_none(),
        "sqlite bootstrap must not allocate an unused database-host pool"
    );
    runtime
        .data_plane
        .store()
        .ping()
        .await
        .expect("store ping must succeed");
}

/// A production lifecycle profile must reject the SQLite engine before any pool is created, and the
/// diagnostic must name the engine operator has to provide.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn production_runtime_rejects_sqlite_before_pool_creation() {
    let _guard = env_test_lock();
    let _env = MemoryEnvScope::new(&[
        ("SDKWORK_MEMORY_ENVIRONMENT", Some("production")),
        ("SDKWORK_MEMORY_CONFIG_PROFILE", Some("production")),
        ("SDKWORK_DATABASE_URL", Some("sqlite::memory:")),
        ("SDKWORK_MEMORY_RUNTIME_TARGET", None),
    ]);

    let error = match bootstrap_memory_runtime_from_env().await {
        Ok(_) => panic!("production SQLite runtime must be rejected"),
        Err(error) => error,
    };
    assert!(
        error.contains("PostgreSQL is required"),
        "production SQLite rejection must name the required engine, got: {error}"
    );
}
