//! Retry policy for outbound provider calls.
//!
//! Transient failures — HTTP 429, 5xx, connection, and timeout errors — are
//! retried at most twice with a fixed exponential backoff. Permanent failures
//! (401/403/404/400 and friends) surface immediately because no backoff can
//! fix a bad key or a missing model. Error messages keep the adapter-wide
//! shape: status code only, never a response body.

use std::time::Duration;

use sdkwork_memory_spi::MemorySpiError;

/// Fixed backoff schedule between attempts; at most two retries per call.
pub(crate) const RETRY_DELAYS_MS: [u64; 2] = [250, 1000];

/// Transport failure categories that separate transient from permanent faults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransportFailureKind {
    Connect,
    Timeout,
    Request,
    Other,
}

impl TransportFailureKind {
    pub(crate) fn of(error: &reqwest::Error) -> Self {
        // reqwest exposes no public constructor for its error kinds, so the
        // mapping is factored over its predicates and tested there.
        Self::of_kind(error.is_connect(), error.is_timeout(), error.is_request())
    }

    fn of_kind(is_connect: bool, is_timeout: bool, is_request: bool) -> Self {
        if is_connect {
            Self::Connect
        } else if is_timeout {
            Self::Timeout
        } else if is_request {
            Self::Request
        } else {
            Self::Other
        }
    }
}

/// Whether a transport failure class is transient enough to retry.
pub(crate) fn is_retryable_failure(kind: TransportFailureKind) -> bool {
    matches!(
        kind,
        TransportFailureKind::Connect
            | TransportFailureKind::Timeout
            | TransportFailureKind::Request
    )
}

/// Whether an HTTP status is transient enough to retry.
///
/// Rate limiting and server-side faults can clear on their own; every 4xx
/// outside 429 is a request or authorization defect that will fail identically
/// on every retry.
pub(crate) fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

/// The backoff before retry number `attempt` (0-based).
pub(crate) fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis(RETRY_DELAYS_MS[attempt.min(RETRY_DELAYS_MS.len() - 1)])
}

/// Sends `builder`, retrying transient failures up to twice with backoff.
///
/// The final response is returned even when its status is a retryable one that
/// never recovered: the caller's existing status handling produces the same
/// error messages as an un-retried call. `error_label` ("embedding" /
/// "completion") and `port` only shape the transport-error message, which
/// keeps the pre-retry wording.
pub(crate) async fn send_with_retries(
    builder: &reqwest::RequestBuilder,
    error_label: &str,
    port: &str,
) -> Result<reqwest::Response, MemorySpiError> {
    let mut attempt = 0_usize;
    loop {
        let request = builder
            .try_clone()
            .ok_or_else(|| MemorySpiError::PortOperationFailed {
                port: port.to_string(),
                message: format!("{error_label} request could not be rebuilt for send"),
            })?;
        match request.send().await {
            Ok(response) => {
                let status = response.status();
                if attempt < RETRY_DELAYS_MS.len() && is_retryable_status(status.as_u16()) {
                    attempt += 1;
                    let delay = retry_delay(attempt - 1);
                    tracing::debug!(
                        status = %status,
                        delay_ms = delay.as_millis() as u64,
                        attempt,
                        "retryable provider response; retrying"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Ok(response);
            }
            Err(error) => {
                if attempt < RETRY_DELAYS_MS.len()
                    && is_retryable_failure(TransportFailureKind::of(&error))
                {
                    attempt += 1;
                    let delay = retry_delay(attempt - 1);
                    tracing::debug!(
                        error = %error,
                        delay_ms = delay.as_millis() as u64,
                        attempt,
                        "transient provider failure; retrying"
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(MemorySpiError::PortOperationFailed {
                    port: port.to_string(),
                    message: format!("{error_label} request failed: {error}"),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_rate_limits_and_server_faults_are_retryable_statuses() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(500));
        assert!(is_retryable_status(502));
        assert!(is_retryable_status(503));
        assert!(is_retryable_status(599));
        for status in [400, 401, 403, 404, 409, 422, 428, 499] {
            assert!(!is_retryable_status(status), "{status} must not retry");
        }
    }

    #[test]
    fn connect_timeout_and_request_failures_are_retryable() {
        assert!(is_retryable_failure(TransportFailureKind::Connect));
        assert!(is_retryable_failure(TransportFailureKind::Timeout));
        assert!(is_retryable_failure(TransportFailureKind::Request));
        assert!(!is_retryable_failure(TransportFailureKind::Other));
    }

    #[test]
    fn transport_failure_kind_maps_reqwest_error_predicates() {
        // reqwest exposes no public constructor for these error kinds, so the
        // mapping is exercised through its inputs: the predicate order is
        // connect, then timeout, then request, then the catch-all.
        assert_eq!(
            TransportFailureKind::of_kind(false, false, false),
            TransportFailureKind::Other
        );
        assert_eq!(
            TransportFailureKind::of_kind(true, false, false),
            TransportFailureKind::Connect
        );
        assert_eq!(
            TransportFailureKind::of_kind(false, true, false),
            TransportFailureKind::Timeout
        );
        assert_eq!(
            TransportFailureKind::of_kind(false, false, true),
            TransportFailureKind::Request
        );
        assert_eq!(
            TransportFailureKind::of_kind(true, true, true),
            TransportFailureKind::Connect
        );
    }

    #[test]
    fn backoff_follows_the_fixed_exponential_schedule() {
        assert_eq!(retry_delay(0), Duration::from_millis(250));
        assert_eq!(retry_delay(1), Duration::from_millis(1000));
        assert_eq!(
            retry_delay(99),
            Duration::from_millis(1000),
            "an out-of-range attempt clamps to the last delay"
        );
        assert_eq!(RETRY_DELAYS_MS.len(), 2, "at most two retries");
    }

    /// End-to-end retry behavior against a raw TCP server; the classification
    /// unit tests above pin the decision, these pin that the loop applies it.
    mod http_loop {
        use super::*;
        use std::io::Read;
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        const TOO_MANY_REQUESTS: &str =
            "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        const BAD_REQUEST: &str =
            "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        const OK_JSON: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";

        fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
            haystack
                .windows(needle.len())
                .position(|window| window == needle)
        }

        /// Reads the whole request (head plus Content-Length body) so closing
        /// the socket never resets the connection mid-request.
        fn drain_request(stream: &mut std::net::TcpStream) {
            let mut buffer = Vec::with_capacity(1024);
            let mut chunk = [0_u8; 2048];
            loop {
                if let Some(head_end) = find_subsequence(&buffer, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buffer[..head_end]).to_ascii_lowercase();
                    let content_length = head
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if buffer.len() >= head_end + 4 + content_length {
                        return;
                    }
                }
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => return,
                    Ok(read) => buffer.extend_from_slice(&chunk[..read]),
                }
            }
        }

        /// Serves one hardcoded response per accepted connection.
        fn serve_responses(responses: &[&'static str]) -> (String, Arc<AtomicUsize>) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
            let url = format!("http://{}", listener.local_addr().expect("local address"));
            let served = Arc::new(AtomicUsize::new(0));
            let served_for_thread = served.clone();
            let responses = responses.to_vec();
            std::thread::spawn(move || {
                for (index, mut stream) in listener.incoming().flatten().enumerate() {
                    served_for_thread.fetch_add(1, Ordering::SeqCst);
                    drain_request(&mut stream);
                    if let Some(response) = responses.get(index) {
                        use std::io::Write;
                        let _ = stream.write_all(response.as_bytes());
                    }
                }
            });
            (url, served)
        }

        fn builder(url: String) -> reqwest::RequestBuilder {
            reqwest::Client::new()
                .post(url)
                .json(&serde_json::json!({ "model": "test-model" }))
        }

        #[tokio::test]
        async fn rate_limit_responses_are_retried_at_most_twice_then_returned() {
            let (url, served) =
                serve_responses(&[TOO_MANY_REQUESTS, TOO_MANY_REQUESTS, TOO_MANY_REQUESTS]);
            let response = send_with_retries(&builder(url), "completion", "LanguageModelPort")
                .await
                .expect("the transport itself never failed");
            assert_eq!(response.status().as_u16(), 429, "the final status surfaces");
            assert_eq!(
                served.load(Ordering::SeqCst),
                3,
                "one initial call plus two retries"
            );
        }

        #[tokio::test]
        async fn a_retryable_status_that_recovers_returns_the_success() {
            let (url, _served) = serve_responses(&[TOO_MANY_REQUESTS, OK_JSON]);
            let response = send_with_retries(&builder(url), "completion", "LanguageModelPort")
                .await
                .expect("the transport itself never failed");
            assert_eq!(response.status().as_u16(), 200);
        }

        #[tokio::test]
        async fn permanent_statuses_are_returned_without_retrying() {
            let (url, served) = serve_responses(&[BAD_REQUEST, TOO_MANY_REQUESTS]);
            let response = send_with_retries(&builder(url), "completion", "LanguageModelPort")
                .await
                .expect("the transport itself never failed");
            assert_eq!(response.status().as_u16(), 400);
            assert_eq!(
                served.load(Ordering::SeqCst),
                1,
                "a 400 must surface on the first response"
            );
        }
    }
}
