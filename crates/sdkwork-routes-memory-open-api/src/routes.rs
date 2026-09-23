use axum::{
    extract::Path,
    http::StatusCode,
    response::Response,
    routing::{get, post},
    Extension, Json, Router,
};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, ListCandidatesQuery, ListMemoriesQuery, MemoryContextPackRequest,
    MemoryEventRequest, MemoryExtractionRequest, MemoryFeedbackRequest, MemoryOpenApi,
    MemoryOpenApiRequestContext, MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalRequest,
    MemorySpaceScopeQuery,
};
use sdkwork_routes_memory_support::{
    created_resource_json, no_content_json, ok_page_json, ok_resource_json, MemoryQuery as Query,
};
use std::sync::Arc;

use crate::{auth::require_context, paths, ApiProblem};

#[derive(Clone)]
pub struct OpenState {
    api: Arc<dyn MemoryOpenApi>,
    product: Option<Arc<OpenMemoryService>>,
}

impl OpenState {
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

pub fn build_router_with_open_api(api: OpenMemoryService) -> Router {
    build_router_with_open_memory_service(Arc::new(api))
}

pub fn build_router_with_open_memory_service(product: Arc<OpenMemoryService>) -> Router {
    let api: Arc<dyn MemoryOpenApi> = product.clone();
    build_open_router(OpenState {
        api,
        product: Some(product),
    })
}

pub fn build_router_with_shared_open_api(api: Arc<dyn MemoryOpenApi>) -> Router {
    build_open_router(OpenState { api, product: None })
}

fn build_open_router(state: OpenState) -> Router {
    Router::new()
        .route(paths::CAPABILITIES, get(retrieve_capabilities))
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
        .route(paths::RETRIEVALS, post(create_retrieval))
        .route(paths::RETRIEVAL, get(retrieve_retrieval))
        .route(paths::CONTEXT_PACKS, post(create_context_pack))
        .route(paths::CONTEXT_PACK, get(retrieve_context_pack))
        .route(paths::FEEDBACK, post(create_feedback))
        .route(paths::EXTRACTIONS, post(create_extraction))
        .route(paths::CANDIDATES, get(list_candidates))
        .route(paths::CANDIDATE, get(retrieve_candidate))
        .route(paths::PROVIDER_HEALTH, get(retrieve_provider_health))
        .merge(crate::commercial_routes::commercial_routes())
        .layer(Extension(state))
}

async fn retrieve_capabilities(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(state.api.retrieve_capabilities(context).await)
}

async fn create_event(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<MemoryEventRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    created_resource_json(state.api.create_event(context, request).await)
}

async fn retrieve_event(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(event_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_event(context, event_id, scope.space_id)
            .await,
    )
}

async fn list_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Query(query): Query<ListMemoriesQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_page_json(state.api.list_memories(context, query).await)
}

async fn create_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<MemoryRecordRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    created_resource_json(state.api.create_memory(context, request).await)
}

async fn retrieve_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_memory(context, memory_id, scope.space_id)
            .await,
    )
}

async fn update_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
    Json(patch): Json<MemoryRecordPatch>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(
        state
            .api
            .update_memory(context, memory_id, scope.space_id, patch)
            .await,
    )
}

async fn delete_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(memory_id): Path<u64>,
    Query(scope): Query<MemorySpaceScopeQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    no_content_json(
        state
            .api
            .delete_memory(context, memory_id, scope.space_id)
            .await,
    )
}

async fn delete_all_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<DeleteAllMemoriesRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(state.api.delete_all_memories(context, request).await)
}

async fn create_retrieval(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<MemoryRetrievalRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    created_resource_json(state.api.create_retrieval(context, request).await)
}

async fn retrieve_retrieval(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(retrieval_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(state.api.retrieve_retrieval(context, retrieval_id).await)
}

async fn create_context_pack(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<MemoryContextPackRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    created_resource_json(state.api.create_context_pack(context, request).await)
}

async fn retrieve_context_pack(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(context_pack_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(
        state
            .api
            .retrieve_context_pack(context, context_pack_id)
            .await,
    )
}

async fn create_feedback(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<MemoryFeedbackRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    created_resource_json(state.api.create_feedback(context, request).await)
}

async fn create_extraction(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Json(request): Json<MemoryExtractionRequest>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    created_resource_json(state.api.create_extraction(context, request).await)
}

async fn list_candidates(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Query(query): Query<ListCandidatesQuery>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_page_json(state.api.list_candidates(context, query).await)
}

async fn retrieve_candidate(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(candidate_id): Path<u64>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(state.api.retrieve_candidate(context, candidate_id).await)
}

async fn retrieve_provider_health(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
) -> Result<Response, ApiProblem> {
    let context = require_context(context)?;
    ok_resource_json(state.api.retrieve_provider_health(context).await)
}
