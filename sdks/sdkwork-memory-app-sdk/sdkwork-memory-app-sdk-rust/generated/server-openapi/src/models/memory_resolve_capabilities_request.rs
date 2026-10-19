use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct MemoryResolveCapabilitiesRequest {
    #[serde(rename = "targetType")]
    pub target_type: String,

    #[serde(rename = "targetId")]
    pub target_id: String,

    /// Opaque server-issued keyset cursor (pagination mode `cursor`, PAGINATION_SPEC section 3). Pass `data.pageInfo.nextCursor` from the previous page; omit for the first page. Client-constructed or forged cursors are rejected as invalid parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,

    /// Page size for this cursor-mode listing: default 20, maximum 200. Page boundaries are stable keyset positions, not ordinal page numbers.
    #[serde(rename = "pageSize")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i64>,
}
