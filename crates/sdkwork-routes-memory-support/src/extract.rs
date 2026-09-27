//! Strict body/path extractors that keep the error contract uniform.
//!
//! Axum's default `Json` and `Path` rejections render as `text/plain` with no
//! numeric `code` and no `traceId`, which breaks the SDKWork response standard
//! (errors use `application/problem+json`). These wrappers map every
//! rejection onto the same `MemoryApiProblem` the handlers use, so malformed
//! bodies and malformed path ids are indistinguishable from any other
//! invalid-parameter failure on the wire.

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{FromRequest, FromRequestParts, Json, Path, Request};
use axum::http::request::Parts;
use serde::de::DeserializeOwned;

use crate::{MemoryApiError, MemoryApiProblem};

pub const MALFORMED_BODY_DETAIL: &str =
    "request body must be well-formed JSON matching the operation schema";

pub const MALFORMED_PATH_DETAIL: &str = "path parameters must use the canonical id format";

/// Strict JSON body extraction with the standard problem+json rejection.
#[derive(Debug, Clone)]
pub struct MemoryJson<T>(pub T);

impl<T, S> FromRequest<S> for MemoryJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = MemoryApiProblem;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(rejection_problem)?;
        Ok(Self(value))
    }
}

/// Strict path extraction with the standard problem+json rejection.
#[derive(Debug, Clone, Copy, Default)]
pub struct MemoryPath<T>(pub T);

impl<T, S> FromRequestParts<S> for MemoryPath<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = MemoryApiProblem;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<T>::from_request_parts(parts, state)
            .await
            .map_err(rejection_problem)?;
        Ok(Self(value))
    }
}

fn rejection_problem(rejection: impl RejectionDetail) -> MemoryApiProblem {
    MemoryApiError::new(
        rejection.status(),
        "invalid_parameter",
        format!("{}: {}", MALFORMED_BODY_DETAIL, rejection.detail()),
    )
    .into()
}

/// Internal adapter so JSON and path rejections share one mapping without
/// duplicating the status/detail extraction.
trait RejectionDetail {
    fn status(&self) -> axum::http::StatusCode;
    fn detail(&self) -> String;
}

impl RejectionDetail for JsonRejection {
    fn status(&self) -> axum::http::StatusCode {
        JsonRejection::status(self)
    }

    fn detail(&self) -> String {
        self.body_text()
    }
}

impl RejectionDetail for PathRejection {
    fn status(&self) -> axum::http::StatusCode {
        PathRejection::status(self)
    }

    fn detail(&self) -> String {
        self.body_text()
    }
}
