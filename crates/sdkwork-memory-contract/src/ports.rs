use async_trait::async_trait;

use crate::dto::{
    ListCandidatesQuery, ListMemoriesQuery, MemoryCandidate, MemoryCandidateList,
    MemoryCapabilities, MemoryContextPack, MemoryContextPackRequest, MemoryEvent,
    MemoryEventRequest, MemoryExtractionRequest, MemoryFeedback, MemoryFeedbackRequest,
    MemoryLearningJob, MemoryProviderHealth, MemoryRecord, MemoryRecordList, MemoryRecordPatch,
    MemoryRecordRequest, MemoryRetrievalRequest, MemoryRetrievalResult,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryOpenApiRequestContext {
    pub api_key_id: String,
    pub tenant_id: u64,
    pub actor_id: Option<u64>,
    /// Backend operators authorized at the router layer may access all tenant spaces.
    pub elevated_tenant_access: bool,
}

impl MemoryOpenApiRequestContext {
    pub fn for_open_surface(
        api_key_id: impl Into<String>,
        tenant_id: u64,
        actor_id: Option<u64>,
    ) -> Self {
        Self {
            api_key_id: api_key_id.into(),
            tenant_id,
            actor_id,
            elevated_tenant_access: false,
        }
    }

    pub fn for_backend_surface(tenant_id: u64, operator_id: Option<u64>) -> Self {
        Self {
            api_key_id: format!("backend-{}", operator_id.unwrap_or(0)),
            tenant_id,
            actor_id: operator_id,
            elevated_tenant_access: true,
        }
    }

    /// Non-elevated context for background workers acting on behalf of a tenant actor.
    pub fn for_background_job(tenant_id: u64, actor_id: Option<u64>) -> Self {
        Self {
            api_key_id: "background-job".to_string(),
            tenant_id,
            actor_id,
            elevated_tenant_access: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryServiceErrorKind {
    NotFound,
    Conflict,
    Validation,
    Forbidden,
    QuotaExceeded,
    Storage,
    NotImplemented,
}

/// Service-layer error carrying the `detail` that the API layer renders into the problem payload.
///
/// # `detail` contract
///
/// `detail` is **authored, operator-facing text and is returned to every caller** — the route layer
/// copies it verbatim into the `application/problem+json` body
/// (`sdkwork-routes-memory-support::problem::MemoryApiError::from`). Two rules follow:
///
/// 1. Every constructor must **preserve** the detail it is given. A constructor that silently
///    replaces an authored diagnostic with a fixed string destroys the only signal an operator
///    has, and turns every distinct failure below it into one indistinguishable message.
///    `storage` in particular used to discard its argument while still accepting one — every
///    call site that passed a real reason was writing dead code.
/// 2. Raw provider/database/plugin errors must **never** be passed here. They are unpredictable
///    and may embed connection strings or row contents. They are logged and masked at the single
///    boundary that consumes them
///    (`sdkwork-intelligence-memory-service::store_error::{map_memory_spi_error,
///    map_native_sql_store_error}`), which emits the generic
///    [`STORAGE_ERROR_DETAIL`] string and records the real cause in the server log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryServiceError {
    pub kind: MemoryServiceErrorKind,
    pub code: String,
    pub detail: String,
}

/// Client-safe stand-in detail for failures whose real cause is a raw provider/database error.
///
/// Only the two masking mappers in `sdkwork-intelligence-memory-service::store_error` may use this
/// constant; the real cause goes to the server log instead.
pub const STORAGE_ERROR_DETAIL: &str = "internal storage error";

impl MemoryServiceError {
    pub fn not_found(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::NotFound,
            code: "not_found".to_string(),
            detail: detail.into(),
        }
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::Conflict,
            code: "conflict".to_string(),
            detail: detail.into(),
        }
    }

    pub fn validation(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::Validation,
            code: "validation_error".to_string(),
            detail: detail.into(),
        }
    }

    pub fn invalid_parameter(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::Validation,
            code: "invalid_parameter".to_string(),
            detail: detail.into(),
        }
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::Forbidden,
            code: "forbidden".to_string(),
            detail: detail.into(),
        }
    }

    pub fn quota_exceeded(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::QuotaExceeded,
            code: "quota_exceeded".to_string(),
            detail: detail.into(),
        }
    }

    /// Storage-layer failure carrying an authored, client-safe diagnostic.
    ///
    /// The `detail` MUST be a message this codebase wrote (for example
    /// `"export jsonl encode failed: {serde_error}"`). Never pass a raw provider or database error
    /// here: let it surface through
    /// `sdkwork-intelligence-memory-service::store_error::map_native_sql_store_error`, which masks
    /// it to [`STORAGE_ERROR_DETAIL`] and logs the cause.
    pub fn storage(detail: impl Into<String>) -> Self {
        Self {
            kind: MemoryServiceErrorKind::Storage,
            code: "storage_error".to_string(),
            detail: detail.into(),
        }
    }

    pub fn not_implemented(operation_id: &'static str) -> Self {
        Self {
            kind: MemoryServiceErrorKind::NotImplemented,
            code: "operation_not_implemented".to_string(),
            detail: format!("operation is not implemented: {operation_id}"),
        }
    }
}

pub type MemoryServiceResult<T> = Result<T, MemoryServiceError>;

#[async_trait]
pub trait MemoryOpenApi: Send + Sync + 'static {
    async fn retrieve_capabilities(
        &self,
        context: MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<MemoryCapabilities>;

    async fn create_event(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryEventRequest,
    ) -> MemoryServiceResult<MemoryEvent>;

    async fn retrieve_event(
        &self,
        context: MemoryOpenApiRequestContext,
        event_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent>;

    async fn list_memories(
        &self,
        context: MemoryOpenApiRequestContext,
        query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList>;

    async fn create_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn retrieve_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn update_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        memory_id: u64,
        space_id: u64,
        patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn delete_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<()>;

    async fn create_retrieval(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryRetrievalRequest,
    ) -> MemoryServiceResult<MemoryRetrievalResult>;

    async fn retrieve_retrieval(
        &self,
        context: MemoryOpenApiRequestContext,
        retrieval_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalResult>;

    async fn create_context_pack(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryContextPackRequest,
    ) -> MemoryServiceResult<MemoryContextPack>;

    async fn retrieve_context_pack(
        &self,
        context: MemoryOpenApiRequestContext,
        context_pack_id: u64,
    ) -> MemoryServiceResult<MemoryContextPack>;

    async fn create_feedback(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryFeedbackRequest,
    ) -> MemoryServiceResult<MemoryFeedback>;

    async fn create_extraction(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_candidates(
        &self,
        context: MemoryOpenApiRequestContext,
        query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList>;

    async fn retrieve_candidate(
        &self,
        context: MemoryOpenApiRequestContext,
        candidate_id: u64,
    ) -> MemoryServiceResult<MemoryCandidate>;

    async fn retrieve_provider_health(
        &self,
        context: MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth>;
}
