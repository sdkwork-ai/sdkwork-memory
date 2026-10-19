use serde::{Deserialize, Serialize};

use crate::models::{MemoryEntity, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEntityList {
    pub items: Vec<MemoryEntity>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
