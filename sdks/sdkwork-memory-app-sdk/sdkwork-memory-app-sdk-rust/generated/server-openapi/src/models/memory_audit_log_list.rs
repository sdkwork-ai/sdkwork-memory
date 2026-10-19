use serde::{Deserialize, Serialize};

use crate::models::{MemoryAuditLog, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryAuditLogList {
    pub items: Vec<MemoryAuditLog>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
