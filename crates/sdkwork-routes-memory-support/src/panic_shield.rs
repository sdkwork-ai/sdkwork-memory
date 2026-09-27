//! Panic shield: turns panicking handlers into standard problem+json 500s.
//!
//! Without this layer a panicking handler unwinds through axum and the
//! connection dies with no response at all — no numeric `code`, no `traceId`,
//! nothing to alert on. The shield catches the unwind at the assembled-router
//! boundary, logs the panic summary, and emits the same `MemoryApiProblem`
//! rendering every other error path uses. The panic detail itself is never
//! echoed to the client; it stays in the server log.

use std::panic::AssertUnwindSafe;
use std::task::{Context, Poll};

use axum::response::IntoResponse;
use futures::FutureExt;
use tower::{Layer, Service};

use crate::{MemoryApiError, MemoryApiProblem};

/// Wraps a router so handler panics become problem+json 500 responses.
#[derive(Clone, Default)]
pub struct MemoryPanicShieldLayer;

impl<S> Layer<S> for MemoryPanicShieldLayer {
    type Service = MemoryPanicShield<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MemoryPanicShield { inner }
    }
}

#[derive(Clone)]
pub struct MemoryPanicShield<S> {
    inner: S,
}

impl<S> Service<axum::extract::Request> for MemoryPanicShield<S>
where
    S: Service<axum::extract::Request, Response = axum::response::Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = axum::response::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::extract::Request) -> Self::Future {
        let future = self.inner.call(req);
        Box::pin(async move {
            match AssertUnwindSafe(future).catch_unwind().await {
                Ok(result) => result,
                Err(panic_payload) => {
                    tracing::error!(
                        panic = %panic_summary(panic_payload.as_ref()),
                        "request handler panicked; responding with problem+json 500"
                    );
                    Ok(MemoryApiProblem::from(MemoryApiError::new(
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        "request handler failed",
                    ))
                    .into_response())
                }
            }
        })
    }
}

fn panic_summary(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}
