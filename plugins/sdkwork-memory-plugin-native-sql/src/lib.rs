//! SDKWork Memory native SQL runtime plugin.

pub mod admin_tables;
pub mod canonical_data;
pub mod commercial_store;
pub mod consolidation;
mod filter_pushdown;
mod governance_access;
pub mod graph_store;
pub mod learning_jobs;
pub mod manifest;
pub mod native_sql_phase1_runtime;
pub mod policy_store;
pub mod pool_backend;
pub mod privacy;
pub mod readiness_store;
pub mod search_index;
mod space_data;
mod sqlx_compat;
pub mod store;

pub use admin_tables::*;
pub use commercial_store::*;
pub use consolidation::*;
pub use graph_store::*;
pub use learning_jobs::{
    FinishLearningJobCommand, InsertLearningJobCommand, NativeSqlClaimedEvalRun,
    NativeSqlLearningJobRow, UpdateEvalRunStateCommand,
};
pub use manifest::{
    build_native_sql_audit_store, build_native_sql_candidate_store, build_native_sql_event_store,
    build_native_sql_governance_access, build_native_sql_habit_store,
    build_native_sql_outbox_store, build_native_sql_record_store,
    build_native_sql_retrieval_trace_store, build_native_sql_retriever,
    build_native_sql_space_store, native_sql_manifest, native_sql_phase1_port_builders,
    validate_native_sql_port_builders, NativeSqlPortBuilder, NATIVE_SQL_PLUGIN_ID,
};
pub use native_sql_phase1_runtime::{
    build_native_sql_executable_runtime, validate_native_sql_phase1_ports, NativeSqlPhase1Runtime,
};
pub use policy_store::*;
pub use pool_backend::{connect_any_pool, normalize_memory_database_config, MemorySqlDialect};
pub use privacy::{escape_like_pattern, like_pattern, ExportCollectedPayload, ForgetScopeStats};
pub use readiness_store::*;
pub use store::*;
