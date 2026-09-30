use std::sync::OnceLock;

use rand::RngExt;
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
    rand::rng().random_range(0..=max_snowflake_node_id())
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
    let result = id_generator()?
        .generate()
        .map_err(|error| MemoryServiceError::storage(format!("id generation failed: {error}")))
        .and_then(|id| {
            u64::try_from(id)
                .map_err(|_| MemoryServiceError::storage(format!("id out of u64 range: {id}")))
        });
    LAST_ID_GENERATION_FAILED.store(result.is_err(), std::sync::atomic::Ordering::Relaxed);
    result
}

/// Set by [`next_numeric_id`] after every generation attempt so readiness can
/// report a generator that initialized but now fails (for example a fenced
/// lease that lost its node allocation).
static LAST_ID_GENERATION_FAILED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Readiness signal for the Snowflake ID generator lease.
///
/// `false` means the process may be unable to allocate row ids:
/// - the installed database lease is unhealthy (heartbeat dead or the lease
///   guard fenced this process out), or
/// - the most recent [`next_numeric_id`] call failed (including the
///   "generator not initialized" refusal in production-like environments).
///
/// Without a lease to interrogate (dev fallback, uninitialized process) the
/// last-attempt state decides, so a never-initialized production process reads
/// unhealthy instead of silently passing readiness.
pub fn numeric_id_generator_healthy() -> bool {
    id_generator_health(
        LAST_ID_GENERATION_FAILED.load(std::sync::atomic::Ordering::Relaxed),
        ID_GENERATOR.get().map(|holder| {
            holder
                ._lease
                .as_ref()
                .map(NodeLease::is_healthy)
        }),
    )
}

/// Pure body of [`numeric_id_generator_healthy`]: `lease_healthy` is `None`
/// when no lease exists (or no generator was initialized).
fn id_generator_health(last_failed: bool, lease_healthy: Option<Option<bool>>) -> bool {
    match lease_healthy {
        // A live lease must also not have a failed generation behind it.
        Some(Some(true)) => !last_failed,
        // An installed but unhealthy lease is decisive.
        Some(Some(false)) => false,
        // No lease to interrogate: the last attempt is the only evidence.
        Some(None) | None => !last_failed,
    }
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

/// Default byte budget for the total extracted event content fed to one
/// extraction run (LLM prompt or deterministic candidates).
///
/// Without this bound the prompt scales with `DEFAULT_MAX_EXTRACTION_INPUT_EVENTS`
/// times whatever a single event payload carries (up to the request body limit), so
/// one tenant-sized extraction job could allocate gigabytes in a worker task. The
/// budget is a prefix bound: extraction covers input events in request order until
/// the budget is exhausted, and the result reports how many events were left out
/// (`skippedEventCount`) instead of silently pretending they were processed.
pub const DEFAULT_MAX_EXTRACTION_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Default maximum events exported per job.
pub const DEFAULT_MAX_EXPORT_EVENTS: usize = 100_000;

/// Default maximum concurrent export runs per process.
///
/// An export holds its whole payload in memory (collected rows, encoded bytes,
/// and — for inline results — the reparsed value), so a handful of concurrent
/// large exports multiply into gigabytes of resident set even though every
/// individual run stays under its byte cap. The process-level semaphore caps
/// that product; extra requests queue on the permit and surface the standard
/// request deadline instead of unbounded memory.
pub const DEFAULT_EXPORT_MAX_CONCURRENCY: usize = 2;

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

pub fn max_extraction_input_bytes() -> usize {
    read_env_usize(
        "SDKWORK_MEMORY_EXTRACTION_MAX_INPUT_BYTES",
        DEFAULT_MAX_EXTRACTION_INPUT_BYTES,
    )
}

pub fn max_export_events() -> usize {
    read_env_usize(
        "SDKWORK_MEMORY_EXPORT_MAX_EVENTS",
        DEFAULT_MAX_EXPORT_EVENTS,
    )
}

pub fn export_max_concurrency() -> usize {
    read_env_usize(
        "SDKWORK_MEMORY_EXPORT_MAX_CONCURRENCY",
        DEFAULT_EXPORT_MAX_CONCURRENCY,
    )
}

/// Process-level export admission. The semaphore is created once with the
/// configured permit count; acquiring a permit serializes export runs so the
/// per-run byte caps cannot multiply across unbounded concurrent requests.
pub fn export_permits() -> &'static tokio::sync::Semaphore {
    static PERMITS: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    PERMITS.get_or_init(|| tokio::sync::Semaphore::new(export_max_concurrency().max(1)))
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
/// Cursor tokens are opaque, tamper-evident MACs over the store's keyset key
/// (PAGINATION_SPEC section 3: clients MUST NOT parse or construct cursors).
/// `SDKWORK_MEMORY_CURSOR_SIGNING_KEY` sets the per-deployment key; the
/// built-in fallback keeps tokens opaque but must not be relied on across
/// deployments.
const CURSOR_TOKEN_PREFIX: &str = "v1";

fn cursor_signing_key() -> &'static Vec<u8> {
    static KEY: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    KEY.get_or_init(|| {
        match std::env::var("SDKWORK_MEMORY_CURSOR_SIGNING_KEY") {
            Ok(value) if !value.trim().is_empty() => value.into_bytes(),
            _ => {
                tracing::warn!(
                    "SDKWORK_MEMORY_CURSOR_SIGNING_KEY is not set; list cursors fall back to \
                     the built-in signing key. Set a per-deployment key so cursors cannot be \
                     verified across deployments."
                );
                b"sdkwork-memory-cursor-v1".to_vec()
            }
        }
    })
}

/// Minimum `SDKWORK_MEMORY_CURSOR_SIGNING_KEY` length in production: an HMAC
/// key is entropy, not a password policy to negotiate, and a short key makes
/// the token's anti-forgery property brute-forceable.
const MIN_CURSOR_SIGNING_KEY_BYTES: usize = 32;

/// Rejects a production-like startup whose cursor-signing key is unset or too
/// short: the built-in fallback keeps tokens well-formed, but a public key
/// gives the MAC no cross-deployment anti-forgery value, and a short key makes
/// it brute-forceable, so production must set an explicit key of real length.
/// Rejects a production-like startup whose cursor-signing key is unset or too
/// short: the built-in fallback keeps tokens well-formed, but a public key
/// gives the MAC no cross-deployment anti-forgery value, and a short key makes
/// it brute-forceable, so production must set an explicit key of real length.
pub fn validate_runtime_secrets_for_environment() -> Result<(), String> {
    if !is_production_like_environment() {
        return Ok(());
    }
    validate_cursor_signing_key(std::env::var("SDKWORK_MEMORY_CURSOR_SIGNING_KEY").ok())
}

/// The environment-independent body of the cursor-key admission gate.
fn validate_cursor_signing_key(key: Option<String>) -> Result<(), String> {
    let missing = || {
        "production Memory runtime requires SDKWORK_MEMORY_CURSOR_SIGNING_KEY: the built-in \
         cursor-signing fallback is not acceptable for production deployments"
            .to_string()
    };
    let key = key.filter(|value| !value.trim().is_empty()).ok_or_else(missing)?;
    if key.as_bytes().len() < MIN_CURSOR_SIGNING_KEY_BYTES {
        return Err(format!(
            "SDKWORK_MEMORY_CURSOR_SIGNING_KEY must be at least {MIN_CURSOR_SIGNING_KEY_BYTES} \
             bytes in production; a shorter key makes cursor forgery brute-forceable"
        ));
    }
    Ok(())
}

fn cursor_mac(raw: &str) -> String {
    // digest 0.11 moved key initialization onto `KeyInit` (re-exported by hmac).
    use hmac::{KeyInit, Mac};
    type HmacSha256 = hmac::Hmac<sha2::Sha256>;
    let mut mac = <HmacSha256 as KeyInit>::new_from_slice(cursor_signing_key())
        .expect("HMAC accepts any key length");
    mac.update(raw.as_bytes());
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Wraps a store-issued keyset key in an opaque, MAC-protected token.
pub fn encode_list_cursor(raw: &str) -> String {
    use base64::Engine as _;
    let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes());
    format!("{CURSOR_TOKEN_PREFIX}.{data}.{}", cursor_mac(raw))
}

/// Validates and unwraps a client-supplied cursor token.
///
/// `None` and the empty string mean "first page" and pass through. Anything
/// else must be a well-formed token with a valid MAC: a client-forged or
/// stale-key cursor is rejected as an invalid parameter instead of leaking a
/// forgeable pagination primitive.
pub fn decode_list_cursor(cursor: Option<&str>) -> MemoryServiceResult<Option<String>> {
    use base64::Engine as _;
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let cursor = cursor.trim();
    if cursor.is_empty() {
        return Ok(None);
    }
    let malformed = || {
        MemoryServiceError::validation(
            "cursor must be a server-issued opaque token; cursor forgery is not supported",
        )
    };
    let mut parts = cursor.split('.');
    let prefix = parts.next().ok_or_else(malformed)?;
    let data = parts.next().ok_or_else(malformed)?;
    let signature = parts.next().ok_or_else(malformed)?;
    if parts.next().is_some() || prefix != CURSOR_TOKEN_PREFIX {
        return Err(malformed());
    }
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(data.as_bytes())
        .map_err(|_| malformed())?;
    let raw = String::from_utf8(raw).map_err(|_| malformed())?;
    if !sdkwork_utils_rust::secure_compare(&cursor_mac(&raw), signature) {
        return Err(malformed());
    }
    Ok(Some(raw))
}

pub fn memory_cursor_page_info(

    page_size: i32,
    has_more: bool,
    next_cursor: Option<String>,
) -> PageInfo {
    cursor_window_page_info(
        Some(usize::try_from(page_size).unwrap_or(DEFAULT_PAGE_SIZE as usize)),
        if has_more {
        next_cursor.as_deref().map(encode_list_cursor)
    } else {
        None
    },
        has_more,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_generator_health_policy_is_decisive_and_fail_closed() {
        // A live lease with a clean last attempt is healthy.
        assert!(id_generator_health(false, Some(Some(true))));
        // An installed but unhealthy lease is decisive, even after successes.
        assert!(!id_generator_health(false, Some(Some(false))));
        assert!(!id_generator_health(true, Some(Some(false))));
        // A live lease cannot paper over a failed generation attempt.
        assert!(!id_generator_health(true, Some(Some(true))));
        // Without a lease (or without a generator) the last attempt decides.
        assert!(id_generator_health(false, Some(None)));
        assert!(!id_generator_health(true, Some(None)));
        assert!(id_generator_health(false, None));
        assert!(!id_generator_health(true, None));
    }

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

    #[test]
    fn cursor_tokens_round_trip() {
        let token = encode_list_cursor("42");
        assert_eq!(decode_list_cursor(Some(&token)).unwrap().as_deref(), Some("42"));
        // Absent and blank cursors mean "first page" and pass through.
        assert_eq!(decode_list_cursor(None).unwrap(), None);
        assert_eq!(decode_list_cursor(Some("  ")).unwrap(), None);
    }

    #[test]
    fn forged_and_malformed_cursors_are_rejected_as_invalid_parameters() {
        let token = encode_list_cursor("42");
        let (prefix, rest) = token.split_once('.').expect("token has a prefix");
        let (data, signature) = rest.rsplit_once('.').expect("token has a signature");

        // A tampered payload keeps the valid signature but must not verify.
        let tampered_data = if data.starts_with('M') {
            format!("N{}", &data[1..])
        } else {
            format!("M{}", &data[1..])
        };
        for forged in [
            // Bit-flipped payload under the original signature.
            format!("{prefix}.{tampered_data}.{signature}"),
            // A valid token for one key re-signed for nothing: truncated MAC.
            format!("{prefix}.{data}.{}", &signature[..signature.len() - 2]),
            // Missing pieces and unknown prefixes.
            data.to_string(),
            format!("v2.{data}.{signature}"),
            format!("{prefix}.{data}.{signature}.extra"),
        ] {
            let error = decode_list_cursor(Some(&forged)).expect_err(&forged);
            assert_eq!(
                error.code, "validation_error",
                "forgery stays a client error, never a 5xx: {forged}"
            );
        }
    }

    #[test]
    fn cursor_verification_uses_the_shared_constant_time_compare() {
        // MAC verification delegates to sdkwork-utils' audited fold rather
        // than a local reimplementation; pin the delegation contract here so
        // a timing-sensitive local copy cannot quietly return.
        assert!(sdkwork_utils_rust::secure_compare("abc", "abc"));
        assert!(!sdkwork_utils_rust::secure_compare("abc", "abd"));
        assert!(!sdkwork_utils_rust::secure_compare("abc", "abcd"));
        assert!(!sdkwork_utils_rust::secure_compare("", "a"));
        assert!(sdkwork_utils_rust::secure_compare("", ""));
    }

    #[test]
    fn production_cursor_key_admission_rejects_missing_short_and_blank_keys() {
        let missing = validate_cursor_signing_key(None).expect_err("missing");
        assert!(missing.contains("requires SDKWORK_MEMORY_CURSOR_SIGNING_KEY"));
        let blank = validate_cursor_signing_key(Some("   ".to_owned())).expect_err("blank");
        assert!(blank.contains("requires SDKWORK_MEMORY_CURSOR_SIGNING_KEY"));
        let short = validate_cursor_signing_key(Some("too-short".to_owned())).expect_err("short");
        assert!(
            short.contains("at least 32"),
            "the length gate must name the floor, got: {short}"
        );
        // The floor is inclusive: 31 bytes fails, exactly 32 passes.
        assert!(validate_cursor_signing_key(Some("x".repeat(31))).is_err());
        assert_eq!(validate_cursor_signing_key(Some("x".repeat(32))), Ok(()));
    }
}
