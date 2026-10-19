use serde::{Deserialize, Serialize};

use crate::models::{MemoryForgetJob, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryForgetJobList {
    pub items: Vec<MemoryForgetJob>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
