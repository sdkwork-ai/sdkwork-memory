use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEvalRunRequest {
    /// Executable evaluation engine. Additional evaluation types remain unavailable until a production worker implementation exists.
    #[serde(rename = "evalType")]
    pub eval_type: String,

    #[serde(rename = "datasetRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_ref: Option<String>,

    #[serde(rename = "profileRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::HashMap<String, serde_json::Value>>,
}
