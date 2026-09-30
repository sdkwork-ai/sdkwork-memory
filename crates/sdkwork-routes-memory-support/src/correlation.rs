use axum::{
    extract::Request,
    http::{request::Parts, HeaderMap},
    middleware::Next,
    response::Response,
};
use sdkwork_web_core::{
    new_request_id, trace_id_from_traceparent, WebRequestContext, TRACEPARENT_HEADER,
};
use tracing::Instrument;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryProblemCorrelation {
    pub request_id: String,
    pub trace_id: Option<String>,
}

tokio::task_local! {
    static CURRENT_PROBLEM_CORRELATION: MemoryProblemCorrelation;
}

fn read_header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

impl MemoryProblemCorrelation {
    pub fn from_request(request: &Request) -> Self {
        Self::from_parts(request.headers(), request.extensions())
    }

    pub fn from_parts(headers: &HeaderMap, extensions: &axum::http::Extensions) -> Self {
        if let Some(context) = extensions.get::<WebRequestContext>() {
            return Self {
                request_id: context.request_id.0.clone(),
                trace_id: context.trace_id.clone(),
            };
        }

        // The request id is always minted here. A caller-supplied
        // `X-Request-Id` is an unauthenticated input, and echoing it back in
        // problem documents would let any client correlate (or collide with)
        // another tenant's incidents, so no header fallback exists for it.
        let trace_id = read_header(headers, TRACEPARENT_HEADER)
            .and_then(|traceparent| trace_id_from_traceparent(&traceparent).map(str::to_owned));
        Self {
            request_id: new_request_id(),
            trace_id,
        }
    }

    pub fn from_parts_only(parts: &Parts) -> Self {
        Self::from_parts(&parts.headers, &parts.extensions)
    }

    pub fn current() -> Option<Self> {
        CURRENT_PROBLEM_CORRELATION.try_with(Clone::clone).ok()
    }
}

pub async fn problem_correlation_middleware(request: Request, next: Next) -> Response {
    let correlation = MemoryProblemCorrelation::from_request(&request);
    let request_id = correlation.request_id.clone();
    let trace_id = correlation
        .trace_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("-")
        .to_owned();
    async move {
        CURRENT_PROBLEM_CORRELATION
            .scope(correlation, async move { next.run(request).await })
            .await
    }
    .instrument(tracing::info_span!(
        "http_request",
        request_id = %request_id,
        trace_id = %trace_id,
    ))
    .await
}

pub fn with_problem_correlation<S>(router: axum::Router<S>) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.layer(axum::middleware::from_fn(problem_correlation_middleware))
}
