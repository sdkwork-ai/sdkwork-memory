use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryImplementationProfile {
    #[serde(rename = "implementationProfileId")]
    pub implementation_profile_id: String,

    pub name: String,

    #[serde(rename = "implementationKind")]
    pub implementation_kind: String,

    pub role: String,

    pub status: String,

    pub capabilities: std::collections::HashMap<String, serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollout: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
