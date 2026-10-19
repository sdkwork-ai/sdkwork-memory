use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryUsageEntry {
    /// UTC calendar day (YYYY-MM-DD) the charge belongs to.
    pub day: String,

    pub metric: String,

    /// Cumulative signed charge for the day (record.delete accumulates negative deltas).
    pub delta: i64,

    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}
