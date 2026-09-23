//! `/v1/embeddings` adapter implementing [`EmbeddingModelPort`].

use sdkwork_memory_spi::{EmbeddingCommand, EmbeddingModelPort, MemorySpiError, MemorySpiResult};
use serde_json::Value;

use crate::config::OpenAiProviderConfig;

/// OpenAI-compatible embeddings client.
pub struct OpenAiEmbeddings {
    http: reqwest::Client,
    config: OpenAiProviderConfig,
}

impl OpenAiEmbeddings {
    pub fn new(config: OpenAiProviderConfig) -> Self {
        let http = config.http_client();
        Self { http, config }
    }

    fn url(&self) -> String {
        format!("{}/embeddings", self.config.base_url)
    }
}

/// Build the request body for the embeddings endpoint.
pub fn build_embeddings_request(model: &str, input: &str) -> Value {
    serde_json::json!({
        "model": model,
        "input": [input],
    })
}

/// Build a batched request body; the API preserves input order in `data`.
pub fn build_batch_embeddings_request(model: &str, inputs: &[String]) -> Value {
    serde_json::json!({
        "model": model,
        "input": inputs,
    })
}

/// Extract vectors in `data` order, validating every entry.
pub fn parse_batch_embeddings_response(payload: &Value) -> MemorySpiResult<Vec<Vec<f32>>> {
    let data = payload
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| MemorySpiError::PortOperationFailed {
            port: "EmbeddingModelPort".to_string(),
            message: "embedding response is missing data".to_string(),
        })?;
    data.iter()
        .map(|entry| {
            parse_embeddings_response(&serde_json::json!({
                "data": [{ "embedding": entry.get("embedding").cloned().unwrap_or(Value::Null) }]
            }))
        })
        .collect()
}

/// Extract the first vector from an `/embeddings` response.
///
/// The response shape is `{"data": [{"embedding": [..], "index": 0}, ..]}`;
/// requests carry a single input, so `index` 0 is the answer. Malformed
/// payloads are provider faults and surface as port errors rather than empty
/// vectors, so a degraded provider cannot silently score everything zero.
pub fn parse_embeddings_response(payload: &Value) -> MemorySpiResult<Vec<f32>> {
    let vector = payload
        .get("data")
        .and_then(Value::as_array)
        .and_then(|data| data.first())
        .and_then(|first| first.get("embedding"))
        .and_then(Value::as_array)
        .ok_or_else(|| MemorySpiError::PortOperationFailed {
            port: "EmbeddingModelPort".to_string(),
            message: "embedding response is missing data[0].embedding".to_string(),
        })?;
    let mut vector = vector
        .iter()
        .map(|component| {
            component
                .as_f64()
                .map(|component| component as f32)
                .ok_or_else(|| MemorySpiError::PortOperationFailed {
                    port: "EmbeddingModelPort".to_string(),
                    message: "embedding vector contains a non-numeric component".to_string(),
                })
        })
        .collect::<MemorySpiResult<Vec<f32>>>()?;
    if vector.is_empty() {
        return Err(MemorySpiError::PortOperationFailed {
            port: "EmbeddingModelPort".to_string(),
            message: "embedding vector is empty".to_string(),
        });
    }
    // Downstream similarity treats vectors as unit-length directions.
    let norm = vector
        .iter()
        .map(|component| (*component as f64) * (*component as f64))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= f64::EPSILON {
        return Err(MemorySpiError::PortOperationFailed {
            port: "EmbeddingModelPort".to_string(),
            message: "embedding vector has zero magnitude".to_string(),
        });
    }
    let norm = norm as f32;
    for component in &mut vector {
        *component /= norm;
    }
    Ok(vector)
}

#[async_trait::async_trait]
impl EmbeddingModelPort for OpenAiEmbeddings {
    fn provider_code(&self) -> &str {
        "openai-compatible"
    }

    fn dimensions(&self) -> usize {
        self.config.embedding_dimensions
    }

    async fn embed(&self, command: EmbeddingCommand) -> MemorySpiResult<Vec<f32>> {
        let payload = build_embeddings_request(&self.config.embedding_model, &command.input);
        let response = self
            .http
            .post(self.url())
            .bearer_auth(&self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding request failed: {error}"),
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding response read failed: {error}"),
            })?;
        if !status.is_success() {
            // Never echo the body verbatim: it can carry account metadata.
            return Err(MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding request returned HTTP {status}"),
            });
        }
        let payload: Value =
            serde_json::from_str(&body).map_err(|error| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding response is not JSON: {error}"),
            })?;
        let vector = parse_embeddings_response(&payload)?;
        if self.config.embedding_dimensions > 0 && vector.len() != self.config.embedding_dimensions
        {
            tracing::warn!(
                returned = vector.len(),
                configured = self.config.embedding_dimensions,
                "embedding model returned an unexpected vector width; trusting the model"
            );
        }
        Ok(vector)
    }

    /// One batched HTTP call per request, mirroring mem0's `embed_batch`
    /// batching; the API preserves input order in `data`.
    async fn embed_batch(&self, commands: Vec<EmbeddingCommand>) -> MemorySpiResult<Vec<Vec<f32>>> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }
        let inputs = commands
            .iter()
            .map(|command| command.input.clone())
            .collect::<Vec<_>>();
        let payload = build_batch_embeddings_request(&self.config.embedding_model, &inputs);
        let response = self
            .http
            .post(self.url())
            .bearer_auth(&self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding request failed: {error}"),
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding response read failed: {error}"),
            })?;
        if !status.is_success() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding request returned HTTP {status}"),
            });
        }
        let payload: Value =
            serde_json::from_str(&body).map_err(|error| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!("embedding response is not JSON: {error}"),
            })?;
        let vectors = parse_batch_embeddings_response(&payload)?;
        if vectors.len() != inputs.len() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: format!(
                    "embedding response returned {} vectors for {} inputs",
                    vectors.len(),
                    inputs.len()
                ),
            });
        }
        Ok(vectors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeddings_request_carries_model_and_single_input() {
        let payload = build_embeddings_request("text-embedding-3-small", "hello");
        assert_eq!(payload["model"], "text-embedding-3-small");
        assert_eq!(payload["input"][0], "hello");
    }

    #[test]
    fn embeddings_response_is_parsed_and_unit_normalized() {
        let payload = serde_json::json!({
            "data": [{ "index": 0, "embedding": [3.0, 4.0] }],
        });
        let vector = parse_embeddings_response(&payload).unwrap();
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn malformed_embedding_responses_are_provider_faults() {
        assert!(parse_embeddings_response(&serde_json::json!({})).is_err());
        assert!(parse_embeddings_response(&serde_json::json!({ "data": [] })).is_err());
        assert!(parse_embeddings_response(
            &serde_json::json!({ "data": [{ "embedding": ["x"] }] })
        )
        .is_err());
        assert!(
            parse_embeddings_response(&serde_json::json!({ "data": [{ "embedding": [] }] }))
                .is_err()
        );
    }
}
