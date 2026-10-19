use serde::{Deserialize, Serialize};

use crate::models::{MemoryRetrievalProfile, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalProfileList {
    pub items: Vec<MemoryRetrievalProfile>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
