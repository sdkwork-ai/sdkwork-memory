use serde::{Deserialize, Serialize};

use crate::models::{MemoryRetrievalTrace, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalTraceList {
    pub items: Vec<MemoryRetrievalTrace>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
