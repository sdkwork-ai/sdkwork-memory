use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryFeedbackRequest {
    #[serde(rename = "targetType")]
    pub target_type: String,

    #[serde(rename = "targetId")]
    pub target_id: String,

    #[serde(rename = "feedbackType")]
    pub feedback_type: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rating: Option<i64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
