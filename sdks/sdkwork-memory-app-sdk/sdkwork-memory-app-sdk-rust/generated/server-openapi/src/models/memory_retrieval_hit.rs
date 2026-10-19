use serde::{Deserialize, Serialize};

use crate::models::{MemoryRecord};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalHit {
    #[serde(rename = "hitId")]
    pub hit_id: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryRecord>,

    #[serde(rename = "memoryId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_id: Option<String>,

    #[serde(rename = "retrieverName")]
    pub retriever_name: String,

    #[serde(rename = "resultRank")]
    pub result_rank: i64,

    #[serde(rename = "rawScore")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_score: Option<f64>,

    #[serde(rename = "fusedScore")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fused_score: Option<f64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<std::collections::HashMap<String, serde_json::Value>>,

    pub status: String,
}
