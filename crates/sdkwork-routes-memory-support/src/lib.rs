//! Shared Memory router auth wiring for sdkwork-web-framework integration.

pub mod correlation;
pub mod metrics;
pub mod principal;
pub mod problem;
pub mod query;
pub mod readiness;
pub mod response;
pub mod web_runtime;

use async_trait::async_trait;
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_memory_contract::{
    memory_is_production_like_environment, memory_use_dev_inline_auth_resolver,
};
use sdkwork_web_core::{WebFrameworkError, WebRequestContextResolver, WebRequestPrincipal};

pub use correlation::{with_problem_correlation, MemoryProblemCorrelation};
pub use metrics::{
    memory_http_metrics, memory_metric_environment_label, refresh_memory_http_metric_dimensions,
};
pub use web_runtime::memory_request_body_limit_bytes;
pub use principal::{parse_principal_optional_u64, parse_principal_u64};
pub use problem::{MemoryApiError, MemoryApiProblem, MemoryApiResult};
pub use query::{MemoryQuery, INVALID_QUERY_DETAIL};
pub use readiness::memory_dependency_ready_check;
pub use response::{
    created_resource_json, finish_created_resource_response, finish_no_content_response,
    finish_page_response, finish_resource_response, no_content_json, ok_page_json,
    ok_resource_json, resolved_trace_id, success_created_page_response,
    success_created_resource_response, success_no_content_response, success_page_response,
    success_resource_response,
};
pub use web_runtime::harden_memory_web_framework_layer;

const PRODUCTION_AUTH_UNAVAILABLE: &str = "production memory auth requires IAM PostgreSQL database";

/// How HTTP routers should resolve request context from environment.
pub enum MemoryWebAuthMode {
    DevInline,
    IamDatabase(Box<IamWebRequestContextResolver>),
    ProductionFailClosed,
}

/// Resolve the Memory web auth mode from runtime environment variables.
pub async fn memory_web_auth_mode_from_env() -> MemoryWebAuthMode {
    if memory_use_dev_inline_auth_resolver() {
        return MemoryWebAuthMode::DevInline;
    }

    let iam_database_explicitly_configured = std::env::var("SDKWORK_DATABASE_URL")
        .or_else(|_| std::env::var("SDKWORK_DATABASE_ENGINE"))
        .is_ok();

    if memory_is_production_like_environment() && !iam_database_explicitly_configured {
        return MemoryWebAuthMode::ProductionFailClosed;
    }

    MemoryWebAuthMode::IamDatabase(Box::new(
        sdkwork_iam_web_adapter::IamWebRequestContextResolver::new(
            readiness::shared_iam_postgres_pool().await,
        ),
    ))
}

#[derive(Clone, Default)]
pub struct ProductionFailClosedResolver;

#[async_trait]
impl WebRequestContextResolver for ProductionFailClosedResolver {
    async fn resolve_api_key(
        &self,
        _raw_api_key: &str,
    ) -> Result<WebRequestPrincipal, WebFrameworkError> {
        Err(WebFrameworkError::dependency_unavailable(
            PRODUCTION_AUTH_UNAVAILABLE,
        ))
    }

    async fn resolve_dual_token(
        &self,
        _raw_auth_token: &str,
        _raw_access_token: &str,
    ) -> Result<WebRequestPrincipal, WebFrameworkError> {
        Err(WebFrameworkError::dependency_unavailable(
            PRODUCTION_AUTH_UNAVAILABLE,
        ))
    }

    async fn resolve_access_token(
        &self,
        _raw_access_token: &str,
    ) -> Result<WebRequestPrincipal, WebFrameworkError> {
        Err(WebFrameworkError::dependency_unavailable(
            PRODUCTION_AUTH_UNAVAILABLE,
        ))
    }

    async fn resolve_oauth_bearer(
        &self,
        _raw_bearer_token: &str,
    ) -> Result<WebRequestPrincipal, WebFrameworkError> {
        Err(WebFrameworkError::dependency_unavailable(
            PRODUCTION_AUTH_UNAVAILABLE,
        ))
    }
}

pub fn gateway_mount() -> axum::Router {
    axum::Router::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EnvironmentRestore {
        values: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvironmentRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self {
                values: keys
                    .iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvironmentRestore {
        fn drop(&mut self) {
            for (key, value) in self.values.drain(..) {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime must initialize")
            .block_on(future)
    }

    #[test]
    fn production_without_iam_database_uses_fail_closed_mode() {
        let _guard = sdkwork_memory_contract::runtime_env::env_test_lock();
        let _restore = EnvironmentRestore::capture(&[
            "SDKWORK_MEMORY_ENVIRONMENT",
            "SDKWORK_MEMORY_DEV_AUTH_BYPASS",
            "SDKWORK_DATABASE_URL",
            "SDKWORK_DATABASE_ENGINE",
        ]);
        std::env::set_var("SDKWORK_MEMORY_ENVIRONMENT", "production");
        std::env::remove_var("SDKWORK_MEMORY_DEV_AUTH_BYPASS");
        std::env::remove_var("SDKWORK_DATABASE_URL");
        std::env::remove_var("SDKWORK_DATABASE_ENGINE");

        let mode = block_on(memory_web_auth_mode_from_env());
        assert!(matches!(mode, MemoryWebAuthMode::ProductionFailClosed));
    }

    #[test]
    fn production_without_iam_database_fails_dependency_ready_check() {
        let _guard = sdkwork_memory_contract::runtime_env::env_test_lock();
        let _restore = EnvironmentRestore::capture(&[
            "SDKWORK_MEMORY_ENVIRONMENT",
            "SDKWORK_MEMORY_DEV_AUTH_BYPASS",
            "SDKWORK_DATABASE_URL",
            "SDKWORK_DATABASE_ENGINE",
        ]);
        std::env::set_var("SDKWORK_MEMORY_ENVIRONMENT", "production");
        std::env::remove_var("SDKWORK_MEMORY_DEV_AUTH_BYPASS");
        std::env::remove_var("SDKWORK_DATABASE_URL");
        std::env::remove_var("SDKWORK_DATABASE_ENGINE");

        assert!(!block_on(super::readiness::memory_dependency_ready_check()));
    }
}
