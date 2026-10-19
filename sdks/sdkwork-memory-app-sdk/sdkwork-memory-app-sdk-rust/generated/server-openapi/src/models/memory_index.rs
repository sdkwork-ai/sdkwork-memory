use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryIndex {
    #[serde(rename = "indexId")]
    pub index_id: String,

    #[serde(rename = "spaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    #[serde(rename = "indexKind")]
    pub index_kind: String,

    #[serde(rename = "implementationProfileId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_profile_id: Option<String>,

    #[serde(rename = "providerBindingId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_binding_id: Option<String>,

    #[serde(rename = "schemaVersion")]
    pub schema_version: String,

    pub status: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "lastRebuiltAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_rebuilt_at: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
