use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicy {
    #[serde(rename = "policyId")]
    pub policy_id: String,

    #[serde(rename = "tenantId")]
    pub tenant_id: String,

    #[serde(rename = "policyType")]
    pub policy_type: String,

    pub scope: String,

    #[serde(rename = "scopeRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_ref: Option<String>,

    pub status: String,

    pub policy: std::collections::HashMap<String, serde_json::Value>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
