use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use sdkwork_memory_contract::{
    ListCandidatesQuery, ListMemoriesQuery, MemoryCandidate, MemoryCandidateList,
    MemoryCapabilities, MemoryContextPack, MemoryContextPackRequest, MemoryEvent,
    MemoryEventRequest, MemoryExtractionRequest, MemoryFeedback, MemoryFeedbackRequest,
    MemoryImplementationKind, MemoryLearningJob, MemoryOpenApi, MemoryOpenApiRequestContext,
    MemoryProviderHealth, MemoryProviderInterface, MemoryRecord, MemoryRecordList,
    MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalRequest, MemoryRetrievalResult,
    MemoryRetrieverKind, MemoryServiceResult,
};
use sdkwork_memory_test_support::web_auth::memory_dev_api_key;
use sdkwork_routes_memory_open_api::{
    build_router_with_shared_open_api, wrap_router_with_web_framework,
};
use sdkwork_web_core::DefaultWebRequestContextResolver;
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

#[tokio::test]
async fn open_router_web_framework_rejects_unauthenticated_requests() {
    let app = wrap_router_with_web_framework(
        DefaultWebRequestContextResolver::default(),
        build_router_with_shared_open_api(Arc::new(RecordingOpenApi::default())),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/mem/v3/api/memory/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn open_router_web_framework_accepts_dev_inline_api_key_before_handler() {
    let service = RecordingOpenApi::default();
    let app = wrap_router_with_web_framework(
        DefaultWebRequestContextResolver::default(),
        build_router_with_shared_open_api(Arc::new(service.clone())),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/mem/v3/api/memory/capabilities")
                .header("x-api-key", memory_dev_api_key("2001", "dev-key"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(service.contexts(), vec![("dev-key".to_owned(), 100_001)]);
}

#[derive(Clone, Default)]
struct RecordingOpenApi {
    contexts: Arc<Mutex<Vec<(String, u64)>>>,
}

impl RecordingOpenApi {
    fn contexts(&self) -> Vec<(String, u64)> {
        self.contexts.lock().unwrap().clone()
    }
}

#[async_trait]
impl MemoryOpenApi for RecordingOpenApi {
    async fn retrieve_capabilities(
        &self,
        ctx: MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<MemoryCapabilities> {
        self.contexts
            .lock()
            .unwrap()
            .push((ctx.api_key_id, ctx.tenant_id));
        Ok(MemoryCapabilities {
            embedding_optional: true,
            retrievers: vec![MemoryRetrieverKind::Keyword],
            provider_interfaces: vec![MemoryProviderInterface::Memory],
            implementation_kinds: vec![MemoryImplementationKind::NativeSql],
            open_api_prefix: "/mem/v3/api".to_string(),
            sdk_family: "sdkwork-memory-sdk".to_string(),
            checked_at: "2026-06-10T00:00:00Z".to_string(),
            metadata: None,
        })
    }

    async fn create_event(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _request: MemoryEventRequest,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("not called in this test")
    }

    async fn retrieve_event(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _event_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("not called in this test")
    }

    async fn list_memories(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        unimplemented!("not called in this test")
    }

    async fn create_memory(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("not called in this test")
    }

    async fn retrieve_memory(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("not called in this test")
    }

    async fn update_memory(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _memory_id: u64,
        _space_id: u64,
        _patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("not called in this test")
    }

    async fn delete_memory(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<()> {
        unimplemented!("not called in this test")
    }

    async fn create_retrieval(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _request: MemoryRetrievalRequest,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        unimplemented!("not called in this test")
    }

    async fn retrieve_retrieval(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _retrieval_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        unimplemented!("not called in this test")
    }

    async fn create_context_pack(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _request: MemoryContextPackRequest,
    ) -> MemoryServiceResult<MemoryContextPack> {
        unimplemented!("not called in this test")
    }

    async fn retrieve_context_pack(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _context_pack_id: u64,
    ) -> MemoryServiceResult<MemoryContextPack> {
        unimplemented!("not called in this test")
    }

    async fn create_feedback(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _request: MemoryFeedbackRequest,
    ) -> MemoryServiceResult<MemoryFeedback> {
        unimplemented!("not called in this test")
    }

    async fn create_extraction(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("not called in this test")
    }

    async fn list_candidates(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        unimplemented!("not called in this test")
    }

    async fn retrieve_candidate(
        &self,
        _ctx: MemoryOpenApiRequestContext,
        _candidate_id: u64,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("not called in this test")
    }

    async fn retrieve_provider_health(
        &self,
        _ctx: MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth> {
        unimplemented!("not called in this test")
    }
}
