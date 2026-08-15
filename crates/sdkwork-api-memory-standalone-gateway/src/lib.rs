//! Thin standalone gateway library surface.
//! Service construction, route composition, readiness, and background workers
//! are owned by `sdkwork-api-memory-assembly`; the gateway binaries only keep
//! process tracing bootstrap (API_ASSEMBLY_SPEC §6.1).
mod observability;

pub use observability::init_tracing;
