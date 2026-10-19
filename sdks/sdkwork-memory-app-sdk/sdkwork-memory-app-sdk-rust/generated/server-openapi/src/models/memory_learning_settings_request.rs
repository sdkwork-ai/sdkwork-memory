use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryLearningSettingsRequest {
    #[serde(rename = "autoPromoteCandidates")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_promote_candidates: Option<bool>,

    #[serde(rename = "habitLearningEnabled")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub habit_learning_enabled: Option<bool>,
}
