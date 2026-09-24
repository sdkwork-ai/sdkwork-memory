use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_iam_web_adapter::IamAuthorizationPolicy;
use sdkwork_memory_contract::memory_is_production_like_environment;
use sdkwork_web_axum::WebFrameworkLayer;
use sdkwork_web_core::{
    AuditEmitter, AuditFact, EnforcePrincipalTenantIsolationPolicy, HttpRouteManifest,
    SecurityEvent, SecurityEventEmitter, SecurityEventKind, WebEnvironment, WebFrameworkError,
    WebFrameworkOptionalFeatures, WebRequestContextResolver,
};

use crate::readiness::shared_iam_postgres_pool;

const DEFAULT_REQUEST_TIMEOUT_SECONDS: u64 = 30;

pub fn harden_memory_web_framework_layer<R>(
    layer: WebFrameworkLayer<R>,
    route_manifest: HttpRouteManifest,
) -> WebFrameworkLayer<R>
where
    R: WebRequestContextResolver + Clone,
{
    if !memory_is_production_like_environment() {
        // Loud, never silent: an operator running with an unintended dev
        // environment must see why authorization hardening, Redis-backed rate
        // limiting, and request deadlines are absent.
        tracing::error!(
            "Memory web runtime is NOT production-like; authorization policy, tenant isolation, \
             Redis rate limiting, idempotency, admission control, and request deadlines are \
             disabled. Set SDKWORK_MEMORY_ENVIRONMENT explicitly (development|staging|production)."
        );
        return layer;
    }

    let redis_url = memory_web_redis_url().unwrap_or_else(|error| panic!("{error}"));
    let rate_limit_store =
        sdkwork_web_bootstrap::shared_rate_limit_store(&redis_url, "sdkwork:memory")
            .unwrap_or_else(|error| {
                panic!("invalid Memory Redis rate-limit configuration: {error}")
            });
    let idempotency_store =
        sdkwork_web_bootstrap::shared_idempotency_store(&redis_url, "sdkwork:memory")
            .unwrap_or_else(|error| {
                panic!("invalid Memory Redis idempotency configuration: {error}")
            });
    let concurrent_admission_store =
        sdkwork_web_bootstrap::shared_concurrent_admission_store(&redis_url, "sdkwork:memory")
            .unwrap_or_else(|error| {
                panic!("invalid Memory Redis admission configuration: {error}")
            });
    let cors_origins =
        sdkwork_web_bootstrap::cors_allowed_origins_from_env(&["SDKWORK_CORS_ALLOWED_ORIGINS"]);
    let security_policy =
        sdkwork_web_bootstrap::security_policy_for_environment(&WebEnvironment::Prod, cors_origins);

    layer
        .with_security_policy(security_policy)
        .with_authorization_policy(Arc::new(IamAuthorizationPolicy::new(route_manifest)))
        .with_tenant_isolation_policy(Arc::new(EnforcePrincipalTenantIsolationPolicy))
        .with_rate_limit_store(rate_limit_store)
        .with_idempotency_store(idempotency_store)
        .with_concurrent_admission_store(concurrent_admission_store)
        .with_audit_emitter(Arc::new(MemoryIamAuditEmitter))
        .with_security_event_emitter(Arc::new(MemoryIamSecurityEventEmitter))
        .with_optional_features(WebFrameworkOptionalFeatures::production_sqlx())
        .with_request_timeout(Duration::from_secs(memory_request_timeout_seconds()))
}

pub async fn memory_redis_ready_check() -> bool {
    if !memory_is_production_like_environment() {
        return true;
    }
    let Ok(redis_url) = memory_web_redis_url() else {
        return false;
    };
    let Ok(check) = sdkwork_web_bootstrap::RedisReadinessCheck::new(redis_url) else {
        return false;
    };
    sdkwork_web_bootstrap::ReadinessCheck::check(&check)
        .await
        .is_ok()
}

fn memory_web_redis_url() -> Result<String, String> {
    ["SDKWORK_MEMORY_WEB_REDIS_URL", "SDKWORK_REDIS_URL"]
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            "production Memory web runtime requires SDKWORK_MEMORY_WEB_REDIS_URL for distributed rate limiting, idempotency, and admission control".to_string()
        })
}

/// Single source of truth for the Memory HTTP request body limit.
///
/// Each API surface applies this bound as the innermost `DefaultBodyLimit`
/// layer so it always wins over the framework's own default; operators tune it
/// through `SDKWORK_MEMORY_MAX_BODY_BYTES` and the resolved value is what the
/// request path actually enforces.
pub fn memory_request_body_limit_bytes() -> usize {
    const DEFAULT: usize = 1024 * 1024;
    const MIN: usize = 64 * 1024;
    const MAX: usize = 16 * 1024 * 1024;
    std::env::var("SDKWORK_MEMORY_MAX_BODY_BYTES")
        .ok()
        .and_then(|value| sdkwork_utils_rust::parse_int(&value))
        .and_then(|value| usize::try_from(value).ok())
        .map(|value| value.clamp(MIN, MAX))
        .unwrap_or(DEFAULT)
}

fn memory_request_timeout_seconds() -> u64 {
    std::env::var("SDKWORK_MEMORY_HTTP_REQUEST_TIMEOUT_SECS")
        .ok()
        .and_then(|value| sdkwork_utils_rust::parse_int(&value))
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| (1..=300).contains(value))
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECONDS)
}

#[derive(Clone, Copy)]
struct MemoryIamAuditEmitter;

#[async_trait]
impl AuditEmitter for MemoryIamAuditEmitter {
    async fn emit(&self, fact: AuditFact) -> Result<(), WebFrameworkError> {
        let result = self.record(fact).await;
        // Audit emission runs in the response `after` phase. Returning an error
        // would let the framework replace an already-committed business
        // response with a 5xx, so clients would retry successful writes and
        // duplicate them. Audit failures are logged for alerting instead; the
        // IAM audit table remains the durability boundary and readiness keeps
        // gating on the IAM database.
        if let Err(error) = &result {
            tracing::error!(
                error = %error,
                "memory IAM audit emission failed after business response; audit trail may be incomplete"
            );
        }
        Ok(())
    }
}

impl MemoryIamAuditEmitter {
    async fn record(&self, fact: AuditFact) -> Result<(), WebFrameworkError> {
        let pool = shared_iam_postgres_pool().await.ok_or_else(|| {
            WebFrameworkError::dependency_unavailable("IAM audit database is unavailable")
        })?;
        let action = fact.operation_id.as_deref().unwrap_or(fact.method.as_str());
        sdkwork_iam_web_adapter::record_audit_event(
            pool.as_ref(),
            fact.tenant_id.as_deref().unwrap_or("0"),
            None,
            fact.user_id.as_deref(),
            action,
            "memory_http_request",
            Some(fact.path.as_str()),
            Some(fact.request_id.as_str()),
            "prod",
            serde_json::json!({
                "apiSurface": format!("{:?}", fact.api_surface),
                "method": fact.method,
                "statusCode": fact.status_code,
                "durationMs": fact.duration_ms,
            }),
        )
        .await
        .map_err(WebFrameworkError::dependency_unavailable)
    }
}

#[derive(Clone, Copy)]
struct MemoryIamSecurityEventEmitter;

#[async_trait]
impl SecurityEventEmitter for MemoryIamSecurityEventEmitter {
    async fn emit(&self, event: SecurityEvent) -> Result<(), WebFrameworkError> {
        let result = self.record(event).await;
        // Same post-response rationale as the audit emitter: a security-event
        // failure must not overwrite an already-committed response with a 5xx.
        if let Err(error) = &result {
            tracing::error!(
                error = %error,
                "memory IAM security event emission failed after business response"
            );
        }
        Ok(())
    }
}

impl MemoryIamSecurityEventEmitter {
    async fn record(&self, event: SecurityEvent) -> Result<(), WebFrameworkError> {
        let pool = shared_iam_postgres_pool().await.ok_or_else(|| {
            WebFrameworkError::dependency_unavailable("IAM security event database is unavailable")
        })?;
        sdkwork_iam_web_adapter::record_security_event(
            pool.as_ref(),
            event.tenant_id.as_deref().unwrap_or("0"),
            None,
            None,
            security_event_kind(event.kind),
            "warning",
            "prod",
            serde_json::json!({
                "requestId": event.request_id,
                "apiSurface": format!("{:?}", event.api_surface),
                "path": event.path,
                "method": event.method,
                "origin": event.origin,
                "detail": event.detail,
            }),
        )
        .await
        .map_err(WebFrameworkError::dependency_unavailable)
    }
}

fn security_event_kind(kind: SecurityEventKind) -> &'static str {
    match kind {
        SecurityEventKind::CorsDenied => "memory.web.cors_denied",
        SecurityEventKind::RateLimitExceeded => "memory.web.rate_limit_exceeded",
        SecurityEventKind::AuthenticationFailed => "memory.web.authentication_failed",
        SecurityEventKind::AuthorizationDenied => "memory.web.authorization_denied",
        SecurityEventKind::TenantIsolationDenied => "memory.web.tenant_isolation_denied",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_configuration_is_bounded() {
        let _guard = sdkwork_memory_contract::runtime_env::env_test_lock();
        std::env::set_var("SDKWORK_MEMORY_HTTP_REQUEST_TIMEOUT_SECS", "0");
        assert_eq!(
            memory_request_timeout_seconds(),
            DEFAULT_REQUEST_TIMEOUT_SECONDS
        );
        std::env::set_var("SDKWORK_MEMORY_HTTP_REQUEST_TIMEOUT_SECS", "120");
        assert_eq!(memory_request_timeout_seconds(), 120);
        std::env::remove_var("SDKWORK_MEMORY_HTTP_REQUEST_TIMEOUT_SECS");
    }

    #[test]
    fn production_requires_distributed_redis_configuration() {
        let _guard = sdkwork_memory_contract::runtime_env::env_test_lock();
        let previous_memory = std::env::var_os("SDKWORK_MEMORY_WEB_REDIS_URL");
        let previous_shared = std::env::var_os("SDKWORK_REDIS_URL");
        std::env::remove_var("SDKWORK_MEMORY_WEB_REDIS_URL");
        std::env::remove_var("SDKWORK_REDIS_URL");
        assert!(memory_web_redis_url().is_err());
        if let Some(value) = previous_memory {
            std::env::set_var("SDKWORK_MEMORY_WEB_REDIS_URL", value);
        }
        if let Some(value) = previous_shared {
            std::env::set_var("SDKWORK_REDIS_URL", value);
        }
    }
}
