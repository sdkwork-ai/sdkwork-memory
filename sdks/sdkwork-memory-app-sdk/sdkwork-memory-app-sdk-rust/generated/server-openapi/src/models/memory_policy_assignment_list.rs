use serde::{Deserialize, Serialize};

use crate::models::{MemoryPolicyAssignment, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryPolicyAssignmentList {
    pub items: Vec<MemoryPolicyAssignment>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
