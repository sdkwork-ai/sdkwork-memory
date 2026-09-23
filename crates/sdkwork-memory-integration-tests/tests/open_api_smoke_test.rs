use axum::body::Body;
use axum::http::{Request, StatusCode};
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::web_auth::{
    memory_access_token, memory_auth_token_bearer, memory_dev_api_key,
};
use sdkwork_routes_memory_open_api::{build_router_with_open_api, wrap_router_with_web_framework};
use sdkwork_web_core::DefaultWebRequestContextResolver;
use tower::util::ServiceExt;

async fn wrapped_open_api_router() -> axum::Router {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let business = build_router_with_open_api(OpenMemoryService::new(store));
    wrap_router_with_web_framework(DefaultWebRequestContextResolver::default(), business)
}

#[tokio::test]
async fn open_api_rejects_missing_api_key() {
    let app = wrapped_open_api_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mem/v3/api/memory/capabilities")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn open_api_rejects_credential_headers_on_api_key_surface() {
    // The authored route manifest declares this surface `auth: { mode: "api-key", required: true }`
    // (sdks/_route-manifests/open-api/sdkwork-routes-memory-open-api.route-manifest.json), so the
    // Web Framework fails closed when the credential profile is contaminated: an api-key surface
    // must not accept `Authorization` / `Access-Token`, otherwise a credential header issued for
    // another surface could be replayed against one that never validated it.
    let app = wrapped_open_api_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mem/v3/api/memory/capabilities")
                .header("Authorization", memory_auth_token_bearer("2001"))
                .header("Access-Token", memory_access_token("2001"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "api-key surfaces must fail closed on credential headers, got: {}",
        String::from_utf8_lossy(&body)
    );
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        payload["reason"],
        "credential-profile-contamination",
        "the rejection must name credential-profile contamination: {body:?}",
        body = String::from_utf8_lossy(&body)
    );
    assert_eq!(payload["failedStage"], "surface-classification");
}

#[tokio::test]
async fn open_api_accepts_api_key_before_handler() {
    let app = wrapped_open_api_router().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mem/v3/api/memory/capabilities")
                .header("X-API-Key", memory_dev_api_key("2001", "key-1"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
