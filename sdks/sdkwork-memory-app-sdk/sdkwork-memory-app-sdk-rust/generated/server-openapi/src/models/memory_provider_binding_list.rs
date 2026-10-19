use serde::{Deserialize, Serialize};

use crate::models::{MemoryProviderBinding, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryProviderBindingList {
    pub items: Vec<MemoryProviderBinding>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
