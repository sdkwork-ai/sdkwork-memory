use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryForgetRequest {
    /// Forget scope. space and user purge the ai_event inputs of the whole footprint; memory and query delete only the named or matched records and report purgedEvents 0 by construction.
    pub scope: String,

    #[serde(rename = "memoryIds")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_ids: Option<Vec<String>>,

    #[serde(rename = "spaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    pub reason: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
