use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryCandidate {
    #[serde(rename = "candidateId")]
    pub candidate_id: String,

    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "candidateType")]
    pub candidate_type: String,

    #[serde(rename = "memoryType")]
    pub memory_type: String,

    #[serde(rename = "proposedText")]
    pub proposed_text: String,

    pub confidence: f64,

    #[serde(rename = "decisionState")]
    pub decision_state: String,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
