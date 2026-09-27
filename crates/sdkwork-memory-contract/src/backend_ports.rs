use async_trait::async_trait;

use crate::admin_dto::{
    ListAdminResourcesQuery, MemoryAuditLogList, MemoryEvalRun, MemoryEvalRunList,
    MemoryEvalRunRequest, MemoryImplementationProfile, MemoryImplementationProfileList,
    MemoryImplementationProfileRequest, MemoryIndex, MemoryIndexList, MemoryIndexRequest,
    MemoryMigrationJobRequest, MemoryProviderBindingList, MemoryProviderBindingRequest,
    MemoryRetentionJobRequest, MemoryRetrievalProfile, MemoryRetrievalProfileList,
    MemoryRetrievalProfileRequest,
};
use crate::dto::{
    ListAuditLogsQuery, ListCandidatesQuery, ListEventsQuery, ListJobsQuery, ListMemoriesQuery,
    ListRetrievalTracesQuery, MemoryCandidate, MemoryCandidateList, MemoryEvent, MemoryEventList,
    MemoryExtractionRequest, MemoryLearningJob, MemoryLearningJobList, MemoryProviderHealth,
    MemoryRecord, MemoryRecordList, MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalTrace,
    MemoryRetrievalTraceList, MemoryReviewRequest,
};
use crate::ports::MemoryServiceResult;
use crate::space::{ListSpacesQuery, MemorySpace, MemorySpaceList, MemorySpaceRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryBackendRequestContext {
    pub tenant_id: u64,
    pub operator_id: Option<u64>,
}

#[async_trait]
pub trait MemoryBackendApi: Send + Sync + 'static {
    async fn list_spaces(
        &self,
        context: MemoryBackendRequestContext,
        query: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList>;

    async fn retrieve_space(
        &self,
        context: MemoryBackendRequestContext,
        space_id: u64,
    ) -> MemoryServiceResult<MemorySpace>;

    async fn update_space(
        &self,
        context: MemoryBackendRequestContext,
        space_id: u64,
        request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace>;

    async fn list_memories(
        &self,
        context: MemoryBackendRequestContext,
        query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList>;

    async fn retrieve_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn update_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        space_id: u64,
        patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn supersede_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn list_events(
        &self,
        context: MemoryBackendRequestContext,
        query: ListEventsQuery,
    ) -> MemoryServiceResult<MemoryEventList>;

    async fn retrieve_event(
        &self,
        context: MemoryBackendRequestContext,
        event_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent>;

    async fn list_candidates(
        &self,
        context: MemoryBackendRequestContext,
        query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList>;

    async fn approve_candidate(
        &self,
        context: MemoryBackendRequestContext,
        candidate_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate>;

    async fn reject_candidate(
        &self,
        context: MemoryBackendRequestContext,
        candidate_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate>;

    async fn create_extraction_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_extraction_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList>;

    async fn retrieve_extraction_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn create_consolidation_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_consolidation_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList>;

    async fn retrieve_consolidation_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_indexes(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryIndexList>;

    async fn create_index(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex>;

    async fn retrieve_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
    ) -> MemoryServiceResult<MemoryIndex>;

    async fn update_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
        request: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex>;

    /// Rebuilds a search index. The declared request body carries the typed
    /// review reason for this destructive operation; it is persisted into the
    /// governance audit record alongside the job result.
    async fn rebuild_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_retrieval_profiles(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalProfileList>;

    async fn create_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile>;

    async fn retrieve_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalProfile>;

    async fn update_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
        request: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile>;

    async fn list_implementation_profiles(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryImplementationProfileList>;

    async fn create_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile>;

    async fn retrieve_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
    ) -> MemoryServiceResult<MemoryImplementationProfile>;

    async fn update_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
        request: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile>;

    async fn list_provider_bindings(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryProviderBindingList>;

    async fn create_provider_binding(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<crate::dto::MemoryProviderBinding>;

    async fn update_provider_binding(
        &self,
        context: MemoryBackendRequestContext,
        provider_binding_id: u64,
        request: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<crate::dto::MemoryProviderBinding>;

    async fn retrieve_provider_health(
        &self,
        context: MemoryBackendRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth>;

    async fn list_eval_runs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryEvalRunList>;

    async fn create_eval_run(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryEvalRunRequest,
    ) -> MemoryServiceResult<MemoryEvalRun>;

    async fn retrieve_eval_run(
        &self,
        context: MemoryBackendRequestContext,
        eval_run_id: u64,
    ) -> MemoryServiceResult<MemoryEvalRun>;

    async fn list_retrieval_traces(
        &self,
        context: MemoryBackendRequestContext,
        query: ListRetrievalTracesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalTraceList>;

    async fn retrieve_retrieval_trace(
        &self,
        context: MemoryBackendRequestContext,
        trace_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalTrace>;

    async fn list_audit_logs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAuditLogsQuery,
    ) -> MemoryServiceResult<MemoryAuditLogList>;

    async fn create_retention_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryRetentionJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_retention_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList>;

    async fn retrieve_retention_job(
        &self,
        context: MemoryBackendRequestContext,
        retention_job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn create_migration_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryMigrationJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_migration_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList>;

    async fn retrieve_migration_job(
        &self,
        context: MemoryBackendRequestContext,
        migration_job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob>;
}
