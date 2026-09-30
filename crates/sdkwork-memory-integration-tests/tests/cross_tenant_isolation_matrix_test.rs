//! Cross-tenant adversarial isolation matrix (G1).
//!
//! `space_isolation_security_test.rs` proves space-level isolation inside one
//! tenant. Nothing proved the tenant boundary itself: tenant `100_001` (A) and
//! tenant `100_002` (B) are seeded side by side in one store, and every
//! assertion below holds from B's side of the wall — reads, writes, deletes,
//! listings, cursor tokens, forget jobs, export jobs, and graph entities must
//! all either answer only with B-scoped data or refuse with 404/403.
//!
//! Refusals are asserted as `NOT_FOUND | FORBIDDEN` on purpose: the exact
//! status depends on whether the handler resolves the id before or after the
//! tenant predicate, and both answers are isolation-correct. A `200` (or any
//! leak-shaped answer) fails the matrix.
//!
//! Positive controls (B reading B's own data) are interleaved so a broken
//! tenant-`100_002` credential chain surfaces as a loud positive-control
//! failure instead of avacuous pass of the refusal assertions.

#![allow(clippy::await_holding_lock)] // Process-wide test environment must remain serialized.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_spi::{
    CreateMemoryCandidateCommand, MemoryCandidateStorePort, MemoryScopeContext,
};
use sdkwork_memory_test_support::api_envelope;
use sdkwork_memory_test_support::space_fixtures;
use sdkwork_memory_test_support::web_auth::{lock_integration_test_env, memory_idempotency_key};
use sdkwork_routes_memory_app_api::{
    build_router_with_app_api, wrap_router_with_iam_database_web_framework,
};
use sdkwork_routes_memory_open_api::build_router_with_shared_open_api;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::util::ServiceExt;

const TENANT_A: i64 = 100_001;
const TENANT_B: i64 = 100_002;
const TENANT_A_USER: &str = "9001";
const TENANT_B_USER: &str = "2002";
const MEMORY_APP_ID: &str = "sdkwork-memory";
const SESSION_ID: &str = "s-1";

/// Auth-token JWT minted for an arbitrary tenant (the shared `web_auth`
/// fixtures pin `DEFAULT_TENANT_ID`, which is exactly what a cross-tenant
/// matrix must not do).
fn tenant_auth_token_bearer(tenant_id: i64, user_id: &str) -> String {
    format!(
        "Bearer {}",
        sdkwork_web_core::auth_token_jwt(
            &tenant_id.to_string(),
            user_id,
            SESSION_ID,
            MEMORY_APP_ID,
        )
    )
}

fn tenant_access_token(tenant_id: i64, user_id: &str) -> String {
    sdkwork_web_core::encode_unsigned_test_jwt(json!({
        "token_type": "access",
        "tenant_id": tenant_id.to_string(),
        "user_id": user_id,
        "session_id": SESSION_ID,
        "app_id": MEMORY_APP_ID,
        "environment": "dev",
        "deployment_mode": "saas",
        "login_scope": "TENANT",
        "permission_scope": ["memory.*"]
    }))
}

fn authed_json_request(
    tenant_id: i64,
    user_id: &str,
    method: &str,
    uri: &str,
    body: Value,
) -> Request<Body> {
    let idempotency_key = memory_idempotency_key(method, uri, &body.to_string());
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("Authorization", tenant_auth_token_bearer(tenant_id, user_id))
        .header("Access-Token", tenant_access_token(tenant_id, user_id))
        .header("Idempotency-Key", idempotency_key)
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn authed_get(tenant_id: i64, user_id: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", tenant_auth_token_bearer(tenant_id, user_id))
        .header("Access-Token", tenant_access_token(tenant_id, user_id))
        .body(Body::empty())
        .unwrap()
}

/// Open-surface (api-key) request with the test request context injected the
/// same way `space_isolation_security_test.rs` does.
fn open_api_request(
    tenant_id: i64,
    actor_id: u64,
    method: &str,
    uri: &str,
    body: Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .extension(
            sdkwork_memory_contract::MemoryOpenApiRequestContext::for_open_surface(
                "key-1",
                tenant_id as u64,
                Some(actor_id),
            ),
        );
    if !body.is_null() {
        builder = builder.header(
            "Idempotency-Key",
            memory_idempotency_key(method, uri, &body.to_string()),
        );
        builder.body(Body::from(body.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap_or(Value::Null)
}

/// Both refusal statuses are isolation-correct; anything else leaks.
fn assert_tenant_refused(status: StatusCode, context: &str) {
    assert!(
        matches!(status, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN),
        "{context} must be refused with 404 or 403 across the tenant boundary, got {status}"
    );
}

fn items_of(envelope: &Value) -> Vec<Value> {
    api_envelope::items(envelope)
        .as_array()
        .expect("list envelope must carry data.items")
        .clone()
}

async fn seeded_two_tenant_store() -> sdkwork_memory_plugin_native_sql::NativeSqlMemoryStore {
    let store = space_fixtures::new_seeded_in_memory_store().await;
    // Tenant B gets its own space; ids are globally unique, so it must not
    // collide with A's seeded spaces 1 and 2.
    space_fixtures::seed_user_space(&store, TENANT_B, 3, TENANT_B_USER).await;
    store
}

#[tokio::test]
async fn tenant_b_cannot_read_mutate_or_delete_tenant_a_resources() {
    let _env = lock_integration_test_env().await;
    let store = seeded_two_tenant_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store.clone())),
    );
    let open = build_router_with_shared_open_api(Arc::new(OpenMemoryService::new(store.clone())));

    // ---- Tenant A creates one resource of every matrix kind. ----
    let create_memory = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "1",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "tenant A secret"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_memory.status(), StatusCode::CREATED, "A memory create");
    let memory_id = api_envelope::item(&response_json(create_memory).await)["memoryId"]
        .as_str()
        .unwrap()
        .to_string();

    let append_event = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/events",
            json!({
                "spaceId": "1",
                "eventType": "user.preference.stated",
                "sourceType": "chat",
                "eventTime": "2026-09-30T00:00:00Z",
                "payload": { "secret": "tenant A evidence" }
            }),
        ))
        .await
        .unwrap();
    assert_eq!(append_event.status(), StatusCode::CREATED, "A event append");
    let event_id = api_envelope::item(&response_json(append_event).await)["eventId"]
        .as_str()
        .unwrap()
        .to_string();

    MemoryCandidateStorePort::create(
        &store,
        CreateMemoryCandidateCommand {
            scope: MemoryScopeContext::for_test(TENANT_A, 1),
            candidate_id: "8101".to_string(),
            candidate_type: "observation".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "tenant A candidate".to_string(),
            proposed_payload_json: None,
            evidence_json: None,
            confidence: 0.8,
            learning_job_uuid: None,
        },
    )
    .await
    .expect("A candidate seed");

    let create_retrieval = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/retrievals",
            json!({
                "query": "tenant A secret",
                "spaceIds": ["1"],
                "topK": 5,
                "contextBudgetTokens": 512
            }),
        ))
        .await
        .unwrap();
    assert_eq!(
        create_retrieval.status(),
        StatusCode::CREATED,
        "A retrieval create"
    );
    let retrieval_id = api_envelope::item(&response_json(create_retrieval).await)["retrievalId"]
        .as_str()
        .unwrap()
        .to_string();

    let create_pack = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/context_packs",
            json!({
                "query": "tenant A secret",
                "spaceIds": ["1"],
                "contextBudgetTokens": 512
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_pack.status(), StatusCode::CREATED, "A pack create");
    let pack_id = api_envelope::item(&response_json(create_pack).await)["contextPackId"]
        .as_str()
        .unwrap()
        .to_string();

    let create_entity = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/entities",
            json!({
                "spaceId": "1",
                "entityType": "person",
                "canonicalName": "Tenant A Entity"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_entity.status(), StatusCode::CREATED, "A entity create");
    let entity_id = api_envelope::item(&response_json(create_entity).await)["entityId"]
        .as_str()
        .unwrap()
        .to_string();

    // ---- Tenant B attempts read / mutate / delete on every one of them. ----
    let forbidden_reads = [
        (
            "memory read",
            authed_get(
                TENANT_B,
                TENANT_B_USER,
                &format!("/app/v3/api/memory/memories/{memory_id}?space_id=3"),
            ),
        ),
        (
            "event read",
            authed_get(
                TENANT_B,
                TENANT_B_USER,
                &format!("/app/v3/api/memory/events/{event_id}?space_id=3"),
            ),
        ),
        (
            "candidate read",
            authed_get(
                TENANT_B,
                TENANT_B_USER,
                "/app/v3/api/memory/candidates/8101?space_id=3",
            ),
        ),
        (
            "retrieval read",
            authed_get(
                TENANT_B,
                TENANT_B_USER,
                &format!("/app/v3/api/memory/retrievals/{retrieval_id}"),
            ),
        ),
        (
            "context pack read",
            authed_get(
                TENANT_B,
                TENANT_B_USER,
                &format!("/app/v3/api/memory/context_packs/{pack_id}"),
            ),
        ),
        (
            "entity read",
            authed_get(
                TENANT_B,
                TENANT_B_USER,
                &format!("/app/v3/api/memory/entities/{entity_id}"),
            ),
        ),
    ];
    for (context, request) in forbidden_reads {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_tenant_refused(response.status(), context);
    }

    let forbidden_writes = [
        (
            "memory patch",
            authed_json_request(
                TENANT_B,
                TENANT_B_USER,
                "PATCH",
                &format!("/app/v3/api/memory/memories/{memory_id}?space_id=3"),
                json!({ "canonicalText": "tenant B overwrite" }),
            ),
        ),
        (
            "candidate approve",
            authed_json_request(
                TENANT_B,
                TENANT_B_USER,
                "POST",
                "/app/v3/api/memory/candidates/8101/approve",
                json!({ "reason": "cross-tenant approval attempt" }),
            ),
        ),
        (
            "entity patch",
            authed_json_request(
                TENANT_B,
                TENANT_B_USER,
                "PATCH",
                &format!("/app/v3/api/memory/entities/{entity_id}"),
                json!({ "canonicalName": "tenant B rename" }),
            ),
        ),
    ];
    for (context, request) in forbidden_writes {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_tenant_refused(response.status(), context);
    }

    let delete_memory = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/app/v3/api/memory/memories/{memory_id}?space_id=3"))
                .header("Authorization", tenant_auth_token_bearer(TENANT_B, TENANT_B_USER))
                .header("Access-Token", tenant_access_token(TENANT_B, TENANT_B_USER))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_tenant_refused(delete_memory.status(), "memory delete");

    // Entities have no owned open surface: the wire answers the named
    // not-implemented refusal, which cannot leak tenant A data either.
    let open_entity_read = open
        .clone()
        .oneshot(open_api_request(
            TENANT_B,
            2002,
            "GET",
            &format!("/mem/v3/api/memory/entities/{entity_id}"),
            Value::Null,
        ))
        .await
        .unwrap();
    assert!(
        matches!(
            open_entity_read.status(),
            StatusCode::NOT_IMPLEMENTED | StatusCode::NOT_FOUND | StatusCode::FORBIDDEN
        ),
        "open-surface entity read must be the named refusal or tenant-refused, got {}",
        open_entity_read.status()
    );

    // Forget scope must not cross tenants: B cannot erase A's memory by id,
    // and the memory survives.
    let cross_forget = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_B,
            TENANT_B_USER,
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "memory",
                "spaceId": "1",
                "memoryIds": [memory_id],
                "reason": "cross-tenant erase attempt"
            }),
        ))
        .await
        .unwrap();
    assert_tenant_refused(cross_forget.status(), "cross-tenant forget request");

    let memory_survives = app
        .clone()
        .oneshot(authed_get(
            TENANT_A,
            TENANT_A_USER,
            &format!("/app/v3/api/memory/memories/{memory_id}?space_id=1"),
        ))
        .await
        .unwrap();
    assert_eq!(
        memory_survives.status(),
        StatusCode::OK,
        "A's memory must survive B's refused erase"
    );

    // Export scope must not cross tenants either.
    let cross_export = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_B,
            TENANT_B_USER,
            "POST",
            "/app/v3/api/memory/export_jobs",
            json!({ "spaceIds": ["1"], "format": "json" }),
        ))
        .await
        .unwrap();
    assert_tenant_refused(cross_export.status(), "cross-tenant export request");

    // Positive control: B's own scope keeps working, so the refusals above are
    // tenant isolation and not a broken tenant-B credential chain.
    let own_export = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_B,
            TENANT_B_USER,
            "POST",
            "/app/v3/api/memory/export_jobs",
            json!({ "spaceIds": ["3"], "format": "json" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        own_export.status(),
        StatusCode::CREATED,
        "B must still export its own space"
    );
}

#[tokio::test]
async fn tenant_b_listings_and_a_cursor_token_never_leak_tenant_a_data() {
    let _env = lock_integration_test_env().await;
    let store = seeded_two_tenant_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store)),
    );

    let mut tenant_a_memory_ids = Vec::new();
    for ordinal in 0..2 {
        let created = app
            .clone()
            .oneshot(authed_json_request(
                TENANT_A,
                TENANT_A_USER,
                "POST",
                "/app/v3/api/memory/memories",
                json!({
                    "spaceId": "1",
                    "scope": "user",
                    "memoryType": "semantic",
                    "canonicalText": format!("tenant A leak probe {ordinal}")
                }),
            ))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        tenant_a_memory_ids.push(
            api_envelope::item(&response_json(created).await)["memoryId"]
                .as_str()
                .unwrap()
                .to_string(),
        );
    }

    let tenant_b_memory = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_B,
            TENANT_B_USER,
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "3",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "tenant B own memory"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(tenant_b_memory.status(), StatusCode::CREATED);

    // Plain B listing returns only B rows.
    let b_list = app
        .clone()
        .oneshot(authed_get(
            TENANT_B,
            TENANT_B_USER,
            "/app/v3/api/memory/memories?space_id=3",
        ))
        .await
        .unwrap();
    assert_eq!(b_list.status(), StatusCode::OK);
    let b_items = items_of(&response_json(b_list).await);
    assert!(
        !b_items.is_empty(),
        "positive control: B must see its own memory"
    );
    for item in &b_items {
        assert_eq!(
            item["spaceId"].as_str(),
            Some("3"),
            "B's listing leaked a foreign-space row: {item}"
        );
    }
    for leaked in &tenant_a_memory_ids {
        assert!(
            !b_items.iter().any(|item| item["memoryId"].as_str() == Some(leaked.as_str())),
            "B's listing leaked tenant A memory {leaked}"
        );
    }

    // A's cursor token replayed in B's same-shape listing: either rejected, or
    // answered with B-scoped rows only — never with A rows.
    let a_page = app
        .clone()
        .oneshot(authed_get(
            TENANT_A,
            TENANT_A_USER,
            "/app/v3/api/memory/memories?space_id=1&page_size=1",
        ))
        .await
        .unwrap();
    assert_eq!(a_page.status(), StatusCode::OK);
    let a_envelope = response_json(a_page).await;
    let a_cursor = api_envelope::page_info(&a_envelope)["nextCursor"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let replayed = app
        .clone()
        .oneshot(authed_get(
            TENANT_B,
            TENANT_B_USER,
            &format!("/app/v3/api/memory/memories?space_id=3&cursor={a_cursor}"),
        ))
        .await
        .unwrap();
    assert!(
        matches!(replayed.status(), StatusCode::OK | StatusCode::BAD_REQUEST),
        "a foreign cursor must be rejected or re-scoped, got {}",
        replayed.status()
    );
    if replayed.status() == StatusCode::OK {
        let replayed_items = items_of(&response_json(replayed).await);
        for leaked in &tenant_a_memory_ids {
            assert!(
                !replayed_items
                    .iter()
                    .any(|item| item["memoryId"].as_str() == Some(leaked.as_str())),
                "replayed tenant A cursor leaked tenant A memory {leaked} into B's page"
            );
        }
    }
}

#[tokio::test]
async fn tenant_b_cannot_reach_tenant_a_governance_jobs() {
    let _env = lock_integration_test_env().await;
    let store = seeded_two_tenant_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store)),
    );

    let create_memory = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "1",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "tenant A governance probe"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_memory.status(), StatusCode::CREATED);
    let memory_id = api_envelope::item(&response_json(create_memory).await)["memoryId"]
        .as_str()
        .unwrap()
        .to_string();

    let forget = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "memory",
                "spaceId": "1",
                "memoryIds": [memory_id],
                "reason": "A-scoped erase"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(forget.status(), StatusCode::CREATED);
    let forget_job_id = api_envelope::item(&response_json(forget).await)["forgetRequestId"]
        .as_str()
        .unwrap()
        .to_string();

    let export = app
        .clone()
        .oneshot(authed_json_request(
            TENANT_A,
            TENANT_A_USER,
            "POST",
            "/app/v3/api/memory/export_jobs",
            json!({ "spaceIds": ["1"], "format": "json" }),
        ))
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::CREATED);
    let export_job_id = api_envelope::item(&response_json(export).await)["exportJobId"]
        .as_str()
        .unwrap()
        .to_string();

    let read_forget = app
        .clone()
        .oneshot(authed_get(
            TENANT_B,
            TENANT_B_USER,
            &format!("/app/v3/api/memory/forget_requests/{forget_job_id}"),
        ))
        .await
        .unwrap();
    assert_tenant_refused(read_forget.status(), "forget job read");

    let read_export = app
        .clone()
        .oneshot(authed_get(
            TENANT_B,
            TENANT_B_USER,
            &format!("/app/v3/api/memory/export_jobs/{export_job_id}"),
        ))
        .await
        .unwrap();
    assert_tenant_refused(read_export.status(), "export job read");

    // B's own forget job listing must not contain A's job id.
    let b_jobs = app
        .oneshot(authed_get(
            TENANT_B,
            TENANT_B_USER,
            "/app/v3/api/memory/forget_requests",
        ))
        .await
        .unwrap();
    assert_eq!(b_jobs.status(), StatusCode::OK);
    let job_items = items_of(&response_json(b_jobs).await);
    assert!(
        !job_items
            .iter()
            .any(|item| item["forgetRequestId"].as_str() == Some(forget_job_id.as_str())),
        "B's forget listing leaked tenant A job {forget_job_id}"
    );
}
