use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyAssignmentPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,

    #[serde(rename = "inheritanceMode")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inheritance_mode: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,

    #[serde(rename = "validFrom")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,

    #[serde(rename = "validTo")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_to: Option<String>,
}
