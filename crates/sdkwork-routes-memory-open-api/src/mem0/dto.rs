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
    /// The v3 identity spelling (`mem0/client/types.py` documents the v3
    /// identity axes here); the four top-level names above are the older
    /// spelling the clients still send. Both are read so a caller following
    /// the v3 documentation does not write ownerless records that a later
    /// `search(filters=...)` can never find.
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    /// Refused by name when present: this surface stamps its own instants and
    /// has no caller-supplied timestamp to honour (same rule as the update).
    #[serde(default)]
    pub timestamp: Option<Value>,
    /// ISO-8601 instant after which the memory expires; persisted as the
    /// canonical record's expiry and served back as `expiration_date`.
    #[serde(default)]
    pub expiration_date: Option<Value>,
    /// Refused by name when present: category vocabularies and extraction
    /// instructions are deployment configuration, not per-request switches,
    /// and this surface files the literal text without an extraction step.
    #[serde(default)]
    pub custom_categories: Option<Value>,
    #[serde(default)]
    pub custom_instructions: Option<Value>,
    #[serde(default)]
    pub agent_custom_instructions: Option<Value>,
    #[serde(default)]
    pub structured_data_schema: Option<Value>,
    /// Accepted for wire compatibility. Extraction runs the canonical pipeline
    /// either way; this surface has no second, non-inferred write shape.
    #[serde(default)]
    pub infer: Option<bool>,
    // Everything below is refused by name when present (see the handler): each
    // is an upstream switch whose work this surface either does not perform or
    // does not let the caller reconfigure per request. They are declared so
    // presence is observable instead of silently dropped.
    /// Refused by name when present: upstream uses it to pin a memory against
    /// later ADD/UPDATE/DELETE arbitration, and this surface has no
    /// immutability flag to honour.
    #[serde(default)]
    pub immutable: Option<Value>,
    /// Refused by name when present: include/exclude lists steer mem0's LLM
    /// extraction, and this surface files the literal text without an
    /// extraction step.
    #[serde(default)]
    pub includes: Option<Value>,
    #[serde(default)]
    pub excludes: Option<Value>,
    /// Refused by name when present: the canonical record has no graph-memory
    /// variant; entity scopes are registered on every write.
    #[serde(default)]
    pub enable_graph: Option<Value>,
    /// Refused by name when present: a response-shape switch would change the
    /// response contract, and the mem0 memory shape here is fixed.
    #[serde(default)]
    pub output_format: Option<Value>,
    /// Refused by name when present: the extraction model profile is
    /// deployment configuration, not a per-request switch.
    #[serde(default)]
    pub prompt_profile_id: Option<Value>,
    /// Refused by name when present: temporal reasoning derives instants the
    /// caller supplies, and this surface stamps its own (same rule as
    /// `timestamp`).
    #[serde(default)]
    pub temporal_reasoning: Option<Value>,
    /// Refused by name when present: a timezone only matters for interpreting
    /// caller-supplied instants, which this surface does not accept.
    #[serde(default)]
    pub timezone: Option<Value>,
    /// Refused by name when present: the observation instant is a
    /// caller-supplied timestamp under another name, and the canonical record
    /// keeps the store's own instants.
    #[serde(default)]
    pub observation_datetime: Option<Value>,
    #[serde(default)]
    pub observation_date: Option<Value>,
}

/// `POST /v3/memories/search/`.
#[derive(Debug, Clone, Deserialize)]
pub struct Mem0SearchRequest {
    pub query: String,
    #[serde(default)]
    pub filters: Option<Value>,
    /// Exact-match metadata conditions, merged into the canonical filter
    /// conjunction: the canonical filter language reads record metadata, so a
    /// flat `{key: value}` entry is one equality condition.
    #[serde(default)]
    pub metadata: Option<Value>,
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
    /// Refused by name when present: a projection changes the response
    /// contract, and the mem0 memory shape here is fixed.
    #[serde(default)]
    pub fields: Option<Value>,
    /// Refused by name when present: mem0's category model has no counterpart
    /// in the canonical record (the response reports `categories` as absent).
    #[serde(default)]
    pub categories: Option<Value>,
    /// Refused by name when present: relative-to-a-reference-date expiry
    /// semantics are undefined on this surface.
    #[serde(default)]
    pub reference_date: Option<Value>,
    /// Refused by name when present: latest-version-only retrieval is a
    /// canonical profile decision, not a per-request switch.
    #[serde(default)]
    pub latest_only: Option<bool>,
    /// Refused by name when present: retriever-kind selection is the
    /// retrieval profile's decision on this surface.
    #[serde(default)]
    pub keyword_search: Option<Value>,
    /// Accepted and ignored: upstream treats it as a telemetry hint and the
    /// canonical retrieval has no per-source ranking switch. Declared here so
    /// the acceptance is part of the contract rather than an accident.
    #[serde(default)]
    pub source: Option<Value>,
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
    /// Mirrors `search.show_expired`; the canonical listing honours it the
    /// same way (expired records are hidden unless explicitly requested).
    #[serde(default)]
    pub show_expired: Option<bool>,
    /// Refused by name when present: the canonical listing has no date or
    /// category narrowing, and a silently ignored window would report a
    /// smaller result set as complete.
    #[serde(default)]
    pub start_date: Option<Value>,
    #[serde(default)]
    pub end_date: Option<Value>,
    #[serde(default)]
    pub categories: Option<Value>,
    /// Refused by name when present: latest-version-only listing is a
    /// canonical profile decision, not a per-request switch.
    #[serde(default)]
    pub latest_only: Option<bool>,
    /// Refused by name when present: a projection changes the response
    /// contract, and the mem0 memory shape on this surface is fixed (the same
    /// rule `search.fields` follows).
    #[serde(default)]
    pub fields: Option<Value>,
    /// Refused by name when present: the canonical listing has no keyword
    /// narrowing, and a silently ignored keyword would report a smaller
    /// result set as complete. `POST /v3/memories/search/` is the surface
    /// that ranks by lexical match.
    #[serde(default)]
    pub keywords: Option<Value>,
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
/// Every spelling an official client produces is read, so that a filter the
/// surface cannot honour is **refused rather than ignored**. The four top-level
/// entity filters are the documented REST spelling; `filters` is what the Python
/// client actually sends, and reading only the former is how a filter-scoped
/// bulk delete silently became an unscoped one. `metadata` is the same kind of
/// narrowing in a third spelling, and gets the same treatment.
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
    /// `MemoryClient.delete_all(filters={...})` passes the dict straight into
    /// `httpx`'s query params, so the wire value is a **`str()` of the dict** —
    /// `{'user_id': 'alice'}` — and not JSON. It is therefore read as opaque
    /// text: the question is only whether a filter was asked for. The same
    /// holds for `metadata`.
    #[serde(default)]
    pub filters: Option<String>,
    #[serde(default)]
    pub metadata: Option<String>,
}

impl Mem0DeleteAllParams {
    /// The entity filter this request asked for, in whichever spelling it used.
    ///
    /// Returns the parameter name and the value as received, so the refusal can
    /// quote what would otherwise have been discarded.
    pub fn requested_filter(&self) -> Option<(&'static str, &str)> {
        for (name, value) in [
            ("user_id", &self.user_id),
            ("agent_id", &self.agent_id),
            ("run_id", &self.run_id),
            ("app_id", &self.app_id),
        ] {
            if let Some(value) = value.as_deref() {
                return Some((name, value));
            }
        }
        self.filters
            .as_deref()
            .filter(|raw| !is_empty_filter_literal(raw))
            .map(|raw| ("filters", raw))
            .or_else(|| {
                self.metadata
                    .as_deref()
                    .filter(|raw| !is_empty_filter_literal(raw))
                    .map(|raw| ("metadata", raw))
            })
    }
}

/// Whether a serialised `filters` value carries no conditions.
///
/// Deliberately syntactic: the value is not necessarily JSON (see
/// [`Mem0DeleteAllParams::filters`]), so all that can be decided is whether
/// anything survives stripping the container punctuation and whitespace.
/// `{}`, `{ }`, `[]` and `""` all mean "no filter, delete the whole space",
/// which is the request the caller made; everything else is a filter, and a
/// filter this surface cannot apply must never be dropped.
fn is_empty_filter_literal(raw: &str) -> bool {
    raw.chars()
        .all(|character| character.is_whitespace() || matches!(character, '{' | '}' | '[' | ']' | ','))
}

#[cfg(test)]
mod tests {
    use super::{is_empty_filter_literal, Mem0DeleteAllParams};

    #[test]
    fn a_declared_entity_filter_is_reported_by_name() {
        let params = Mem0DeleteAllParams {
            user_id: Some("alice".to_owned()),
            ..Default::default()
        };
        assert_eq!(params.requested_filter(), Some(("user_id", "alice")));
    }

    /// The Python client's spelling, verbatim off the wire: a `str()` of the
    /// dict, which is not JSON and must still be recognised as a filter.
    #[test]
    fn the_python_client_filters_parameter_counts_as_a_filter() {
        let params = Mem0DeleteAllParams {
            filters: Some("{'user_id': 'alice'}".to_owned()),
            ..Default::default()
        };
        assert_eq!(params.requested_filter(), Some(("filters", "{'user_id': 'alice'}")));
    }

    #[test]
    fn a_json_spelled_filter_counts_too() {
        let params = Mem0DeleteAllParams {
            filters: Some(r#"{"agent_id":"planner"}"#.to_owned()),
            ..Default::default()
        };
        assert!(params.requested_filter().is_some());
    }

    /// `delete_all(metadata={...})` narrows the deletion just as much as a
    /// `filters` dict, so it must be reported by its own name rather than
    /// dropped.
    #[test]
    fn the_python_client_metadata_parameter_counts_as_a_filter() {
        let params = Mem0DeleteAllParams {
            metadata: Some("{'topic': 'batch'}".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            params.requested_filter(),
            Some(("metadata", "{'topic': 'batch'}"))
        );
    }

    /// An explicitly empty `metadata` object is "no narrowing", matching the
    /// `filters` spelling: refusing it would be a false refusal.
    #[test]
    fn an_empty_metadata_object_is_not_a_filter() {
        let params = Mem0DeleteAllParams {
            metadata: Some("{ }".to_owned()),
            ..Default::default()
        };
        assert_eq!(params.requested_filter(), None);
    }

    /// An explicitly empty filter set is the request the caller made: delete the
    /// space. Refusing it would be a false refusal.
    #[test]
    fn an_empty_filter_set_is_not_a_filter() {
        for raw in ["{}", "{ }", "[]", "  ", ""] {
            assert!(
                is_empty_filter_literal(raw),
                "`{raw}` must read as no filter at all"
            );
            let params = Mem0DeleteAllParams {
                filters: Some(raw.to_owned()),
                ..Default::default()
            };
            assert_eq!(params.requested_filter(), None, "`{raw}`");
        }
    }

    #[test]
    fn no_parameter_at_all_is_not_a_filter() {
        assert_eq!(Mem0DeleteAllParams::default().requested_filter(), None);
    }
}

/// Query parameters of `GET /v1/entities/`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Mem0EntityParams {
    /// Refused by name when not the first page: the canonical listing is
    /// cursor-ordered and only its first page is addressable, exactly like
    /// `POST /v3/memories/?page=`.
    #[serde(default)]
    pub page: Option<i64>,
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
/// per-item result channel**. The batch handlers therefore validate every
/// entry's shape *before* mutating any of it, and the service layer's batched
/// precheck read resolves existence once for the whole batch: when a slot
/// fails, the batch answers with that one error (404 naming the entry as the
/// caller sent it, or the count-honest conflict message), so a caller never
/// mistakes a partially applied batch for a complete one.
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
///
/// `status` is not decoration: `MemoryClient.ping()` in the JavaScript client
/// throws `APIError("API Key is invalid")` unless this reads exactly `"ok"`.
/// The constructor does *not* propagate that — it logs it and resolves its
/// identity with `organizationId`/`projectId` left unset — so the visible
/// damage is a failing `ping()` plus every identity-dependent call
/// (`getProject`/`updateProject`), while the memory calls still go out. The
/// Python client is content with `org_id`/`project_id` alone and never looks at
/// `status`. Emitting it is what makes the endpoint acceptable to *both*
/// official clients rather than just the one whose run happened to be written
/// first.
#[derive(Debug, Clone, Serialize)]
pub struct Mem0PingResponse {
    pub org_id: Option<String>,
    pub project_id: Option<String>,
    pub user_email: Option<String>,
    pub status: &'static str,
}
