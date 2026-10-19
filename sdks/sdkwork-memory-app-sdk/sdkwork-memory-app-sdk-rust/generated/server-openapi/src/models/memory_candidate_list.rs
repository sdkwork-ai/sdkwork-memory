use serde::{Deserialize, Serialize};

use crate::models::{MemoryCandidate, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryCandidateList {
    pub items: Vec<MemoryCandidate>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
