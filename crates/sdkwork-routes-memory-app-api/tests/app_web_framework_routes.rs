use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
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
use sdkwork_memory_test_support::web_auth::{
    lock_integration_test_env, memory_access_token, memory_auth_token_bearer,
};
use sdkwork_routes_memory_app_api::{
    build_router_with_shared_app_api, wrap_router_with_iam_database_web_framework,
};
use std::sync::{Arc, Mutex};
use tower::util::ServiceExt;

#[tokio::test]
async fn app_router_web_framework_rejects_unauthenticated_requests() {
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_shared_app_api(Arc::new(RecordingAppApi::default())),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/app/v3/api/memory/learning_settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn app_router_web_framework_accepts_dev_jwt_dual_tokens_before_handler() {
    let _env = lock_integration_test_env().await;
    let service = RecordingAppApi::default();
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_shared_app_api(Arc::new(service.clone())),
    );

    let response = app
        .oneshot(
            Request::builder()
                .uri("/app/v3/api/memory/learning_settings")
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
struct RecordingAppApi {
    tenant_ids: Arc<Mutex<Vec<u64>>>,
}

impl RecordingAppApi {
    fn tenant_ids(&self) -> Vec<u64> {
        self.tenant_ids.lock().unwrap().clone()
    }
}

#[async_trait]
impl MemoryAppApi for RecordingAppApi {
    async fn retrieve_learning_settings(
        &self,
        ctx: MemoryAppRequestContext,
    ) -> MemoryServiceResult<MemoryLearningSettings> {
        self.tenant_ids.lock().unwrap().push(ctx.tenant_id);
        Ok(MemoryLearningSettings {
            auto_promote_candidates: false,
            habit_learning_enabled: true,
            updated_at: "2026-06-10T00:00:00Z".to_string(),
        })
    }

    async fn list_spaces(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_space(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_space(
        &self,
        _context: MemoryAppRequestContext,
        _space_id: u64,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_space(
        &self,
        _context: MemoryAppRequestContext,
        _space_id: u64,
        _request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_event(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryEventRequest,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_event(
        &self,
        _context: MemoryAppRequestContext,
        _event_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_memories(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_memory(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_memory(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_memory(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _space_id: u64,
        _patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn delete_memory(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _space_id: u64,
    ) -> MemoryServiceResult<()> {
        unimplemented!("stub -- not called in web-framework test")
    }
    async fn delete_all_memories(
        &self,
        _context: MemoryAppRequestContext,
        _request: sdkwork_memory_contract::DeleteAllMemoriesRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::DeleteAllMemoriesResult> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_memory_sources(
        &self,
        _context: MemoryAppRequestContext,
        _memory_id: u64,
        _query: ListMemorySourcesQuery,
    ) -> MemoryServiceResult<MemoryRecordSourceList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_forget_request(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryForgetRequest,
    ) -> MemoryServiceResult<MemoryForgetJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_forget_requests(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryForgetJobList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_forget_request(
        &self,
        _context: MemoryAppRequestContext,
        _forget_job_id: u64,
    ) -> MemoryServiceResult<MemoryForgetJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_extraction(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_candidates(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_candidate(
        &self,
        _context: MemoryAppRequestContext,
        _candidate_id: u64,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn approve_candidate(
        &self,
        _context: MemoryAppRequestContext,
        _candidate_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn reject_candidate(
        &self,
        _context: MemoryAppRequestContext,
        _candidate_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_habits(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListHabitsQuery,
    ) -> MemoryServiceResult<MemoryHabitList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
        _request: MemoryHabitRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn confirm_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn reject_habit(
        &self,
        _context: MemoryAppRequestContext,
        _habit_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_retrieval(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryRetrievalRequest,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_retrieval(
        &self,
        _context: MemoryAppRequestContext,
        _retrieval_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_context_pack(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryContextPackRequest,
    ) -> MemoryServiceResult<MemoryContextPack> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_context_pack(
        &self,
        _context: MemoryAppRequestContext,
        _context_pack_id: u64,
    ) -> MemoryServiceResult<MemoryContextPack> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_feedback(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryFeedbackRequest,
    ) -> MemoryServiceResult<MemoryFeedback> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn create_export_job(
        &self,
        _context: MemoryAppRequestContext,
        _request: MemoryExportRequest,
    ) -> MemoryServiceResult<MemoryExportJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn list_export_jobs(
        &self,
        _context: MemoryAppRequestContext,
        _query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryExportJobList> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn retrieve_export_job(
        &self,
        _context: MemoryAppRequestContext,
        _export_job_id: u64,
    ) -> MemoryServiceResult<MemoryExportJob> {
        unimplemented!("stub -- not called in web-framework test")
    }

    async fn update_learning_settings(
        &self,
        _context: MemoryAppRequestContext,
        _patch: MemoryLearningSettingsPatch,
    ) -> MemoryServiceResult<MemoryLearningSettings> {
        unimplemented!("stub -- not called in web-framework test")
    }
}
