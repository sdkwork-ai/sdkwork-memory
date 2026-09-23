use std::sync::Arc;

use sdkwork_memory_contract::{
    memory_is_production_like_environment, memory_use_dev_inline_auth_resolver,
};
use sqlx::PgPool;

/// Handle to the IAM PostgreSQL database backing readiness, audit, and security-event wiring.
///
/// `create_pool_from_env` resolves through the installed process-shared pool registry
/// (SDKWork Process-Shared Database Pool Standard sections 1 and 2), so this reuses the canonical
/// pool for the database identity instead of opening a private one. `PgPool` is a reference-counted
/// handle, so re-resolving it is a registry lookup, not a new connection pool.
///
/// This deliberately does **not** cache the handle. The previous implementation kept it in a
/// process-global `tokio::sync::Mutex` that was held across the pool-acquisition `await`, which
/// serialized every readiness probe, audit emit, and security-event emit behind a single lock and
/// duplicated caching the registry already performs.
pub async fn shared_iam_postgres_pool() -> Option<Arc<PgPool>> {
    match sdkwork_database_sqlx::create_pool_from_env("IAM").await {
        Ok(Some(pool)) => pool.as_postgres().cloned().map(Arc::new),
        _ => None,
    }
}

/// Validates runtime dependencies required before serving authenticated traffic.
pub async fn memory_dependency_ready_check() -> bool {
    if memory_use_dev_inline_auth_resolver() {
        return true;
    }

    let iam_database_configured = std::env::var("SDKWORK_DATABASE_URL")
        .or_else(|_| std::env::var("SDKWORK_DATABASE_ENGINE"))
        .is_ok();

    if memory_is_production_like_environment() && !iam_database_configured {
        tracing::warn!("memory readiness blocked: production requires SDKWORK_DATABASE_URL");
        return false;
    }

    if !iam_database_configured {
        return true;
    }

    let iam_ready = match shared_iam_postgres_pool().await {
        Some(postgres) => sqlx::query("SELECT 1")
            .execute(postgres.as_ref())
            .await
            .is_ok(),
        None => {
            tracing::warn!("memory readiness blocked: IAM database pool unavailable");
            false
        }
    };
    iam_ready && crate::web_runtime::memory_redis_ready_check().await
}
