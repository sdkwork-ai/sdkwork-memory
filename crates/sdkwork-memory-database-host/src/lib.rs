use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};
use sdkwork_database_lifecycle::{lifecycle_options_from_env, LifecycleOrchestrator};
use sdkwork_database_spi::{DatabaseAssetProvider, DatabaseManifest, DefaultDatabaseModule};
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool};

pub struct MemoryDatabaseHost {
    pool: DatabasePool,
    module: Arc<DefaultDatabaseModule>,
}

impl MemoryDatabaseHost {
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    pub fn module(&self) -> Arc<DefaultDatabaseModule> {
        self.module.clone()
    }
}

/// Fails closed when the pool's engine is not an engine this module declares.
///
/// `database/database.manifest.json` declares `databaseRole: "authoritative-server"` with
/// `engines: ["postgres"]`, and `DATABASE_FRAMEWORK_SPEC` section 344/421 requires exactly that
/// pairing. Without this check the lifecycle orchestrator simply resolves zero migrations for an
/// undeclared engine and reports success — a server process pointed at SQLite would "bootstrap"
/// against an empty schema and only fail later, at the first query, with an unrelated-looking
/// error. The check runs before `init()` so the rejection never touches the migration history
/// table.
fn ensure_pool_engine_is_declared(
    engine: DatabaseEngine,
    manifest: &DatabaseManifest,
) -> Result<(), String> {
    if manifest
        .engines
        .iter()
        .any(|declared| declared == &engine.to_string())
    {
        return Ok(());
    }
    Err(format!(
        "memory database module declares engines {declared:?}, so it cannot run its lifecycle on a \
         {engine} pool: DATABASE_FRAMEWORK_SPEC section 344 requires an authoritative-server \
         module to declare exactly [\"postgres\"]. Point SDKWORK_DATABASE_URL at the workspace \
         PostgreSQL profile (materialize .env.postgres from .env.postgres.example). The SQLite \
         dialect is not a module engine here — it is the client-local/test plane and is \
         materialized from tests/fixtures/database/sqlite by the native SQL store.",
        declared = manifest.engines,
    ))
}

/// Minimum PostgreSQL server version for the memory baseline: the schema uses
/// PostgreSQL 15 `NULLS NOT DISTINCT` unique indexes, so a PG14 server fails
/// here with an actionable gate instead of a raw syntax error from the first
/// DDL statement. Mirrors the data-plane assertion in
/// `sdkwork-intelligence-memory-repository-sqlx` (which sits downstream of this
/// crate, so the constant is restated rather than imported).
const MIN_POSTGRES_VERSION_NUM: i64 = 150_000;

/// Fails closed on a PostgreSQL server older than the baseline's requirements.
///
/// Runs before `init()` so an undersized server is named directly instead of
/// surfacing as an unrelated-looking `NULLS NOT DISTINCT` syntax error in the
/// middle of the migration.
async fn assert_postgres_server_version(pool: &sdkwork_database_sqlx::DatabasePool) -> Result<(), String> {
    let postgres_pool = pool.as_postgres().ok_or_else(|| {
        "memory database version gate expected a PostgreSQL pool".to_string()
    })?;
    let row: (i64,) = sqlx::query_as("SELECT current_setting('server_version_num')::bigint")
        .fetch_one(postgres_pool)
        .await
        .map_err(|error| format!("memory database could not read the PostgreSQL server version: {error}"))?;
    if row.0 < MIN_POSTGRES_VERSION_NUM {
        return Err(format!(
            "memory database requires PostgreSQL 15 or newer (server_version_num {} < {}): \
             the baseline schema uses NULLS NOT DISTINCT unique indexes",
            row.0, MIN_POSTGRES_VERSION_NUM
        ));
    }
    Ok(())
}

pub async fn bootstrap_memory_database(pool: DatabasePool) -> Result<MemoryDatabaseHost, String> {
    let app_root = resolve_app_root();
    let module = Arc::new(
        DefaultDatabaseModule::from_app_root(&app_root)
            .map_err(|error| format!("load memory database module failed: {error}"))?,
    );
    let manifest = DatabaseManifest::from_file(module.manifest_path())
        .map_err(|error| format!("read memory database manifest failed: {error}"))?;
    ensure_pool_engine_is_declared(pool.engine(), &manifest)
        .map_err(|error| format!("memory database engine admission failed: {error}"))?;
    if pool.engine() == DatabaseEngine::Postgres {
        assert_postgres_server_version(&pool)
            .await
            .map_err(|error| format!("memory database server version admission failed: {error}"))?;
    }
    let options = lifecycle_options_from_env("MEMORY", &manifest);
    let orchestrator =
        LifecycleOrchestrator::new(pool.clone(), module.clone()).with_applied_by("sdkwork-memory");

    orchestrator
        .init()
        .await
        .map_err(|error| format!("memory database init failed: {error}"))?;

    if options.auto_migrate {
        orchestrator
            .migrate()
            .await
            .map_err(|error| format!("memory database migrate failed: {error}"))?;
    }

    Ok(MemoryDatabaseHost { pool, module })
}

pub async fn bootstrap_memory_database_from_env() -> Result<MemoryDatabaseHost, String> {
    let _ = dotenvy::dotenv();
    let config = DatabaseConfig::from_env("MEMORY")
        .map_err(|error| format!("read memory database config failed: {error}"))?;
    let pool = create_pool_from_config(config)
        .await
        .map_err(|error| format!("create memory database pool failed: {error}"))?;
    bootstrap_memory_database(pool).await
}

fn resolve_app_root() -> PathBuf {
    std::env::var("SDKWORK_MEMORY_APP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        })
}
