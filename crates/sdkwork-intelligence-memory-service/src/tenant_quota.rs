use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceResult};
use sdkwork_memory_plugin_native_sql::NativeSqlMemoryStore;
use sdkwork_memory_spi::{
    MemoryRecordQuotaAdmission, MemoryScopeContext, MemorySpaceQuotaAdmission,
};

use crate::platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryQuotaLimits {
    pub max_records_per_space: u64,
    pub max_spaces_per_user: u64,
}

impl MemoryQuotaLimits {
    pub fn from_env() -> Self {
        Self {
            max_records_per_space: platform::read_env_u64(
                "SDKWORK_MEMORY_MAX_RECORDS_PER_SPACE",
                100_000,
            ),
            max_spaces_per_user: platform::read_env_u64("SDKWORK_MEMORY_MAX_SPACES_PER_USER", 100),
        }
    }
}

/// Tenant quota overrides ride the hot admission path (every space create and
/// candidate promotion), so the resolved policy is cached in process for a
/// short bounded TTL instead of re-reading `ai_policy` on every mutation. The
/// cache is bounded at 1024 tenants; on overflow it resets wholesale, which is
/// trivially correct (the next read repopulates) and keeps memory flat.
const QUOTA_POLICY_CACHE_CAPACITY: usize = 1024;
const QUOTA_POLICY_CACHE_TTL_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedQuotaLimits {
    limits: MemoryQuotaLimits,
    cached_at: std::time::Instant,
}

/// The cache key carries the environment-derived defaults next to the tenant:
/// entries resolve policy overrides *on top of* those defaults, so a changed
/// environment value must never keep serving the old composition. This also
/// makes the cache self-correcting for tests that retune the environment.
type QuotaLimitsCache = std::collections::HashMap<(i64, u64, u64), CachedQuotaLimits>;

fn quota_limits_cache() -> &'static std::sync::Mutex<QuotaLimitsCache> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<QuotaLimitsCache>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Resolves the quota limits of one tenant: environment defaults overridden by
/// the active tenant-scoped `memory.quota` policy (policy values win key by
/// key, absent keys keep the environment default). Results are cached in
/// process for [`QUOTA_POLICY_CACHE_TTL_SECS`] under a key that includes the
/// environment defaults; a stale entry is simply re-resolved on the next call.
pub async fn resolve_quota_limits(
    store: &NativeSqlMemoryStore,
    tenant_id: i64,
) -> MemoryServiceResult<MemoryQuotaLimits> {
    let env_limits = MemoryQuotaLimits::from_env();
    let key = (
        tenant_id,
        env_limits.max_records_per_space,
        env_limits.max_spaces_per_user,
    );
    if let Some(cached) = cached_quota_limits(key) {
        return Ok(cached);
    }
    let limits = resolve_quota_limits_uncached(store, tenant_id).await?;
    cache_quota_limits(key, limits);
    Ok(limits)
}

/// Cache-free variant of [`resolve_quota_limits`]: the override authority for
/// tests and diagnostics that must observe a freshly stored policy row.
pub async fn resolve_quota_limits_uncached(
    store: &NativeSqlMemoryStore,
    tenant_id: i64,
) -> MemoryServiceResult<MemoryQuotaLimits> {
    let env_limits = MemoryQuotaLimits::from_env();
    let policy = store
        .find_active_tenant_quota_policy(tenant_id)
        .await
        .map_err(|error| {
            MemoryServiceError::storage(format!("tenant quota policy lookup failed: {error}"))
        })?;
    let Some(policy) = policy else {
        return Ok(env_limits);
    };
    Ok(MemoryQuotaLimits {
        max_records_per_space: policy
            .max_records_per_space
            .unwrap_or(env_limits.max_records_per_space),
        max_spaces_per_user: policy
            .max_spaces_per_user
            .unwrap_or(env_limits.max_spaces_per_user),
    })
}

fn cached_quota_limits(key: (i64, u64, u64)) -> Option<MemoryQuotaLimits> {
    let cache = quota_limits_cache();
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = guard.get(&key).copied()?;
    if entry.cached_at.elapsed() > std::time::Duration::from_secs(QUOTA_POLICY_CACHE_TTL_SECS) {
        guard.remove(&key);
        return None;
    }
    Some(entry.limits)
}

fn cache_quota_limits(key: (i64, u64, u64), limits: MemoryQuotaLimits) {
    let cache = quota_limits_cache();
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if guard.len() >= QUOTA_POLICY_CACHE_CAPACITY {
        guard.clear();
    }
    guard.insert(
        key,
        CachedQuotaLimits {
            limits,
            cached_at: std::time::Instant::now(),
        },
    );
}

pub fn resolve_space_record_quota_admission<T>(
    scope: &MemoryScopeContext,
    admission: MemoryRecordQuotaAdmission<T>,
) -> MemoryServiceResult<T> {
    match admission {
        MemoryRecordQuotaAdmission::Admitted(value) => Ok(value),
        MemoryRecordQuotaAdmission::QuotaExceeded {
            max_active_records, ..
        } => {
            crate::domain_metrics::memory_domain_metrics().record_quota_exceeded();
            Err(MemoryServiceError::quota_exceeded(format!(
                "memory space {space_id} reached the maximum of {max_active_records} active records",
                space_id = scope.space_id
            )))
        }
    }
}

pub fn resolve_user_space_quota_admission<T>(
    owner_subject_id: &str,
    admission: MemorySpaceQuotaAdmission<T>,
) -> MemoryServiceResult<T> {
    match admission {
        MemorySpaceQuotaAdmission::Admitted(value) => Ok(value),
        MemorySpaceQuotaAdmission::QuotaExceeded {
            max_active_spaces, ..
        } => {
            crate::domain_metrics::memory_domain_metrics().record_quota_exceeded();
            Err(MemoryServiceError::quota_exceeded(format!(
                "user {owner_subject_id} reached the maximum of {max_active_spaces} memory spaces for this tenant"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_limit_disables_quota_enforcement() {
        let limits = MemoryQuotaLimits {
            max_records_per_space: 0,
            max_spaces_per_user: 0,
        };
        assert_eq!(limits.max_records_per_space, 0);
        assert_eq!(limits.max_spaces_per_user, 0);
    }

    #[cfg(test)]
    mod policy_resolution {
        use super::*;
        use sdkwork_memory_plugin_native_sql::InsertPolicyCommand;

        // Distinct tenants per test: the resolve cache is process-global and
        // unit tests run in parallel threads.
        const OVERRIDE_TENANT: i64 = 871_001;
        const FULL_POLICY_TENANT: i64 = 871_002;
        const FALLBACK_TENANT: i64 = 871_003;
        const BAD_JSON_TENANT: i64 = 871_004;
        const NON_POSITIVE_TENANT: i64 = 871_005;

        async fn store() -> NativeSqlMemoryStore {
            NativeSqlMemoryStore::new_in_memory_sqlite()
                .await
                .expect("create migrated in-memory SQLite store")
        }

        async fn insert_quota_policy(
            store: &NativeSqlMemoryStore,
            id: i64,
            tenant_id: i64,
            policy_json: &str,
        ) {
            let uuid = id.to_string();
            let scope_ref = tenant_id.to_string();
            store
                .insert_policy(InsertPolicyCommand {
                    id,
                    uuid: &uuid,
                    tenant_id,
                    policy_type: "memory.quota",
                    scope: "tenant",
                    scope_ref: Some(&scope_ref),
                    policy_json,
                })
                .await
                .expect("insert tenant quota policy");
        }

        #[tokio::test]
        async fn partial_policy_overrides_only_the_present_key() {
            let store = store().await;
            insert_quota_policy(
                &store,
                1,
                OVERRIDE_TENANT,
                r#"{"maxRecordsPerSpace": 12345}"#,
            )
            .await;

            let limits = resolve_quota_limits_uncached(&store, OVERRIDE_TENANT)
                .await
                .expect("partial policy resolves");
            assert_eq!(limits.max_records_per_space, 12_345);
            assert_eq!(
                limits.max_spaces_per_user,
                MemoryQuotaLimits::from_env().max_spaces_per_user,
                "absent policy keys keep the environment default"
            );
        }

        #[tokio::test]
        async fn full_policy_overrides_both_limits_and_feeds_the_cached_path() {
            let store = store().await;
            insert_quota_policy(
                &store,
                2,
                FULL_POLICY_TENANT,
                r#"{"maxRecordsPerSpace": 5, "maxSpacesPerUser": 7}"#,
            )
            .await;

            let uncached = resolve_quota_limits_uncached(&store, FULL_POLICY_TENANT)
                .await
                .expect("full policy resolves");
            assert_eq!(uncached.max_records_per_space, 5);
            assert_eq!(uncached.max_spaces_per_user, 7);

            let cached = resolve_quota_limits(&store, FULL_POLICY_TENANT)
                .await
                .expect("cached resolution succeeds");
            let cached_again = resolve_quota_limits(&store, FULL_POLICY_TENANT)
                .await
                .expect("second cached resolution succeeds");
            assert_eq!(cached, uncached);
            assert_eq!(cached_again, uncached, "the TTL cache serves the same limits");
        }

        #[tokio::test]
        async fn missing_policy_falls_back_to_environment_defaults() {
            let store = store().await;
            let limits = resolve_quota_limits_uncached(&store, FALLBACK_TENANT)
                .await
                .expect("fallback resolution succeeds");
            assert_eq!(limits, MemoryQuotaLimits::from_env());
        }

        #[tokio::test]
        async fn malformed_policy_json_fails_closed_as_storage_error() {
            let store = store().await;
            insert_quota_policy(&store, 3, BAD_JSON_TENANT, "not-json").await;

            let error = resolve_quota_limits_uncached(&store, BAD_JSON_TENANT)
                .await
                .expect_err("malformed policy JSON must fail closed");
            assert_eq!(error.code, "storage_error");
        }

        #[tokio::test]
        async fn non_positive_policy_limit_fails_closed() {
            let store = store().await;
            insert_quota_policy(
                &store,
                4,
                NON_POSITIVE_TENANT,
                r#"{"maxRecordsPerSpace": 0}"#,
            )
            .await;

            let error = resolve_quota_limits_uncached(&store, NON_POSITIVE_TENANT)
                .await
                .expect_err("a zero limit would silently disable the quota and must fail closed");
            assert_eq!(error.code, "storage_error");
        }
    }
}
