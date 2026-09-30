//! Shared response-body reader that caps how much of an outbound provider
//! response is buffered into memory. A compromised or oversized endpoint
//! could otherwise stream unbounded bytes into the process within the request
//! timeout.

use sdkwork_memory_spi::MemorySpiError;

/// Hard ceiling for a buffered provider response body.
pub(crate) const MAX_PROVIDER_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Reads the response body incrementally, aborting once the byte ceiling is
/// crossed instead of trusting `Content-Length` (chunked responses omit it).
pub(crate) async fn read_body_capped(
    response: reqwest::Response,
) -> Result<String, MemorySpiError> {
    let mut body: Vec<u8> = Vec::new();
    let mut response = response;
    while let Some(chunk) =
        response
            .chunk()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "provider-http".to_string(),
                message: format!("provider response read failed: {error}"),
            })?
    {
        if body.len().saturating_add(chunk.len()) > MAX_PROVIDER_RESPONSE_BYTES {
            return Err(MemorySpiError::PortOperationFailed {
                port: "provider-http".to_string(),
                message: format!(
                    "provider response exceeds the {MAX_PROVIDER_RESPONSE_BYTES} byte read cap"
                ),
            });
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).map_err(|error| MemorySpiError::PortOperationFailed {
        port: "provider-http".to_string(),
        message: format!("provider response is not valid UTF-8: {error}"),
    })
}
