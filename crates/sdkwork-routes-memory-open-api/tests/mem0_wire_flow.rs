//! End-to-end verification of the mem0 platform compatibility wire.
//!
//! Everything here goes through the **full web framework layer** with a **real
//! `OpenMemoryService`**, authenticated exactly the way the official clients
//! authenticate: `Authorization: Token <api_key>` and nothing else. That is the
//! point of the suite — the credential bridge, the surface classification, the
//! route-manifest auth profile, the problem-document rewrite, and the handlers
//! are only exercised together this way.
//!
//! No `X-Api-Key` header is ever sent, so a passing run proves the bridge is
//! what authenticates these requests rather than the credential happening to be
//! in the header the framework looks at.

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_test_support::web_auth::{lock_integration_test_env, memory_dev_api_key};
use sdkwork_routes_memory_open_api::{build_router_with_open_api, wrap_router_with_web_framework};
use sdkwork_web_core::DefaultWebRequestContextResolver;
use serde_json::{json, Value};
use tower::util::ServiceExt;

const CONVERSATION: &str = "The operator prefers concise bullet-point summaries over prose.";

/// The dev credential, wrapped in the scheme the official mem0 clients send.
fn mem0_authorization() -> String {
    format!("Token {}", memory_dev_api_key("2001", "mem0-compat-key"))
}

async fn build_app() -> Router {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    wrap_router_with_web_framework(
        DefaultWebRequestContextResolver::default(),
        build_router_with_open_api(OpenMemoryService::new(store)),
    )
}

struct Response {
    status: StatusCode,
    content_type: Option<String>,
    body: Option<Value>,
}

impl Response {
    /// mem0's clients read `detail` from a failure body only when the media type
    /// is `application/json` (`mem0/client/utils.py::_handle_http_error`). Being
    /// valid JSON is not enough; the declared type is part of the contract.
    fn assert_json_media_type(&self, context: &str) {
        let content_type = self
            .content_type
            .as_deref()
            .unwrap_or_else(|| panic!("{context}: response declared no content type"));
        assert!(
            content_type.starts_with("application/json"),
            "{context}: mem0 clients only parse `application/json`, got `{content_type}`"
        );
    }

    fn detail(&self) -> String {
        self.body
            .as_ref()
            .and_then(|body| body.get("detail"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("expected a `detail` field, got {:?}", self.body))
            .to_owned()
    }

    fn item(&self) -> &Value {
        self.body
            .as_ref()
            .expect("response has a body")
            .get("results")
            .and_then(|results| results.as_array())
            .and_then(|results| results.first())
            .unwrap_or_else(|| panic!("expected a non-empty `results` array, got {:?}", self.body))
    }
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    authorized: bool,
) -> Response {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(body) = body {
        // `content-length` is set explicitly because a real HTTP/1.1 client
        // always sets it (or `transfer-encoding`) for a JSON body, and the
        // framework reads it to decide whether a request *has* a body at all
        // (`content_length_from_headers`, `client_context_guard.rs`). An
        // in-process `oneshot` request built from `Body::from(..)` carries no
        // such header, so without this line the body-inspection stages are
        // skipped and the suite would pass against a transport no client uses.
        let text = body.to_string();
        builder = builder
            .header("content-type", "application/json")
            .header("content-length", text.len().to_string());
        let request = builder.body(Body::from(text)).expect("request builds");
        return finish(app, request, authorized).await;
    }
    let request = builder.body(Body::empty()).expect("request builds");
    finish(app, request, authorized).await
}

async fn finish(app: &Router, request: Request<Body>, authorized: bool) -> Response {
    let request = if authorized {
        let (mut parts, body) = request.into_parts();
        parts.headers.insert(
            "authorization",
            mem0_authorization()
                .parse()
                .expect("authorization header builds"),
        );
        Request::from_parts(parts, body)
    } else {
        request
    };
    let response = app.clone().oneshot(request).await.expect("router responds");
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body reads");
    let body = if bytes.is_empty() {
        None
    } else {
        // A compatibility failure must be JSON the official clients can parse;
        // `serde_json` failing here is itself a contract violation.
        Some(serde_json::from_slice(&bytes).unwrap_or_else(|error| {
            panic!(
                "mem0 wire responses must be JSON ({error}); body was {}",
                String::from_utf8_lossy(&bytes)
            )
        }))
    };
    Response {
        status,
        content_type,
        body,
    }
}

/// One pass over the ten operations the official clients drive.
#[tokio::test]
async fn mem0_platform_wire_serves_the_official_client_flow() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    // 1. `GET /v1/ping/` — both clients resolve their identity through this
    //    call and read `org_id`/`project_id`; a non-2xx makes them raise.
    let ping = send(&app, "GET", "/v1/ping/", None, true).await;
    assert_eq!(ping.status, StatusCode::OK, "ping: {:?}", ping.body);
    let ping_body = ping.body.as_ref().expect("ping body");
    assert_eq!(ping_body["org_id"], "100001");
    assert_eq!(ping_body["project_id"], "100001");
    assert!(ping_body["user_email"].is_null());
    // Pinned at the wire, not only through a client: the JavaScript client's
    // `ping()` throws unless this literal is exactly `"ok"` (its constructor
    // then leaves the identity unresolved), and a run that only ever drove the
    // Python client would never notice its absence.
    assert_eq!(
        ping_body["status"], "ok",
        "the JavaScript MemoryClient requires this literal: {:?}",
        ping.body
    );

    // 2. `POST /v3/memories/add/`
    let add = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({
            "messages": [
                { "role": "user", "content": CONVERSATION },
                { "role": "assistant", "content": "" }
            ],
            "user_id": "alice",
            "agent_id": "planner",
            "metadata": { "topic": "formatting" }
        })),
        true,
    )
    .await;
    assert_eq!(add.status, StatusCode::OK, "add: {:?}", add.body);
    let created = add.item().clone();
    let memory_id = created["id"]
        .as_str()
        .expect("add must report the memory id")
        .to_owned();
    assert_eq!(created["event"], "ADD");
    assert_eq!(created["memory"], CONVERSATION);
    assert_eq!(created["user_id"], "alice");
    assert_eq!(created["agent_id"], "planner");
    assert_eq!(created["metadata"]["topic"], "formatting");
    // Empty turns are not content and must not appear in the record text.
    assert!(
        !created["memory"].as_str().expect("memory text").contains("\n\n"),
        "blank message content must not contribute a blank line"
    );
    assert!(created["hash"].as_str().is_some_and(|hash| hash.len() == 64));

    // 3. `GET /v1/memories/{memory_id}/`
    let fetched = send(
        &app,
        "GET",
        &format!("/v1/memories/{memory_id}/"),
        None,
        true,
    )
    .await;
    assert_eq!(fetched.status, StatusCode::OK, "get: {:?}", fetched.body);
    assert_eq!(fetched.body.as_ref().expect("get body")["id"], memory_id);

    // 4. `POST /v3/memories/search/`
    let search = send(
        &app,
        "POST",
        "/v3/memories/search/",
        Some(json!({ "query": "bullet-point summaries", "top_k": 5 })),
        true,
    )
    .await;
    assert_eq!(search.status, StatusCode::OK, "search: {:?}", search.body);
    let results = search.body.as_ref().expect("search body")["results"]
        .as_array()
        .expect("search results array")
        .clone();
    assert!(
        results
            .iter()
            .any(|hit| hit["id"].as_str() == Some(memory_id.as_str())),
        "the stored memory must be retrievable by its own text: {results:?}"
    );

    // 5. `POST /v3/memories/` (get_all)
    let listed = send(&app, "POST", "/v3/memories/", Some(json!({})), true).await;
    assert_eq!(listed.status, StatusCode::OK, "list: {:?}", listed.body);
    let listed_body = listed.body.as_ref().expect("list body");
    assert_eq!(listed_body["count"], 1, "count: {:?}", listed_body);
    assert!(listed_body["next"].is_null());
    assert!(listed_body["previous"].is_null());
    assert_eq!(
        listed_body["results"]
            .as_array()
            .expect("list results")
            .len(),
        1
    );

    // 6. `PUT /v1/memories/{memory_id}/`
    let updated = send(
        &app,
        "PUT",
        &format!("/v1/memories/{memory_id}/"),
        Some(json!({ "text": "The operator prefers one-line answers." })),
        true,
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "update: {:?}", updated.body);
    assert_eq!(
        updated.body.as_ref().expect("update body")["memory"],
        "The operator prefers one-line answers."
    );

    // 7. `GET /v1/memories/{memory_id}/history/` — an event log, newest first.
    let history = send(
        &app,
        "GET",
        &format!("/v1/memories/{memory_id}/history/"),
        None,
        true,
    )
    .await;
    assert_eq!(history.status, StatusCode::OK, "history: {:?}", history.body);
    let entries = history.body.as_ref().expect("history body")
        .as_array()
        .expect("history is a JSON array")
        .clone();
    let events: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry["event"].as_str())
        .collect();
    assert!(
        events.contains(&"ADD") && events.contains(&"UPDATE"),
        "history must record the add and the update, got {events:?}"
    );
    assert!(
        events
            .iter()
            .all(|event| ["ADD", "UPDATE", "DELETE"].contains(event)),
        "mem0 declares `event` as a closed enum, got {events:?}"
    );
    for entry in &entries {
        assert_eq!(entry["memory_id"], memory_id);
        assert_eq!(entry["user_id"], "alice");
        // The canonical journal keeps no text, so content fields are absent
        // rather than fabricated. The official TS type allows null for both.
        assert!(entry["old_memory"].is_null());
        assert!(entry["new_memory"].is_null());
    }

    // 8. `GET /v1/entities/` — the scopes memories actually exist for.
    let entities = send(&app, "GET", "/v1/entities/", None, true).await;
    assert_eq!(
        entities.status,
        StatusCode::OK,
        "entities: {:?}",
        entities.body
    );
    let entities_body = entities.body.as_ref().expect("entities body");
    let scopes = entities_body["results"]
        .as_array()
        .expect("entities results array");
    assert_eq!(entities_body["count"], 2, "entities: {:?}", entities_body);
    assert!(scopes.iter().any(|entity| {
        entity["type"] == "user" && entity["name"] == "alice" && entity["owner"] == "100001"
    }));
    assert!(scopes
        .iter()
        .any(|entity| entity["type"] == "agent" && entity["name"] == "planner"));

    // 9. `DELETE /v1/memories/{memory_id}/`
    let deleted = send(
        &app,
        "DELETE",
        &format!("/v1/memories/{memory_id}/"),
        None,
        true,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "delete: {:?}", deleted.body);
    assert_eq!(
        deleted.body.as_ref().expect("delete body")["message"],
        "Memory deleted successfully"
    );
    let gone = send(
        &app,
        "GET",
        &format!("/v1/memories/{memory_id}/"),
        None,
        true,
    )
    .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND, "gone: {:?}", gone.body);
    gone.assert_json_media_type("deleted memory read");
    // This surface's own wording, not the framework's status-derived
    // "Not found": the message has to survive the framework's response
    // interceptor for the caller to be told what was missing.
    assert_eq!(gone.detail(), "memory not found");

    // The history of a deleted memory stays readable: verifying a deletion
    // through the audit trail is the primary client flow for this endpoint,
    // and upstream keeps serving it after a delete. Authorization moves to the
    // space (which survives), so the DELETE event is still on the wire.
    let deleted_history = send(
        &app,
        "GET",
        &format!("/v1/memories/{memory_id}/history/"),
        None,
        true,
    )
    .await;
    assert_eq!(
        deleted_history.status,
        StatusCode::OK,
        "history after delete: {:?}",
        deleted_history.body
    );
    deleted_history.assert_json_media_type("history after delete");
    let deleted_events: Vec<String> = deleted_history
        .body
        .as_ref()
        .expect("history body")
        .as_array()
        .expect("history is a JSON array")
        .iter()
        .filter_map(|entry| entry["event"].as_str().map(str::to_owned))
        .collect();
    assert!(
        deleted_events.iter().any(|event| event == "DELETE"),
        "the deleted memory's history must record the DELETE event, got {deleted_events:?}"
    );

    // 10. `DELETE /v1/memories/`
    let swept = send(&app, "DELETE", "/v1/memories/", None, true).await;
    assert_eq!(swept.status, StatusCode::OK, "sweep: {:?}", swept.body);
    assert!(swept
        .body
        .as_ref()
        .expect("sweep body")["message"]
        .as_str()
        .expect("sweep message")
        .ends_with("memories deleted successfully"));
}

/// A mem0 caller that forgot its key must get an answer it can read.
#[tokio::test]
async fn mem0_wire_reports_authentication_failures_in_its_own_dialect() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    let response = send(&app, "GET", "/v1/ping/", None, false).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    // Not `{"detail": ...}` from this crate — this failure is the framework's,
    // and the media-type bridge is what makes it readable. Both halves matter:
    // the type must be `application/json`, and the payload must still carry a
    // `detail` (the bridge re-types the framework's problem document, it does
    // not shred it).
    response.assert_json_media_type("unauthenticated ping");
    assert!(
        !response.detail().is_empty(),
        "the framework's own failure must carry a readable detail: {:?}",
        response.body
    );
}

/// Parameters this surface cannot honour are refused by name, never narrowed
/// silently. A caller must be able to tell "not supported" from "no results".
#[tokio::test]
async fn mem0_wire_refuses_untranslatable_parameters_by_name() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    let add = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({
            "messages": [{ "role": "user", "content": "Pinned memory" }],
            "user_id": "bob"
        })),
        true,
    )
    .await;
    assert_eq!(add.status, StatusCode::OK, "add: {:?}", add.body);
    let memory_id = add.item()["id"].as_str().expect("id").to_owned();

    // Two kinds of refusal, kept distinct on purpose:
    //  * a value outside the range this surface accepts is a 400 — the caller's
    //    parameter is wrong;
    //  * a capability this surface does not have at all is a 501 — the caller's
    //    request is fine, the translation cannot honour it.
    // What both share, and what the test pins, is that the failure names the
    // thing it refused instead of being answered with a narrower result.
    let cases: Vec<(&str, String, Option<Value>, StatusCode, &str)> = vec![
        (
            "POST",
            "/v3/memories/search/".to_owned(),
            Some(json!({ "query": "pinned", "top_k": 5000 })),
            StatusCode::BAD_REQUEST,
            "top_k",
        ),
        (
            "POST",
            "/v3/memories/?page=2".to_owned(),
            Some(json!({})),
            StatusCode::NOT_IMPLEMENTED,
            "page-based pagination",
        ),
        (
            "POST",
            "/v3/memories/".to_owned(),
            Some(json!({ "filters": { "user_id": "bob" } })),
            StatusCode::NOT_IMPLEMENTED,
            "filtered listing",
        ),
        (
            "PUT",
            format!("/v1/memories/{memory_id}/"),
            Some(json!({ "text": "x", "timestamp": "2026-01-01T00:00:00Z" })),
            StatusCode::NOT_IMPLEMENTED,
            "timestamp update",
        ),
        (
            "DELETE",
            format!("/v1/memories/{memory_id}/?delete_linked=true"),
            None,
            StatusCode::NOT_IMPLEMENTED,
            "delete_linked",
        ),
        (
            "DELETE",
            "/v1/memories/?user_id=bob".to_owned(),
            None,
            StatusCode::NOT_IMPLEMENTED,
            "filtered delete_all",
        ),
        // The same filter, in the spelling the Python client actually produces:
        // `delete_all(filters={...})` reaches the wire as one `filters` parameter
        // holding a `str()` of the dict. Reading only the four top-level names let
        // this call delete the entire space and answer 200 — the refusal has to
        // cover every spelling, or the guard it implements does not exist.
        (
            "DELETE",
            "/v1/memories/?filters=%7B%27user_id%27%3A+%27bob%27%7D".to_owned(),
            None,
            StatusCode::NOT_IMPLEMENTED,
            "filtered delete_all",
        ),
        // `metadata` narrows the deletion just as much as a `filters` dict, so
        // it gets the same named refusal instead of being dropped.
        (
            "DELETE",
            "/v1/memories/?metadata=%7B%27topic%27%3A+%27batch%27%7D".to_owned(),
            None,
            StatusCode::NOT_IMPLEMENTED,
            "filtered delete_all",
        ),
        // --- the add surface's remaining upstream switches -------------------
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "immutable": true
            })),
            StatusCode::NOT_IMPLEMENTED,
            "immutable",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "includes": ["names"]
            })),
            StatusCode::NOT_IMPLEMENTED,
            "includes",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "excludes": ["small talk"]
            })),
            StatusCode::NOT_IMPLEMENTED,
            "excludes",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "enable_graph": true
            })),
            StatusCode::NOT_IMPLEMENTED,
            "enable_graph",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "output_format": "v1.1"
            })),
            StatusCode::NOT_IMPLEMENTED,
            "output_format",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "prompt_profile_id": "profile-1"
            })),
            StatusCode::NOT_IMPLEMENTED,
            "prompt_profile_id",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "temporal_reasoning": true
            })),
            StatusCode::NOT_IMPLEMENTED,
            "temporal_reasoning",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "timezone": "UTC"
            })),
            StatusCode::NOT_IMPLEMENTED,
            "timezone",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "observation_datetime": "2026-01-01T00:00:00Z"
            })),
            StatusCode::NOT_IMPLEMENTED,
            "observation_datetime",
        ),
        (
            "POST",
            "/v3/memories/add/".to_owned(),
            Some(json!({
                "messages": [{ "role": "user", "content": "Pinned memory" }],
                "observation_date": "2026-01-01"
            })),
            StatusCode::NOT_IMPLEMENTED,
            "observation_date",
        ),
        // --- the listing's projection and keyword narrowings -----------------
        (
            "POST",
            "/v3/memories/".to_owned(),
            Some(json!({ "fields": ["id", "memory"] })),
            StatusCode::NOT_IMPLEMENTED,
            "fields",
        ),
        (
            "POST",
            "/v3/memories/".to_owned(),
            Some(json!({ "keywords": ["pinned"] })),
            StatusCode::NOT_IMPLEMENTED,
            "keywords",
        ),
    ];

    for (method, uri, body, status, expected) in cases {
        let response = send(&app, method, &uri, body, true).await;
        assert_eq!(
            response.status, status,
            "{method} {uri} must be refused, got {:?}",
            response.body
        );
        response.assert_json_media_type(&format!("{method} {uri} refusal"));
        assert!(
            response.detail().contains(expected),
            "{method} {uri} refusal must name `{expected}`, got `{}`",
            response.detail()
        );
    }
}

/// `GET /v1/memories/{memory_id}/history/` refuses a journal it can only serve
/// as a prefix.
///
/// The compatibility history is a single bounded page — the platform's maximum
/// list page size (200) — and a journal that fills it means the page could be
/// a prefix rather than the log. mem0's `history` is defined as the memory's
/// *whole* history, so the endpoint refuses by name and points at the canonical
/// events API, exactly like a truncated entity listing, instead of answering
/// with a subset that is indistinguishable from a complete one.
#[tokio::test]
async fn mem0_wire_refuses_a_truncated_history_instead_of_serving_a_prefix() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    let add = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({
            "messages": [{ "role": "user", "content": "History beyond one page" }],
            "user_id": "alice"
        })),
        true,
    )
    .await;
    assert_eq!(add.status, StatusCode::OK, "add: {:?}", add.body);
    let memory_id = add.item()["id"].as_str().expect("id").to_owned();

    // The creation is journal entry one and each accepted update adds one
    // more; 199 updates bring the journal to the 200-entry history bound, at
    // which point the read cannot look past a full page and must refuse.
    for index in 0..199 {
        let updated = send(
            &app,
            "PUT",
            &format!("/v1/memories/{memory_id}/"),
            Some(json!({ "text": format!("Rewrite {index}") })),
            true,
        )
        .await;
        assert_eq!(
            updated.status,
            StatusCode::OK,
            "update {index}: {:?}",
            updated.body
        );
    }

    let history = send(
        &app,
        "GET",
        &format!("/v1/memories/{memory_id}/history/"),
        None,
        true,
    )
    .await;
    assert_eq!(
        history.status,
        StatusCode::NOT_IMPLEMENTED,
        "history: {:?}",
        history.body
    );
    history.assert_json_media_type("truncated history");
    let detail = history.detail();
    assert!(detail.contains("history"), "{detail}");
    assert!(detail.contains("whole event log"), "{detail}");
    assert!(
        detail.contains("/mem/v3/api/memory/events"),
        "the refusal must point at the canonical events API: {detail}"
    );

    // A journal under the bound stays answerable, unchanged.
    let other = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({
            "messages": [{ "role": "user", "content": "History within one page" }],
            "user_id": "alice"
        })),
        true,
    )
    .await;
    assert_eq!(other.status, StatusCode::OK, "add: {:?}", other.body);
    let other_id = other.item()["id"].as_str().expect("id").to_owned();
    let short = send(
        &app,
        "GET",
        &format!("/v1/memories/{other_id}/history/"),
        None,
        true,
    )
    .await;
    assert_eq!(short.status, StatusCode::OK, "history: {:?}", short.body);
    assert_eq!(
        short.body.as_ref().expect("history body")
            .as_array()
            .expect("history is a JSON array")
            .len(),
        1
    );
}

/// Malformed input is answered in the mem0 dialect too, not as a problem
/// document: the official clients read `detail` from every failure body.
#[tokio::test]
async fn mem0_wire_reports_malformed_bodies_in_its_own_dialect() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    let response = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({ "messages": [] })),
        true,
    )
    .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    response.assert_json_media_type("empty messages");
    assert!(
        response.detail().contains("at least one entry"),
        "detail was `{}`",
        response.detail()
    );

    let response = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({ "messages": "not-a-list" })),
        true,
    )
    .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    response.assert_json_media_type("messages of the wrong type");
    assert!(
        response.detail().contains("well-formed JSON"),
        "detail was `{}`",
        response.detail()
    );
}

/// The same memory must not be reachable through another principal's space.
#[tokio::test]
async fn mem0_wire_scopes_memories_to_the_authenticated_principal() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    let add = send(
        &app,
        "POST",
        "/v3/memories/add/",
        Some(json!({
            "messages": [{ "role": "user", "content": "Owned by the first principal" }],
            "user_id": "alice"
        })),
        true,
    )
    .await;
    assert_eq!(add.status, StatusCode::OK, "add: {:?}", add.body);
    let memory_id = add.item()["id"].as_str().expect("id").to_owned();

    let mut builder = Request::builder()
        .method("GET")
        .uri(format!("/v1/memories/{memory_id}/"));
    let other = format!("Token {}", memory_dev_api_key("9001", "other-key"));
    builder = builder.header("authorization", other);
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).expect("request builds"))
        .await
        .expect("router responds");
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a second principal must not be able to read the first principal's memory"
    );
}

/// `POST /v1/feedback/` and `PUT`/`DELETE /v1/batch/`.
///
/// The batch half exists because upstream's 200 carries **only** a count message
/// — there is no per-item result channel. Every entry's shape is therefore
/// validated before anything runs, the service layer resolves existence for the
/// whole batch in one precheck read, and a failing entry is answered with *its*
/// error naming the id exactly as the caller sent it — a caller never mistakes
/// a partially applied batch for a complete one. Each entry commits atomically
/// with its own journal, so the entries before a failure stay applied and the
/// error message says exactly how many did.
#[tokio::test]
async fn mem0_wire_serves_feedback_and_batches() {
    let _env = lock_integration_test_env().await;
    let app = build_app().await;

    let mut ids = Vec::new();
    for text in ["First batch subject", "Second batch subject"] {
        let add = send(
            &app,
            "POST",
            "/v3/memories/add/",
            Some(json!({
                "messages": [{ "role": "user", "content": text }],
                "user_id": "alice"
            })),
            true,
        )
        .await;
        assert_eq!(add.status, StatusCode::OK, "add: {:?}", add.body);
        ids.push(add.item()["id"].as_str().expect("id").to_owned());
    }
    let (first, second) = (ids[0].clone(), ids[1].clone());

    // 1. `POST /v1/feedback/`
    let feedback = send(
        &app,
        "POST",
        "/v1/feedback/",
        Some(json!({
            "memory_id": first,
            "feedback": "POSITIVE",
            "feedback_reason": "matched what the operator asked for"
        })),
        true,
    )
    .await;
    assert_eq!(feedback.status, StatusCode::OK, "feedback: {:?}", feedback.body);
    feedback.assert_json_media_type("feedback");
    let body = feedback.body.as_ref().expect("feedback body");
    assert!(
        body["id"].as_str().is_some_and(|id| !id.is_empty()),
        "feedback must report an id: {body:?}"
    );
    assert_eq!(body["feedback"], "POSITIVE");
    // The canonical write takes a comment but its record does not read it back,
    // so there is nothing to report. Echoing the request would claim a round
    // trip this surface cannot prove.
    assert!(
        body["feedback_reason"].is_null(),
        "feedback_reason must be reported null, not echoed: {body:?}"
    );

    // 2. `PUT /v1/batch/`
    let updated = send(
        &app,
        "PUT",
        "/v1/batch/",
        Some(json!({
            "memories": [
                { "memory_id": first, "text": "First, rewritten" },
                { "memory_id": second, "text": "Second, rewritten", "metadata": { "topic": "batch" } }
            ]
        })),
        true,
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK, "batch update: {:?}", updated.body);
    updated.assert_json_media_type("batch update");
    assert_eq!(
        updated.body.as_ref().expect("ack")["message"],
        "Successfully updated 2 memories"
    );
    for (id, expected) in [(&first, "First, rewritten"), (&second, "Second, rewritten")] {
        let fetched = send(&app, "GET", &format!("/v1/memories/{id}/"), None, true).await;
        assert_eq!(fetched.status, StatusCode::OK, "get: {:?}", fetched.body);
        assert_eq!(
            fetched.body.as_ref().expect("get body")["memory"],
            expected
        );
    }

    // 3. Duplicate ids collapse. Without this a repeated delete would fail
    //    *after* the first occurrence had already removed the record — a
    //    half-applied batch reported as an error, for no caller mistake.
    let deduped = send(
        &app,
        "PUT",
        "/v1/batch/",
        Some(json!({
            "memories": [
                { "memory_id": first, "text": "Applied once" },
                { "memory_id": first, "text": "Applied twice" }
            ]
        })),
        true,
    )
    .await;
    assert_eq!(deduped.status, StatusCode::OK, "deduped: {:?}", deduped.body);
    assert_eq!(
        deduped.body.as_ref().expect("ack")["message"],
        "Successfully updated 1 memories"
    );

    // 4. An entry the store cannot resolve fails the batch with its own error:
    //    404, naming the id exactly as the caller sent it. The resolvable
    //    entry *is* written — the batched precheck read and the writes are one
    //    service call and each entry commits atomically — and the failure
    //    message says so instead of pretending nothing happened.
    let unknown = "999999999999999999";
    let aborted = send(
        &app,
        "PUT",
        "/v1/batch/",
        Some(json!({
            "memories": [
                { "memory_id": second, "text": "Must not be written" },
                { "memory_id": unknown, "text": "names nothing" }
            ]
        })),
        true,
    )
    .await;
    assert_eq!(aborted.status, StatusCode::NOT_FOUND, "aborted: {:?}", aborted.body);
    aborted.assert_json_media_type("aborted batch");
    assert!(
        aborted.detail().contains(unknown),
        "the refusal must name the entry that could not be resolved: {}",
        aborted.detail()
    );
    let applied = send(&app, "GET", &format!("/v1/memories/{second}/"), None, true).await;
    assert_eq!(
        applied.body.as_ref().expect("get body")["memory"],
        "Must not be written",
        "the resolvable entry is applied: the batch is per-item atomic, not all-or-nothing"
    );

    // 5. An update entry with neither payload is refused before anything runs.
    let empty_patch = send(
        &app,
        "PUT",
        "/v1/batch/",
        Some(json!({ "memories": [{ "memory_id": first }] })),
        true,
    )
    .await;
    assert_eq!(empty_patch.status, StatusCode::BAD_REQUEST, "{:?}", empty_patch.body);
    empty_patch.assert_json_media_type("empty patch");
    assert!(empty_patch.detail().contains(&first));

    // 6. Refusals specific to feedback: a withdrawal the canonical record
    //    cannot express, and a value outside the closed enum.
    let withdrawal = send(
        &app,
        "POST",
        "/v1/feedback/",
        Some(json!({ "memory_id": first })),
        true,
    )
    .await;
    assert_eq!(withdrawal.status, StatusCode::NOT_IMPLEMENTED, "{:?}", withdrawal.body);
    withdrawal.assert_json_media_type("feedback withdrawal");
    assert!(withdrawal.detail().contains("feedback withdrawal"));

    let bad_value = send(
        &app,
        "POST",
        "/v1/feedback/",
        Some(json!({ "memory_id": first, "feedback": "MAYBE" })),
        true,
    )
    .await;
    assert_eq!(bad_value.status, StatusCode::BAD_REQUEST, "{:?}", bad_value.body);
    assert!(bad_value.detail().contains("VERY_NEGATIVE"));

    // 7. `DELETE /v1/batch/`
    let deleted = send(
        &app,
        "DELETE",
        "/v1/batch/",
        Some(json!({ "memories": [{ "memory_id": first }, { "memory_id": second }] })),
        true,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "batch delete: {:?}", deleted.body);
    deleted.assert_json_media_type("batch delete");
    assert_eq!(
        deleted.body.as_ref().expect("ack")["message"],
        "Successfully deleted 2 memories"
    );
    for id in [&first, &second] {
        let gone = send(&app, "GET", &format!("/v1/memories/{id}/"), None, true).await;
        assert_eq!(gone.status, StatusCode::NOT_FOUND, "{id} must be gone");
    }
}
