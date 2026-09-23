use std::sync::OnceLock;

use rand::Rng;
use sdkwork_database_id::{NodeLease, SnowflakeIdGenerator};
use sdkwork_id_core::max_snowflake_node_id;
use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceResult};

pub fn tenant_id_i64(tenant_id: u64) -> MemoryServiceResult<i64> {
    i64::try_from(tenant_id)
        .map_err(|_| MemoryServiceError::validation("tenantId is out of storage range"))
}

pub fn space_id_i64(space_id: u64) -> MemoryServiceResult<i64> {
    i64::try_from(space_id)
        .map_err(|_| MemoryServiceError::validation("spaceId is out of storage range"))
}

pub fn optional_u64_as_i64(value: Option<u64>) -> MemoryServiceResult<Option<i64>> {
    value.map(space_id_i64).transpose()
}

pub fn optional_i64_as_u64(value: Option<i64>) -> Option<u64> {
    value.and_then(|id| u64::try_from(id.max(0)).ok())
}

// ---------------------------------------------------------------------------
// Snowflake ID generator — database-backed with env fallback
// ---------------------------------------------------------------------------

/// Holds the initialized generator and an optional database lease.
///
/// The lease keeps the database heartbeat alive while the generator is in use.
/// When the process exits, the heartbeat stops and the lease expires after its
/// TTL, allowing another process to reclaim the node_id.
struct IdGeneratorHolder {
    generator: SnowflakeIdGenerator,
    _lease: Option<NodeLease>,
}

static ID_GENERATOR: OnceLock<IdGeneratorHolder> = OnceLock::new();

/// Initialize the global ID generator from a database-allocated node_id.
///
/// This is the recommended initialization path for production. Call this
/// during application bootstrap after the database pool is available.
///
/// The `lease` must be kept alive for as long as the process generates IDs.
/// It is stored in the global holder and dropped when the process exits.
pub fn init_id_generator(
    generator: SnowflakeIdGenerator,
    lease: Option<NodeLease>,
) -> SnowflakeIdGenerator {
    let _ = ID_GENERATOR.set(IdGeneratorHolder {
        generator,
        _lease: lease,
    });
    ID_GENERATOR
        .get()
        .expect("snowflake generator must be initialized")
        .generator
        .clone()
}

/// Fallback: resolve a node_id from env var or a random value.
///
/// Used only when database-backed allocation is not available (e.g. dev/test).
/// Uses `SDKWORK_MEMORY_SNOWFLAKE_NODE_ID` if set, otherwise a random u16
/// in `0..1024` to avoid collisions between processes on the same host.
fn resolve_snowflake_node_id() -> u16 {
    if let Ok(value) = std::env::var("SDKWORK_MEMORY_SNOWFLAKE_NODE_ID") {
        if let Some(parsed) =
            sdkwork_utils_rust::parse_int(&value).and_then(|parsed| u16::try_from(parsed).ok())
        {
            return parsed;
        }
    }

    // Random node_id to avoid collisions between processes on the same host.
    rand::thread_rng().gen_range(0..=max_snowflake_node_id())
}

/// Install the development/test fallback node_id **explicitly**.
///
/// [`id_generator`]'s lazy path is deliberately strict: it refuses to invent a random node_id
/// whenever [`memory_id_fallback_is_forbidden`] holds, which is any process that declared an
/// explicit runtime target (and therefore every server deployment). That guard is exactly right —
/// but it also made the fallback unreachable for the modes that *cannot* have a shared node
/// registry at all (an embedded SQLite store has no registry table to allocate from). The
/// bootstrap resolves the deployment mode, decides whether a registry-allocated node id is
/// architecturally required, and — for the modes where it is not — calls this function to install
/// the fallback as a deliberate, logged decision instead of an implicit one.
///
/// Idempotent: if a generator is already installed (for example a database-allocated one) this
/// returns it unchanged and never overwrites it.
pub fn init_snowflake_fallback_generator() -> MemoryServiceResult<SnowflakeIdGenerator> {
    if let Some(holder) = ID_GENERATOR.get() {
        return Ok(holder.generator.clone());
    }
    let node_id = resolve_snowflake_node_id();
    tracing::warn!(
        node_id,
        "memory snowflake generator installed from the env/random fallback node_id; \
         this process has no shared node registry to allocate from"
    );
    let generator = SnowflakeIdGenerator::new(node_id).map_err(|error| {
        MemoryServiceError::storage(format!(
            "snowflake fallback generator rejected node_id {node_id}: {error}"
        ))
    })?;
    Ok(init_id_generator(generator, None))
}

fn id_generator() -> MemoryServiceResult<&'static SnowflakeIdGenerator> {
    if memory_id_fallback_is_forbidden() && ID_GENERATOR.get().is_none() {
        return Err(MemoryServiceError::storage(
            "snowflake ID generator is not initialized; database bootstrap must allocate a node_id in production-like environments",
        ));
    }

    let holder = ID_GENERATOR.get_or_init(|| {
        let node_id = resolve_snowflake_node_id();
        tracing::warn!(
            node_id,
            "memory snowflake generator using dev fallback (env/random node_id) — \
             database-backed allocation was not initialized"
        );
        let generator = SnowflakeIdGenerator::new(node_id).unwrap_or_else(|error| {
            tracing::error!(%error, node_id, "memory snowflake generator init failed");
            // Last-resort dev-only fallback with a fixed node to avoid panic.
            SnowflakeIdGenerator::new(0).expect("snowflake node_id 0 must initialize")
        });
        IdGeneratorHolder {
            generator,
            _lease: None,
        }
    });
    Ok(&holder.generator)
}

pub fn shared_id_generator() -> MemoryServiceResult<SnowflakeIdGenerator> {
    id_generator().cloned()
}

pub fn next_numeric_id() -> MemoryServiceResult<u64> {
    let id = id_generator()?
        .generate()
        .map_err(|error| MemoryServiceError::storage(format!("id generation failed: {error}")))?;
    u64::try_from(id).map_err(|_| MemoryServiceError::storage(format!("id out of u64 range: {id}")))
}

pub fn snowflake_initialized() -> bool {
    ID_GENERATOR.get().is_some()
}

pub fn current_timestamp() -> String {
    sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None)
}

/// Returns true when the runtime is configured for a production-like environment.
pub fn is_production_like_environment() -> bool {
    sdkwork_memory_contract::memory_is_production_like_environment()
}

/// ID generation is stricter than legacy feature environment detection:
/// unknown or absent release configuration must never enable a random node.
pub fn memory_id_fallback_is_forbidden() -> bool {
    let lifecycle = [
        "SDKWORK_MEMORY_ENVIRONMENT",
        "SDKWORK_MEMORY_CONFIG_PROFILE",
        "SDKWORK_CLOUDROUTER_ENVIRONMENT",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
    });
    let deployment_is_explicit = [
        "SDKWORK_MEMORY_DEPLOYMENT_PROFILE",
        "SDKWORK_CLOUDROUTER_DEPLOYMENT_PROFILE",
        "SDKWORK_MEMORY_RUNTIME_TARGET",
        "SDKWORK_CLOUDROUTER_RUNTIME_TARGET",
    ]
    .into_iter()
    .any(|key| std::env::var(key).is_ok());
    memory_id_fallback_policy(
        lifecycle.as_deref(),
        deployment_is_explicit,
        cfg!(debug_assertions),
    )
}

fn memory_id_fallback_policy(
    lifecycle: Option<&str>,
    deployment_is_explicit: bool,
    debug_build: bool,
) -> bool {
    if let Some(value) = lifecycle {
        return !matches!(value, "development" | "dev" | "test");
    }
    deployment_is_explicit || !debug_build
}

pub fn deployment_environment_label() -> &'static str {
    if is_production_like_environment() {
        "production"
    } else {
        "development"
    }
}

pub fn elapsed_millis_i64(started: std::time::Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis())
        .unwrap_or(i64::MAX)
        .max(0)
}

pub fn stable_query_hash(query: &str) -> String {
    let normalized = query.trim().to_lowercase();
    format!(
        "sha256:{}",
        sdkwork_utils_rust::sha256_hash(normalized.as_bytes())
    )
}

pub fn parse_numeric_id(value: &str) -> Option<u64> {
    sdkwork_utils_rust::parse_int(value).and_then(|parsed| u64::try_from(parsed).ok())
}

pub fn parse_required_numeric_id(value: &str, field: &str) -> MemoryServiceResult<u64> {
    parse_numeric_id(value)
        .ok_or_else(|| MemoryServiceError::storage(format!("{field} must be numeric")))
}

pub fn non_negative_i64_as_u64(value: i64, field: &str) -> MemoryServiceResult<u64> {
    u64::try_from(value.max(0))
        .map_err(|_| MemoryServiceError::storage(format!("{field} must be non-negative")))
}

pub fn read_env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| sdkwork_utils_rust::parse_int(&value))
        .and_then(|parsed| u64::try_from(parsed).ok())
        .unwrap_or(default)
}

pub fn read_env_usize(key: &str, default: usize) -> usize {
    read_env_u64(key, default as u64)
        .try_into()
        .unwrap_or(default)
}

pub use sdkwork_utils_rust::{
    cursor_window_page_info, PageInfo, DEFAULT_LIST_PAGE_SIZE as DEFAULT_PAGE_SIZE,
    MAX_LIST_PAGE_SIZE as MAX_PAGE_SIZE,
};

#[cfg(test)]
mod id_policy_tests {
    use super::memory_id_fallback_policy;

    #[test]
    fn id_fallback_policy_is_explicit_and_release_safe() {
        assert!(!memory_id_fallback_policy(Some("dev"), true, false));
        assert!(memory_id_fallback_policy(Some("production"), false, true));
        assert!(memory_id_fallback_policy(Some("unknown"), false, true));
        assert!(memory_id_fallback_policy(None, true, true));
        assert!(memory_id_fallback_policy(None, false, false));
        assert!(!memory_id_fallback_policy(None, false, true));
    }
}

/// Validates a page size against the platform range, defaulting to
/// `DEFAULT_PAGE_SIZE` when absent.
pub fn validated_page_size(page_size: Option<i32>) -> MemoryServiceResult<i32> {
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
        return Err(MemoryServiceError::invalid_parameter(format!(
            "page_size must be between 1 and {MAX_PAGE_SIZE}"
        )));
    }
    Ok(page_size)
}

/// OpenAPI `topK` maximum for retrieval requests.
pub const MAX_RETRIEVAL_TOP_K: i32 = 100;

/// Maximum space ids per retrieval or export request.
pub const MAX_SCOPE_SPACE_IDS: usize = 32;

/// Default maximum input events per extraction request.
pub const DEFAULT_MAX_EXTRACTION_INPUT_EVENTS: usize = 1_000;

/// Default maximum events exported per job.
pub const DEFAULT_MAX_EXPORT_EVENTS: usize = 100_000;

/// Maximum memory ids per targeted `scope: "memory"` forget request.
///
/// The targeted forget path runs one retrieve plus one hard delete per id, each
/// in its own transaction that takes a space lock, so an unbounded id list is an
/// unbounded N+1 write path driven by request content. The bound equals one list
/// page: a caller can forget exactly what a single `memories.list` response
/// returned, no more.
///
/// Deliberately not environment-tunable, unlike the worker-cadence knobs above:
/// the app-api contract declares this as `maxItems` on `MemoryForgetRequest.
/// memoryIds`, so a tunable ceiling would let the runtime accept payloads the
/// published contract rejects. `check_sdkwork_memory_architecture_alignment.mjs`
/// asserts the two never diverge.
pub const MAX_FORGET_MEMORY_IDS: usize = sdkwork_utils_rust::MAX_LIST_PAGE_SIZE as usize;

/// Default maximum provider bindings materialized for health aggregation.
pub const DEFAULT_MAX_PROVIDER_HEALTH_BINDINGS: usize = 500;

pub fn max_extraction_input_events() -> usize {
    read_env_usize(
        "SDKWORK_MEMORY_EXTRACTION_MAX_EVENTS",
        DEFAULT_MAX_EXTRACTION_INPUT_EVENTS,
    )
}

pub fn max_export_events() -> usize {
    read_env_usize(
        "SDKWORK_MEMORY_EXPORT_MAX_EVENTS",
        DEFAULT_MAX_EXPORT_EVENTS,
    )
}

pub fn max_provider_health_bindings() -> usize {
    read_env_usize(
        "SDKWORK_MEMORY_PROVIDER_HEALTH_MAX_BINDINGS",
        DEFAULT_MAX_PROVIDER_HEALTH_BINDINGS,
    )
}

pub fn clamp_retrieval_top_k(top_k: i32) -> i32 {
    top_k.clamp(1, MAX_RETRIEVAL_TOP_K)
}

/// Standard cursor-mode pagination metadata for memory list responses.
pub fn memory_cursor_page_info(
    page_size: i32,
    has_more: bool,
    next_cursor: Option<String>,
) -> PageInfo {
    cursor_window_page_info(
        Some(usize::try_from(page_size).unwrap_or(DEFAULT_PAGE_SIZE as usize)),
        if has_more { next_cursor } else { None },
        has_more,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_required_numeric_id_rejects_non_numeric_values() {
        assert_eq!(parse_required_numeric_id("42", "id").unwrap(), 42);
        assert!(parse_required_numeric_id("abc", "id").is_err());
    }

    #[test]
    fn non_negative_i64_as_u64_rejects_negative_storage_values() {
        assert_eq!(non_negative_i64_as_u64(7, "field").unwrap(), 7);
        assert_eq!(non_negative_i64_as_u64(-3, "field").unwrap(), 0);
    }

    #[test]
    fn stable_query_hash_is_normalized_and_cryptographic() {
        let first = stable_query_hash("  Preferred Editor ");
        let second = stable_query_hash("preferred editor");

        assert_eq!(first, second);
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), "sha256:".len() + 64);
        assert!(!first.contains("preferred editor"));
    }

    #[test]
    fn page_size_uses_default_and_rejects_out_of_range_values() {
        assert_eq!(validated_page_size(None).unwrap(), DEFAULT_PAGE_SIZE);
        assert_eq!(validated_page_size(Some(1)).unwrap(), 1);
        assert_eq!(
            validated_page_size(Some(MAX_PAGE_SIZE)).unwrap(),
            MAX_PAGE_SIZE
        );
        assert!(validated_page_size(Some(0)).is_err());
        assert!(validated_page_size(Some(-1)).is_err());
        assert!(validated_page_size(Some(MAX_PAGE_SIZE + 1)).is_err());
        assert_eq!(
            validated_page_size(Some(0)).unwrap_err().code,
            "invalid_parameter"
        );
    }
}
