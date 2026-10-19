use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEntityRequest {
    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "entityType")]
    pub entity_type: String,

    #[serde(rename = "canonicalName")]
    pub canonical_name: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aliases: Option<Vec<String>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "sensitivityLevel")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity_level: Option<String>,
}
