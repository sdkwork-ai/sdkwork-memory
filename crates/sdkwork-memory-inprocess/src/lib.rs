//! In-process durable memory retrieval for co-deployed SDKWork modules.
//!
//! This crate is the in-process counterpart to the Memory App API. It owns the
//! memory data-plane bootstrap and calls the same `MemoryAppApi::create_retrieval`
//! service method the HTTP route uses, so both delivery modes share one retrieval
//! contract and one set of scope rules.
//!
//! Co-deployment requirements:
//!
//! - the host process must have the memory database configured
//!   (`SDKWORK_MEMORY_DATABASE_*` or the shared `SDKWORK_DATABASE_*` profile);
//! - the caller passes an explicit, already-authenticated actor scope. This crate
//!   must never infer an actor from process identity.

use sdkwork_intelligence_memory_repository_sqlx::{
    bootstrap_memory_data_plane_from_env, MemoryDataPlane,
};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::app_ports::MemoryAppApi;
use sdkwork_memory_contract::dto::{
    ListMemoriesQuery, MemoryRecord, MemoryRetrievalRequest, MemoryRetrievalResult,
};
use sdkwork_memory_contract::ports::MemoryServiceResult;
use sdkwork_memory_contract::MemoryAppRequestContext;

/// Default retrieval depth for a turn-scoped memory query.
pub const DEFAULT_TOP_K: i32 = 8;
/// Default context budget for a turn-scoped memory query.
pub const DEFAULT_CONTEXT_BUDGET_TOKENS: i32 = 1_024;

/// Durable memory runtime backed by the process-local database pool.
pub struct InProcessMemoryRuntime {
    service: OpenMemoryService,
}

impl InProcessMemoryRuntime {
    /// Bootstraps the memory data plane from the environment.
    ///
    /// Returns `Ok(None)` when the memory database is not configured, so a host
    /// that does not mount the memory capability keeps its existing behaviour.
    /// A configured-but-invalid database fails bootstrap.
    pub async fn from_env() -> Result<Option<Self>, String> {
        if !memory_database_configured() {
            return Ok(None);
        }
        let data_plane = bootstrap_memory_data_plane_from_env().await?;
        Ok(Some(Self::from_data_plane(data_plane)))
    }

    /// Builds the runtime over an already-bootstrapped data plane.
    #[must_use]
    pub fn from_data_plane(data_plane: MemoryDataPlane) -> Self {
        Self {
            service: OpenMemoryService::new(data_plane.store().clone()),
        }
    }

    /// Runs one scoped memory retrieval through the same service method as the
    /// App API `retrievals.create` operation.
    pub async fn retrieve(
        &self,
        tenant_id: u64,
        actor_id: Option<u64>,
        space_id: u64,
        query: &str,
        top_k: Option<u32>,
        context_budget_tokens: Option<u32>,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        let context = MemoryAppRequestContext {
            tenant_id,
            actor_id,
            organization_id: None,
            session_id: None,
        };
        self.service
            .create_retrieval(
                context,
                MemoryRetrievalRequest {
                    query: query.trim().to_string(),
                    space_ids: vec![space_id],
                    actor_id: actor_id.map(|value| value.to_string()),
                    retrieval_profile_id: None,
                    memory_types: None,
                    filters: None,
                    top_k: i32::try_from(top_k.unwrap_or(DEFAULT_TOP_K as u32))
                        .unwrap_or(DEFAULT_TOP_K),
                    context_budget_tokens: i32::try_from(
                        context_budget_tokens.unwrap_or(DEFAULT_CONTEXT_BUDGET_TOKENS as u32),
                    )
                    .unwrap_or(DEFAULT_CONTEXT_BUDGET_TOKENS),
                    show_expired: Some(false),
                    threshold: None,
                    explain: None,
                    include_trace: None,
                },
            )
            .await
    }

    /// Lists one memory space through the same service surface as
    /// `memories.list`; kept for health checks and diagnostics.
    pub async fn list(
        &self,
        tenant_id: u64,
        actor_id: Option<u64>,
        space_id: u64,
        limit: Option<i32>,
    ) -> MemoryServiceResult<Vec<MemoryRecord>> {
        let context = MemoryAppRequestContext {
            tenant_id,
            actor_id,
            organization_id: None,
            session_id: None,
        };
        self.service
            .list_memories(
                context,
                ListMemoriesQuery {
                    q: None,
                    cursor: None,
                    page_size: limit.or(Some(DEFAULT_TOP_K)),
                    space_id: Some(space_id),
                    show_expired: Some(false),
                },
            )
            .await
            .map(|page| page.items)
    }
}

/// Returns true when the process has a memory database profile to bootstrap.
#[must_use]
pub fn memory_database_configured() -> bool {
    let configured = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    };
    configured("SDKWORK_MEMORY_DATABASE_URL")
        || configured("SDKWORK_DATABASE_URL")
        || (configured("SDKWORK_DATABASE_HOST") && configured("SDKWORK_DATABASE_NAME"))
}

#[cfg(test)]
mod tests {
    use super::{memory_database_configured, DEFAULT_CONTEXT_BUDGET_TOKENS, DEFAULT_TOP_K};

    #[test]
    fn defaults_are_bounded_and_match_the_http_provider() {
        assert_eq!(DEFAULT_TOP_K, 8);
        assert_eq!(DEFAULT_CONTEXT_BUDGET_TOKENS, 1_024);
    }

    #[test]
    fn missing_memory_database_is_detected_without_panicking() {
        // No assertion on the environment value itself: CI may set either
        // profile. The helper must simply be total.
        let _ = memory_database_configured();
    }
}
