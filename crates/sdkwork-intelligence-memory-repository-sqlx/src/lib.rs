//! SQL storage support for SDKWork Memory.

mod bootstrap;
pub mod db;
pub mod runtime;

// DATABASE_SPEC.md section 34: repository-sqlx anchors the shared repository crate.
use sdkwork_database_repository as _;

pub use bootstrap::{
    bootstrap_memory_data_plane_from_env, bootstrap_memory_database,
    bootstrap_memory_database_from_env, connect_and_bootstrap_memory_database_from_env,
    ensure_server_role_database_engine_from_env, reject_sqlite_server_role_engine, MemoryDataPlane,
};
pub use db::{connect_memory_pool_from_env, open_native_sql_store_from_pool, MemoryDatabasePool};
pub use runtime::{
    bootstrap_memory_plugin_registry, bootstrap_memory_runtime_from_env,
    resolve_memory_deployment_mode_from_env, resolve_memory_implementation_profile_from_env,
    resolve_memory_profile_for_runtime, resolve_memory_retrieval_strategy_from_env,
    resolve_native_sql_phase1_profile, resolve_native_sql_profile_for_dialect,
    resolve_native_sql_profile_for_runtime, validate_memory_plugin_registry,
    MemoryImplementationProfileSelection, MemoryRuntime,
};
