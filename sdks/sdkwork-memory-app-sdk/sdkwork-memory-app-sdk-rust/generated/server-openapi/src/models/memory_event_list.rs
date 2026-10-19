use serde::{Deserialize, Serialize};

use crate::models::{MemoryEvent, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEventList {
    pub items: Vec<MemoryEvent>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
