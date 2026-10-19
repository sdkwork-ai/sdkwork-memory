use serde::{Deserialize, Serialize};

use crate::models::{MemoryImplementationProfile, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryImplementationProfileList {
    pub items: Vec<MemoryImplementationProfile>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
