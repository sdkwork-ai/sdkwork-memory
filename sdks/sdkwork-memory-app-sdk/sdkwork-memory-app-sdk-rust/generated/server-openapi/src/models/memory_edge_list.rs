use serde::{Deserialize, Serialize};

use crate::models::{MemoryEdge, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEdgeList {
    pub items: Vec<MemoryEdge>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
