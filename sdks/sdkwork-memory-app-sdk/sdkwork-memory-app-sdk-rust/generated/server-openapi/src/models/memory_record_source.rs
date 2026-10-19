use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRecordSource {
    #[serde(rename = "sourceId")]
    pub source_id: String,

    #[serde(rename = "memoryId")]
    pub memory_id: String,

    #[serde(rename = "eventId")]
    pub event_id: String,

    #[serde(rename = "sourceRole")]
    pub source_role: String,

    #[serde(rename = "confidenceDelta")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_delta: Option<f64>,

    #[serde(rename = "createdAt")]
    pub created_at: String,
}
