use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryProviderBindingRequest {
    #[serde(rename = "providerKind")]
    pub provider_kind: String,

    #[serde(rename = "providerCode")]
    pub provider_code: String,

    #[serde(rename = "displayName")]
    pub display_name: String,

    #[serde(rename = "endpointRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_ref: Option<String>,

    #[serde(rename = "secretRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,

    #[serde(rename = "modelRef")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_ref: Option<String>,

    pub capabilities: std::collections::HashMap<String, serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "healthState")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_state: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}
