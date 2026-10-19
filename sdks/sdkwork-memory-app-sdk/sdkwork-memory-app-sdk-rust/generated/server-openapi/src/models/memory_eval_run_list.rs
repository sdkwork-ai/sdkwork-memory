use serde::{Deserialize, Serialize};

use crate::models::{MemoryEvalRun, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryEvalRunList {
    pub items: Vec<MemoryEvalRun>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
