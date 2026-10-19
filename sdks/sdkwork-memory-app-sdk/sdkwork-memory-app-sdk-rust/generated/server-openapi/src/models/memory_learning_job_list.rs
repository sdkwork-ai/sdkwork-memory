use serde::{Deserialize, Serialize};

use crate::models::{MemoryLearningJob, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryLearningJobList {
    pub items: Vec<MemoryLearningJob>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
