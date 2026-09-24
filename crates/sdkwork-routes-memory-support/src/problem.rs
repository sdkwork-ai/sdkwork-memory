use axum::{
    http::{header, HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceErrorKind};
use sdkwork_utils_rust::{SdkWorkProblemDetail, SdkWorkResultCode, SDKWORK_TRACE_ID_HEADER};
use sdkwork_web_core::trace::resolve_problem_trace_id;

use crate::correlation::MemoryProblemCorrelation;

pub type MemoryApiResult<T> = Result<T, MemoryApiError>;

#[derive(Debug, Clone)]
pub struct MemoryApiError {
    status: StatusCode,
    code: String,
    detail: String,
}

impl MemoryApiError {
    pub fn new(status: StatusCode, code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            detail: detail.into(),
        }
    }

    pub fn not_implemented(operation_id: &'static str) -> Self {
        Self::new(
            StatusCode::NOT_IMPLEMENTED,
            "operation_not_implemented",
            format!("operation is not implemented: {operation_id}"),
        )
    }

    pub fn invalid_parameter(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_parameter", detail)
    }

    fn result_code(&self) -> SdkWorkResultCode {
        match self.code.as_str() {
            "validation_error" => SdkWorkResultCode::ValidationError,
            "malformed_request" => SdkWorkResultCode::MalformedRequest,
            "invalid_parameter" => SdkWorkResultCode::InvalidParameter,
            "missing_required_field" => SdkWorkResultCode::MissingRequiredField,
            "authentication_required"
            | "missing_app_request_context"
            | "missing_backend_request_context"
            | "missing_open_request_context" => SdkWorkResultCode::AuthenticationRequired,
            "invalid_token" => SdkWorkResultCode::InvalidToken,
            "forbidden" | "permission_required" => SdkWorkResultCode::PermissionRequired,
            "tenant_access_denied" => SdkWorkResultCode::TenantAccessDenied,
            "not_found" => SdkWorkResultCode::NotFound,
            "method_not_allowed" => SdkWorkResultCode::MethodNotAllowed,
            "request_timeout" => SdkWorkResultCode::RequestTimeout,
            "conflict" => SdkWorkResultCode::Conflict,
            "payload_too_large" => SdkWorkResultCode::PayloadTooLarge,
            "rate_limited" => SdkWorkResultCode::RateLimitExceeded,
            "quota_exceeded" => SdkWorkResultCode::QuotaExceeded,
            "dependency_unavailable" => SdkWorkResultCode::ServiceUnavailable,
            "storage_error" => SdkWorkResultCode::InternalError,
            _ => match self.status {
                StatusCode::BAD_REQUEST => SdkWorkResultCode::ValidationError,
                StatusCode::UNAUTHORIZED => SdkWorkResultCode::AuthenticationRequired,
                StatusCode::FORBIDDEN => SdkWorkResultCode::PermissionRequired,
                StatusCode::NOT_FOUND => SdkWorkResultCode::NotFound,
                StatusCode::METHOD_NOT_ALLOWED => SdkWorkResultCode::MethodNotAllowed,
                StatusCode::REQUEST_TIMEOUT => SdkWorkResultCode::RequestTimeout,
                StatusCode::CONFLICT => SdkWorkResultCode::Conflict,
                StatusCode::PAYLOAD_TOO_LARGE => SdkWorkResultCode::PayloadTooLarge,
                StatusCode::TOO_MANY_REQUESTS => SdkWorkResultCode::RateLimitExceeded,
                StatusCode::SERVICE_UNAVAILABLE => SdkWorkResultCode::ServiceUnavailable,
                _ => SdkWorkResultCode::InternalError,
            },
        }
    }
}

impl From<MemoryServiceError> for MemoryApiError {
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
        Self::new(status, error.code, error.detail)
    }
}

#[derive(Debug, Clone)]
pub struct MemoryApiProblem {
    error: MemoryApiError,
}

impl MemoryApiProblem {
    pub fn new(status: StatusCode, code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            error: MemoryApiError::new(status, code, detail),
        }
    }
}

impl From<MemoryApiError> for MemoryApiProblem {
    fn from(error: MemoryApiError) -> Self {
        Self { error }
    }
}

impl From<MemoryServiceError> for MemoryApiProblem {
    fn from(error: MemoryServiceError) -> Self {
        MemoryApiError::from(error).into()
    }
}

impl IntoResponse for MemoryApiProblem {
    fn into_response(self) -> Response {
        let correlation = MemoryProblemCorrelation::current();
        let trace_id = correlation
            .as_ref()
            .and_then(|value| value.trace_id.clone())
            .or_else(|| {
                correlation
                    .as_ref()
                    .map(|value| resolve_problem_trace_id(value.request_id.as_str(), None))
            })
            // Contract: `traceId` is always a server-generated UUID v4 string
            // (see the OpenAPI ProblemDetail schema); never a placeholder.
            .unwrap_or_else(sdkwork_utils_rust::id::uuid);
        let result_code = self.error.result_code();
        let mut problem =
            SdkWorkProblemDetail::platform(result_code, self.error.detail, trace_id.clone());
        problem.status = self.error.status.as_u16();
        let mut response = (
            self.error.status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(problem),
        )
            .into_response();
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(SDKWORK_TRACE_ID_HEADER.as_bytes()),
            HeaderValue::from_str(&trace_id),
        ) {
            response.headers_mut().insert(name, value);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn;
    use axum::routing::get;
    use axum::Router;
    use sdkwork_web_core::{REQUEST_ID_HEADER, TRACEPARENT_HEADER};
    use tower::util::ServiceExt;

    use crate::correlation::problem_correlation_middleware;
    use crate::problem::MemoryApiProblem;

    async fn failing_handler() -> Result<&'static str, MemoryApiProblem> {
        Err(MemoryApiProblem::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            "spaceId query parameter is required",
        ))
    }

    async fn invalid_parameter_handler() -> Result<&'static str, MemoryApiProblem> {
        Err(super::MemoryApiError::invalid_parameter("invalid page_size").into())
    }

    #[tokio::test]
    async fn problem_response_includes_trace_id_and_numeric_code() {
        let app = Router::new()
            .route("/test", get(failing_handler))
            .layer(from_fn(problem_correlation_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header(REQUEST_ID_HEADER, "req-memory-1")
                    .header(
                        TRACEPARENT_HEADER,
                        "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(payload.get("requestId").is_none());
        assert_eq!(payload["traceId"], "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(40001, payload["code"].as_i64().unwrap());
    }

    #[tokio::test]
    async fn problem_response_preserves_invalid_parameter_code() {
        let app = Router::new()
            .route("/test", get(invalid_parameter_handler))
            .layer(from_fn(problem_correlation_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/test")
                    .header(REQUEST_ID_HEADER, "req-memory-2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers()[axum::http::header::CONTENT_TYPE],
            "application/problem+json"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(40003, payload["code"].as_i64().unwrap());
        assert_eq!("Invalid parameter", payload["title"]);
    }
}
