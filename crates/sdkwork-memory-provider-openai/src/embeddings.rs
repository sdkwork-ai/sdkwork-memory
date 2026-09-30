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
        let http = config.resolve_http_client();
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
        let builder = self
            .http
            .post(self.url())
            .bearer_auth(&self.config.api_key)
            .json(&payload);
        let response =
            crate::retry::send_with_retries(&builder, "embedding", "EmbeddingModelPort").await?;
        let status = response.status();
        let body = crate::response::read_body_capped(response).await.map_err(
            |error: MemorySpiError| MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message: error.to_string(),
            },
        )?;
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

    /// Batched embeddings, split into one HTTP request per byte-budget chunk
    /// and mirroring mem0's `embed_batch` batching; the API preserves input
    /// order within each chunk's `data`, so appending chunk responses in chunk
    /// order realigns vectors with `commands`.
    async fn embed_batch(&self, commands: Vec<EmbeddingCommand>) -> MemorySpiResult<Vec<Vec<f32>>> {
        if commands.is_empty() {
            return Ok(Vec::new());
        }
        let inputs = commands
            .into_iter()
            .map(|command| command.input)
            .collect::<Vec<_>>();
        let chunks = split_embed_inputs(inputs, self.config.embed_batch_max_bytes);
        let mut vectors = Vec::new();
        for chunk in &chunks {
            let payload = build_batch_embeddings_request(&self.config.embedding_model, chunk);
            let builder = self
                .http
                .post(self.url())
                .bearer_auth(&self.config.api_key)
                .json(&payload);
            let response =
                crate::retry::send_with_retries(&builder, "embedding", "EmbeddingModelPort")
                    .await?;
            let status = response.status();
            let body = crate::response::read_body_capped(response).await.map_err(
                |error: MemorySpiError| MemorySpiError::PortOperationFailed {
                    port: "EmbeddingModelPort".to_string(),
                    message: error.to_string(),
                },
            )?;
            if !status.is_success() {
                return Err(MemorySpiError::PortOperationFailed {
                    port: "EmbeddingModelPort".to_string(),
                    message: format!("embedding request returned HTTP {status}"),
                });
            }
            let payload: Value = serde_json::from_str(&body).map_err(|error| {
                MemorySpiError::PortOperationFailed {
                    port: "EmbeddingModelPort".to_string(),
                    message: format!("embedding response is not JSON: {error}"),
                }
            })?;
            let chunk_vectors = parse_batch_embeddings_response(&payload)?;
            if chunk_vectors.len() != chunk.len() {
                return Err(MemorySpiError::PortOperationFailed {
                    port: "EmbeddingModelPort".to_string(),
                    message: format!(
                        "embedding response returned {} vectors for {} inputs",
                        chunk_vectors.len(),
                        chunk.len()
                    ),
                });
            }
            vectors.extend(chunk_vectors);
        }
        Ok(vectors)
    }
}

/// Splits `inputs` into per-request chunks whose input bytes stay within
/// `budget_bytes`, moving each text into exactly one chunk so the input order
/// survives the split.
///
/// A single text larger than the budget is truncated onto the budget at a
/// UTF-8 character boundary (debug-logged) instead of failing the batch: a
/// memory that oversized is degraded for this one request, not dropped.
pub(crate) fn split_embed_inputs(inputs: Vec<String>, budget_bytes: usize) -> Vec<Vec<String>> {
    let budget_bytes = budget_bytes.max(1);
    let mut chunks: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    let mut current_bytes = 0_usize;
    for mut input in inputs {
        if input.len() > budget_bytes {
            tracing::debug!(
                bytes = input.len(),
                budget_bytes,
                "embedding input exceeds the per-request byte budget; truncating"
            );
            input = truncate_to_byte_budget(&input, budget_bytes);
        }
        if !current.is_empty() && current_bytes + input.len() > budget_bytes {
            chunks.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current_bytes += input.len();
        current.push(input);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Cuts `text` to at most `budget_bytes` UTF-8 bytes without splitting a
/// character. The budget is never zero ([`split_embed_inputs`] floors it), so
/// the cut always lands on a boundary at or before `budget_bytes`.
fn truncate_to_byte_budget(text: &str, budget_bytes: usize) -> String {
    let mut end = budget_bytes.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
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

    #[test]
    fn inputs_fitting_the_budget_stay_in_one_chunk() {
        let inputs = vec!["a".to_string(), "bb".to_string(), "ccc".to_string()];
        assert_eq!(
            split_embed_inputs(inputs, 64),
            vec![vec!["a".to_string(), "bb".to_string(), "ccc".to_string()]]
        );
    }

    #[test]
    fn chunks_never_exceed_the_byte_budget_and_keep_input_order() {
        let inputs = ["alpha", "beta", "gamma", "delta", "epsilon"]
            .iter()
            .map(|text| text.to_string())
            .collect::<Vec<_>>();
        let chunks = split_embed_inputs(inputs.clone(), 10);
        assert!(chunks.len() > 1, "the batch must actually be split");
        for chunk in &chunks {
            let chunk_bytes = chunk.iter().map(|text| text.len()).sum::<usize>();
            assert!(
                chunk_bytes <= 10,
                "chunk of {chunk_bytes} bytes escaped the budget"
            );
        }
        let flattened = chunks.into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(
            flattened, inputs,
            "splitting must not reorder or drop inputs"
        );
    }

    #[test]
    fn an_oversized_single_input_is_truncated_to_the_budget_not_dropped() {
        let long = "你知道么".to_string();
        let chunks = split_embed_inputs(vec![long.clone()], 5);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 1);
        let truncated = &chunks[0][0];
        assert!(truncated.len() <= 5);
        assert_eq!(truncated, "你", "the cut must stay on a character boundary");
        assert!(long.starts_with(truncated));
    }

    #[test]
    fn byte_budget_truncation_never_splits_a_multibyte_character() {
        assert_eq!(truncate_to_byte_budget("你对吗", 5), "你");
        assert_eq!(truncate_to_byte_budget("plain ascii", 4), "plai");
        assert_eq!(truncate_to_byte_budget("short", 64), "short");
        assert_eq!(truncate_to_byte_budget("", 64), "");
    }

    #[test]
    fn empty_batches_and_empty_inputs_do_not_break_the_split() {
        assert!(split_embed_inputs(Vec::new(), 1024).is_empty());
        let chunks = split_embed_inputs(vec![String::new(), String::new()], 1024);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 2);
    }
}
