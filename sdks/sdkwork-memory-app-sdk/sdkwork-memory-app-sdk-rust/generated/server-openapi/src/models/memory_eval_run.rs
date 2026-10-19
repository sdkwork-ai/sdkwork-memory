use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEvalRun {
    #[serde(rename = "evalRunId")]
    pub eval_run_id: String,

    #[serde(rename = "evalType")]
    pub eval_type: String,

    pub state: String,

    #[serde(rename = "datasetRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_ref: Option<String>,

    #[serde(rename = "profileRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "startedAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,

    #[serde(rename = "finishedAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
