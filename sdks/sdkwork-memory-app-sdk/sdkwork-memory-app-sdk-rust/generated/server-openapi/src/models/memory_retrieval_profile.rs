use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalProfile {
    #[serde(rename = "retrievalProfileId")]
    pub retrieval_profile_id: String,

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

    pub status: String,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
