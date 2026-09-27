//! Commercial entity and edge routes for the Open API.

use sdkwork_routes_memory_support::{MemoryJson, MemoryPath};
use axum::{response::Response, routing::get, Extension, Router};
use sdkwork_memory_contract::{
    CreateEdgeCommand, CreateEntityCommand, ListEdgesQuery, ListEntitiesQuery,
    MemoryOpenApiRequestContext, UpdateEdgeCommand, UpdateEntityCommand,
};
use sdkwork_routes_memory_support::{
    created_resource_json, no_content_json, ok_page_json, ok_resource_json, MemoryQuery as Query,
};

use crate::{auth::require_context, paths, routes::OpenState, ApiProblem};

pub fn commercial_routes() -> Router {
    Router::new()
        .route(paths::ENTITIES, get(list_entities).post(create_entity))
        .route(paths::ENTITY, get(retrieve_entity).patch(update_entity))
        .route(paths::EDGES, get(list_edges).post(create_edge))
        .route(
            paths::EDGE,
            get(retrieve_edge).patch(update_edge).delete(delete_edge),
        )
}

async fn create_entity(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateEntityCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    cmd.tenant_id = context.tenant_id;
    created_resource_json(product.create_entity(context.clone(), cmd).await)
}

async fn retrieve_entity(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryPath(entity_id): MemoryPath<String>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    ok_resource_json(
        product
            .retrieve_entity(context.clone(), context.tenant_id, &entity_id)
            .await,
    )
}

async fn list_entities(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Query(mut query): Query<ListEntitiesQuery>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    query.tenant_id = context.tenant_id;
    ok_page_json(product.list_entities(context.clone(), query).await)
}

async fn update_entity(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryPath(entity_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdateEntityCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    ok_resource_json(
        product
            .update_entity(context.clone(), context.tenant_id, &entity_id, cmd)
            .await,
    )
}

async fn create_edge(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateEdgeCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    cmd.tenant_id = context.tenant_id;
    created_resource_json(product.create_edge(context.clone(), cmd).await)
}

async fn retrieve_edge(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryPath(edge_id): MemoryPath<String>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    ok_resource_json(
        product
            .retrieve_edge(context.clone(), context.tenant_id, &edge_id)
            .await,
    )
}

async fn list_edges(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Query(mut query): Query<ListEdgesQuery>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    query.tenant_id = context.tenant_id;
    ok_page_json(product.list_edges(context.clone(), query).await)
}

async fn update_edge(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryPath(edge_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdateEdgeCommand>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    ok_resource_json(
        product
            .update_edge(context.clone(), context.tenant_id, &edge_id, cmd)
            .await,
    )
}

async fn delete_edge(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    MemoryPath(edge_id): MemoryPath<String>,
) -> Result<Response, ApiProblem> {
    let product = state.require_product()?;
    let context = require_context(context)?;
    no_content_json(
        product
            .delete_edge(context.clone(), context.tenant_id, &edge_id)
            .await,
    )
}
