use serde::{Deserialize, Serialize};

use crate::models::{MemorySubject, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemorySubjectList {
    pub items: Vec<MemorySubject>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
