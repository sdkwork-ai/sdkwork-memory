//! Commercial entity and policy assignment routes for the App API.

use axum::{
    response::Response,
    routing::{get, patch},
    Extension, Router,
};
use sdkwork_routes_memory_support::{MemoryJson, MemoryPath};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    CreateEntityCommand, CreatePolicyAssignmentCommand, ListEntitiesQuery,
    ListPolicyAssignmentsQuery, MemoryAppRequestContext, UpdateEntityCommand,
    UpdatePolicyAssignmentCommand,
};
use sdkwork_routes_memory_support::{
    created_resource_json, ok_page_json, ok_resource_json, MemoryQuery as Query,
};

use crate::{auth::require_app_context, paths, routes::AppState, ApiProblem};

pub fn commercial_routes() -> Router {
    Router::new()
        .route(paths::ENTITIES, get(list_entities).post(create_entity))
        .route(paths::ENTITY, get(retrieve_entity).patch(update_entity))
        .route(
            paths::POLICY_ASSIGNMENTS,
            get(list_policy_assignments).post(create_policy_assignment),
        )
        .route(paths::POLICY_ASSIGNMENT, patch(update_policy_assignment))
}

async fn create_entity(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateEntityCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    cmd.tenant_id = context.tenant_id;
    created_resource_json(
        product
            .create_entity(OpenMemoryService::to_open_context(&context), cmd)
            .await,
    )
}

async fn retrieve_entity(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    MemoryPath(entity_id): MemoryPath<String>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    ok_resource_json(
        product
            .retrieve_entity(
                OpenMemoryService::to_open_context(&context),
                context.tenant_id,
                &entity_id,
            )
            .await,
    )
}

async fn list_entities(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(mut query): Query<ListEntitiesQuery>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    query.tenant_id = context.tenant_id;
    ok_page_json(
        product
            .list_entities(OpenMemoryService::to_open_context(&context), query)
            .await,
    )
}

async fn update_entity(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    MemoryPath(entity_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdateEntityCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    ok_resource_json(
        product
            .update_entity(
                OpenMemoryService::to_open_context(&context),
                context.tenant_id,
                &entity_id,
                cmd,
            )
            .await,
    )
}

async fn create_policy_assignment(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    MemoryJson(cmd): MemoryJson<CreatePolicyAssignmentCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    created_resource_json(
        product
            .create_policy_assignment_for_actor(OpenMemoryService::to_open_context(&context), cmd)
            .await,
    )
}

async fn list_policy_assignments(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    Query(mut query): Query<ListPolicyAssignmentsQuery>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    query.tenant_id = context.tenant_id;
    ok_page_json(product.list_policy_assignments(query).await)
}

async fn update_policy_assignment(
    Extension(state): Extension<AppState>,
    context: Option<Extension<MemoryAppRequestContext>>,
    MemoryPath(assignment_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdatePolicyAssignmentCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_app_context(context)?;
    ok_resource_json(
        product
            .update_policy_assignment_for_actor(
                OpenMemoryService::to_open_context(&context),
                context.tenant_id,
                &assignment_id,
                cmd,
            )
            .await,
    )
}
