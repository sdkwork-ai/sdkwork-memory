use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyPatch {
    #[serde(rename = "policyType")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_type: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    #[serde(rename = "scopeRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}
