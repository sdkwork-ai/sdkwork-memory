use axum::{
    extract::Path,
    http::StatusCode,
    response::Response,
    routing::{get, post},
    Extension, Json, Router,
};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, ListCandidatesQuery, ListHabitsQuery, ListJobsQuery,
    ListMemoriesQuery, ListMemorySourcesQuery, ListSpacesQuery, MemoryAppApi,
    MemoryAppRequestContext, MemoryContextPackRequest, MemoryEventRequest, MemoryExportRequest,
    MemoryExtractionRequest, MemoryFeedbackRequest, MemoryForgetRequest, MemoryHabitRequest,
    MemoryLearningSettingsPatch, MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalRequest,
    MemoryReviewRequest, MemorySpaceRequest, MemorySpaceScopeQuery,
};
use sdkwork_routes_memory_support::{
    created_resource_json, no_content_json, ok_page_json, ok_resource_json, MemoryQuery as Query,
};
use std::sync::Arc;

use crate::{auth::require_app_context, paths, ApiProblem};

#[derive(Clone)]
pub struct AppState {
    api: Arc<dyn MemoryAppApi>,
    product: Option<Arc<OpenMemoryService>>,
}

impl AppState {
    pub(crate) fn require_product(&self) -> Result<Arc<OpenMemoryService>, ApiProblem> {
        self.product.clone().ok_or_else(|| {
            ApiProblem::new(
                StatusCode::NOT_IMPLEMENTED,
                "not_implemented",
                "commercial management requires OpenMemoryService",
            )
        })
    }
}

pub fn build_router_with_app_api(api: OpenMemoryService) -> Router {
    build_router_with_open_memory_service(Arc::new(api))
}

pub fn build_router_with_open_memory_service(product: Arc<OpenMemoryService>) -> Router {
    let api: Arc<dyn MemoryAppApi> = product.clone();
    build_app_router(AppState {
        api,
        product: Some(product),
    })
}

pub fn build_router_with_shared_app_api(api: Arc<dyn MemoryAppApi>) -> Router {
    build_app_router(AppState { api, product: None })
}

fn build_app_router(state: AppState) -> Router {
    Router::new()
        .route(paths::SPACES, get(list_spaces).post(create_space))
        .route(paths::SPACE, get(retrieve_space).patch(update_space))
        .route(paths::EVENTS, post(create_event))
        .route(paths::EVENT, get(retrieve_event))
        .route(paths::MEMORIES, get(list_memories).post(create_memory))
        .route(paths::MEMORIES_DELETE_ALL, post(delete_all_memories))
        .route(
            paths::MEMORY,
            get(retrieve_memory)
                .patch(update_memory)
                .delete(delete_memory),
        )
        .route(paths::MEMORY_SOURCES, get(list_memory_sources))
        .route(
            paths::FORGET_REQUESTS,
            get(list_forget_requests).post(create_forget_request),
        )
        .route(paths::FORGET_REQUEST, get(retrieve_forget_request))
        .route(paths::EXTRACTIONS, post(create_extraction))
        .route(paths::CANDIDATES, get(list_candidates))
        .route(paths::CANDIDATE, get(retrieve_candidate))
        .route(paths::CANDIDATE_APPROVE, post(approve_candidate))
        .route(paths::CANDIDATE_REJECT, post(reject_candidate))
        .route(paths::HABITS, get(list_habits))
        .route(paths::HABIT, get(retrieve_habit).patch(update_habit))
        .route(paths::HABIT_CONFIRM, post(confirm_habit))
        .route(paths::HABIT_REJECT, post(reject_habit))
        .route(paths::RETRIEVALS, post(create_retrieval))
        .route(paths::RETRIEVAL, get(retrieve_retrieval))
        .route(paths::CONTEXT_PACKS, post(create_context_pack))
        .route(paths::CONTEXT_PACK, get(retrieve_context_pack))
        .route(paths::FEEDBACK, post(create_feedback))
        .route(
            paths::EXPORT_JOBS,
            get(list_export_jobs).post(create_export_job),
        )
        .route(paths::EXPORT_JOB, get(retrieve_export_job))
        .route(
            paths::LEARNING_SETTINGS,
            get(retrieve_learning_settings).patch(update_learning_settings),
        )
        .merge(crate::commercial_routes::commercial_routes())
        .layer(Extension(state))
}

async fn list_spaces(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(query): Query<ListSpacesQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(state.api.list_spaces(context, query).await)
}

async fn create_space(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemorySpaceRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_space(context, request).await)
}

async fn retrieve_space(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(space_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.retrieve_space(context, space_id).await)
}

async fn update_space(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(space_id): Path<u64>,
    Json(request): Json<MemorySpaceRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.update_space(context, space_id, request).await)
}

async fn create_event(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryEventRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_event(context, request).await)
}

async fn retrieve_event(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(event_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_event(context, event_id, scope.space_id)
            .await,
    )
}

async fn list_memories(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(query): Query<ListMemoriesQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(state.api.list_memories(context, query).await)
}

async fn create_memory(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryRecordRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_memory(context, request).await)
}

async fn retrieve_memory(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_memory(context, memory_id, scope.space_id)
            .await,
    )
}

async fn update_memory(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
    Json(patch): Json<MemoryRecordPatch>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .update_memory(context, memory_id, scope.space_id, patch)
            .await,
    )
}

async fn delete_memory(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    no_content_json(
        state
            .api
            .delete_memory(context, memory_id, scope.space_id)
            .await,
    )
}

async fn delete_all_memories(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<DeleteAllMemoriesRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.delete_all_memories(context, request).await)
}

async fn list_memory_sources(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(query): Query<ListMemorySourcesQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(
        state
            .api
            .list_memory_sources(context, memory_id, query)
            .await,
    )
}

async fn create_forget_request(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryForgetRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_forget_request(context, request).await)
}

async fn list_forget_requests(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(state.api.list_forget_requests(context, query).await)
}

async fn retrieve_forget_request(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(forget_request_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_forget_request(context, forget_request_id)
            .await,
    )
}

async fn create_extraction(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryExtractionRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_extraction(context, request).await)
}

async fn list_candidates(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(query): Query<ListCandidatesQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(state.api.list_candidates(context, query).await)
}

async fn retrieve_candidate(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(candidate_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.retrieve_candidate(context, candidate_id).await)
}

async fn approve_candidate(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(candidate_id): Path<u64>,
    Json(request): Json<MemoryReviewRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .approve_candidate(context, candidate_id, request)
            .await,
    )
}

async fn reject_candidate(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(candidate_id): Path<u64>,
    Json(request): Json<MemoryReviewRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .reject_candidate(context, candidate_id, request)
            .await,
    )
}

async fn list_habits(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(query): Query<ListHabitsQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(state.api.list_habits(context, query).await)
}

async fn retrieve_habit(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(habit_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.retrieve_habit(context, habit_id).await)
}

async fn update_habit(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(habit_id): Path<u64>,
    Json(request): Json<MemoryHabitRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.update_habit(context, habit_id, request).await)
}

async fn confirm_habit(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(habit_id): Path<u64>,
    Json(request): Json<MemoryReviewRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.confirm_habit(context, habit_id, request).await)
}

async fn reject_habit(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(habit_id): Path<u64>,
    Json(request): Json<MemoryReviewRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.reject_habit(context, habit_id, request).await)
}

async fn create_retrieval(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryRetrievalRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_retrieval(context, request).await)
}

async fn retrieve_retrieval(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(retrieval_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.retrieve_retrieval(context, retrieval_id).await)
}

async fn create_context_pack(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryContextPackRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_context_pack(context, request).await)
}

async fn retrieve_context_pack(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(context_pack_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_context_pack(context, context_pack_id)
            .await,
    )
}

async fn create_feedback(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryFeedbackRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_feedback(context, request).await)
}

async fn create_export_job(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(request): Json<MemoryExportRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    created_resource_json(state.api.create_export_job(context, request).await)
}

async fn list_export_jobs(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_page_json(state.api.list_export_jobs(context, query).await)
}

async fn retrieve_export_job(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Path(export_job_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.retrieve_export_job(context, export_job_id).await)
}

async fn retrieve_learning_settings(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.retrieve_learning_settings(context).await)
}

async fn update_learning_settings(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Json(patch): Json<MemoryLearningSettingsPatch>,
) -> Result<Response, ApiProblem> {
    let context = require_app_context(context)?;
    ok_resource_json(state.api.update_learning_settings(context, patch).await)
}
