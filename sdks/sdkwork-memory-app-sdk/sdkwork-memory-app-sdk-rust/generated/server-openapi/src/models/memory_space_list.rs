use serde::{Deserialize, Serialize};

use crate::models::{MemorySpace, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemorySpaceList {
    pub items: Vec<MemorySpace>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
