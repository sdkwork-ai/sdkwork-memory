use serde::{Deserialize, Serialize};

use crate::models::{MemoryIndex, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryIndexList {
    pub items: Vec<MemoryIndex>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
