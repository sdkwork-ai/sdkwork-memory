use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Extension, Router,
};
use sdkwork_routes_memory_support::{MemoryJson, MemoryPath};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    ListAdminResourcesQuery, ListAuditLogsQuery, ListCandidatesQuery, ListEventsQuery,
    ListJobsQuery, ListMemoriesQuery, ListRetrievalTracesQuery, ListSpacesQuery, MemoryBackendApi,
    MemoryBackendRequestContext, MemoryEvalRunRequest, MemoryExtractionRequest,
    MemoryImplementationProfileRequest, MemoryIndexRequest, MemoryMigrationJobRequest,
    MemoryProviderBindingRequest, MemoryRecordPatch, MemoryRecordRequest,
    MemoryRetentionJobRequest, MemoryRetrievalProfileRequest, MemoryReviewRequest,
    MemorySpaceRequest, MemorySpaceScopeQuery,
};
use sdkwork_routes_memory_support::{
    created_resource_json, ok_page_json, ok_resource_json, MemoryQuery as Query,
};
use std::sync::Arc;

use crate::{auth::require_backend_context, paths, BackendApiProblem};

#[derive(Clone)]
pub struct BackendState {
    api: Arc<dyn MemoryBackendApi>,
    product: Option<Arc<OpenMemoryService>>,
}

impl BackendState {
    #[allow(clippy::result_large_err)]
    pub(crate) fn require_product(&self) -> Result<Arc<OpenMemoryService>, Response> {
        self.product.clone().ok_or_else(|| {
            BackendApiProblem::new(
                StatusCode::NOT_IMPLEMENTED,
                "not_implemented",
                "commercial management requires OpenMemoryService",
            )
            .into_response()
        })
    }
}

pub fn build_router_with_backend_api(api: OpenMemoryService) -> Router {
    build_router_with_open_memory_service(Arc::new(api))
}

pub fn build_router_with_open_memory_service(product: Arc<OpenMemoryService>) -> Router {
    let api: Arc<dyn MemoryBackendApi> = product.clone();
    build_backend_router(BackendState {
        api,
        product: Some(product),
    })
}

pub fn build_router_with_shared_backend_api(api: Arc<dyn MemoryBackendApi>) -> Router {
    build_backend_router(BackendState { api, product: None })
}

fn build_backend_router(state: BackendState) -> Router {
    Router::new()
        .route(paths::SPACES, get(list_spaces))
        .route(paths::SPACE, get(retrieve_space).patch(update_space))
        .route(paths::MEMORIES, get(list_memories))
        .route(paths::MEMORY, get(retrieve_memory).patch(update_memory))
        .route(paths::MEMORY_SUPERSEDE, post(supersede_memory))
        .route(paths::EVENTS, get(list_events))
        .route(paths::EVENT, get(retrieve_event))
        .route(paths::CANDIDATES, get(list_candidates))
        .route(paths::CANDIDATE_APPROVE, post(approve_candidate))
        .route(paths::CANDIDATE_REJECT, post(reject_candidate))
        .route(
            paths::EXTRACTION_JOBS,
            get(list_extraction_jobs).post(create_extraction_job),
        )
        .route(paths::EXTRACTION_JOB, get(retrieve_extraction_job))
        .route(
            paths::CONSOLIDATION_JOBS,
            get(list_consolidation_jobs).post(create_consolidation_job),
        )
        .route(paths::CONSOLIDATION_JOB, get(retrieve_consolidation_job))
        .route(paths::INDEXES, get(list_indexes).post(create_index))
        .route(paths::INDEX, get(retrieve_index).patch(update_index))
        .route(paths::INDEX_REBUILD, post(rebuild_index))
        .route(
            paths::RETRIEVAL_PROFILES,
            get(list_retrieval_profiles).post(create_retrieval_profile),
        )
        .route(
            paths::RETRIEVAL_PROFILE,
            get(retrieve_retrieval_profile).patch(update_retrieval_profile),
        )
        .route(
            paths::IMPLEMENTATION_PROFILES,
            get(list_implementation_profiles).post(create_implementation_profile),
        )
        .route(
            paths::IMPLEMENTATION_PROFILE,
            get(retrieve_implementation_profile).patch(update_implementation_profile),
        )
        .route(
            paths::PROVIDER_BINDINGS,
            get(list_provider_bindings).post(create_provider_binding),
        )
        .route(paths::PROVIDER_BINDING, patch(update_provider_binding))
        .route(paths::PROVIDER_HEALTH, get(retrieve_provider_health))
        .route(paths::EVAL_RUNS, get(list_eval_runs).post(create_eval_run))
        .route(paths::EVAL_RUN, get(retrieve_eval_run))
        .route(paths::RETRIEVAL_TRACES, get(list_retrieval_traces))
        .route(paths::RETRIEVAL_TRACE, get(retrieve_retrieval_trace))
        .route(paths::AUDIT_LOGS, get(list_audit_logs))
        .route(
            paths::RETENTION_JOBS,
            get(list_retention_jobs).post(create_retention_job),
        )
        .route(paths::RETENTION_JOB, get(retrieve_retention_job))
        .route(
            paths::MIGRATION_JOBS,
            get(list_migration_jobs).post(create_migration_job),
        )
        .route(paths::MIGRATION_JOB, get(retrieve_migration_job))
        .merge(crate::commercial_routes::commercial_routes())
        .layer(Extension(state))
}

async fn list_spaces(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListSpacesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_spaces(context, query).await)
}

async fn retrieve_space(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(space_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_space(context, space_id).await)
}

async fn update_space(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(space_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemorySpaceRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.update_space(context, space_id, request).await)
}

async fn list_memories(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListMemoriesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_memories(context, query).await)
}

async fn retrieve_memory(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(memory_id): MemoryPath<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_memory(context, memory_id, scope.space_id)
            .await,
    )
}

async fn update_memory(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(memory_id): MemoryPath<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
    MemoryJson(patch): MemoryJson<MemoryRecordPatch>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .update_memory(context, memory_id, scope.space_id, patch)
            .await,
    )
}

async fn supersede_memory(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(memory_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryRecordRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .supersede_memory(context, memory_id, request)
            .await,
    )
}

async fn list_events(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListEventsQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_events(context, query).await)
}

async fn retrieve_event(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(event_id): MemoryPath<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_event(context, event_id, scope.space_id)
            .await,
    )
}

async fn list_candidates(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListCandidatesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_candidates(context, query).await)
}

async fn approve_candidate(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(candidate_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryReviewRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .approve_candidate(context, candidate_id, request)
            .await,
    )
}

async fn reject_candidate(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(candidate_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryReviewRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .reject_candidate(context, candidate_id, request)
            .await,
    )
}

async fn create_extraction_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryExtractionRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_extraction_job(context, request).await)
}

async fn list_extraction_jobs(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_extraction_jobs(context, query).await)
}

async fn retrieve_extraction_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(job_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_extraction_job(context, job_id).await)
}

async fn create_consolidation_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryExtractionRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_consolidation_job(context, request).await)
}

async fn list_consolidation_jobs(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_consolidation_jobs(context, query).await)
}

async fn retrieve_consolidation_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(job_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_consolidation_job(context, job_id).await)
}

async fn list_indexes(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListAdminResourcesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_indexes(context, query).await)
}

async fn create_index(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryIndexRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_index(context, request).await)
}

async fn retrieve_index(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(index_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_index(context, index_id).await)
}

async fn update_index(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(index_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryIndexRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.update_index(context, index_id, request).await)
}

async fn rebuild_index(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(index_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryReviewRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .rebuild_index(context, index_id, request)
            .await,
    )
}

async fn list_retrieval_profiles(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListAdminResourcesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_retrieval_profiles(context, query).await)
}

async fn create_retrieval_profile(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryRetrievalProfileRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_retrieval_profile(context, request).await)
}

async fn retrieve_retrieval_profile(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(profile_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_retrieval_profile(context, profile_id)
            .await,
    )
}

async fn update_retrieval_profile(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(profile_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryRetrievalProfileRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .update_retrieval_profile(context, profile_id, request)
            .await,
    )
}

async fn list_implementation_profiles(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListAdminResourcesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_implementation_profiles(context, query).await)
}

async fn create_implementation_profile(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryImplementationProfileRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(
        state
            .api
            .create_implementation_profile(context, request)
            .await,
    )
}

async fn retrieve_implementation_profile(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(implementation_profile_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_implementation_profile(context, implementation_profile_id)
            .await,
    )
}

async fn update_implementation_profile(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(implementation_profile_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryImplementationProfileRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .update_implementation_profile(context, implementation_profile_id, request)
            .await,
    )
}

async fn list_provider_bindings(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListAdminResourcesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_provider_bindings(context, query).await)
}

async fn create_provider_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryProviderBindingRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_provider_binding(context, request).await)
}

async fn update_provider_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(provider_binding_id): MemoryPath<u64>,
    MemoryJson(request): MemoryJson<MemoryProviderBindingRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .update_provider_binding(context, provider_binding_id, request)
            .await,
    )
}

async fn retrieve_provider_health(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_provider_health(context).await)
}

async fn list_eval_runs(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListAdminResourcesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_eval_runs(context, query).await)
}

async fn create_eval_run(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryEvalRunRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_eval_run(context, request).await)
}

async fn retrieve_eval_run(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(eval_run_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_eval_run(context, eval_run_id).await)
}

async fn list_retrieval_traces(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListRetrievalTracesQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_retrieval_traces(context, query).await)
}

async fn retrieve_retrieval_trace(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(trace_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(state.api.retrieve_retrieval_trace(context, trace_id).await)
}

async fn list_audit_logs(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListAuditLogsQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_audit_logs(context, query).await)
}

async fn create_retention_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryRetentionJobRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_retention_job(context, request).await)
}

async fn list_retention_jobs(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_retention_jobs(context, query).await)
}

async fn retrieve_retention_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(retention_job_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_retention_job(context, retention_job_id)
            .await,
    )
}

async fn create_migration_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(request): MemoryJson<MemoryMigrationJobRequest>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    created_resource_json(state.api.create_migration_job(context, request).await)
}

async fn list_migration_jobs(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(query): Query<ListJobsQuery>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_page_json(state.api.list_migration_jobs(context, query).await)
}

async fn retrieve_migration_job(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(migration_job_id): MemoryPath<u64>,
) -> Result<Response, BackendApiProblem> {
    let context = require_backend_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_migration_job(context, migration_job_id)
            .await,
    )
}
