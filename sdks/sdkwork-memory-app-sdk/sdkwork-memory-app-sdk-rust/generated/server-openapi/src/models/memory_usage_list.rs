use serde::{Deserialize, Serialize};

use crate::models::{MemoryUsageEntry, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryUsageList {
    pub items: Vec<MemoryUsageEntry>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
