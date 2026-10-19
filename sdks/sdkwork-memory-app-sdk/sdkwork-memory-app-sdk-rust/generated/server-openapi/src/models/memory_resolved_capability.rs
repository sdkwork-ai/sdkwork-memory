use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryResolvedCapability {
    #[serde(rename = "capabilityCode")]
    pub capability_code: String,

    pub mode: String,

    pub priority: i64,

    pub source: String,
}
