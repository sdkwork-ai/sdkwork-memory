//! Cross-space isolation regression tests for memory CRUD.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::api_envelope;
use sdkwork_routes_memory_open_api::build_router_with_open_api;
use serde_json::json;
use tower::util::ServiceExt;

#[tokio::test]
async fn open_api_rejects_memory_retrieve_when_space_id_does_not_match_record() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = build_router_with_open_api(OpenMemoryService::new(store));
    let context = sdkwork_memory_contract::MemoryOpenApiRequestContext::for_backend_surface(
        100_001,
        Some(9001),
    );

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mem/v3/api/memory/memories")
                .header("content-type", "application/json")
                .extension(context.clone())
                .body(Body::from(
                    json!({
                        "spaceId": "1",
                        "scope": "user",
                        "memoryType": "semantic",
                        "canonicalText": "space one memory"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
    let memory_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let memory_id = api_envelope::item(&memory_json)["memoryId"]
        .as_str()
        .unwrap();

    let wrong_space = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/mem/v3/api/memory/memories/{memory_id}?space_id=2"
                ))
                .extension(context)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_space.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn open_api_list_memories_requires_space_id_query_parameter() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let app = build_router_with_open_api(OpenMemoryService::new(store));

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mem/v3/api/memory/memories")
                .extension(
                    sdkwork_memory_contract::MemoryOpenApiRequestContext::for_open_surface(
                        "key-1",
                        100_001,
                        Some(2001),
                    ),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
