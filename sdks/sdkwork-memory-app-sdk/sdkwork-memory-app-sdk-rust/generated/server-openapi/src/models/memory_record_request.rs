use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRecordRequest {
    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "userId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    pub scope: String,

    #[serde(rename = "memoryType")]
    pub memory_type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predicate: Option<String>,

    #[serde(rename = "objectText")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_text: Option<String>,

    #[serde(rename = "canonicalText")]
    pub canonical_text: String,

    #[serde(rename = "summaryText")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_text: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    #[serde(rename = "expiresAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,

    #[serde(rename = "sensitivityLevel")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}
