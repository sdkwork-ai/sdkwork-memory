//! Environment-driven configuration for the OpenAI-compatible adapter.
//!
//! Every deployment turns the semantic channel on by setting
//! `SDKWORK_MEMORY_OPENAI_API_KEY`; without it the adapter is not constructed
//! and the rest of the system keeps its embedding-optional behavior. The key
//! is a secret: it is held in the config, redacted from `Debug`, and never
//! appears in logs or errors.

use std::fmt;

const ENV_API_KEY: &str = "SDKWORK_MEMORY_OPENAI_API_KEY";
const ENV_BASE_URL: &str = "SDKWORK_MEMORY_OPENAI_BASE_URL";
const ENV_EMBEDDING_MODEL: &str = "SDKWORK_MEMORY_OPENAI_EMBEDDING_MODEL";
const ENV_EMBEDDING_DIMENSIONS: &str = "SDKWORK_MEMORY_OPENAI_EMBEDDING_DIMENSIONS";
const ENV_CHAT_MODEL: &str = "SDKWORK_MEMORY_OPENAI_CHAT_MODEL";
const ENV_TIMEOUT_SECS: &str = "SDKWORK_MEMORY_OPENAI_TIMEOUT_SECS";

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const DEFAULT_EMBEDDING_MODEL: &str = "text-embedding-3-small";
const DEFAULT_EMBEDDING_DIMENSIONS: usize = 1536;
const DEFAULT_CHAT_MODEL: &str = "gpt-4o-mini";
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Connection settings for an OpenAI-compatible endpoint.
#[derive(Clone)]
pub struct OpenAiProviderConfig {
    /// Root of the REST API, without a trailing slash
    /// (e.g. `https://api.openai.com/v1`).
    pub base_url: String,
    pub api_key: String,
    /// Model used for embedding requests.
    pub embedding_model: String,
    /// Vector width the embedding model returns.
    pub embedding_dimensions: usize,
    /// Chat model used for completions.
    pub chat_model: String,
    pub timeout_secs: u64,
}

impl OpenAiProviderConfig {
    /// Read the configuration from the environment. Returns `None` when no API
    /// key is configured, which is the deployment's way of keeping the
    /// semantic channel off.
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var(ENV_API_KEY).ok()?;
        let api_key = api_key.trim().to_string();
        if api_key.is_empty() {
            return None;
        }
        Some(Self {
            base_url: std::env::var(ENV_BASE_URL)
                .ok()
                .map(|value| value.trim_end_matches('/').to_string())
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            api_key,
            embedding_model: std::env::var(ENV_EMBEDDING_MODEL)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_EMBEDDING_MODEL.to_string()),
            embedding_dimensions: std::env::var(ENV_EMBEDDING_DIMENSIONS)
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(DEFAULT_EMBEDDING_DIMENSIONS),
            chat_model: std::env::var(ENV_CHAT_MODEL)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_CHAT_MODEL.to_string()),
            timeout_secs: std::env::var(ENV_TIMEOUT_SECS)
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_TIMEOUT_SECS),
        })
    }

    /// Build an HTTP client honoring the configured timeout.
    pub fn http_client(&self) -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build()
            .unwrap_or_default()
    }
}

impl fmt::Debug for OpenAiProviderConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiProviderConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"<redacted>")
            .field("embedding_model", &self.embedding_model)
            .field("embedding_dimensions", &self.embedding_dimensions)
            .field("chat_model", &self.chat_model)
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_api_key_disables_the_provider() {
        // SAFETY-free env probing is serialized by the test harness being
        // single-threaded for env mutations; these keys are reserved to this
        // crate's tests.
        std::env::remove_var(ENV_API_KEY);
        assert!(OpenAiProviderConfig::from_env().is_none());
    }

    #[test]
    fn blank_api_key_disables_the_provider() {
        std::env::set_var(ENV_API_KEY, "   ");
        assert!(OpenAiProviderConfig::from_env().is_none());
        std::env::remove_var(ENV_API_KEY);
    }

    #[test]
    fn debug_output_never_carries_the_key() {
        let config = OpenAiProviderConfig {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key: "sk-super-secret".to_string(),
            embedding_model: DEFAULT_EMBEDDING_MODEL.to_string(),
            embedding_dimensions: DEFAULT_EMBEDDING_DIMENSIONS,
            chat_model: DEFAULT_CHAT_MODEL.to_string(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("sk-super-secret"));
        assert!(debug.contains("<redacted>"));
    }
}
