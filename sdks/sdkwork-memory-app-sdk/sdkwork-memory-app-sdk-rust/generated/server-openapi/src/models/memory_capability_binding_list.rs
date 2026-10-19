use serde::{Deserialize, Serialize};

use crate::models::{MemoryCapabilityBinding, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryCapabilityBindingList {
    pub items: Vec<MemoryCapabilityBinding>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
