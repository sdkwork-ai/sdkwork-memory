//! `/v1/chat/completions` adapter implementing [`LanguageModelPort`].

use sdkwork_memory_spi::{
    LanguageModelCommand, LanguageModelPort, MemorySpiError, MemorySpiResult,
};
use serde_json::Value;

use crate::config::OpenAiProviderConfig;

/// OpenAI-compatible chat completions client.
pub struct OpenAiLlm {
    http: reqwest::Client,
    config: OpenAiProviderConfig,
}

impl OpenAiLlm {
    pub fn new(config: OpenAiProviderConfig) -> Self {
        let http = config.http_client();
        Self { http, config }
    }

    fn url(&self) -> String {
        format!("{}/chat/completions", self.config.base_url)
    }
}

/// Build the request body for a single-turn completion with JSON output.
pub fn build_chat_request(model: &str, prompt: &str) -> Value {
    serde_json::json!({
        "model": model,
        "messages": [
            { "role": "user", "content": prompt }
        ],
        "response_format": { "type": "json_object" }
    })
}

/// Extract the assistant message from a chat completions response.
///
/// The extraction pipeline consumes JSON; refusing to guess here means a
/// provider that returns prose surfaces as a port error instead of flowing
/// into JSON parsing downstream.
pub fn parse_chat_response(payload: &Value) -> MemorySpiResult<String> {
    let content = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|first| first.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .ok_or_else(|| MemorySpiError::PortOperationFailed {
            port: "LanguageModelPort".to_string(),
            message: "chat response is missing choices[0].message.content".to_string(),
        })?;
    Ok(content.to_string())
}

#[async_trait::async_trait]
impl LanguageModelPort for OpenAiLlm {
    fn provider_code(&self) -> &str {
        "openai-compatible"
    }

    async fn generate(&self, command: LanguageModelCommand) -> MemorySpiResult<String> {
        let payload = build_chat_request(&self.config.chat_model, &command.prompt);
        let response = self
            .http
            .post(self.url())
            .bearer_auth(&self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "LanguageModelPort".to_string(),
                message: format!("completion request failed: {error}"),
            })?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| MemorySpiError::PortOperationFailed {
                port: "LanguageModelPort".to_string(),
                message: format!("completion response read failed: {error}"),
            })?;
        if !status.is_success() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "LanguageModelPort".to_string(),
                message: format!("completion request returned HTTP {status}"),
            });
        }
        let payload: Value =
            serde_json::from_str(&body).map_err(|error| MemorySpiError::PortOperationFailed {
                port: "LanguageModelPort".to_string(),
                message: format!("completion response is not JSON: {error}"),
            })?;
        parse_chat_response(&payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_request_uses_json_output_mode() {
        let payload = build_chat_request("gpt-4o-mini", "extract facts");
        assert_eq!(payload["model"], "gpt-4o-mini");
        assert_eq!(payload["messages"][0]["content"], "extract facts");
        assert_eq!(payload["response_format"]["type"], "json_object");
    }

    #[test]
    fn chat_response_content_is_extracted() {
        let payload = serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": "{\"memory\":[]}" } }],
        });
        assert_eq!(parse_chat_response(&payload).unwrap(), "{\"memory\":[]}");
    }

    #[test]
    fn malformed_chat_responses_are_provider_faults() {
        assert!(parse_chat_response(&serde_json::json!({})).is_err());
        assert!(parse_chat_response(&serde_json::json!({ "choices": [] })).is_err());
    }
}
