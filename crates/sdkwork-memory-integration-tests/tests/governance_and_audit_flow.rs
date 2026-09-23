use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::api_envelope;
use sdkwork_memory_test_support::web_auth::{
    lock_integration_test_env, memory_access_token, memory_auth_token_bearer,
    memory_idempotency_key,
};
use sdkwork_routes_memory_app_api::{
    build_router_with_app_api, wrap_router_with_iam_database_web_framework,
};
use sdkwork_routes_memory_backend_api::{
    build_router_with_open_memory_service as build_backend_router_with_product,
    wrap_router_with_iam_database_web_framework as wrap_backend_router,
};
use sdkwork_routes_memory_open_api::build_router_with_open_memory_service as build_open_router_with_product;
use serde_json::json;
use std::sync::Arc;
use tower::util::ServiceExt;

fn authed_get(user_id: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("Authorization", memory_auth_token_bearer(user_id))
        .header("Access-Token", memory_access_token(user_id))
        .body(Body::empty())
        .unwrap()
}

fn authed_json_request(
    user_id: &str,
    method: &str,
    uri: &str,
    body: serde_json::Value,
) -> Request<Body> {
    let idempotency_key = memory_idempotency_key(method, uri, &body.to_string());
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("Authorization", memory_auth_token_bearer(user_id))
        .header("Access-Token", memory_access_token(user_id))
        .header("Idempotency-Key", idempotency_key)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[tokio::test]
async fn app_api_forget_and_export_jobs_round_trip_via_dual_token() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store)),
    );

    let create_memory = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "2",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "temporary preference"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_memory.status(), StatusCode::CREATED);
    let memory_body = to_bytes(create_memory.into_body(), usize::MAX)
        .await
        .unwrap();
    let memory_json: serde_json::Value = serde_json::from_slice(&memory_body).unwrap();
    let memory_item = api_envelope::item(&memory_json);
    let memory_id = memory_item["memoryId"].as_str().unwrap();

    let forget = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "memory",
                "spaceId": "2",
                "memoryIds": [memory_id],
                "reason": "user requested deletion"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(forget.status(), StatusCode::CREATED);
    let forget_body = to_bytes(forget.into_body(), usize::MAX).await.unwrap();
    let forget_json: serde_json::Value = serde_json::from_slice(&forget_body).unwrap();
    let forget_item = api_envelope::item(&forget_json);
    assert_eq!(forget_item["state"], "succeeded");
    assert_eq!(forget_item["result"]["deletedCount"], 1);
    let forget_request_id = forget_item["forgetRequestId"].as_str().unwrap();

    let deleted_memory = app
        .clone()
        .oneshot(authed_get(
            "2001",
            &format!("/app/v3/api/memory/memories/{memory_id}?space_id=2"),
        ))
        .await
        .unwrap();
    assert_eq!(deleted_memory.status(), StatusCode::NOT_FOUND);

    let retrieve_forget = app
        .clone()
        .oneshot(authed_get(
            "2001",
            &format!("/app/v3/api/memory/forget_requests/{forget_request_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(retrieve_forget.status(), StatusCode::OK);

    let export = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/export_jobs",
            json!({
                "spaceIds": ["2"],
                "format": "json",
                "includeEvents": true
            }),
        ))
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::CREATED);
    let export_body = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let export_json: serde_json::Value = serde_json::from_slice(&export_body).unwrap();
    let export_item = api_envelope::item(&export_json);
    assert_eq!(export_item["state"], "succeeded");
    assert!(export_item["result"]["exportPayload"].is_object());
    let export_job_id = export_item["exportJobId"].as_str().unwrap();

    let retrieve_export = app
        .oneshot(authed_get(
            "2001",
            &format!("/app/v3/api/memory/export_jobs/{export_job_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(retrieve_export.status(), StatusCode::OK);
    let stored_export_body = to_bytes(retrieve_export.into_body(), usize::MAX)
        .await
        .unwrap();
    let stored_export_json: serde_json::Value =
        serde_json::from_slice(&stored_export_body).unwrap();
    let stored_export_item = api_envelope::item(&stored_export_json);
    assert!(stored_export_item["result"]["exportRef"].is_string());
    assert!(stored_export_item["result"].get("exportPayload").is_none());
}

#[tokio::test]
async fn app_api_drive_export_job_uploads_through_drive() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(
            sdkwork_memory_test_support::drive_export::open_memory_service_with_drive(
                store.clone(),
            ),
        ),
    );

    let create_memory = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "2",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "drive export preference"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_memory.status(), StatusCode::CREATED);

    let export = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/export_jobs",
            json!({
                "spaceIds": ["2"],
                "format": "json",
                "driveTargetRef": "drive://app-upload/sdkwork-memory/exports"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::CREATED);
    let export_body = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let export_json: serde_json::Value = serde_json::from_slice(&export_body).unwrap();
    let export_item = api_envelope::item(&export_json);
    assert_eq!(export_item["state"], "completed");
    assert!(export_item["driveObjectRef"]
        .as_str()
        .unwrap()
        .starts_with("drive://nodes/"));
    assert!(export_item["result"].get("exportPayload").is_none());
    let export_job_id = export_item["exportJobId"].as_str().unwrap();

    let pending = store
        .list_pending_outbox_events(
            &sdkwork_memory_spi::MemoryScopeContext::for_test(100_001, 1),
            10,
        )
        .await
        .unwrap();
    assert!(
        pending
            .iter()
            .any(|event| event.event_type == "memory.export.drive_upload_completed"),
        "drive export must emit completed domain outbox event"
    );
    assert!(
        !pending
            .iter()
            .any(|event| event.event_type == "memory.export.drive_upload_requested"),
        "drive export must not emit legacy requested outbox event"
    );
    let _ = export_job_id;
}

#[tokio::test]
async fn backend_api_lists_audit_logs_after_open_api_feedback() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let service = Arc::new(OpenMemoryService::new(store));
    let open_app = build_open_router_with_product(service.clone());
    let backend_app = wrap_backend_router(
        IamWebRequestContextResolver::new(None),
        build_backend_router_with_product(service.clone()),
    );

    let create_memory = open_app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mem/v3/api/memory/memories")
                .header("content-type", "application/json")
                .extension(
                    sdkwork_memory_contract::MemoryOpenApiRequestContext::for_open_surface(
                        "key-1",
                        100_001,
                        Some(2001),
                    ),
                )
                .body(Body::from(
                    json!({
                        "spaceId": "2",
                        "scope": "user",
                        "memoryType": "semantic",
                        "canonicalText": "feedback target memory"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_memory.status(), StatusCode::CREATED);
    let memory_body = to_bytes(create_memory.into_body(), usize::MAX)
        .await
        .unwrap();
    let memory_json: serde_json::Value = serde_json::from_slice(&memory_body).unwrap();
    let memory_item = api_envelope::item(&memory_json);
    let memory_id = memory_item["memoryId"].as_str().unwrap();

    let feedback = open_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mem/v3/api/memory/feedback")
                .header("content-type", "application/json")
                .extension(
                    sdkwork_memory_contract::MemoryOpenApiRequestContext::for_open_surface(
                        "key-1",
                        100_001,
                        Some(2001),
                    ),
                )
                .body(Body::from(
                    json!({
                        "targetType": "memory",
                        "targetId": memory_id,
                        "feedbackType": "helpful"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(feedback.status(), StatusCode::CREATED);

    let audits = backend_app
        .oneshot(authed_get(
            "9001",
            "/backend/v3/api/memory/audit_logs?action=feedback.create",
        ))
        .await
        .unwrap();
    assert_eq!(audits.status(), StatusCode::OK);
    let audits_body = to_bytes(audits.into_body(), usize::MAX).await.unwrap();
    let audits_json: serde_json::Value = serde_json::from_slice(&audits_body).unwrap();
    assert!(api_envelope::items(&audits_json)
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["action"] == "feedback.create"));
}

#[tokio::test]
async fn app_api_rejects_foreign_actor_retrieving_forget_job() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store)),
    );

    let create_memory = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "2",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "forget idor target"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(create_memory.status(), StatusCode::CREATED);
    let memory_body = to_bytes(create_memory.into_body(), usize::MAX)
        .await
        .unwrap();
    let memory_json: serde_json::Value = serde_json::from_slice(&memory_body).unwrap();
    let memory_item = api_envelope::item(&memory_json);
    let memory_id = memory_item["memoryId"].as_str().unwrap();

    let forget = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "memory",
                "spaceId": "2",
                "memoryIds": [memory_id],
                "reason": "user requested deletion"
            }),
        ))
        .await
        .unwrap();
    assert_eq!(forget.status(), StatusCode::CREATED);
    let forget_body = to_bytes(forget.into_body(), usize::MAX).await.unwrap();
    let forget_json: serde_json::Value = serde_json::from_slice(&forget_body).unwrap();
    let forget_request_id = api_envelope::item(&forget_json)["forgetRequestId"]
        .as_str()
        .unwrap();

    let foreign_retrieve = app
        .oneshot(authed_get(
            "3002",
            &format!("/app/v3/api/memory/forget_requests/{forget_request_id}"),
        ))
        .await
        .unwrap();
    assert_eq!(foreign_retrieve.status(), StatusCode::FORBIDDEN);
}

/// The app-api contract declares `MemoryForgetRequest.memoryIds` with
/// `maxItems`, so the server must reject a list past that ceiling with the
/// standard validation problem instead of running an unbounded N+1 write path.
#[tokio::test]
async fn app_api_rejects_forget_memory_id_lists_beyond_the_contract_bound() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store)),
    );

    let bound = sdkwork_intelligence_memory_service::platform::MAX_FORGET_MEMORY_IDS;
    let ids: Vec<String> = (1..=bound + 1).map(|id| id.to_string()).collect();
    assert_eq!(ids.len(), bound + 1);

    let response = app
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "memory",
                "spaceId": "2",
                "memoryIds": ids,
                "reason": "bound probe"
            }),
        ))
        .await
        .expect("over-bound forget response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/problem+json"),
    );
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("bounded problem body");
    let problem: serde_json::Value = serde_json::from_slice(&body).expect("problem json");
    // 40001 is the service-level validation code; 40003 is reserved for the
    // invalid-parameter path, which is why the export and retrieval ceilings use
    // the same 40001 shape as this one.
    assert_eq!(problem["code"], 40001);
    let detail = problem["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains(&format!("must not exceed {bound}")),
        "diagnostic must name the ceiling, got {detail:?}"
    );
}

/// `ai_event` rows are memory inputs linked to `ai_space` only, never to
/// `ai_record`, so a targeted `scope: "memory"` forget reports purgedEvents 0 by
/// construction while `scope: "space"` purges the events that fed the space.
/// This pins both halves: the targeted forget must not purge events, and the
/// space forget must still be able to account for them afterwards.
#[tokio::test]
async fn app_api_targeted_forget_reports_zero_purged_events_while_space_scope_purges_them() {
    let _env = lock_integration_test_env().await;
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = wrap_router_with_iam_database_web_framework(
        IamWebRequestContextResolver::new(None),
        build_router_with_app_api(OpenMemoryService::new(store)),
    );

    for index in 0..2 {
        let appended = app
            .clone()
            .oneshot(authed_json_request(
                "2001",
                "POST",
                "/app/v3/api/memory/events",
                json!({
                    "spaceId": "2",
                    "eventType": "user.preference.stated",
                    "sourceType": "chat",
                    "eventTime": "2026-09-23T00:00:00Z",
                    "payload": { "ordinal": index }
                }),
            ))
            .await
            .expect("append event response");
        assert_eq!(appended.status(), StatusCode::CREATED, "event {index}");
    }

    let created = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/memories",
            json!({
                "spaceId": "2",
                "scope": "user",
                "memoryType": "semantic",
                "canonicalText": "a preference that does not cover the space"
            }),
        ))
        .await
        .expect("create memory response");
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
    let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
    let memory_id = api_envelope::item(&created_json)["memoryId"]
        .as_str()
        .expect("memory id")
        .to_owned();

    let targeted = app
        .clone()
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "memory",
                "spaceId": "2",
                "memoryIds": [memory_id],
                "reason": "targeted forget must not purge shared inputs"
            }),
        ))
        .await
        .expect("targeted forget response");
    assert_eq!(targeted.status(), StatusCode::CREATED);
    let targeted_body = to_bytes(targeted.into_body(), usize::MAX).await.unwrap();
    let targeted_json: serde_json::Value = serde_json::from_slice(&targeted_body).unwrap();
    let targeted_item = api_envelope::item(&targeted_json);
    assert_eq!(targeted_item["state"], "succeeded");
    assert_eq!(targeted_item["result"]["deletedCount"], 1);
    assert_eq!(
        targeted_item["result"]["purgedEvents"], 0,
        "a targeted record forget must not purge ai_event inputs shared with other records"
    );

    let space_wide = app
        .oneshot(authed_json_request(
            "2001",
            "POST",
            "/app/v3/api/memory/forget_requests",
            json!({
                "scope": "space",
                "spaceId": "2",
                "reason": "space-wide forget must purge the event inputs"
            }),
        ))
        .await
        .expect("space forget response");
    assert_eq!(space_wide.status(), StatusCode::CREATED);
    let space_body = to_bytes(space_wide.into_body(), usize::MAX).await.unwrap();
    let space_json: serde_json::Value = serde_json::from_slice(&space_body).unwrap();
    let space_item = api_envelope::item(&space_json);
    assert_eq!(space_item["state"], "succeeded");
    assert!(
        space_item["result"]["purgedEvents"]
            .as_u64()
            .is_some_and(|purged| purged >= 2),
        "the two appended events must still be present for the space-scope forget to purge, got {:?}",
        space_item["result"]["purgedEvents"]
    );
}
