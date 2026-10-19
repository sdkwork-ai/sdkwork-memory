use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryLearningSettings {
    #[serde(rename = "autoPromoteCandidates")]
    pub auto_promote_candidates: bool,

    #[serde(rename = "habitLearningEnabled")]
    pub habit_learning_enabled: bool,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
