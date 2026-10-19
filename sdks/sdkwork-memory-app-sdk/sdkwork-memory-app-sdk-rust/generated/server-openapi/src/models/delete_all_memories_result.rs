use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct DeleteAllMemoriesResult {
    #[serde(rename = "deletedCount")]
    pub deleted_count: String,
}
