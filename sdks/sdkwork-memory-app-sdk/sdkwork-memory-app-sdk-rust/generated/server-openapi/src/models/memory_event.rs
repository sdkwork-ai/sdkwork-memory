use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEvent {
    #[serde(rename = "eventId")]
    pub event_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "userId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    #[serde(rename = "actorType")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_type: Option<String>,

    #[serde(rename = "actorId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,

    #[serde(rename = "eventType")]
    pub event_type: String,

    #[serde(rename = "sourceType")]
    pub source_type: String,

    #[serde(rename = "eventTime")]
    pub event_time: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "payloadHash")]
    pub payload_hash: String,

    #[serde(rename = "sensitivityLevel")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<String>,

    #[serde(rename = "ingestionStatus")]
    pub ingestion_status: String,

    #[serde(rename = "createdAt")]
    pub created_at: String,
}
