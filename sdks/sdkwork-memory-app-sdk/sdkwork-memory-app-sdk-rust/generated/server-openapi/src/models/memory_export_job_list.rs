use serde::{Deserialize, Serialize};

use crate::models::{MemoryExportJob, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryExportJobList {
    pub items: Vec<MemoryExportJob>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
