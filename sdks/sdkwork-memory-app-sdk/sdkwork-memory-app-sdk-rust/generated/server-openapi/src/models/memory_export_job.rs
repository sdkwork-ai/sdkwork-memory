use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryExportJob {
    #[serde(rename = "exportJobId")]
    pub export_job_id: String,

    pub state: String,

    pub format: String,

    #[serde(rename = "driveObjectRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive_object_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
