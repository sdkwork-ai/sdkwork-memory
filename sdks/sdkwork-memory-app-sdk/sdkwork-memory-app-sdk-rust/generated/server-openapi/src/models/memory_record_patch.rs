use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryRecordPatch {
    #[serde(rename = "canonicalText")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_text: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,

    #[serde(rename = "summaryText")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary_text: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}
