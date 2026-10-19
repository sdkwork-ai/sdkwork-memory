use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryImplementationProfileRequest {
    pub name: String,

    #[serde(rename = "implementationKind")]
    pub implementation_kind: String,

    pub role: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    pub capabilities: std::collections::HashMap<String, serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}
