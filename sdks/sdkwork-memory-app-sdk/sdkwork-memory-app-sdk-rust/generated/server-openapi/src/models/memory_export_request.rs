use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryExportRequest {
    #[serde(rename = "spaceIds")]
    pub space_ids: Vec<String>,

    pub format: String,

    #[serde(rename = "includeEvents")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_events: Option<bool>,

    #[serde(rename = "driveTargetRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive_target_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
