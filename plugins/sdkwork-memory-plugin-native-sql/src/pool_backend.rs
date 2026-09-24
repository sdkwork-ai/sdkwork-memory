use crate::sqlx_compat as sqlx;
use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};
use sdkwork_database_sqlx::any::create_any_pool;
use sqlx::AnyPool;

use crate::store::NativeSqlStoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySqlDialect {
    Sqlite,
    Postgres,
}

impl MemorySqlDialect {
    pub fn from_config(config: &DatabaseConfig) -> Self {
        match config.engine {
            DatabaseEngine::Postgres => Self::Postgres,
            DatabaseEngine::Sqlite => Self::Sqlite,
        }
    }
}

pub fn normalize_memory_database_url(url: &str) -> String {
    match url {
        "sqlite::memory:" | "sqlite:memory:" => "sqlite::memory:?cache=shared".to_string(),
        other => other.to_string(),
    }
}

pub fn normalize_memory_database_config(mut config: DatabaseConfig) -> DatabaseConfig {
    config.url = normalize_memory_database_url(&config.url);
    if matches!(config.engine, DatabaseEngine::Sqlite) {
        // sqlx AnyPool cannot carry SQLite's connection-level PRAGMAs through
        // its URL parser or a connect hook, so the store enables them once on
        // the pool's single connection after creation. Pool recycling would
        // silently drop those PRAGMAs (foreign key enforcement included), so
        // the connection is pinned for the process lifetime instead.
        config.max_connections = 1;
        config.idle_timeout_secs = u64::MAX;
        config.max_lifetime_secs = u64::MAX;
    }
    config
}

pub async fn connect_any_pool(
    config: &DatabaseConfig,
) -> Result<(AnyPool, MemorySqlDialect), NativeSqlStoreError> {
    sqlx::any::install_default_drivers();
    let config = normalize_memory_database_config(config.clone());
    let dialect = MemorySqlDialect::from_config(&config);
    let pool = create_any_pool(&config).await?;
    if matches!(dialect, MemorySqlDialect::Sqlite) {
        // Applied once to the single pinned connection (see
        // `normalize_memory_database_config`): foreign key enforcement plus a
        // busy timeout so cross-process lock contention degrades into a wait
        // instead of an immediate SQLITE_BUSY error.
        sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;
        sqlx::query("PRAGMA busy_timeout = 5000").execute(&pool).await?;
    }
    Ok((pool, dialect))
}
