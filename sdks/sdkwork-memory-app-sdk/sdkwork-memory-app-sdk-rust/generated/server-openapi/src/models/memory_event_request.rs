use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEventRequest {
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

    #[serde(rename = "sessionId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    #[serde(rename = "traceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    #[serde(rename = "eventType")]
    pub event_type: String,

    #[serde(rename = "sourceType")]
    pub source_type: String,

    #[serde(rename = "sourceRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,

    #[serde(rename = "eventTime")]
    pub event_time: String,

    pub payload: std::collections::HashMap<String, serde_json::Value>,

    #[serde(rename = "sensitivityLevel")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<String>,
}
