//! JWT dual-token fixtures aligned with `sdkwork-web-framework` parsers.

use sdkwork_utils_rust::sha256_hash;
use sdkwork_web_core::{auth_token_jwt, encode_unsigned_test_jwt};
use serde_json::json;
use tokio::sync::{Mutex, MutexGuard};

pub const MEMORY_APP_ID: &str = "sdkwork-memory";
pub const MEMORY_TEST_IDEMPOTENCY_KEY: &str = "memory-integration-idempotency-key";
pub const DEFAULT_TENANT_ID: &str = "100001";
pub const DEFAULT_SESSION_ID: &str = "s-1";

static INTEGRATION_TEST_ENV_LOCK: Mutex<()> = Mutex::const_new(());

/// Stable idempotency key scoped by HTTP method, path, and request body.
pub fn memory_idempotency_key(method: &str, uri: &str, body: &str) -> String {
    format!(
        "{MEMORY_TEST_IDEMPOTENCY_KEY}:{method}:{uri}:{}",
        sha256_hash(body.as_bytes())
    )
}

/// Body fingerprint header required for idempotent commands with a payload.
pub fn memory_content_sha256(body: &str) -> String {
    sha256_hash(body.as_bytes())
}

/// Serializes env mutation and enables IAM JWT fallback for integration tests.
pub async fn lock_integration_test_env() -> MutexGuard<'static, ()> {
    let guard = INTEGRATION_TEST_ENV_LOCK.lock().await;
    std::env::set_var("SDKWORK_ENV", "dev");
    std::env::set_var("SDKWORK_IAM_ALLOW_DEV_AUTH_FALLBACK", "true");
    guard
}

pub fn memory_auth_token_bearer(user_id: &str) -> String {
    format!(
        "Bearer {}",
        auth_token_jwt(DEFAULT_TENANT_ID, user_id, DEFAULT_SESSION_ID, MEMORY_APP_ID)
    )
}

pub fn memory_access_token(user_id: &str) -> String {
    encode_unsigned_test_jwt(json!({
        "token_type": "access",
        "tenant_id": DEFAULT_TENANT_ID,
        "user_id": user_id,
        "session_id": DEFAULT_SESSION_ID,
        "app_id": MEMORY_APP_ID,
        "environment": "dev",
        "deployment_mode": "saas",
        "login_scope": "TENANT",
        // token-claims-gate: legacy-fixture — constructs a pre-slimming credential so this
        // test can assert the claim is no longer an authorization source.
        "permission_scope": ["memory.*"]
    }))
}

pub fn memory_dev_api_key(user_id: &str, api_key_id: &str) -> String {
    format!(
        "api_key_id={api_key_id};tenant_id={DEFAULT_TENANT_ID};user_id={user_id};app_id={MEMORY_APP_ID};permission_scope=memory.*"
    )
}

/// Legacy semicolon claim-string dual tokens rejected by `WEB_FRAMEWORK_SPEC` JWT parsers.
pub fn legacy_inline_dual_tokens(user_id: &str) -> (String, String) {
    (
        format!(
            "Bearer tenant_id={DEFAULT_TENANT_ID};user_id={user_id};session_id={DEFAULT_SESSION_ID};app_id={MEMORY_APP_ID};auth_level=password"
        ),
        format!(
            "tenant_id={DEFAULT_TENANT_ID};user_id={user_id};session_id={DEFAULT_SESSION_ID};app_id={MEMORY_APP_ID};environment=dev;deployment_mode=saas"
        ),
    )
}
