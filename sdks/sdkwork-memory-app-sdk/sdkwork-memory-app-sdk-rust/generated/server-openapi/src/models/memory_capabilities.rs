use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryCapabilities {
    #[serde(rename = "embeddingOptional")]
    pub embedding_optional: bool,

    pub retrievers: Vec<String>,

    #[serde(rename = "providerInterfaces")]
    pub provider_interfaces: Vec<String>,

    #[serde(rename = "implementationKinds")]
    pub implementation_kinds: Vec<String>,

    #[serde(rename = "openApiPrefix")]
    pub open_api_prefix: String,

    #[serde(rename = "sdkFamily")]
    pub sdk_family: String,

    #[serde(rename = "checkedAt")]
    pub checked_at: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
