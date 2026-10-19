use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyAssignmentRequest {
    #[serde(rename = "policyId")]
    pub policy_id: String,

    #[serde(rename = "targetType")]
    pub target_type: String,

    #[serde(rename = "targetId")]
    pub target_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,

    #[serde(rename = "inheritanceMode")]
    pub inheritance_mode: String,

    #[serde(rename = "validFrom")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,

    #[serde(rename = "validTo")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
}
