//! API assembly for sdkwork-memory.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
// SDKWORK-ASSEMBLY-LIB-CUSTOM

mod bootstrap;
mod generated;

pub use bootstrap::{
    assemble_api_router, assemble_api_router_from_env, ApiAssembly, ApiAssemblyContribution,
    MemoryReadinessCheck, MemoryStandaloneApplication, run_database_migrate_only,
};

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
