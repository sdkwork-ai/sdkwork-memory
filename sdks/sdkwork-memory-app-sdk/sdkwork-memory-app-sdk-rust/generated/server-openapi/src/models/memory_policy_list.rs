use serde::{Deserialize, Serialize};

use crate::models::{MemoryPolicy, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyList {
    pub items: Vec<MemoryPolicy>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
