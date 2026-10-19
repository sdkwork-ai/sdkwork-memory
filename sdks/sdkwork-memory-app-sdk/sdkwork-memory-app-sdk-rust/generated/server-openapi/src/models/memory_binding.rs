use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryBinding {
    #[serde(rename = "bindingId")]
    pub binding_id: String,

    #[serde(rename = "tenantId")]
    pub tenant_id: String,

    #[serde(rename = "spaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    #[serde(rename = "bindingKind")]
    pub binding_kind: String,

    #[serde(rename = "bindingRole")]
    pub binding_role: String,

    #[serde(rename = "sourceSubjectId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_subject_id: Option<String>,

    #[serde(rename = "targetSubjectId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_subject_id: Option<String>,

    #[serde(rename = "targetSpaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_space_id: Option<String>,

    #[serde(rename = "capabilityCodes")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_codes: Option<Vec<String>>,

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
