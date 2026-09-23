use std::path::PathBuf;

use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};
use sdkwork_database_sqlx::create_pool_from_config;
use sdkwork_memory_database_host::bootstrap_memory_database;

/// The memory module is `databaseRole: "authoritative-server"`, so its manifest declares exactly
/// `engines: ["postgres"]` (DATABASE_FRAMEWORK_SPEC section 344/421). A SQLite pool must therefore
/// be rejected **before** the lifecycle touches the migration history table.
///
/// This replaces an earlier test that drove the module's migration chain from a SQLite pool and
/// expected 10 applied migrations. That premise cannot hold: the module declares no SQLite engine,
/// so the orchestrator resolved zero migrations and reported success — a fail-open that made a
/// server process believe it had a schema when it had none. The SQLite dialect itself is covered
/// where it actually lives, by
/// `plugins/sdkwork-memory-plugin-native-sql/tests/sqlite_store_contract.rs`.
#[tokio::test]
async fn sqlite_pool_is_rejected_before_the_module_lifecycle_runs() {
    let database_path = temporary_database_path();
    let database_url = format!("sqlite://{}?mode=rwc", database_path.display());
    let config = DatabaseConfig {
        engine: DatabaseEngine::Sqlite,
        url: database_url,
        max_connections: 1,
        ..DatabaseConfig::default()
    };
    let pool = create_pool_from_config(config)
        .await
        .expect("create SQLite admission-probe pool");

    let error = match bootstrap_memory_database(pool.clone()).await {
        Ok(_) => panic!("a SQLite pool must not drive the authoritative-server module lifecycle"),
        Err(error) => error,
    };

    for expected in [
        "engine admission failed",
        "declares engines [\"postgres\"]",
        "cannot run its lifecycle on a sqlite pool",
        "DATABASE_FRAMEWORK_SPEC section 344",
        "SDKWORK_DATABASE_URL",
    ] {
        assert!(
            error.contains(expected),
            "admission diagnostic must contain {expected:?}, got: {error}"
        );
    }

    // The rejection must happen before `init()`, so no migration history table may exist.
    let sqlite = pool.as_sqlite().expect("SQLite pool");
    let history_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE name = 'ops_schema_migration_history'",
    )
    .fetch_one(sqlite)
    .await
    .expect("inspect SQLite schema after rejection");
    assert_eq!(
        history_tables, 0,
        "engine admission must reject before any migration history table is created"
    );

    drop(pool);
    let _ = std::fs::remove_file(database_path);
}

fn temporary_database_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "sdkwork-memory-admission-{}-{}.sqlite",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos()
    ))
}
