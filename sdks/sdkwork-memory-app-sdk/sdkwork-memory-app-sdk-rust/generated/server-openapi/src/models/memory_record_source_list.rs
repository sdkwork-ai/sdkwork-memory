use serde::{Deserialize, Serialize};

use crate::models::{MemoryRecordSource, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRecordSourceList {
    pub items: Vec<MemoryRecordSource>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
