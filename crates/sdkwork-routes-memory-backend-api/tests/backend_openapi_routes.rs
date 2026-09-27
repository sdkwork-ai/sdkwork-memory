use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use sdkwork_memory_contract::{
    ListAdminResourcesQuery, ListAuditLogsQuery, ListCandidatesQuery, ListEventsQuery,
    ListJobsQuery, ListMemoriesQuery, ListRetrievalTracesQuery, ListSpacesQuery,
    MemoryAuditLogList, MemoryBackendApi, MemoryBackendRequestContext, MemoryCandidate,
    MemoryCandidateList, MemoryEvalRun, MemoryEvalRunList, MemoryEvalRunRequest, MemoryEvent,
    MemoryEventList, MemoryExtractionRequest, MemoryImplementationProfile,
    MemoryImplementationProfileList, MemoryImplementationProfileRequest, MemoryIndex,
    MemoryIndexList, MemoryIndexRequest, MemoryLearningJob, MemoryLearningJobList,
    MemoryMigrationJobRequest, MemoryProviderBinding, MemoryProviderBindingList,
    MemoryProviderBindingRequest, MemoryProviderHealth, MemoryRecord, MemoryRecordList,
    MemoryRecordPatch, MemoryRecordRequest, MemoryRetentionJobRequest, MemoryRetrievalProfile,
    MemoryRetrievalProfileList, MemoryRetrievalProfileRequest, MemoryRetrievalTrace,
    MemoryRetrievalTraceList, MemoryReviewRequest, MemoryServiceResult, MemorySpace,
    MemorySpaceList, MemorySpaceRequest,
};
use sdkwork_routes_memory_backend_api::build_router_with_shared_backend_api;
use serde_json::Value;
use std::sync::Arc;
use tower::util::ServiceExt;

#[tokio::test]
async fn backend_router_mounts_every_backend_openapi_operation_path() {
    let spec: Value = serde_json::from_str(include_str!(
        "../../../sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json"
    ))
    .unwrap();
    let app = build_router_with_shared_backend_api(Arc::new(StubBackendApi));

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

struct StubBackendApi;

#[async_trait]
impl MemoryBackendApi for StubBackendApi {
    async fn list_spaces(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_space(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn update_space(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_memories(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_memory(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _s: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn update_memory(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _s: u64,
        _p: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn supersede_memory(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_events(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListEventsQuery,
    ) -> MemoryServiceResult<MemoryEventList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_event(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _s: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_candidates(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn approve_candidate(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn reject_candidate(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_extraction_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_extraction_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_extraction_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_consolidation_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_consolidation_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_consolidation_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_indexes(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryIndexList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_index(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_index(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryIndex> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn update_index(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn rebuild_index(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_retrieval_profiles(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalProfileList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_retrieval_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_retrieval_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn update_retrieval_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_implementation_profiles(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryImplementationProfileList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_implementation_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_implementation_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn update_implementation_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_provider_bindings(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryProviderBindingList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_provider_binding(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn update_provider_binding(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_provider_health(
        &self,
        _c: MemoryBackendRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_eval_runs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryEvalRunList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_eval_run(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryEvalRunRequest,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_eval_run(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_retrieval_traces(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListRetrievalTracesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalTraceList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_retrieval_trace(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalTrace> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_audit_logs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAuditLogsQuery,
    ) -> MemoryServiceResult<MemoryAuditLogList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_retention_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryRetentionJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_retention_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_retention_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn create_migration_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryMigrationJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn list_migration_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in route-mount test")
    }
    async fn retrieve_migration_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
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
        .replace("{memoryId}", "1")
        .replace("{eventId}", "1")
        .replace("{candidateId}", "1")
        .replace("{extractionJobId}", "1")
        .replace("{consolidationJobId}", "1")
        .replace("{indexId}", "1")
        .replace("{retrievalProfileId}", "1")
        .replace("{implementationProfileId}", "1")
        .replace("{providerBindingId}", "1")
        .replace("{evalRunId}", "1")
        .replace("{retrievalTraceId}", "1")
        .replace("{migrationJobId}", "1")
        .replace("{retentionJobId}", "1")
        .replace("{subjectId}", "1")
        .replace("{bindingId}", "1")
        .replace("{capabilityBindingId}", "1")
        .replace("{entityId}", "1")
        .replace("{edgeId}", "1")
        .replace("{policyAssignmentId}", "1")
        .replace("{jobId}", "1")
}

fn request_body(_operation_id: &str) -> &'static str {
    "{}"
}
