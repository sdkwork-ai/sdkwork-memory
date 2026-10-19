use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRetrievalTrace {
    #[serde(rename = "traceId")]
    pub trace_id: String,

    #[serde(rename = "spaceId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub space_id: Option<String>,

    #[serde(rename = "retrievalProfileId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_profile_id: Option<String>,

    #[serde(rename = "actorId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,

    #[serde(rename = "queryText")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_text: Option<String>,

    #[serde(rename = "queryHash")]
    pub query_hash: String,

    #[serde(rename = "resultCount")]
    pub result_count: i64,

    pub degraded: bool,

    #[serde(rename = "createdAt")]
    pub created_at: String,
}
