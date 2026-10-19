use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyRequest {
    #[serde(rename = "policyType")]
    pub policy_type: String,

    pub scope: String,

    #[serde(rename = "scopeRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<String>,

    pub policy: std::collections::HashMap<String, serde_json::Value>,
}
