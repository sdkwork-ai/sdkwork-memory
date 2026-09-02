//! API assembly for sdkwork-memory.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{assemble_api_router, ApiAssembly, ApiAssemblyContribution, assemble_api_router_from_env, MemoryReadinessCheck, MemoryStandaloneApplication, run_database_migrate_only, web_module};

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
