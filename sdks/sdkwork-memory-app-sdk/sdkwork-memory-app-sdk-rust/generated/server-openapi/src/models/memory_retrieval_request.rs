use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalRequest {
    /// Natural-language query text. Requests past 8192 characters are rejected as invalid parameters.
    pub query: String,

    #[serde(rename = "spaceIds")]
    pub space_ids: Vec<String>,

    #[serde(rename = "actorId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,

    #[serde(rename = "retrievalProfileId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_profile_id: Option<String>,

    #[serde(rename = "memoryTypes")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_types: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "topK")]
    pub top_k: i64,

    #[serde(rename = "contextBudgetTokens")]
    pub context_budget_tokens: i64,

    #[serde(rename = "showExpired")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_expired: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain: Option<bool>,

    #[serde(rename = "includeTrace")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_trace: Option<bool>,
}
