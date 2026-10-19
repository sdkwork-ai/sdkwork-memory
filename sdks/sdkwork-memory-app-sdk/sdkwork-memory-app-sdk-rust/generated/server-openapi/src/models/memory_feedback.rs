use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryFeedback {
    #[serde(rename = "feedbackId")]
    pub feedback_id: String,

    #[serde(rename = "targetType")]
    pub target_type: String,

    #[serde(rename = "targetId")]
    pub target_id: String,

    #[serde(rename = "feedbackType")]
    pub feedback_type: String,

    #[serde(rename = "createdAt")]
    pub created_at: String,
}
