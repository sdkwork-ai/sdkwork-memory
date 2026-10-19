use serde::{Deserialize, Serialize};

use crate::models::{MemoryProviderBinding};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryProviderHealth {
    pub status: String,

    #[serde(rename = "checkedAt")]
    pub checked_at: String,

    pub providers: Vec<MemoryProviderBinding>,
}
