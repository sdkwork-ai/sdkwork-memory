use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemorySubjectRequest {
    #[serde(rename = "organizationId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,

    #[serde(rename = "subjectType")]
    pub subject_type: String,

    #[serde(rename = "subjectRef")]
    pub subject_ref: String,

    #[serde(rename = "displayName")]
    pub display_name: String,

    #[serde(rename = "defaultSpaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_space_id: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
