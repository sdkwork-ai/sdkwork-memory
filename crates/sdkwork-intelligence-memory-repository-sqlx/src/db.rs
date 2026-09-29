use sdkwork_database_config::DatabaseConfig;
use sdkwork_database_id::SnowflakeIdGenerator;
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool, PoolError};
use sdkwork_memory_plugin_native_sql::{
    pool_backend::normalize_memory_database_config, NativeSqlMemoryStore,
};
use sdkwork_utils_rust::is_blank;

pub type MemoryDatabasePool = DatabasePool;

pub async fn connect_memory_pool_from_env() -> Result<MemoryDatabasePool, PoolError> {
    // DATABASE_SPEC serviceCode `MEMORY` → env prefix `SDKWORK_MEMORY_*`
    let config = DatabaseConfig::from_env("MEMORY")?;
    let config = normalize_memory_database_config(config);
    create_pool_from_config(config).await
}

pub async fn open_native_sql_store_from_pool(
    pool: &MemoryDatabasePool,
    id_generator: SnowflakeIdGenerator,
) -> Result<NativeSqlMemoryStore, String> {
    // The SQLite pool variant only exists in builds that opted in to the
    // crate's `sqlite` feature; server builds stay PostgreSQL-only by
    // default (DATABASE_SPEC: authoritative-server persistence).
    #[cfg(feature = "sqlite")]
    let sqlite_pool_present = pool.as_sqlite().is_some();
    #[cfg(not(feature = "sqlite"))]
    let sqlite_pool_present = false;
    if !sqlite_pool_present && pool.as_postgres().is_none() {
        let configured_engine = std::env::var("SDKWORK_DATABASE_ENGINE").ok();
        if is_blank(configured_engine.as_deref()) {
            return Err(
                "memory native sql store requires sqlite or postgres database pool".to_string(),
            );
        }
        return Err(format!(
            "memory native sql store requires sqlite or postgres; SDKWORK_DATABASE_ENGINE={} is not supported",
            configured_engine.unwrap_or_default()
        ));
    }

    NativeSqlMemoryStore::from_database_pool_with_id_generator(pool, id_generator)
        .await
        .map_err(|error| error.to_string())
}
