use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryExtractionRequest {
    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "inputEvents")]
    pub input_events: Vec<String>,

    #[serde(rename = "extractionMode")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extraction_mode: Option<String>,

    #[serde(rename = "customInstructions")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,

    #[serde(rename = "observationDate")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_date: Option<String>,
}
