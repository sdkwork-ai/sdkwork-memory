//! mem0-shaped error rendering.
//!
//! Two media types are in play, and confusing them breaks the wire:
//!
//! * **On the wire** a mem0 failure is `application/json`, `{"detail": ...}`.
//!   mem0's clients read every failure body as `detail` — the Python client does
//!   exactly that in `_validate_api_key` and `api_error_handler`
//!   (`external/mem0/mem0/client/main.py`), and it only parses the body at all
//!   when the content type is `application/json`. The SDKWork problem document
//!   would make those handlers report `Error: None` instead of the actual cause,
//!   so the wire shape is part of the contract, not a style choice. That
//!   `application/json` framing is produced by
//!   [`crate::mem0::mem0_problem_document_bridge`], which rewrites the content
//!   type of these responses on their way out.
//! * **In front of the framework's response interceptor** the same body must
//!   present as `application/problem+json` — see [`MEM0_ERROR_CONTENT_TYPE`].
//!
//! The body itself is identical either way: a single `detail` string.

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceErrorKind};
use serde::Serialize;

/// Content type these errors present **inside the process**.
///
/// Not cosmetic, and not the type the wire sees. The framework's response
/// interceptor (`sdkwork-web-core` `interceptors.rs`, `ResponseIdentity` stage)
/// ends with `problem::normalize_problem_response`, which *replaces the whole
/// body* of every 4xx/5xx response whose content type is not
/// `application/problem+json`:
///
/// ```text
/// if is_problem_json(response) { enrich_problem_response(..) }   // body kept
/// else { replace_with_standard_problem(status, ..) }             // body discarded
/// ```
///
/// The replacement is status-derived and carries none of the handler's message:
/// a `404` came back as `{"detail": "Not found", "code": 40401, ...}` instead of
/// this surface's `memory not found`, a refused parameter came back as
/// `{"detail": "Malformed request", "code": 40002, ...}`, and a deliberate `501`
/// boundary would come back as `code 50001`/"An internal error occurred" —
/// `result_code_from_status` collapses any unmapped status onto `InternalError`.
/// Every one of those is a lie about a failure this layer raised on purpose.
///
/// Declaring the body as a problem document therefore does not buy the mem0
/// wire a problem document — it buys *pass-through*. `enrich_problem_response`
/// only adds the routing fields a handler omitted (`instance`, `operationId`,
/// `traceId`) and never touches `detail`, so the mem0 `detail` survives
/// byte-for-byte, and the outer bridge restores `application/json` before the
/// response leaves the process.
pub const MEM0_ERROR_CONTENT_TYPE: &str = "application/problem+json";

/// mem0's error envelope.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0ErrorBody {
    pub detail: String,
}

/// A failure rendered in the mem0 dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mem0Error {
    status: StatusCode,
    detail: String,
}

impl Mem0Error {
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
        }
    }

    pub fn invalid_parameter(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, detail)
    }

    pub fn unauthorized(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, detail)
    }

    /// A boundary of this surface, named explicitly instead of degraded
    /// silently: the caller asked for something the translation cannot honour,
    /// and must be able to tell that apart from "no results".
    pub fn unsupported(operation: &str, detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            format!("{operation} is not supported on this surface: {}", detail.into()),
        )
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl From<MemoryServiceError> for Mem0Error {
    fn from(error: MemoryServiceError) -> Self {
        let status = match error.kind {
            MemoryServiceErrorKind::NotFound => StatusCode::NOT_FOUND,
            MemoryServiceErrorKind::Conflict => StatusCode::CONFLICT,
            MemoryServiceErrorKind::Validation => StatusCode::BAD_REQUEST,
            MemoryServiceErrorKind::Forbidden => StatusCode::FORBIDDEN,
            MemoryServiceErrorKind::QuotaExceeded => StatusCode::TOO_MANY_REQUESTS,
            MemoryServiceErrorKind::Storage => StatusCode::INTERNAL_SERVER_ERROR,
            MemoryServiceErrorKind::NotImplemented => StatusCode::NOT_IMPLEMENTED,
        };
        Self::new(status, error.detail)
    }
}

impl IntoResponse for Mem0Error {
    fn into_response(self) -> Response {
        (
            self.status,
            [(header::CONTENT_TYPE, MEM0_ERROR_CONTENT_TYPE)],
            Json(Mem0ErrorBody {
                detail: self.detail,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    /// The in-process content type is what stops the framework from discarding
    /// the mem0 `detail`. It is pinned because the failure it prevents is
    /// invisible from this crate: the body is still valid JSON after
    /// replacement, only the message is gone.
    #[test]
    fn errors_declare_themselves_as_problem_documents_for_the_framework() {
        let response = Mem0Error::new(StatusCode::NOT_FOUND, "memory not found").into_response();
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some(MEM0_ERROR_CONTENT_TYPE),
        );
    }

    #[tokio::test]
    async fn the_body_carries_only_the_mem0_detail() {
        let response =
            Mem0Error::invalid_parameter("top_k must be between 1 and 100").into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body reads");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).expect("json body"),
            serde_json::json!({ "detail": "top_k must be between 1 and 100" }),
        );
    }
}
