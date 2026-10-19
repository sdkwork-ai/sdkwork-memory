use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryIndexRequest {
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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}
