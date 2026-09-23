//! SDKWork Memory database pool bootstrap via `sdkwork-database`.

use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};
use sdkwork_database_id::{NodeAllocatorConfig, SnowflakeIdGenerator, SnowflakeNodeAllocator};
use sdkwork_database_sqlx::create_pool_from_config;
use sdkwork_memory_plugin_native_sql::{
    normalize_memory_database_config, MemorySqlDialect, NativeSqlMemoryStore, NativeSqlPhase1Runtime,
};
use sdkwork_memory_spi::MemoryDeploymentMode;

use crate::db::{open_native_sql_store_from_pool, MemoryDatabasePool};
use crate::runtime::resolve_memory_deployment_mode_from_env;

pub use sdkwork_memory_database_host::{
    bootstrap_memory_database, bootstrap_memory_database_from_env, MemoryDatabaseHost,
};

/// Materialized phase-1 SQL runtime plus optional postgres host pool for database-host migrations.
pub struct MemoryDataPlane {
    pub phase1: NativeSqlPhase1Runtime,
    /// Set when postgres host bootstrap runs via `sdkwork-memory-database-host`.
    pub host_pool: Option<MemoryDatabasePool>,
}

impl MemoryDataPlane {
    pub fn store(&self) -> &NativeSqlMemoryStore {
        self.phase1.store()
    }
}

pub async fn connect_and_bootstrap_memory_database_from_env() -> Result<MemoryDatabaseHost, String>
{
    let config = DatabaseConfig::from_env("MEMORY").map_err(|error| error.to_string())?;
    let config = normalize_memory_database_config(config);
    let dialect = database_dialect(&config);
    reject_sqlite_server_role_engine(dialect, resolve_memory_deployment_mode_from_env(dialect)?)?;
    let pool = create_pool_from_config(config)
        .await
        .map_err(|error| error.to_string())?;
    bootstrap_memory_database(pool).await
}

/// Maps the resolved database engine onto the plugin dialect used by profile and mode resolution.
fn database_dialect(config: &DatabaseConfig) -> MemorySqlDialect {
    match config.engine {
        DatabaseEngine::Postgres => MemorySqlDialect::Postgres,
        DatabaseEngine::Sqlite => MemorySqlDialect::Sqlite,
    }
}

/// Fails closed when this process — server-authoritative by architecture — is configured with the
/// SQLite engine outside an explicit test-runner declaration.
///
/// `database/database.manifest.json` declares `databaseRole: authoritative-server` with
/// `engines: ["postgres"]`, so ENVIRONMENT_SPEC section 7.2 applies: "A module that is
/// server-authoritative by architecture MUST NOT silently accept SQLite when the SQLite URL is
/// present; its host resolves the server role and rejects a non-PostgreSQL pool with an
/// actionable diagnostic."
///
/// Every server entrypoint must apply this one rule — the runtime data-plane bootstrap, the
/// database-host bootstrap, and the assembly's `db-migrate` lifecycle all funnel through here —
/// so the diagnostic is identical no matter which path the operator hits first.
pub fn reject_sqlite_server_role_engine(
    dialect: MemorySqlDialect,
    deployment_mode: MemoryDeploymentMode,
) -> Result<(), String> {
    if dialect == MemorySqlDialect::Postgres || deployment_mode == MemoryDeploymentMode::Test {
        return Ok(());
    }
    Err(format!(
        "authoritative-server Memory rejects the SQLite engine in deployment mode {deployment_mode:?}: \
         PostgreSQL is required for every server role (ENVIRONMENT_SPEC section 7.2). \
         Resolve the workspace PostgreSQL profile by setting SDKWORK_DATABASE_URL, or by \
         materializing .env.postgres from .env.postgres.example at the application root. \
         SQLite resolves only for an explicit test runner: set \
         SDKWORK_MEMORY_RUNTIME_TARGET=test-runner to declare one."
    ))
}

/// Applies [`reject_sqlite_server_role_engine`] to the process environment without building a
/// pool, for entrypoints that only need the admission decision (for example the assembly's
/// `db-migrate` lifecycle, which must reject SQLite before touching any migration history table).
pub fn ensure_server_role_database_engine_from_env() -> Result<(), String> {
    let config = DatabaseConfig::from_env("MEMORY").map_err(|error| error.to_string())?;
    let config = normalize_memory_database_config(config);
    let dialect = database_dialect(&config);
    reject_sqlite_server_role_engine(dialect, resolve_memory_deployment_mode_from_env(dialect)?)
}

/// Whether `deployment_mode` requires a node id allocated from the shared database registry.
///
/// This is an **architectural** question, not an environment-variable one: a mode needs a
/// registry-allocated node id exactly when several replicas can write to the same store with the
/// same id space. Server, Container and Private deployments are replica sets, so a colliding random
/// node id would produce duplicate snowflake ids — they must allocate from the registry and fail
/// loudly if they cannot.
///
/// `Local`, `Test` and `EvalOnly` are single-process planes by construction: the local-embedded
/// profile owns its SQLite file outright and the test/eval planes are ephemeral, so they have no
/// registry table to allocate from and no peer that could collide with them. For these the
/// bootstrap installs the env/random node id explicitly via
/// `platform::init_snowflake_fallback_generator`, which is a deliberate, logged decision rather
/// than a silent one.
fn registry_node_allocation_required(deployment_mode: MemoryDeploymentMode) -> bool {
    matches!(
        deployment_mode,
        MemoryDeploymentMode::Server | MemoryDeploymentMode::Container | MemoryDeploymentMode::Private
    )
}

/// Single bootstrap entry for the API server and integration tests.
pub async fn bootstrap_memory_data_plane_from_env() -> Result<MemoryDataPlane, String> {
    let config = DatabaseConfig::from_env("MEMORY").map_err(|error| error.to_string())?;
    let config = normalize_memory_database_config(config);

    // Connection capacity is a process budget owned once per OS process by
    // `SDKWORK_DATABASE_MAX_CONNECTIONS` / `SDKWORK_DATABASE_MIN_CONNECTIONS`
    // (SDKWork Process-Shared Database Pool Standard section 5). The retired
    // module-prefixed `SDKWORK_MEMORY_DB_*` overrides must not be read here: they
    // would multiply the budget per module and silently disagrees with the
    // framework-reserved temporary driver capacity.
    tracing::info!(
        engine = ?config.engine,
        max_connections = config.max_connections,
        min_connections = config.min_connections,
        url_safe = !config.url.contains("password"),
        "memory database pool configured from the process budget"
    );

    let auto_migrate = std::env::var("SDKWORK_DATABASE_AUTO_MIGRATE")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(false);

    let dialect = database_dialect(&config);
    let deployment_mode = resolve_memory_deployment_mode_from_env(dialect)?;
    reject_sqlite_server_role_engine(dialect, deployment_mode)?;

    // Always create a pool up front so we can use it for both Snowflake
    // node_id allocation and store creation. This is especially important
    // for SQLite in-memory databases which must share the same connection.
    let pool = create_pool_from_config(config.clone())
        .await
        .map_err(|error| format!("create memory database pool failed: {error}"))?;

    // Allocate a Snowflake node_id from the database before creating the
    // store. This prevents ID collisions in multi-instance deployments.
    let id_generator = allocate_and_init_snowflake_node(&pool, deployment_mode).await?;

    // Create the phase-1 runtime from the shared pool to avoid duplicate connections.
    let (phase1, host_pool) = if config.engine == DatabaseEngine::Postgres {
        if auto_migrate {
            bootstrap_memory_database(pool.clone())
                .await
                .map_err(|error| format!("memory database migrate failed: {error}"))?;
        }
        let store = open_native_sql_store_from_pool(&pool, id_generator.clone())
            .await
            .map_err(|error| error.to_string())?;
        let phase1 = NativeSqlPhase1Runtime::from_store(store);
        (phase1, Some(pool))
    } else if auto_migrate {
        let phase1 = NativeSqlPhase1Runtime::connect(&config)
            .await
            .map_err(|error| error.to_string())?;
        (phase1, None)
    } else {
        let store = open_native_sql_store_from_pool(&pool, id_generator)
            .await
            .map_err(|error| error.to_string())?;
        let phase1 = NativeSqlPhase1Runtime::from_store(store);
        (phase1, None)
    };

    sdkwork_memory_plugin_native_sql::validate_native_sql_phase1_ports(phase1.store())
        .await
        .map_err(|error| error.to_string())?;

    Ok(MemoryDataPlane { phase1, host_pool })
}

/// Allocate a Snowflake node_id from the database and initialize the global ID generator.
///
/// Replica-set modes must get their node id from the shared registry; single-process modes install
/// the env/random fallback explicitly. The real allocator failure is always reported, whether or
/// not the fallback is allowed, so a registry outage is never silently absorbed.
async fn allocate_and_init_snowflake_node(
    pool: &MemoryDatabasePool,
    deployment_mode: MemoryDeploymentMode,
) -> Result<SnowflakeIdGenerator, String> {
    let config = NodeAllocatorConfig::from_service_name("memory-service");
    match SnowflakeNodeAllocator::allocate_process_generator(pool, &config).await {
        Ok((generator, lease)) => {
            let node_id = generator.node_id();
            tracing::info!(
                node_id,
                "memory snowflake node_id allocated from database registry"
            );
            Ok(
                sdkwork_intelligence_memory_service::platform::init_id_generator(
                    generator,
                    Some(lease),
                ),
            )
        }
        Err(error) if registry_node_allocation_required(deployment_mode) => Err(format!(
            "memory snowflake node_id allocation from the shared registry failed in the \
             {deployment_mode:?} deployment mode: {error}. Replica-set modes require a \
             registry-allocated node id because a random one can collide across replicas."
        )),
        Err(error) => {
            tracing::warn!(
                %error,
                ?deployment_mode,
                "memory snowflake node_id allocation from the shared registry failed; \
                 this deployment mode has no registry, so the explicit env/random fallback is used"
            );
            sdkwork_intelligence_memory_service::platform::init_snowflake_fallback_generator()
                .map_err(|fallback_error| {
                    format!(
                        "memory snowflake node_id allocation failed ({error}) and the \
                         {deployment_mode:?} fallback generator could not be installed: {}",
                        fallback_error.detail
                    )
                })
        }
    }
}
