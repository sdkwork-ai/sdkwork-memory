use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalProfileRequest {
    #[serde(rename = "spaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    pub name: String,

    pub strategy: String,

    pub retrievers: std::collections::HashMap<String, serde_json::Value>,

    #[serde(rename = "fusionPolicy")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fusion_policy: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "rerankPolicy")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerank_policy: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "topK")]
    pub top_k: i64,

    #[serde(rename = "contextBudgetTokens")]
    pub context_budget_tokens: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}
