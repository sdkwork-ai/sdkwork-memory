use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_memory_contract::{
    ListAdminResourcesQuery, ListAuditLogsQuery, ListCandidatesQuery, ListEventsQuery,
    ListJobsQuery, ListMemoriesQuery, ListRetrievalTracesQuery, ListSpacesQuery,
    MemoryAuditLogList, MemoryBackendApi, MemoryBackendRequestContext, MemoryCandidate,
    MemoryCandidateList, MemoryEvalRun, MemoryEvalRunList, MemoryEvalRunRequest, MemoryEvent,
    MemoryEventList, MemoryExtractionRequest, MemoryImplementationProfile,
    MemoryImplementationProfileList, MemoryImplementationProfileRequest, MemoryIndex,
    MemoryIndexList, MemoryIndexRequest, MemoryLearningJob, MemoryLearningJobList,
    MemoryMigrationJobRequest, MemoryProviderBinding, MemoryProviderBindingList,
    MemoryProviderBindingRequest, MemoryProviderHealth, MemoryProviderHealthStatus, MemoryRecord,
    MemoryRecordList, MemoryRecordPatch, MemoryRecordRequest, MemoryRetentionJobRequest,
    MemoryRetrievalProfile, MemoryRetrievalProfileList, MemoryRetrievalProfileRequest,
    MemoryRetrievalTrace, MemoryRetrievalTraceList, MemoryReviewRequest, MemoryServiceResult,
    MemorySpace, MemorySpaceList, MemorySpaceRequest,
};
use sdkwork_memory_test_support::web_auth::{
    lock_integration_test_env, memory_access_token, memory_auth_token_bearer,
};
use sdkwork_routes_memory_backend_api::{
    build_router_with_shared_backend_api, wrap_router_with_iam_database_web_framework,
};
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

#[tokio::test]
async fn backend_router_web_framework_rejects_unauthenticated_requests() {
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_shared_backend_api(Arc::new(RecordingBackendApi::default())),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/memory/provider_health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn backend_router_web_framework_accepts_dev_jwt_dual_tokens_before_handler() {
    let _env = lock_integration_test_env().await;
    let service = RecordingBackendApi::default();
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_shared_backend_api(Arc::new(service.clone())),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/backend/v3/api/memory/provider_health")
                .header("Authorization", memory_auth_token_bearer("9001"))
                .header("Access-Token", memory_access_token("9001"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(service.tenant_ids(), vec![100_001]);
}

#[derive(Clone, Default)]
struct RecordingBackendApi {
    tenant_ids: Arc<Mutex<Vec<u64>>>,
}

impl RecordingBackendApi {
    fn tenant_ids(&self) -> Vec<u64> {
        self.tenant_ids.lock().unwrap().clone()
    }
}

#[async_trait]
impl MemoryBackendApi for RecordingBackendApi {
    async fn list_spaces(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_space(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_space(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_memories(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_memory(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _s: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_memory(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _s: u64,
        _p: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn supersede_memory(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_events(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListEventsQuery,
    ) -> MemoryServiceResult<MemoryEventList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_event(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _s: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_candidates(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn approve_candidate(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn reject_candidate(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_extraction_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_extraction_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_extraction_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_consolidation_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_consolidation_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_consolidation_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_indexes(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryIndexList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_index(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_index(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryIndex> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_index(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn rebuild_index(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_retrieval_profiles(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalProfileList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_retrieval_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_retrieval_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_retrieval_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_implementation_profiles(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryImplementationProfileList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_implementation_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_implementation_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_implementation_profile(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_provider_bindings(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryProviderBindingList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_provider_binding(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_provider_binding(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
        _r: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_provider_health(
        &self,
        ctx: MemoryBackendRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth> {
        self.tenant_ids.lock().unwrap().push(ctx.tenant_id);
        Ok(MemoryProviderHealth {
            status: MemoryProviderHealthStatus::Healthy,
            checked_at: "2026-06-10T00:00:00Z".to_string(),
            providers: vec![],
        })
    }

    async fn list_eval_runs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryEvalRunList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_eval_run(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryEvalRunRequest,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_eval_run(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_retrieval_traces(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListRetrievalTracesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalTraceList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_retrieval_trace(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalTrace> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_audit_logs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListAuditLogsQuery,
    ) -> MemoryServiceResult<MemoryAuditLogList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_retention_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryRetentionJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_retention_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_retention_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_migration_job(
        &self,
        _c: MemoryBackendRequestContext,
        _r: MemoryMigrationJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_migration_jobs(
        &self,
        _c: MemoryBackendRequestContext,
        _q: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_migration_job(
        &self,
        _c: MemoryBackendRequestContext,
        _id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }
}
