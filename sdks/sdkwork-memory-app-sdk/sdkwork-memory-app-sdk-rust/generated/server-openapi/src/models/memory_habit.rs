use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryHabit {
    #[serde(rename = "habitId")]
    pub habit_id: String,

    #[serde(rename = "spaceId")]
    pub space_id: String,

    #[serde(rename = "userId")]
    pub user_id: String,

    #[serde(rename = "habitKey")]
    pub habit_key: String,

    #[serde(rename = "habitType")]
    pub habit_type: String,

    pub description: String,

    pub stage: String,

    pub strength: f64,

    pub confidence: f64,

    #[serde(rename = "supportCount")]
    pub support_count: i64,

    #[serde(rename = "lastSignalAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_signal_at: Option<String>,

    #[serde(rename = "promotedMemoryId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promoted_memory_id: Option<String>,

    #[serde(rename = "decayAfter")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decay_after: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,

    #[serde(rename = "createdAt")]
    pub created_at: String,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    pub version: String,
}
