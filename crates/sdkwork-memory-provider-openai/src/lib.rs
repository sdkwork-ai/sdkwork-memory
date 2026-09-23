//! OpenAI-compatible provider adapter for SDKWork Memory.
//!
//! Implements the SPI [`EmbeddingModelPort`] and [`LanguageModelPort`] against
//! any server that speaks the OpenAI REST dialect (`/v1/embeddings` and
//! `/v1/chat/completions`): OpenAI itself, Azure OpenAI gateways, Ollama,
//! vLLM, LiteLLM, and friends. OpenAI-compatibility is mem0's own default
//! provider stack, which makes this the least lock-in way to light up the
//! semantic channel.
//!
//! The adapter carries no policy: endpoints, models, and the API key come
//! from the environment (see [`OpenAiProviderConfig::from_env`]), the key is
//! never logged, and nothing in this crate is reachable unless the deployment
//! explicitly configures it.

mod config;
mod embeddings;
mod llm;

pub use config::OpenAiProviderConfig;
pub use embeddings::{
    build_batch_embeddings_request, build_embeddings_request, parse_batch_embeddings_response,
    parse_embeddings_response, OpenAiEmbeddings,
};
pub use llm::{parse_chat_response, OpenAiLlm};
