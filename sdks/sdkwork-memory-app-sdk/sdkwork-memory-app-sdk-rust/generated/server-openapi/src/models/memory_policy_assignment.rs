use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyAssignment {
    #[serde(rename = "policyAssignmentId")]
    pub policy_assignment_id: String,

    #[serde(rename = "tenantId")]
    pub tenant_id: String,

    #[serde(rename = "policyId")]
    pub policy_id: String,

    #[serde(rename = "targetType")]
    pub target_type: String,

    #[serde(rename = "targetId")]
    pub target_id: String,

    pub priority: i64,

    #[serde(rename = "inheritanceMode")]
    pub inheritance_mode: String,

    pub status: String,

    #[serde(rename = "validFrom")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,

    #[serde(rename = "validTo")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
