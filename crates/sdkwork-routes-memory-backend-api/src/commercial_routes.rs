//! Commercial management route handlers for the backend API.

use axum::{
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use sdkwork_routes_memory_support::{MemoryJson, MemoryPath};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    CreateBindingCommand, CreateCapabilityBindingCommand, CreateEdgeCommand, CreateEntityCommand,
    CreatePolicyAssignmentCommand, CreatePolicyCommand, CreateSubjectCommand, ListBindingsQuery,
    ListCapabilityBindingsQuery, ListEdgesQuery, ListEntitiesQuery, ListPoliciesQuery,
    ListPolicyAssignmentsQuery, ListSubjectsQuery, ListUsageQuery, MemoryBackendRequestContext,
    RebuildCommercialReadinessCommand, ResolveCapabilitiesQuery, UpdateEdgeCommand,
    UpdateEntityCommand, UpdatePolicyAssignmentCommand, UpdatePolicyCommand, UpdateSubjectCommand,
};
use sdkwork_routes_memory_support::{
    success_created_resource_response, success_no_content_response, success_page_response,
    success_resource_response, MemoryQuery as Query,
};

use crate::{auth::require_backend_context, paths, routes::BackendState, BackendApiProblem};

pub fn commercial_routes() -> Router {
    Router::new()
        .route(paths::SUBJECTS, get(list_subjects).post(create_subject))
        .route(
            paths::SUBJECT,
            get(retrieve_subject)
                .patch(update_subject)
                .delete(delete_subject),
        )
        .route(paths::BINDINGS, get(list_bindings).post(create_binding))
        .route(paths::BINDING, get(retrieve_binding).delete(delete_binding))
        .route(
            paths::CAPABILITY_BINDINGS,
            get(list_capability_bindings).post(create_capability_binding),
        )
        .route(
            paths::CAPABILITY_BINDING,
            get(retrieve_capability_binding).delete(delete_capability_binding),
        )
        .route(paths::CAPABILITIES_RESOLVE, post(resolve_capabilities))
        .route(paths::ENTITIES, get(list_entities).post(create_entity))
        .route(paths::ENTITY, get(retrieve_entity).patch(update_entity))
        .route(paths::EDGES, get(list_edges).post(create_edge))
        .route(
            paths::EDGE,
            get(retrieve_edge).patch(update_edge).delete(delete_edge),
        )
        .route(paths::POLICIES, get(list_policies).post(create_policy))
        .route(
            paths::POLICY,
            get(retrieve_policy)
                .patch(update_policy)
                .delete(delete_policy),
        )
        .route(
            paths::POLICY_ASSIGNMENTS,
            get(list_policy_assignments).post(create_policy_assignment),
        )
        .route(
            paths::POLICY_ASSIGNMENT,
            get(retrieve_policy_assignment)
                .patch(update_policy_assignment)
                .delete(delete_policy_assignment),
        )
        .route(
            paths::COMMERCIAL_READINESS,
            get(retrieve_commercial_readiness),
        )
        .route(
            paths::COMMERCIAL_READINESS_REBUILD,
            post(rebuild_commercial_readiness),
        )
        .route(paths::USAGE, get(list_usage))
}

// --- Subject handlers ---

async fn create_subject(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateSubjectCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product.create_subject(cmd).await {
        Ok(subject) => success_created_resource_response(subject),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_subject(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(subject_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.retrieve_subject(tenant_id, &subject_id).await {
        Ok(subject) => success_resource_response(subject),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_subjects(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListSubjectsQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product.list_subjects(query).await {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn update_subject(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(subject_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdateSubjectCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.update_subject(tenant_id, &subject_id, cmd).await {
        Ok(subject) => success_resource_response(subject),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn delete_subject(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(subject_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.delete_subject(tenant_id, &subject_id).await {
        Ok(()) => success_no_content_response(),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Binding handlers ---

async fn create_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateBindingCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product.create_binding(cmd).await {
        Ok(binding) => success_created_resource_response(binding),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(binding_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.retrieve_binding(tenant_id, &binding_id).await {
        Ok(binding) => success_resource_response(binding),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_bindings(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListBindingsQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product.list_bindings(query).await {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn delete_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(binding_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.delete_binding(tenant_id, &binding_id).await {
        Ok(()) => success_no_content_response(),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Capability binding handlers ---

async fn create_capability_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateCapabilityBindingCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product.create_capability_binding(cmd).await {
        Ok(cap) => success_created_resource_response(cap),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_capability_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(cap_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .retrieve_capability_binding(tenant_id, &cap_id)
        .await
    {
        Ok(cap) => success_resource_response(cap),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_capability_bindings(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListCapabilityBindingsQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product.list_capability_bindings(query).await {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn delete_capability_binding(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(cap_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.delete_capability_binding(tenant_id, &cap_id).await {
        Ok(()) => success_no_content_response(),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Capability resolution ---

async fn resolve_capabilities(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut req): MemoryJson<ResolveCapabilitiesQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    req.tenant_id = context.tenant_id;
    match product.resolve_capabilities(req).await {
        Ok(page) => success_page_response(page),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Entity handlers ---

async fn create_entity(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateEntityCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product
        .create_entity(OpenMemoryService::to_open_context_backend(&context), cmd)
        .await
    {
        Ok(entity) => success_created_resource_response(entity),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_entity(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(entity_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .retrieve_entity(
            OpenMemoryService::to_open_context_backend(&context),
            tenant_id,
            &entity_id,
        )
        .await
    {
        Ok(entity) => success_resource_response(entity),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_entities(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListEntitiesQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product
        .list_entities(OpenMemoryService::to_open_context_backend(&context), query)
        .await
    {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn update_entity(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(entity_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdateEntityCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .update_entity(
            OpenMemoryService::to_open_context_backend(&context),
            tenant_id,
            &entity_id,
            cmd,
        )
        .await
    {
        Ok(entity) => success_resource_response(entity),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Edge handlers ---

async fn create_edge(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreateEdgeCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product
        .create_edge(OpenMemoryService::to_open_context_backend(&context), cmd)
        .await
    {
        Ok(edge) => success_created_resource_response(edge),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_edge(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(edge_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .retrieve_edge(
            OpenMemoryService::to_open_context_backend(&context),
            tenant_id,
            &edge_id,
        )
        .await
    {
        Ok(edge) => success_resource_response(edge),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_edges(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListEdgesQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product
        .list_edges(OpenMemoryService::to_open_context_backend(&context), query)
        .await
    {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn update_edge(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(edge_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdateEdgeCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .update_edge(
            OpenMemoryService::to_open_context_backend(&context),
            tenant_id,
            &edge_id,
            cmd,
        )
        .await
    {
        Ok(edge) => success_resource_response(edge),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn delete_edge(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(edge_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .delete_edge(
            OpenMemoryService::to_open_context_backend(&context),
            tenant_id,
            &edge_id,
        )
        .await
    {
        Ok(()) => success_no_content_response(),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Policy handlers ---

async fn create_policy(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreatePolicyCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product.create_policy(cmd).await {
        Ok(policy) => success_created_resource_response(policy),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_policy(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(policy_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.retrieve_policy(tenant_id, &policy_id).await {
        Ok(policy) => success_resource_response(policy),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_policies(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListPoliciesQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product.list_policies(query).await {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn update_policy(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(policy_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdatePolicyCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.update_policy(tenant_id, &policy_id, cmd).await {
        Ok(policy) => success_resource_response(policy),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn delete_policy(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(policy_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.delete_policy(tenant_id, &policy_id).await {
        Ok(()) => success_no_content_response(),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Policy assignment handlers ---

async fn create_policy_assignment(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<CreatePolicyAssignmentCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product.create_policy_assignment(cmd).await {
        Ok(assignment) => success_created_resource_response(assignment),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn retrieve_policy_assignment(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(assignment_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .retrieve_policy_assignment(tenant_id, &assignment_id)
        .await
    {
        Ok(assignment) => success_resource_response(assignment),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn list_policy_assignments(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListPolicyAssignmentsQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product.list_policy_assignments(query).await {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn update_policy_assignment(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(assignment_id): MemoryPath<String>,
    MemoryJson(cmd): MemoryJson<UpdatePolicyAssignmentCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .update_policy_assignment(tenant_id, &assignment_id, cmd)
        .await
    {
        Ok(assignment) => success_resource_response(assignment),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn delete_policy_assignment(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryPath(assignment_id): MemoryPath<String>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product
        .delete_policy_assignment(tenant_id, &assignment_id)
        .await
    {
        Ok(()) => success_no_content_response(),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Commercial readiness handlers ---

async fn retrieve_commercial_readiness(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    let tenant_id = context.tenant_id;
    match product.retrieve_commercial_readiness(tenant_id).await {
        Ok(readiness) => success_resource_response(readiness),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

async fn rebuild_commercial_readiness(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    MemoryJson(mut cmd): MemoryJson<RebuildCommercialReadinessCommand>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    cmd.tenant_id = context.tenant_id;
    match product.rebuild_commercial_readiness(cmd).await {
        Ok(readiness) => success_resource_response(readiness),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}

// --- Usage metering handlers ---

async fn list_usage(
    Extension(state): Extension<BackendState>,
    context: Option<Extension<MemoryBackendRequestContext>>,
    Query(mut query): Query<ListUsageQuery>,
) -> Response {
    let product = match state.require_product() {
        Ok(product) => product,
        Err(resp) => return resp,
    };
    let context = match require_backend_context(context) {
        Ok(ctx) => ctx,
        Err(problem) => return problem.into_response(),
    };
    query.tenant_id = context.tenant_id;
    match product.list_usage(query).await {
        Ok(list) => success_page_response(list),
        Err(error) => BackendApiProblem::from(error).into_response(),
    }
}
