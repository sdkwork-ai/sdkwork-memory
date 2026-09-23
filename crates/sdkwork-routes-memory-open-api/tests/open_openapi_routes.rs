use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use sdkwork_memory_contract::MemoryOpenApi;
use sdkwork_routes_memory_open_api::build_router_with_shared_open_api;
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

#[tokio::test]
async fn open_router_mounts_every_open_openapi_operation_path() {
    let spec: Value = serde_json::from_str(include_str!(
        "../../../sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json"
    ))
    .unwrap();
    let app = build_router_with_shared_open_api(Arc::new(StubOpenApi));

    let paths = spec["paths"].as_object().unwrap();
    for (template_path, methods) in paths {
        for (method_name, operation) in methods.as_object().unwrap() {
            if !["get", "post", "put", "patch", "delete"].contains(&method_name.as_str()) {
                continue;
            }
            let operation_id = operation["operationId"].as_str().unwrap();
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method_from_openapi(method_name))
                        .uri(concrete_uri(template_path))
                        .header("content-type", "application/json")
                        .body(Body::from(request_body(operation_id)))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_ne!(
                response.status(),
                StatusCode::NOT_FOUND,
                "{operation_id} route from OpenAPI is not mounted: {method_name} {template_path}",
            );
        }
    }
}

struct StubOpenApi;

#[async_trait]
impl MemoryOpenApi for StubOpenApi {
    async fn retrieve_capabilities(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryCapabilities>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn create_event(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::MemoryEventRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryEvent> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn retrieve_event(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _event_id: u64,
        _space_id: u64,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryEvent> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn list_memories(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _query: sdkwork_memory_contract::ListMemoriesQuery,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryRecordList>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn create_memory(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::MemoryRecordRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn retrieve_memory(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn update_memory(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _memory_id: u64,
        _space_id: u64,
        _patch: sdkwork_memory_contract::MemoryRecordPatch,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn delete_memory(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> sdkwork_memory_contract::MemoryServiceResult<()> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn delete_all_memories(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::DeleteAllMemoriesRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<
        sdkwork_memory_contract::DeleteAllMemoriesResult,
    > {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn create_retrieval(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::MemoryRetrievalRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryRetrievalResult>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn retrieve_retrieval(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _retrieval_id: u64,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryRetrievalResult>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn create_context_pack(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::MemoryContextPackRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryContextPack>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn retrieve_context_pack(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _context_pack_id: u64,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryContextPack>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn create_feedback(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::MemoryFeedbackRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryFeedback> {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn create_extraction(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _request: sdkwork_memory_contract::MemoryExtractionRequest,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryLearningJob>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn list_candidates(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _query: sdkwork_memory_contract::ListCandidatesQuery,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryCandidateList>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn retrieve_candidate(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
        _candidate_id: u64,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryCandidate>
    {
        unimplemented!("stub — not called in route-mount test")
    }

    async fn retrieve_provider_health(
        &self,
        _context: sdkwork_memory_contract::MemoryOpenApiRequestContext,
    ) -> sdkwork_memory_contract::MemoryServiceResult<sdkwork_memory_contract::MemoryProviderHealth>
    {
        unimplemented!("stub — not called in route-mount test")
    }
}

fn method_from_openapi(method_name: &str) -> Method {
    match method_name {
        "delete" => Method::DELETE,
        "get" => Method::GET,
        "patch" => Method::PATCH,
        "post" => Method::POST,
        "put" => Method::PUT,
        value => panic!("unsupported OpenAPI method: {value}"),
    }
}

fn concrete_uri(template_path: &str) -> String {
    template_path
        .replace("{eventId}", "1")
        .replace("{memoryId}", "1")
        .replace("{retrievalId}", "1")
        .replace("{contextPackId}", "1")
        .replace("{candidateId}", "1")
}

fn request_body(_operation_id: &str) -> &'static str {
    "{}"
}
