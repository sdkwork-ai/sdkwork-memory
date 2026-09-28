//! mem0 platform wire shapes.
//!
//! Field names are the upstream ones verbatim — the mem0 wire is snake_case
//! (`mem0-ts/src/client/utils.ts` converts snake_case *inbound* keys to camelCase
//! for its own callers, and the Python client returns the body untouched), so
//! these structs deliberately carry no `rename_all`.
//!
//! Request structs must tolerate unknown keys: every upstream request schema is
//! `additionalProperties: true`, and the official clients forward user `kwargs`
//! straight into the body.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One conversation turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mem0Message {
    pub role: String,
    pub content: String,
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/// `POST /v3/memories/add/`.
#[derive(Debug, Clone, Deserialize)]
pub struct Mem0AddRequest {
    pub messages: Vec<Mem0Message>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    /// Accepted for wire compatibility. Extraction runs the canonical pipeline
    /// either way; this surface has no second, non-inferred write shape.
    #[serde(default)]
    pub infer: Option<bool>,
}

/// `POST /v3/memories/search/`.
#[derive(Debug, Clone, Deserialize)]
pub struct Mem0SearchRequest {
    pub query: String,
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(default)]
    pub top_k: Option<i64>,
    #[serde(default)]
    pub threshold: Option<f64>,
    #[serde(default)]
    pub explain: Option<bool>,
    /// Accepted for wire compatibility; reranking is a retrieval-profile
    /// decision on this surface, not a per-request switch.
    #[serde(default)]
    pub rerank: Option<bool>,
    /// Accepted for wire compatibility; the response is always the v1.1 shape.
    #[serde(default)]
    pub output_format: Option<String>,
    #[serde(default)]
    pub show_expired: Option<bool>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
}

/// `POST /v3/memories/`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0ListRequest {
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
}

/// Query parameters of `POST /v3/memories/`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0ListParams {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
}

/// `PUT /v1/memories/{memory_id}/`.
///
/// `timestamp` and `expiration_date` are read as [`Option<Value>`] rather than
/// their nominal types so that *presence* is observable: the canonical update
/// operation cannot change either, and silently dropping a field the caller
/// explicitly sent would be a false success.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0UpdateRequest {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub timestamp: Option<Value>,
    #[serde(default)]
    pub expiration_date: Option<Value>,
}

/// Query parameters of `DELETE /v1/memories/{memory_id}/`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0DeleteParams {
    #[serde(default)]
    pub delete_linked: Option<bool>,
}

/// Query parameters of `DELETE /v1/memories/`.
///
/// The four entity filters are read so their presence is observable; the
/// canonical bulk deletion is space-scoped, so a filter this surface cannot
/// honour is refused rather than ignored.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0DeleteAllParams {
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub app_id: Option<String>,
}

/// Query parameters of `GET /v1/entities/`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0EntityParams {
    #[serde(default)]
    pub page_size: Option<i64>,
}

/// `POST /v1/feedback/`.
///
/// Upstream requires only `memory_id`; `feedback` is a nullable member of the
/// closed enum `[POSITIVE, NEGATIVE, VERY_NEGATIVE]`. `None` is meaningful
/// upstream — it withdraws the feedback — and the canonical feedback record has
/// no way to express that, so it is refused by name rather than written as a
/// fourth value.
#[derive(Debug, Clone, Deserialize)]
pub struct Mem0FeedbackRequest {
    pub memory_id: String,
    #[serde(default)]
    pub feedback: Option<String>,
    #[serde(default)]
    pub feedback_reason: Option<String>,
}

/// One entry of a `PUT`/`DELETE /v1/batch/` request.
///
/// `text`/`metadata` are the update payload; a delete carries only
/// `memory_id`, which is why both are optional here and validated per operation
/// instead of by the deserializer.
#[derive(Debug, Clone, Deserialize)]
pub struct Mem0BatchItem {
    pub memory_id: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

/// `PUT /v1/batch/` and `DELETE /v1/batch/`.
#[derive(Debug, Clone, Deserialize)]
pub struct Mem0BatchRequest {
    pub memories: Vec<Mem0BatchItem>,
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// The mem0 memory object.
///
/// `id` is the only key mem0 always emits. The keys it emits only on some
/// operations (`messages`, `event`, `categories`) are omitted when absent; the
/// keys it emits unconditionally are always present and `null` when unknown,
/// because the TypeScript client types those `X | null` rather than optional.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0Memory {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<Mem0Message>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    pub memory: Option<String>,
    pub user_id: Option<String>,
    pub agent_id: Option<String>,
    pub run_id: Option<String>,
    pub app_id: Option<String>,
    pub hash: Option<String>,
    pub score: Option<f64>,
    pub metadata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub expiration_date: Option<String>,
}

/// `POST /v3/memories/add/` response (v1.1 shape).
#[derive(Debug, Clone, Serialize)]
pub struct Mem0AddResult {
    pub results: Vec<Mem0Memory>,
}

/// `POST /v3/memories/search/` response (v1.1 shape).
#[derive(Debug, Clone, Serialize)]
pub struct Mem0SearchResult {
    pub results: Vec<Mem0Memory>,
}

/// `POST /v3/memories/` response.
///
/// `next`/`previous` are present-and-null rather than omitted: the TypeScript
/// client types them `string | null`, and this surface never emits
/// cross-page URLs (see the page boundary in the handlers).
#[derive(Debug, Clone, Serialize)]
pub struct Mem0PaginatedMemories {
    pub count: i64,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub results: Vec<Mem0Memory>,
}

/// Acknowledgement for a mutation that has no object to return.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0MutationAck {
    pub message: String,
}

/// `POST /v1/feedback/` response.
///
/// Upstream's 200 is `{id, feedback, feedback_reason}`, none required, with
/// `id` declared `format: uuid`. This surface reports the canonical feedback
/// row's identifier instead, so `id` is declared a plain string in the
/// authority — the same "declare what we emit" rule the rest of this module
/// follows, rather than advertising a uuid we do not produce.
///
/// `feedback_reason` is always absent: the canonical write accepts a `comment`
/// and persists it, but the canonical feedback record does not read it back, so
/// there is nothing to report. Echoing the caller's own input would claim a
/// round trip this surface cannot prove.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0FeedbackResult {
    pub id: String,
    pub feedback: Option<String>,
    pub feedback_reason: Option<String>,
}

/// `PUT`/`DELETE /v1/batch/` response.
///
/// Upstream's 200 carries a single `message` and nothing else — there is **no
/// per-item result channel**. That is the whole reason the batch handlers
/// validate and prove existence for every entry *before* mutating any of them:
/// with only a count to report, a partially applied batch would be
/// indistinguishable from a complete one.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0BatchAck {
    pub message: String,
}

/// One entry of `GET /v1/memories/{memory_id}/history/`.
///
/// Key set follows the platform schema (`external/mem0/docs/openapi.json`,
/// `/v1/memories/{memory_id}/history/` 200): `id`, `memory_id`, `event`,
/// `created_at`, `updated_at`, and `user_id` are required there; `old_memory`,
/// `categories`, and `input` are declared too. `is_deleted` is not part of the
/// platform item, so it is not emitted.
///
/// `input`, `old_memory`, and `new_memory` are always null here: the canonical
/// audit trail records that a mutation was accepted, not the memory text. That
/// is a deliberate privacy choice, not an omission — see the `history` handler.
/// `categories` is null because mem0's category model has no counterpart in the
/// canonical record. `user_id` is the record's own mem0 `user_id` scope.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0HistoryEntry {
    pub id: String,
    pub memory_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Vec<Mem0Message>>,
    pub old_memory: Option<String>,
    pub new_memory: Option<String>,
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    pub event: String,
    pub created_at: String,
    pub updated_at: String,
}

/// One entity scope as mem0's `GET /v1/entities/` reports it.
///
/// Field set is the platform schema verbatim (`external/mem0/docs/openapi.json`,
/// `/v1/entities/` 200): `id`, `name`, `created_at`, `updated_at`, `owner`, and
/// `type` are required there, `metadata` is optional. `name` is the addressable
/// value — the official clients delete entities by
/// `DELETE /v2/entities/{type}/{name}/`, so `name` must be the reference the
/// caller filed the memory under, while `id` is the entity's own unique handle.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0EntityScope {
    pub id: String,
    pub name: String,
    pub owner: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

/// `GET /v1/entities/` response.
///
/// The platform schema also declares optional `total_users` / `total_agents` /
/// `total_apps` / `total_runs` ("Total number of X entities **in the project**").
/// This surface does not emit them: its entity listing is scoped to the caller's
/// compatibility space, and reporting a space-derived number under a
/// project-wide name would be a false total.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0EntityList {
    pub count: i64,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub results: Vec<Mem0EntityScope>,
}

/// `GET /v1/ping/` response.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0PingResponse {
    pub org_id: Option<String>,
    pub project_id: Option<String>,
    pub user_email: Option<String>,
}
