use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRecord {
    #[serde(rename = "memoryId")]
    pub memory_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

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

    pub confidence: f64,

    #[serde(rename = "evidenceCount")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_count: Option<i64>,

    #[serde(rename = "contradictionCount")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contradiction_count: Option<i64>,

    #[serde(rename = "expiresAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,

    pub status: String,

    #[serde(rename = "sensitivityLevel")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "supersedesMemoryId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes_memory_id: Option<String>,

    #[serde(rename = "supersededByMemoryId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by_memory_id: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
