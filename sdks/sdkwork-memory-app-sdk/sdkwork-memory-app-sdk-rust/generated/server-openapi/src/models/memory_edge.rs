use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEdge {
    #[serde(rename = "edgeId")]
    pub edge_id: String,

    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "sourceEntityId")]
    pub source_entity_id: String,

    #[serde(rename = "targetEntityId")]
    pub target_entity_id: String,

    #[serde(rename = "relationType")]
    pub relation_type: String,

    #[serde(rename = "sourceMemoryId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_memory_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,

    pub status: String,

    #[serde(rename = "validFrom")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,

    #[serde(rename = "validTo")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
