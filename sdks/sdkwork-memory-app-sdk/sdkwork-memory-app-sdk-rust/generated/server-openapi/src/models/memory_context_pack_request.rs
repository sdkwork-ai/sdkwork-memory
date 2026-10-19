use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryContextPackRequest {
    pub query: String,

    #[serde(rename = "spaceIds")]
    pub space_ids: Vec<String>,

    #[serde(rename = "actorId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_id: Option<String>,

    #[serde(rename = "retrievalProfileId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_profile_id: Option<String>,

    #[serde(rename = "contextBudgetTokens")]
    pub context_budget_tokens: i64,

    #[serde(rename = "includeCitations")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_citations: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<std::collections::HashMap<String, serde_json::Value>>,
}
