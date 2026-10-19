use serde::{Deserialize, Serialize};

use crate::models::{MemoryRetrievalHit, MemoryRetrievalTrace};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalResult {
    #[serde(rename = "retrievalId")]
    pub retrieval_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<MemoryRetrievalTrace>,

    pub hits: Vec<MemoryRetrievalHit>,

    pub degraded: bool,
}
