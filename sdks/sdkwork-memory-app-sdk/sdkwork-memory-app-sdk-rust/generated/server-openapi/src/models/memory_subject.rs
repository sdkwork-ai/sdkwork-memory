use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemorySubject {
    #[serde(rename = "subjectId")]
    pub subject_id: String,

    #[serde(rename = "tenantId")]
    pub tenant_id: String,

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

    pub status: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
