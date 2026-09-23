use async_trait::async_trait;

use crate::dto::{
    DeleteAllMemoriesRequest, DeleteAllMemoriesResult, ListCandidatesQuery, ListHabitsQuery,
    ListJobsQuery, ListMemoriesQuery, ListMemorySourcesQuery, MemoryCandidate, MemoryCandidateList,
    MemoryContextPack, MemoryContextPackRequest, MemoryEvent, MemoryEventRequest, MemoryExportJob,
    MemoryExportJobList, MemoryExportRequest, MemoryExtractionRequest, MemoryFeedback,
    MemoryFeedbackRequest, MemoryForgetJob, MemoryForgetJobList, MemoryForgetRequest, MemoryHabit,
    MemoryHabitList, MemoryHabitRequest, MemoryLearningJob, MemoryLearningSettings,
    MemoryLearningSettingsPatch, MemoryRecord, MemoryRecordList, MemoryRecordPatch,
    MemoryRecordRequest, MemoryRecordSourceList, MemoryRetrievalRequest, MemoryRetrievalResult,
    MemoryReviewRequest,
};
use crate::ports::MemoryServiceResult;
use crate::space::{ListSpacesQuery, MemorySpace, MemorySpaceList, MemorySpaceRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryAppRequestContext {
    pub tenant_id: u64,
    pub actor_id: Option<u64>,
    pub organization_id: Option<u64>,
    pub session_id: Option<String>,
}

#[async_trait]
pub trait MemoryAppApi: Send + Sync + 'static {
    async fn list_spaces(
        &self,
        context: MemoryAppRequestContext,
        query: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList>;

    async fn create_space(
        &self,
        context: MemoryAppRequestContext,
        request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace>;

    async fn retrieve_space(
        &self,
        context: MemoryAppRequestContext,
        space_id: u64,
    ) -> MemoryServiceResult<MemorySpace>;

    async fn update_space(
        &self,
        context: MemoryAppRequestContext,
        space_id: u64,
        request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace>;

    async fn create_event(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryEventRequest,
    ) -> MemoryServiceResult<MemoryEvent>;

    async fn retrieve_event(
        &self,
        context: MemoryAppRequestContext,
        event_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent>;

    async fn list_memories(
        &self,
        context: MemoryAppRequestContext,
        query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList>;

    async fn create_memory(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn retrieve_memory(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn update_memory(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        space_id: u64,
        patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord>;

    async fn delete_memory(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<()>;

    /// Bulk-delete every active record in the space (optionally narrowed to
    /// one owner), the analogue of mem0's `delete_all`.
    async fn delete_all_memories(
        &self,
        context: MemoryAppRequestContext,
        request: DeleteAllMemoriesRequest,
    ) -> MemoryServiceResult<DeleteAllMemoriesResult>;

    async fn list_memory_sources(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        query: ListMemorySourcesQuery,
    ) -> MemoryServiceResult<MemoryRecordSourceList>;

    async fn create_forget_request(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryForgetRequest,
    ) -> MemoryServiceResult<MemoryForgetJob>;

    async fn list_forget_requests(
        &self,
        context: MemoryAppRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryForgetJobList>;

    async fn retrieve_forget_request(
        &self,
        context: MemoryAppRequestContext,
        forget_job_id: u64,
    ) -> MemoryServiceResult<MemoryForgetJob>;

    async fn create_extraction(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob>;

    async fn list_candidates(
        &self,
        context: MemoryAppRequestContext,
        query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList>;

    async fn retrieve_candidate(
        &self,
        context: MemoryAppRequestContext,
        candidate_id: u64,
    ) -> MemoryServiceResult<MemoryCandidate>;

    async fn approve_candidate(
        &self,
        context: MemoryAppRequestContext,
        candidate_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate>;

    async fn reject_candidate(
        &self,
        context: MemoryAppRequestContext,
        candidate_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate>;

    async fn list_habits(
        &self,
        context: MemoryAppRequestContext,
        query: ListHabitsQuery,
    ) -> MemoryServiceResult<MemoryHabitList>;

    async fn retrieve_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
    ) -> MemoryServiceResult<MemoryHabit>;

    async fn update_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
        request: MemoryHabitRequest,
    ) -> MemoryServiceResult<MemoryHabit>;

    async fn confirm_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit>;

    async fn reject_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit>;

    async fn create_retrieval(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryRetrievalRequest,
    ) -> MemoryServiceResult<MemoryRetrievalResult>;

    async fn retrieve_retrieval(
        &self,
        context: MemoryAppRequestContext,
        retrieval_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalResult>;

    async fn create_context_pack(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryContextPackRequest,
    ) -> MemoryServiceResult<MemoryContextPack>;

    async fn retrieve_context_pack(
        &self,
        context: MemoryAppRequestContext,
        context_pack_id: u64,
    ) -> MemoryServiceResult<MemoryContextPack>;

    async fn create_feedback(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryFeedbackRequest,
    ) -> MemoryServiceResult<MemoryFeedback>;

    async fn create_export_job(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryExportRequest,
    ) -> MemoryServiceResult<MemoryExportJob>;

    async fn list_export_jobs(
        &self,
        context: MemoryAppRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryExportJobList>;

    async fn retrieve_export_job(
        &self,
        context: MemoryAppRequestContext,
        export_job_id: u64,
    ) -> MemoryServiceResult<MemoryExportJob>;

    async fn retrieve_learning_settings(
        &self,
        context: MemoryAppRequestContext,
    ) -> MemoryServiceResult<MemoryLearningSettings>;

    async fn update_learning_settings(
        &self,
        context: MemoryAppRequestContext,
        patch: MemoryLearningSettingsPatch,
    ) -> MemoryServiceResult<MemoryLearningSettings>;
}
