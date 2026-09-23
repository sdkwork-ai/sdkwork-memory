use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use sdkwork_memory_contract::space::ListSpacesQuery;
use sdkwork_memory_contract::{
    ListCandidatesQuery, ListHabitsQuery, ListJobsQuery, ListMemoriesQuery, ListMemorySourcesQuery,
    MemoryAppApi, MemoryAppRequestContext, MemoryCandidate, MemoryCandidateList, MemoryContextPack,
    MemoryContextPackRequest, MemoryEvent, MemoryEventRequest, MemoryExportJob,
    MemoryExportJobList, MemoryExportRequest, MemoryExtractionRequest, MemoryFeedback,
    MemoryFeedbackRequest, MemoryForgetJob, MemoryForgetJobList, MemoryForgetRequest, MemoryHabit,
    MemoryHabitList, MemoryHabitRequest, MemoryLearningJob, MemoryLearningSettings,
    MemoryLearningSettingsPatch, MemoryRecord, MemoryRecordList, MemoryRecordPatch,
    MemoryRecordRequest, MemoryRecordSourceList, MemoryRetrievalRequest, MemoryRetrievalResult,
    MemoryReviewRequest, MemoryServiceResult, MemorySpace, MemorySpaceList, MemorySpaceRequest,
};
use sdkwork_routes_memory_app_api::build_router_with_shared_app_api;
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

#[tokio::test]
async fn app_router_mounts_every_app_openapi_operation_path() {
    let spec: Value = serde_json::from_str(include_str!(
        "../../../sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json"
    ))
    .unwrap();
    let app = build_router_with_shared_app_api(Arc::new(StubAppApi));

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

struct StubAppApi;

#[async_trait]
impl MemoryAppApi for StubAppApi {
    async fn list_spaces(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_space(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_space(
        &self,
        _context: MemoryAppRequestContext,
        _space_id: u64,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn update_space(
        &self,
        _context: MemoryAppRequestContext,
        _space_id: u64,
        _request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_event(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryEventRequest,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_event(
        &self,
        _context: MemoryAppRequestContext,
        _event_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn list_memories(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_memory(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_memory(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn update_memory(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _space_id: u64,
        _patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn delete_memory(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<()> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn delete_all_memories(
        &self,
        _context: MemoryAppRequestContext,
        _request: sdkwork_memory_contract::DeleteAllMemoriesRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::DeleteAllMemoriesResult> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn list_memory_sources(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _query: ListMemorySourcesQuery,
    ) -> MemoryServiceResult<MemoryRecordSourceList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_forget_request(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryForgetRequest,
    ) -> MemoryServiceResult<MemoryForgetJob> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn list_forget_requests(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryForgetJobList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_forget_request(
        &self,
        _context: MemoryAppRequestContext,
        _forget_job_id: u64,
    ) -> MemoryServiceResult<MemoryForgetJob> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_extraction(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn list_candidates(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_candidate(
        &self,
        _context: MemoryAppRequestContext,
        _candidate_id: u64,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn approve_candidate(
        &self,
        _context: MemoryAppRequestContext,
        _candidate_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn reject_candidate(
        &self,
        _context: MemoryAppRequestContext,
        _candidate_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn list_habits(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListHabitsQuery,
    ) -> MemoryServiceResult<MemoryHabitList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn update_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
        _request: MemoryHabitRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn confirm_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn reject_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_retrieval(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryRetrievalRequest,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_retrieval(
        &self,
        _context: MemoryAppRequestContext,
        _retrieval_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_context_pack(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryContextPackRequest,
    ) -> MemoryServiceResult<MemoryContextPack> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_context_pack(
        &self,
        _context: MemoryAppRequestContext,
        _context_pack_id: u64,
    ) -> MemoryServiceResult<MemoryContextPack> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_feedback(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryFeedbackRequest,
    ) -> MemoryServiceResult<MemoryFeedback> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn create_export_job(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryExportRequest,
    ) -> MemoryServiceResult<MemoryExportJob> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn list_export_jobs(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryExportJobList> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_export_job(
        &self,
        _context: MemoryAppRequestContext,
        _export_job_id: u64,
    ) -> MemoryServiceResult<MemoryExportJob> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn retrieve_learning_settings(
        &self,
        _context: MemoryAppRequestContext,
    ) -> MemoryServiceResult<MemoryLearningSettings> {
        unimplemented!("stub -- not called in route-mount test")
    }

    async fn update_learning_settings(
        &self,
        _context: MemoryAppRequestContext,
        _patch: MemoryLearningSettingsPatch,
    ) -> MemoryServiceResult<MemoryLearningSettings> {
        unimplemented!("stub -- not called in route-mount test")
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
        .replace("{spaceId}", "1")
        .replace("{eventId}", "1")
        .replace("{memoryId}", "1")
        .replace("{forgetRequestId}", "1")
        .replace("{candidateId}", "1")
        .replace("{habitId}", "1")
        .replace("{retrievalId}", "1")
        .replace("{contextPackId}", "1")
        .replace("{exportJobId}", "1")
}

fn request_body(_operation_id: &str) -> &'static str {
    "{}"
}
