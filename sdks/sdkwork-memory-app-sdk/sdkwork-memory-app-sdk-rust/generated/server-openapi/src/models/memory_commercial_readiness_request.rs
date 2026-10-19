use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryCommercialReadinessRequest {
    #[serde(rename = "implementationProfileId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation_profile_id: Option<String>,
}
