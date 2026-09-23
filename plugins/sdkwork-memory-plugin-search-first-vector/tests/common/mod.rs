//! Shared test doubles for the injected provider ports.
//!
//! The plugin reaches providers only through SPI ports, so a test can drive every
//! path with a deterministic double and no network, no credential, and no
//! provider client.

#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sdkwork_memory_spi::{
    EmbeddingCommand, EmbeddingModelPort, LanguageModelCommand, LanguageModelPort, MemorySpiError,
    MemorySpiResult, RerankMemoryHitsCommand, RerankMemoryHitsResult, RerankModelPort,
};

/// Embedding double that maps known input text to a configured vector.
pub struct MappedEmbeddingProvider {
    dimensions: usize,
    table: Mutex<HashMap<String, Vec<f32>>>,
    fallback: Vec<f32>,
    forced: Mutex<Option<Vec<f32>>>,
    failure: Mutex<Option<String>>,
    inputs: Mutex<Vec<String>>,
}

impl MappedEmbeddingProvider {
    /// Build a provider with a fallback vector for unmapped inputs.
    pub fn new(dimensions: usize, fallback: Vec<f32>) -> Arc<Self> {
        Arc::new(Self {
            dimensions,
            table: Mutex::new(HashMap::new()),
            fallback,
            forced: Mutex::new(None),
            failure: Mutex::new(None),
            inputs: Mutex::new(Vec::new()),
        })
    }

    /// Map one input text to a specific vector.
    pub fn with_entry(self: &Arc<Self>, text: &str, vector: Vec<f32>) -> Arc<Self> {
        self.table
            .lock()
            .expect("embedding table lock")
            .insert(text.to_string(), vector);
        Arc::clone(self)
    }

    /// Force every call to return this vector, whatever the input.
    pub fn with_forced_vector(self: &Arc<Self>, vector: Vec<f32>) -> Arc<Self> {
        *self.forced.lock().expect("forced lock") = Some(vector);
        Arc::clone(self)
    }

    /// Make every call fail with `message`.
    pub fn failing(self: &Arc<Self>, message: &str) -> Arc<Self> {
        *self.failure.lock().expect("failure lock") = Some(message.to_string());
        Arc::clone(self)
    }

    /// Inputs observed, in call order.
    pub fn recorded_inputs(&self) -> Vec<String> {
        self.inputs.lock().expect("inputs lock").clone()
    }
}

#[async_trait]
impl EmbeddingModelPort for MappedEmbeddingProvider {
    fn provider_code(&self) -> &str {
        "stub-embedding"
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    async fn embed(&self, command: EmbeddingCommand) -> MemorySpiResult<Vec<f32>> {
        self.inputs
            .lock()
            .expect("inputs lock")
            .push(command.input.clone());

        if let Some(message) = self.failure.lock().expect("failure lock").clone() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "EmbeddingModelPort".to_string(),
                message,
            });
        }

        if let Some(forced) = self.forced.lock().expect("forced lock").clone() {
            return Ok(forced);
        }

        let mapped = self
            .table
            .lock()
            .expect("embedding table lock")
            .get(&command.input)
            .cloned();

        Ok(mapped.unwrap_or_else(|| self.fallback.clone()))
    }
}

/// Language model double that serves scripted responses in order.
pub struct ScriptedLanguageProvider {
    responses: Mutex<VecDeque<String>>,
    failure: Mutex<Option<String>>,
    prompts: Mutex<Vec<String>>,
}

impl ScriptedLanguageProvider {
    /// Build a provider that answers with `responses` in order.
    pub fn new(responses: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(responses.into()),
            failure: Mutex::new(None),
            prompts: Mutex::new(Vec::new()),
        })
    }

    /// Make every call fail with `message`.
    pub fn failing(self: &Arc<Self>, message: &str) -> Arc<Self> {
        *self.failure.lock().expect("failure lock") = Some(message.to_string());
        Arc::clone(self)
    }

    /// Prompts observed, in call order.
    pub fn recorded_prompts(&self) -> Vec<String> {
        self.prompts.lock().expect("prompts lock").clone()
    }
}

#[async_trait]
impl LanguageModelPort for ScriptedLanguageProvider {
    fn provider_code(&self) -> &str {
        "stub-language-model"
    }

    async fn generate(&self, command: LanguageModelCommand) -> MemorySpiResult<String> {
        self.prompts
            .lock()
            .expect("prompts lock")
            .push(command.prompt);

        if let Some(message) = self.failure.lock().expect("failure lock").clone() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "LanguageModelPort".to_string(),
                message,
            });
        }

        self.responses
            .lock()
            .expect("responses lock")
            .pop_front()
            .ok_or_else(|| MemorySpiError::PortOperationFailed {
                port: "LanguageModelPort".to_string(),
                message: "no scripted response remains".to_string(),
            })
    }
}

/// Rerank double that serves scripted orderings in order.
pub struct ScriptedRerankProvider {
    responses: Mutex<VecDeque<Vec<String>>>,
    failure: Mutex<Option<String>>,
}

impl ScriptedRerankProvider {
    /// Build a provider that answers with `orderings` in order.
    pub fn new(orderings: Vec<Vec<String>>) -> Arc<Self> {
        Arc::new(Self {
            responses: Mutex::new(orderings.into()),
            failure: Mutex::new(None),
        })
    }

    /// Make every call fail with `message`.
    pub fn failing(self: &Arc<Self>, message: &str) -> Arc<Self> {
        *self.failure.lock().expect("failure lock") = Some(message.to_string());
        Arc::clone(self)
    }
}

#[async_trait]
impl RerankModelPort for ScriptedRerankProvider {
    fn provider_code(&self) -> &str {
        "stub-rerank"
    }

    async fn rerank(
        &self,
        _command: RerankMemoryHitsCommand,
    ) -> MemorySpiResult<RerankMemoryHitsResult> {
        if let Some(message) = self.failure.lock().expect("failure lock").clone() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "RerankModelPort".to_string(),
                message,
            });
        }

        let memory_ids = self
            .responses
            .lock()
            .expect("responses lock")
            .pop_front()
            .ok_or_else(|| MemorySpiError::PortOperationFailed {
                port: "RerankModelPort".to_string(),
                message: "no scripted ordering remains".to_string(),
            })?;

        Ok(RerankMemoryHitsResult { memory_ids })
    }
}
