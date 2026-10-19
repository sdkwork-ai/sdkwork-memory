use serde::{Deserialize, Serialize};

use crate::models::{MemoryRecord, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRecordList {
    pub items: Vec<MemoryRecord>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
