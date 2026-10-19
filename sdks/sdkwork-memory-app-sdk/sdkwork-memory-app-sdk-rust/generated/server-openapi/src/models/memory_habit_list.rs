use serde::{Deserialize, Serialize};

use crate::models::{MemoryHabit, PageInfo};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryHabitList {
    pub items: Vec<MemoryHabit>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
