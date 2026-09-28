//! API assembly for sdkwork-memory.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{
    app_api_route_manifest, assemble_api_router, assemble_api_router_from_env,
    assemble_api_router_retaining_background_from_env, assemble_app_api_contribution_from_env,
    run_database_migrate_only, web_module, ApiAssembly, ApiAssemblyContribution,
    MemoryBackgroundWorkers, MemoryReadinessCheck,
};

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
