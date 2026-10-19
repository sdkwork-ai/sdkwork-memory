use serde::{Deserialize, Serialize};

use crate::models::{MemoryBinding, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryBindingList {
    pub items: Vec<MemoryBinding>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
