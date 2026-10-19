use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemorySpace {
    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,

    #[serde(rename = "tenantId")]
    pub tenant_id: String,

    #[serde(rename = "organizationId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,

    #[serde(rename = "ownerSubjectType")]
    pub owner_subject_type: String,

    #[serde(rename = "ownerSubjectId")]
    pub owner_subject_id: String,

    #[serde(rename = "spaceType")]
    pub space_type: String,

    #[serde(rename = "displayName")]
    pub display_name: String,

    #[serde(rename = "defaultScope")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scope: Option<String>,

    #[serde(rename = "lifecycleStatus")]
    pub lifecycle_status: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
