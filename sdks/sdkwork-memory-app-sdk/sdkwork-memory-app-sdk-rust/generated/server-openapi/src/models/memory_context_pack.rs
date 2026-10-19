use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryContextPack {
    #[serde(rename = "contextPackId")]
    pub context_pack_id: String,

    #[serde(rename = "retrievalId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    pub pack: std::collections::HashMap<String, serde_json::Value>,

    #[serde(rename = "estimatedTokens")]
    pub estimated_tokens: i64,

    pub truncated: bool,

    #[serde(rename = "createdAt")]
    pub created_at: String,
}
