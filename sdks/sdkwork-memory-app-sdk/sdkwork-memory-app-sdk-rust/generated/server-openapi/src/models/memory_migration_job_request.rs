use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryMigrationJobRequest {
    #[serde(rename = "sourceImplementationProfileId")]
    pub source_implementation_profile_id: String,

    #[serde(rename = "targetImplementationProfileId")]
    pub target_implementation_profile_id: String,

    pub mode: String,

    pub reason: String,

    #[serde(rename = "spaceIds")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_ids: Option<Vec<String>>,

    #[serde(rename = "dryRun")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
