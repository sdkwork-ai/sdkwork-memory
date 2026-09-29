//! mem0 platform compatibility wire.
//!
//! `sdkwork-specs/API_SPEC.md` section 4.5.2 sanctions `open-api` operations
//! that mirror an upstream third-party wire verbatim. This module is the
//! `mem0-platform` protocol surface: the REST dialect the official `mem0ai`
//! Python and TypeScript clients speak, declared in the generated OpenAPI
//! authority and route manifest under
//! `x-sdkwork-external-protocol-id: mem0-platform`.
//!
//! Three things live here and nowhere else:
//!
//! 1. **The wire shapes** ([`dto`]) — mem0 JSON in, mem0 JSON out, matching
//!    `external/mem0/docs/openapi.json` field for field.
//! 2. **The failure dialect** ([`error`]) — mem0's clients read every failure
//!    body as `{"detail": ...}`, and only parse it at all under
//!    `application/json`. Getting that body past the framework's response
//!    interceptor takes two steps, which is what [`mem0_problem_document_bridge`]
//!    and [`error::MEM0_ERROR_CONTENT_TYPE`] are: the body is declared as a
//!    problem document *inward* so the interceptor enriches it instead of
//!    discarding the message, and the media type is set to `application/json`
//!    *outward* so the clients parse it. Neither step changes the payload.
//! 3. **The credential bridge** ([`mem0_credential_bridge`]) — the official
//!    clients authenticate with `Authorization: Token <api_key>`, while every
//!    route on this crate is declared `api-key`. The rewrite is the whole
//!    translation between the two, and it is scoped to mem0 paths alone.
//!
//! Memory semantics are not re-implemented: [`handlers`] resolves the caller's
//! compatibility space and reuses the canonical `MemoryOpenApi` operations.
//! Where a mem0 parameter cannot be honoured, the handler refuses it by name
//! (501 with a reason) rather than answering with a narrower result that would
//! be indistinguishable from success.
//!
//! Three prefixes carry the wire: `/v1/`, `/v3/`, and `/v2/`. The first two hold
//! the operations this surface implements; `/v2/` holds the shapes the official
//! clients call that it deliberately does not — entity-scope deletion and the
//! whole profile/settings family — each answered `501` with its own reason.
//! `/v2/` is declared rather than left out because a prefix that is not declared
//! is answered by the framework's surface classifier *before* either bridge
//! below runs, which turns a mem0 call into a `401` carrying a media type the
//! clients cannot read.

pub mod dto;
pub mod error;
pub mod extract;
pub mod handlers;
pub mod translate;

use axum::extract::Request;
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{delete, get, post, put};
use axum::Router;
use sdkwork_web_core::constants::AUTHORIZATION_HEADER;

use crate::paths;

/// Authorization scheme the official mem0 clients use.
///
/// `mem0/client/main.py` (`_client_headers`) and `mem0-ts/src/client/mem0.ts`
/// both send `Authorization: Token <api_key>` — an RFC 6750-shaped header with a
/// non-`Bearer` scheme, which is why the rewrite below has to look at the
/// scheme rather than at `Authorization` alone.
const MEM0_AUTHORIZATION_SCHEME: &str = "Token";

/// The API-key header name in the lowercase form `HeaderName::from_static`
/// requires. `sdkwork_web_core::constants::API_KEY_HEADER` spells it `X-Api-Key`,
/// which is the same header; a test below pins that they cannot drift apart.
const API_KEY_HEADER_LOWER: &str = "x-api-key";

/// Adapter that presents the mem0 credential as the canonical API key.
///
/// **Layer position is load-bearing.** The framework validates credential
/// headers during surface classification
/// (`SecurityPolicy::validate_route_auth_credentials` — invoked from the
/// `SurfaceClassification` interceptor), and for an `ApiKey` route it allows
/// `X-Api-Key` alone: any `Authorization` header is rejected outright with
/// `credential-profile-contamination`. The rewrite therefore has to run
/// *outside* the web framework layer, which is where
/// `web_bootstrap` installs it.
///
/// Scoped to mem0 paths only: nothing else on this crate accepts that scheme, so
/// rewriting beyond them would let a caller smuggle a credential through a
/// surface that never promised to look at it.
pub async fn mem0_credential_bridge(mut request: Request, next: Next) -> Response {
    if paths::is_mem0_compat_path(request.uri().path()) {
        if let Some(api_key) = token_scheme_credential(request.headers()) {
            let headers = request.headers_mut();
            headers.remove(AUTHORIZATION_HEADER);
            headers.insert(
                header::HeaderName::from_static(API_KEY_HEADER_LOWER),
                api_key,
            );
        }
    }
    next.run(request).await
}

/// Reads an `Authorization: Token <value>` credential.
///
/// Returns the credential only for that scheme, so an inbound
/// `Authorization: Bearer ...` — which no mem0 client sends — is left untouched
/// and the framework reports it as the credential-profile violation it is.
///
/// The scheme is matched case-insensitively (RFC 7235 defines it that way); a
/// non-UTF-8 value is not a usable credential and is ignored rather than
/// rejected here, leaving that decision to the framework.
fn token_scheme_credential(headers: &axum::http::HeaderMap) -> Option<HeaderValue> {
    let raw = headers.get(AUTHORIZATION_HEADER)?.to_str().ok()?;
    let (scheme, credential) = raw.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case(MEM0_AUTHORIZATION_SCHEME) {
        return None;
    }
    let credential = credential.trim();
    if credential.is_empty() {
        return None;
    }
    HeaderValue::from_str(credential).ok()
}

/// Presents every failure on a mem0 path in the dialect the mem0 clients read.
///
/// Two producers, one media type:
///
/// * **Framework failures** this crate does not render — missing credentials, a
///   rejected credential profile, the body limit, rate limits, admission control
///   — come out as RFC 9457 `application/problem+json`. mem0's Python client
///   parses a failure body only when the content type starts with
///   `application/json` (`mem0/client/utils.py::_handle_http_error`), so a
///   problem document reaches the caller as raw text and its `detail` is never
///   read.
/// * **This crate's own failures** ([`error::Mem0Error`]) are declared
///   `application/problem+json` *inward* precisely so the framework's
///   `ResponseIdentity` interceptor enriches them instead of replacing the body
///   (see [`error::MEM0_ERROR_CONTENT_TYPE`]). Their wire framing is this
///   bridge's job.
///
/// Only the declared media type is changed, and only on mem0 paths: the payload
/// is a JSON document either way, and it keeps `detail`, `code`, and `trace_id`
/// (`SdkWorkProblemDetail`) intact for the caller and the operator. Rewriting the
/// body into a bare `{"detail": ...}` would discard exactly the diagnostics that
/// make a 429 or a 500 actionable.
///
/// **Layer position is load-bearing in the other direction from the credential
/// bridge.** This one has to sit *outside* the framework layer — which
/// `web_bootstrap` arranges — because it can only re-type a response after the
/// interceptor has finished with it.
pub async fn mem0_problem_document_bridge(request: Request, next: Next) -> Response {
    let is_mem0 = paths::is_mem0_compat_path(request.uri().path());
    let mut response = next.run(request).await;
    if is_mem0 && is_problem_json(&response) {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    response
}

fn is_problem_json(response: &Response) -> bool {
    response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/problem+json"))
}

/// The mem0 compatibility routes.
///
/// Paths and methods are exactly the generated manifest entries; mounting them
/// here must not diverge from that manifest, which is what
/// `open_router_mounts_every_open_openapi_operation_path` checks.
///
/// The `/v2/` half of this router is refusal-only. Those operations exist so the
/// declared `/v2/` prefix carries real paths, and so a caller reaching them gets
/// a named `501` in the mem0 failure dialect instead of the framework's
/// mis-framed `401`. They are registered here and nowhere else — the router is
/// the only place that decides which paths this surface answers at all.
pub fn mem0_routes() -> Router {
    Router::new()
        .route(paths::MEM0_PING, get(handlers::ping))
        .route(paths::MEM0_ADD, post(handlers::add_memory))
        .route(paths::MEM0_SEARCH, post(handlers::search_memories))
        .route(paths::MEM0_LIST, post(handlers::list_memories))
        .route(paths::MEM0_DELETE_ALL, delete(handlers::delete_all_memories))
        .route(
            paths::MEM0_MEMORY,
            get(handlers::get_memory)
                .put(handlers::update_memory)
                .delete(handlers::delete_memory),
        )
        .route(paths::MEM0_HISTORY, get(handlers::memory_history))
        .route(paths::MEM0_ENTITIES, get(handlers::list_entities))
        .route(paths::MEM0_FEEDBACK, post(handlers::submit_feedback))
        .route(
            paths::MEM0_BATCH,
            put(handlers::batch_update_memories).delete(handlers::batch_delete_memories),
        )
        // `/v2/` — named refusals. See `handlers` for why each one cannot be
        // answered rather than refused.
        .route(
            paths::MEM0_V2_ENTITIES,
            delete(handlers::refuse_v2_entity_delete),
        )
        .route(
            paths::MEM0_V2_ENTITY_PROFILE,
            get(handlers::refuse_v2_entity_profile),
        )
        .route(
            paths::MEM0_V2_PROFILE_JOBS,
            post(handlers::refuse_v2_profile_job_create),
        )
        .route(
            paths::MEM0_V2_PROFILE_JOB,
            get(handlers::refuse_v2_profile_job_retrieve),
        )
        .route(
            paths::MEM0_V2_PROFILE_SETTINGS,
            get(handlers::refuse_v2_profile_settings_read)
                .post(handlers::refuse_v2_profile_settings_update),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use axum::response::IntoResponse;
    use sdkwork_web_core::constants::API_KEY_HEADER;

    /// The hardcoded lowercase literal must keep naming the framework's API-key
    /// header; `HeaderName::from_static` needs the lowercase spelling and cannot
    /// consume the constant directly.
    #[test]
    fn api_key_header_literal_matches_the_framework_constant() {
        assert_eq!(API_KEY_HEADER_LOWER, API_KEY_HEADER.to_ascii_lowercase());
    }

    #[test]
    fn token_scheme_is_read_case_insensitively() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            HeaderValue::from_static("Token abc123"),
        );
        assert_eq!(
            token_scheme_credential(&headers).and_then(|value| value
                .to_str()
                .ok()
                .map(str::to_owned)),
            Some("abc123".to_string())
        );

        headers.insert(
            AUTHORIZATION_HEADER,
            HeaderValue::from_static("token  spaced-out  "),
        );
        assert_eq!(
            token_scheme_credential(&headers).and_then(|value| value
                .to_str()
                .ok()
                .map(str::to_owned)),
            Some("spaced-out".to_string())
        );
    }

    #[test]
    fn non_token_schemes_are_left_alone() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            AUTHORIZATION_HEADER,
            HeaderValue::from_static("Bearer sk-not-a-token-scheme"),
        );
        assert!(token_scheme_credential(&headers).is_none());

        headers.insert(AUTHORIZATION_HEADER, HeaderValue::from_static("Token"));
        assert!(token_scheme_credential(&headers).is_none());

        headers.insert(AUTHORIZATION_HEADER, HeaderValue::from_static("Token    "));
        assert!(token_scheme_credential(&headers).is_none());
    }

    /// The rewrite itself, driven through the middleware rather than the helper,
    /// so the layer position and the path scoping are both exercised.
    #[tokio::test]
    async fn bridge_moves_the_token_scheme_credential_onto_the_api_key_header() {
        use tower::ServiceExt;
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let recorder = seen.clone();
        let inner = Router::new().route(
            "/v1/ping/",
            get(move |request: Request| {
                let recorder = recorder.clone();
                async move {
                    let authorization = request
                        .headers()
                        .get(AUTHORIZATION_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let api_key = request
                        .headers()
                        .get(API_KEY_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    *recorder.lock().expect("lock") = Some((authorization, api_key));
                    StatusCode::OK
                }
            }),
        );
        let router = inner.layer(axum::middleware::from_fn(mem0_credential_bridge));

        let response = router
            .oneshot(
                HttpRequest::builder()
                    .uri("/v1/ping/")
                    .header(AUTHORIZATION_HEADER, "Token abc123")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            *seen.lock().expect("lock"),
            Some((None, Some("abc123".to_string())))
        );
    }

    #[tokio::test]
    async fn bridge_does_not_touch_other_surfaces() {
        use tower::ServiceExt;
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let recorder = seen.clone();
        let inner = Router::new().route(
            "/mem/v3/api/memory/capabilities",
            get(move |request: Request| {
                let recorder = recorder.clone();
                async move {
                    let authorization = request
                        .headers()
                        .get(AUTHORIZATION_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    let api_key = request
                        .headers()
                        .get(API_KEY_HEADER)
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    *recorder.lock().expect("lock") = Some((authorization, api_key));
                    StatusCode::OK
                }
            }),
        );
        let router = inner.layer(axum::middleware::from_fn(mem0_credential_bridge));

        let response = router
            .oneshot(
                HttpRequest::builder()
                    .uri("/mem/v3/api/memory/capabilities")
                    .header(AUTHORIZATION_HEADER, "Token abc123")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            *seen.lock().expect("lock"),
            Some((Some("Token abc123".to_string()), None))
        );
    }

    fn problem_response() -> Response {
        (
            StatusCode::TOO_MANY_REQUESTS,
            [(
                header::CONTENT_TYPE,
                "application/problem+json",
            )],
            r#"{"type":"about:blank","title":"rate limited","status":429,"detail":"slow down","code":42901,"trace_id":"t-1"}"#,
        )
            .into_response()
    }

    #[tokio::test]
    async fn problem_document_bridge_makes_framework_failures_readable_on_mem0_paths() {
        use tower::ServiceExt;
        let inner = Router::new().route(
            "/v3/memories/search/",
            post(|| async { problem_response() }),
        );
        let router = inner.layer(axum::middleware::from_fn(mem0_problem_document_bridge));

        let response = router
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v3/memories/search/")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body reads");
        let body: serde_json::Value = serde_json::from_slice(&body).expect("body is JSON");
        assert_eq!(body["detail"], "slow down");
        assert_eq!(body["trace_id"], "t-1");
    }

    #[tokio::test]
    async fn problem_document_bridge_leaves_sdkwork_surfaces_alone() {
        use tower::ServiceExt;
        let inner = Router::new().route(
            "/mem/v3/api/memory/capabilities",
            get(|| async { problem_response() }),
        );
        let router = inner.layer(axum::middleware::from_fn(mem0_problem_document_bridge));

        let response = router
            .oneshot(
                HttpRequest::builder()
                    .uri("/mem/v3/api/memory/capabilities")
                    .body(Body::empty())
                    .expect("request builds"),
            )
            .await
            .expect("router responds");
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/problem+json")
        );
    }
}
