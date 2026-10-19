use serde::{Deserialize, Serialize};

use crate::models::{MemoryResolvedCapability, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryResolvedCapabilityList {
    pub items: Vec<MemoryResolvedCapability>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
