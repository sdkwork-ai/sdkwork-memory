//! Handlers for the mem0 compatibility wire.
//!
//! Every operation resolves the caller's compatibility space and then reuses the
//! canonical `MemoryOpenApi` operation. Nothing here re-implements memory
//! semantics: what this layer owns is the *translation* — mem0 JSON in, mem0
//! JSON out — plus the boundaries that translation cannot cross, which are
//! refused by name instead of degraded silently.

use std::sync::Arc;

use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::Json;
use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, ListEntitiesQuery, ListMemoriesQuery, MemoryFeedbackRequest,
    MemoryOpenApi, MemoryOpenApiRequestContext, MemoryRecordPatch, MemoryRecordRequest,
    MemoryRetrievalRequest, MemoryType,
};
use sdkwork_utils_rust::MAX_LIST_PAGE_SIZE;
use serde_json::{Map, Value};

use crate::mem0::dto::{
    Mem0AddRequest, Mem0AddResult, Mem0BatchAck, Mem0BatchItem, Mem0BatchRequest,
    Mem0DeleteAllParams, Mem0DeleteParams, Mem0EntityList, Mem0EntityParams, Mem0EntityScope,
    Mem0FeedbackRequest, Mem0FeedbackResult, Mem0HistoryEntry, Mem0ListParams, Mem0ListRequest,
    Mem0Memory, Mem0MutationAck, Mem0PaginatedMemories, Mem0PingResponse, Mem0SearchRequest,
    Mem0SearchResult, Mem0UpdateRequest,
};
use crate::mem0::error::Mem0Error;
use crate::mem0::extract::{Mem0Json, Mem0Query};
use crate::mem0::translate::{
    entity_scope_from_record, history_entry_from_audit, memory_from_record, MEM0_CONTEXT_BUDGET_TOKENS,
    MEM0_DEFAULT_TOP_K, MEM0_ENTITY_SCOPES, MEM0_MAX_TOP_K, MEM0_RECORD_SCOPE,
};
use crate::routes::OpenState;

/// Entity maps a bounded number of entries onto the platform's per-space record
/// ceiling, so a single page is the whole history for any space this surface can
/// fill. The list path is bounded rather than unpaged on principle.
const MEM0_HISTORY_PAGE_SIZE: i32 = MAX_LIST_PAGE_SIZE;

fn require_context(
    context: Option<Extension<MemoryOpenApiRequestContext>>,
) -> Result<MemoryOpenApiRequestContext, Mem0Error> {
    context.map(|Extension(context)| context).ok_or_else(|| {
        Mem0Error::unauthorized(
            "authentication required: send the mem0 API key as `Authorization: Token <key>`",
        )
    })
}

/// The concrete product service.
///
/// Only a trait-only mount lacks it; production assemblies always bind the
/// product, so this is a deployment error surfaced as 501 rather than a silent
/// empty answer.
fn product(state: &OpenState) -> Result<Arc<OpenMemoryService>, Mem0Error> {
    state.require_product().map_err(|_| {
        Mem0Error::new(
            StatusCode::NOT_IMPLEMENTED,
            "the mem0 compatibility surface requires the Memory product service",
        )
    })
}

async fn space_id(
    product: &OpenMemoryService,
    context: &MemoryOpenApiRequestContext,
) -> Result<u64, Mem0Error> {
    product.mem0_space_id(context).await.map_err(Mem0Error::from)
}

/// Resolves the mem0 `id` onto the canonical record identity.
///
/// An identifier this service never issued cannot name a stored memory, and
/// upstream answers an unknown id with 404, so both cases take the same path —
/// the format of our identifiers is not disclosed and no lookup is attempted.
fn memory_id(value: &str) -> Result<u64, Mem0Error> {
    value
        .parse::<u64>()
        .map_err(|_| Mem0Error::new(StatusCode::NOT_FOUND, "memory not found"))
}

fn entity_value<'a>(metadata_key: &str, request: &'a Mem0AddRequest) -> Option<&'a str> {
    match metadata_key {
        "user_id" => request.user_id.as_deref(),
        "agent_id" => request.agent_id.as_deref(),
        "run_id" => request.run_id.as_deref(),
        "app_id" => request.app_id.as_deref(),
        _ => None,
    }
}

fn conversation_text(messages: &[crate::mem0::dto::Mem0Message]) -> Result<String, Mem0Error> {
    let parts: Vec<&str> = messages
        .iter()
        .map(|message| message.content.trim())
        .filter(|content| !content.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(Mem0Error::invalid_parameter(
            "messages must contain at least one entry with non-empty content",
        ));
    }
    Ok(parts.join("\n"))
}

/// Builds the record metadata for an add.
///
/// The mem0 entity identifiers are persisted under their upstream names, which
/// is what makes `filters: {"user_id": "..."}` answerable: the canonical filter
/// language reads record metadata. They are written after the caller's own
/// metadata so a caller cannot shadow the entity a record was actually filed
/// under and make it invisible to that filter.
fn add_metadata(request: &Mem0AddRequest) -> Result<Option<Value>, Mem0Error> {
    let mut merged = match request.metadata.clone() {
        None => Map::new(),
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(Mem0Error::invalid_parameter(
                "metadata must be a JSON object",
            ))
        }
    };
    for (_, metadata_key) in MEM0_ENTITY_SCOPES {
        if let Some(reference) = entity_value(metadata_key, request) {
            merged.insert(
                metadata_key.to_string(),
                Value::String(reference.to_string()),
            );
        }
    }
    if merged.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(merged)))
    }
}

/// Merges explicit `filters` with any top-level entity parameters.
///
/// mem0's filter language is a conjunction at the root, and `AND` groups are
/// flattened into it, so adding entity conditions to the caller's object keeps
/// both sets of constraints in force.
fn entity_filter(
    explicit: &Option<Value>,
    top_level: [(&str, Option<&str>); 4],
) -> Result<Option<Value>, Mem0Error> {
    let mut merged = match explicit.clone() {
        None => Map::new(),
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(Mem0Error::invalid_parameter("filters must be a JSON object"))
        }
    };
    for (key, value) in top_level {
        if let Some(value) = value {
            merged
                .entry(key.to_string())
                .or_insert_with(|| Value::String(value.to_string()));
        }
    }
    if merged.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(merged)))
    }
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// `GET /v1/ping/`.
///
/// Called by the official clients from their constructor, which requires a 2xx
/// and then reads `org_id`/`project_id`/`user_email` if present. The mem0 project
/// is the SDKWork tenant on this surface, so it is reported as both; the account
/// email is not disclosed here and is reported absent.
pub(crate) async fn ping(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
) -> Result<Json<Mem0PingResponse>, Mem0Error> {
    product(&state)?;
    let context = require_context(context)?;
    let tenant = context.tenant_id.to_string();
    Ok(Json(Mem0PingResponse {
        org_id: Some(tenant.clone()),
        project_id: Some(tenant),
        user_email: None,
    }))
}

/// `POST /v3/memories/add/`.
pub(crate) async fn add_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0AddRequest>,
) -> Result<Json<Mem0AddResult>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;
    let text = conversation_text(&request.messages)?;
    let metadata = add_metadata(&request)?;

    // Entity scopes are registered before the record so a failure here cannot
    // leave a memory that `GET /v1/entities/` does not account for. They are
    // derived metadata, so an entity without a memory is the harmless direction.
    for (entity_kind, metadata_key) in MEM0_ENTITY_SCOPES {
        if let Some(reference) = entity_value(metadata_key, &request) {
            product
                .mem0_register_entity_scope(&context, space_id, entity_kind, reference)
                .await
                .map_err(Mem0Error::from)?;
        }
    }

    let record = product
        .create_memory(
            context.clone(),
            MemoryRecordRequest {
                space_id,
                user_id: context.actor_id,
                scope: MEM0_RECORD_SCOPE.to_string(),
                memory_type: MemoryType::Semantic,
                subject: None,
                predicate: None,
                object_text: Some(text.clone()),
                canonical_text: text,
                summary_text: None,
                language: None,
                sensitivity_level: None,
                expires_at: None,
                metadata,
                tags: None,
            },
        )
        .await
        .map_err(Mem0Error::from)?;

    Ok(Json(Mem0AddResult {
        results: vec![memory_from_record(&record, Some("ADD"), None)],
    }))
}

/// `POST /v3/memories/search/`.
pub(crate) async fn search_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0SearchRequest>,
) -> Result<Json<Mem0SearchResult>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;

    let top_k = match request.top_k {
        None => MEM0_DEFAULT_TOP_K,
        Some(value) if (1..=MEM0_MAX_TOP_K).contains(&value) => value,
        Some(value) => {
            return Err(Mem0Error::invalid_parameter(format!(
                "top_k must be between 1 and {MEM0_MAX_TOP_K} on this surface, received {value}; \
                 the canonical retriever is bounded at {MEM0_MAX_TOP_K} and answering with fewer \
                 results than requested would be indistinguishable from an empty match"
            )))
        }
    };
    let top_k = i32::try_from(top_k)
        .map_err(|_| Mem0Error::invalid_parameter("top_k must fit in a 32-bit integer"))?;

    let filters = entity_filter(
        &request.filters,
        [
            ("user_id", request.user_id.as_deref()),
            ("agent_id", request.agent_id.as_deref()),
            ("run_id", request.run_id.as_deref()),
            ("app_id", request.app_id.as_deref()),
        ],
    )?;

    let retrieval = product
        .create_retrieval(
            context.clone(),
            MemoryRetrievalRequest {
                query: request.query,
                space_ids: vec![space_id],
                actor_id: None,
                retrieval_profile_id: None,
                memory_types: None,
                filters,
                top_k,
                context_budget_tokens: MEM0_CONTEXT_BUDGET_TOKENS,
                show_expired: request.show_expired,
                threshold: request.threshold,
                explain: request.explain,
                include_trace: None,
            },
        )
        .await
        .map_err(Mem0Error::from)?;

    let results = retrieval
        .hits
        .into_iter()
        .filter_map(|hit| {
            let record = hit.memory.as_ref()?;
            Some(memory_from_record(
                record,
                None,
                hit.fused_score.or(hit.raw_score),
            ))
        })
        .collect();

    Ok(Json(Mem0SearchResult { results }))
}

/// `POST /v3/memories/`.
pub(crate) async fn list_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Query(params): Mem0Query<Mem0ListParams>,
    Mem0Json(request): Mem0Json<Mem0ListRequest>,
) -> Result<Json<Mem0PaginatedMemories>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;

    // mem0 pages by ordinal against its own storage; the canonical listing is
    // keyset-ordered and its cursor is an opaque server token. Only the first
    // page is addressable, so a later page is refused by name: silently
    // re-serving page one would look like a successful, empty-ended walk.
    if let Some(page) = params.page {
        if page != 1 {
            return Err(Mem0Error::unsupported(
                "page-based pagination",
                format!(
                    "page {page} was requested; this surface returns the first page only, because \
                     the canonical listing is cursor-ordered and its cursor is not an ordinal. \
                     Narrow the result set with filters instead."
                ),
            ));
        }
    }

    if let Some(filters) = request.filters.as_ref() {
        let is_empty = filters.as_object().is_none_or(|object| object.is_empty());
        if !is_empty {
            return Err(Mem0Error::unsupported(
                "filtered listing",
                "the canonical listing has no metadata filter; use POST /v3/memories/search/ with \
                 `filters`, which applies them to candidate selection",
            ));
        }
    }
    for (key, value) in [
        ("user_id", &request.user_id),
        ("agent_id", &request.agent_id),
        ("run_id", &request.run_id),
        ("app_id", &request.app_id),
    ] {
        if value.is_some() {
            return Err(Mem0Error::unsupported(
                "filtered listing",
                format!(
                    "the `{key}` filter cannot be applied by the canonical listing; use POST \
                     /v3/memories/search/ with `filters` instead"
                ),
            ));
        }
    }

    let page_size = params
        .page_size
        .map(|value| i32::try_from(value).map_err(|_| invalid_page_size()))
        .transpose()?;

    let space_id = space_id(&product, &context).await?;
    let page = product
        .list_memories(
            context.clone(),
            ListMemoriesQuery {
                q: None,
                cursor: None,
                page_size,
                space_id: Some(space_id),
                show_expired: None,
            },
        )
        .await
        .map_err(Mem0Error::from)?;
    let count = product
        .mem0_record_count(&context, space_id)
        .await
        .map_err(Mem0Error::from)?;

    Ok(Json(Mem0PaginatedMemories {
        count: i64::try_from(count).unwrap_or(i64::MAX),
        next: None,
        previous: None,
        results: page
            .items
            .iter()
            .map(|record| memory_from_record(record, None, None))
            .collect(),
    }))
}

fn invalid_page_size() -> Mem0Error {
    Mem0Error::invalid_parameter(format!(
        "page_size must be between 1 and {MAX_LIST_PAGE_SIZE}"
    ))
}

/// `GET /v1/memories/{memory_id}/`.
pub(crate) async fn get_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(raw_memory_id): Path<String>,
) -> Result<Json<Mem0Memory>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;
    let record = product
        .retrieve_memory(context.clone(), memory_id(&raw_memory_id)?, space_id)
        .await
        .map_err(Mem0Error::from)?;
    Ok(Json(memory_from_record(&record, None, None)))
}

/// `PUT /v1/memories/{memory_id}/`.
pub(crate) async fn update_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(raw_memory_id): Path<String>,
    Mem0Json(request): Mem0Json<Mem0UpdateRequest>,
) -> Result<Json<Mem0Memory>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;

    // Refused rather than dropped. The canonical update rewrites text and
    // metadata only; answering 200 to a request that asked for a timestamp or
    // an expiry change would report a mutation that never happened.
    if request.timestamp.is_some() {
        return Err(Mem0Error::unsupported(
            "timestamp update",
            "the canonical record keeps the store's own creation and update instants and does not \
             accept a caller-supplied timestamp",
        ));
    }
    if request.expiration_date.is_some() {
        return Err(Mem0Error::unsupported(
            "expiration_date update",
            "the canonical update operation cannot change a record's expiry; set it at write time \
             on a surface that exposes `expiresAt`",
        ));
    }
    if request.text.is_none() && request.metadata.is_none() {
        return Err(Mem0Error::invalid_parameter(
            "at least one of text or metadata must be provided",
        ));
    }

    let space_id = space_id(&product, &context).await?;
    let record = product
        .update_memory(
            context.clone(),
            memory_id(&raw_memory_id)?,
            space_id,
            MemoryRecordPatch {
                canonical_text: request.text,
                subject: None,
                summary_text: None,
                metadata: request.metadata,
            },
        )
        .await
        .map_err(Mem0Error::from)?;
    Ok(Json(memory_from_record(&record, None, None)))
}

/// `DELETE /v1/memories/{memory_id}/`.
pub(crate) async fn delete_memory(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(raw_memory_id): Path<String>,
    Mem0Query(params): Mem0Query<Mem0DeleteParams>,
) -> Result<Json<Mem0MutationAck>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;

    // `delete_linked` walks the superseded chain transitively. The canonical
    // single delete removes exactly the addressed record, so a request for the
    // walk is refused rather than answered with a narrower deletion than asked.
    if params.delete_linked == Some(true) {
        return Err(Mem0Error::unsupported(
            "delete_linked",
            "this surface deletes only the addressed memory; superseded predecessors are removed \
             explicitly",
        ));
    }

    let space_id = space_id(&product, &context).await?;
    product
        .delete_memory(context.clone(), memory_id(&raw_memory_id)?, space_id)
        .await
        .map_err(Mem0Error::from)?;
    Ok(Json(Mem0MutationAck {
        message: "Memory deleted successfully".to_string(),
    }))
}

/// `DELETE /v1/memories/`.
pub(crate) async fn delete_all_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Query(params): Mem0Query<Mem0DeleteAllParams>,
) -> Result<Json<Mem0MutationAck>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;

    // The canonical bulk deletion is space-scoped. Ignoring an entity filter
    // would delete far more than the caller asked for, so any filter is refused.
    for (key, value) in [
        ("user_id", &params.user_id),
        ("agent_id", &params.agent_id),
        ("run_id", &params.run_id),
        ("app_id", &params.app_id),
    ] {
        if value.is_some() {
            return Err(Mem0Error::unsupported(
                "filtered delete_all",
                format!(
                    "the `{key}` filter cannot be applied by the canonical bulk deletion, which is \
                     space-scoped; delete the matching memories individually"
                ),
            ));
        }
    }

    let space_id = space_id(&product, &context).await?;
    let deleted = product
        .delete_all_memories(
            context.clone(),
            DeleteAllMemoriesRequest {
                space_id,
                user_id: None,
            },
        )
        .await
        .map_err(Mem0Error::from)?;
    Ok(Json(Mem0MutationAck {
        message: format!("{} memories deleted successfully", deleted.deleted_count),
    }))
}

/// `GET /v1/memories/{memory_id}/history/`.
///
/// This endpoint answers with an event log, not a content diff. `old_memory` and
/// `new_memory` are therefore always null: the canonical journal records that a
/// mutation was accepted, by whom, and when. Persisting record text alongside it
/// would create a second copy of personal data under a different retention and
/// erasure path, which the privacy contract does not sanction for an audit
/// trail. The events themselves — creation, updates, deletion — are reported
/// exactly as recorded.
///
/// mem0 declares `event` as a closed `ADD`/`UPDATE`/`DELETE` enum, so an audit
/// action outside [`MEM0_AUDIT_ACTION_EVENTS`] has no representation. Such a
/// trail is refused by name rather than answered with the representable subset:
/// a partial event log would be indistinguishable from a complete one, and
/// mem0's `history` is defined as the memory's whole history.
pub(crate) async fn memory_history(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Path(raw_memory_id): Path<String>,
) -> Result<Json<Vec<Mem0HistoryEntry>>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;
    let id = memory_id(&raw_memory_id)?;

    let history = product
        .mem0_memory_history(&context, space_id, id, MEM0_HISTORY_PAGE_SIZE)
        .await
        .map_err(Mem0Error::from)?;

    let mut entries = Vec::with_capacity(history.events.len());
    for event in &history.events {
        let entry = history_entry_from_audit(
            &event.audit_id,
            &raw_memory_id,
            &event.action,
            &event.created_at,
            history.user_id.as_deref(),
        )
        .ok_or_else(|| {
            Mem0Error::unsupported(
                "memory history event",
                format!(
                    "the journal for this memory holds `{}`, which mem0's `event` field cannot \
                     name: the platform schema declares it as a closed ADD/UPDATE/DELETE enum. The \
                     full journal for this record is available on the canonical surface at \
                     /mem/v3/api/memory/events",
                    event.action
                ),
            )
        })?;
        entries.push(entry);
    }
    Ok(Json(entries))
}

/// `GET /v1/entities/`.
///
/// mem0's entity scopes are registered as canonical graph entities when a memory
/// is added through this wire, so this reports the scopes memories actually
/// exist for instead of a synthesized list.
///
/// `count` is the exact number of entity scopes in the caller's compatibility
/// space: the platform schema defines it as the total number of *matching*
/// entities, not the page length, so a page that had to be truncated cannot be
/// reported under it. The listing is therefore bounded and a truncated result is
/// refused by name. The platform schema also declares optional project-wide
/// `total_users`/`total_agents`/`total_apps`/`total_runs`; those are not emitted,
/// because this listing is space-scoped and a space-derived number under a
/// project-wide name would be a false total.
pub(crate) async fn list_entities(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Query(params): Mem0Query<Mem0EntityParams>,
) -> Result<Json<Mem0EntityList>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;

    let page_size = match params.page_size {
        None => MAX_LIST_PAGE_SIZE,
        Some(value) => i32::try_from(value).map_err(|_| invalid_page_size())?,
    };

    let page = product
        .list_entities(
            context.clone(),
            ListEntitiesQuery {
                tenant_id: context.tenant_id,
                space_id: Some(space_id),
                entity_type: None,
                status: None,
                cursor: None,
                page_size: Some(page_size),
            },
        )
        .await
        .map_err(Mem0Error::from)?;

    if page.page_info.has_more == Some(true) {
        return Err(Mem0Error::unsupported(
            "page-based entity listing",
            format!(
                "this compatibility space holds more entity scopes than the {page_size}-entry \
                 listing bound, and the platform `count` field is defined as the total number of \
                 matching entities rather than the page length, so a truncated page cannot be \
                 answered"
            ),
        ));
    }

    let owner = context.tenant_id.to_string();
    let results: Vec<Mem0EntityScope> = page
        .items
        .iter()
        .map(|entity| entity_scope_from_record(entity, &owner))
        .collect();
    Ok(Json(Mem0EntityList {
        count: i64::try_from(results.len()).unwrap_or(i64::MAX),
        next: None,
        previous: None,
        results,
    }))
}

// ---------------------------------------------------------------------------
// Feedback and batch
// ---------------------------------------------------------------------------

/// Upstream's `maxItems` for one batch (`external/mem0/docs/openapi.json`,
/// `/v1/batch/` both operations).
const MEM0_MAX_BATCH_ITEMS: usize = 1000;

/// The feedback vocabulary upstream declares as a closed enum.
const MEM0_FEEDBACK_VALUES: [&str; 3] = ["POSITIVE", "NEGATIVE", "VERY_NEGATIVE"];

/// `POST /v1/feedback/`.
///
/// Maps onto the canonical feedback record with `targetType = "memory"`, which
/// is the only target kind mem0's feedback can address. The canonical write
/// resolves `targetId` and fails with `memory not found` when it names nothing,
/// so the 404 the clients expect is produced by the shared path rather than by a
/// second existence check here.
///
/// `feedback` is refused when absent. Upstream models `feedback: null` as
/// *withdrawing* the feedback, and the canonical record has no value for "no
/// feedback" — it is only ever written, never cleared. Answering 200 to a
/// withdrawal that did not happen would report a mutation that never occurred.
pub(crate) async fn submit_feedback(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0FeedbackRequest>,
) -> Result<Json<Mem0FeedbackResult>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let target_id = memory_id(&request.memory_id)?;

    let feedback = request.feedback.as_deref().ok_or_else(|| {
        Mem0Error::unsupported(
            "feedback withdrawal",
            "upstream treats an omitted `feedback` as withdrawing the feedback, but the canonical \
             feedback record has no value for \"no feedback\" and is never cleared; send one of \
             POSITIVE, NEGATIVE, or VERY_NEGATIVE",
        )
    })?;
    if !MEM0_FEEDBACK_VALUES.contains(&feedback) {
        return Err(Mem0Error::invalid_parameter(format!(
            "feedback must be one of {}, received `{feedback}`",
            MEM0_FEEDBACK_VALUES.join(", ")
        )));
    }

    let created = product
        .create_feedback(
            context.clone(),
            MemoryFeedbackRequest {
                target_type: "memory".to_string(),
                target_id,
                feedback_type: feedback.to_string(),
                rating: None,
                comment: request.feedback_reason,
                metadata: None,
            },
        )
        .await
        .map_err(Mem0Error::from)?;

    Ok(Json(Mem0FeedbackResult {
        id: created.feedback_id.to_string(),
        // Read back from the stored row rather than echoing the request: the
        // value reported is the one the store now holds.
        feedback: Some(created.feedback_type),
        feedback_reason: None,
    }))
}

/// One batch entry resolved to the addressing this service works in.
struct Mem0BatchTarget {
    /// The `memory_id` exactly as the caller sent it, for error messages.
    raw_id: String,
    id: u64,
    text: Option<String>,
    metadata: Option<Value>,
}

/// Validates a whole batch and proves every entry exists **before** any of it is
/// applied.
///
/// This is the load-bearing part of the batch contract. Upstream's 200 carries
/// only `{"message": "Successfully updated N memories"}` — there is no per-item
/// result channel — so a batch that partially applied and then failed could not
/// be reported as anything but a success. Everything that can be checked without
/// mutating is therefore checked first:
///
/// * every `memory_id` must name something the canonical store can address;
/// * an update entry must carry `text` or `metadata` (the canonical update
///   refuses an empty patch, and one such entry must not be discovered after its
///   predecessors were already written);
/// * duplicate `memory_id`s are collapsed to their first occurrence. Repeating
///   an id is not an upstream error, and without collapsing, a repeated delete
///   would fail *after* the first occurrence had already removed the record.
///
/// What remains afterwards is a genuine race (the record vanishes between the
/// check and the write). Those failures surface as errors, never as a count.
async fn resolve_batch_targets(
    product: &OpenMemoryService,
    context: &MemoryOpenApiRequestContext,
    space_id: u64,
    items: Vec<Mem0BatchItem>,
    require_payload: bool,
) -> Result<Vec<Mem0BatchTarget>, Mem0Error> {
    if items.len() > MEM0_MAX_BATCH_ITEMS {
        return Err(Mem0Error::invalid_parameter(format!(
            "memories must hold at most {MEM0_MAX_BATCH_ITEMS} entries, received {}",
            items.len()
        )));
    }

    let mut targets: Vec<Mem0BatchTarget> = Vec::with_capacity(items.len());
    for item in items {
        let id = memory_id(&item.memory_id)?;
        if require_payload && item.text.is_none() && item.metadata.is_none() {
            return Err(Mem0Error::invalid_parameter(format!(
                "memory {} must carry `text` or `metadata` to be updated",
                item.memory_id
            )));
        }
        if targets.iter().any(|target| target.id == id) {
            continue;
        }
        targets.push(Mem0BatchTarget {
            raw_id: item.memory_id,
            id,
            text: item.text,
            metadata: item.metadata,
        });
    }

    for target in &targets {
        product
            .retrieve_memory(context.clone(), target.id, space_id)
            .await
            .map_err(|_| Mem0Error::new(StatusCode::NOT_FOUND, format!("memory {} not found", target.raw_id)))?;
    }

    Ok(targets)
}

/// `PUT /v1/batch/`.
pub(crate) async fn batch_update_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0BatchRequest>,
) -> Result<Json<Mem0BatchAck>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;

    let targets =
        resolve_batch_targets(&product, &context, space_id, request.memories, true).await?;

    let mut updated = 0usize;
    for target in &targets {
        product
            .update_memory(
                context.clone(),
                target.id,
                space_id,
                MemoryRecordPatch {
                    canonical_text: target.text.clone(),
                    subject: None,
                    summary_text: None,
                    metadata: target.metadata.clone(),
                },
            )
            .await
            .map_err(|error| {
                // Reached only when the record changed underneath the pre-flight
                // check. The count is reported because by now it is the only
                // truthful thing this surface can say. `detail` is the
                // caller-safe authored text (`MemoryServiceError` docs);
                // raw store causes are masked before they ever reach it.
                Mem0Error::new(
                    StatusCode::CONFLICT,
                    format!(
                        "memory {} failed to update after {updated} of {} entries were written; \
                         the batch is not atomic and was not rolled back: {}",
                        target.raw_id,
                        targets.len(),
                        error.detail
                    ),
                )
            })?;
        updated += 1;
    }

    Ok(Json(Mem0BatchAck {
        message: format!("Successfully updated {updated} memories"),
    }))
}

/// `DELETE /v1/batch/`.
pub(crate) async fn batch_delete_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0BatchRequest>,
) -> Result<Json<Mem0BatchAck>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;

    let targets =
        resolve_batch_targets(&product, &context, space_id, request.memories, false).await?;

    let mut deleted = 0usize;
    for target in &targets {
        product
            .delete_memory(context.clone(), target.id, space_id)
            .await
            .map_err(|error| {
                Mem0Error::new(
                    StatusCode::CONFLICT,
                    format!(
                        "memory {} failed to delete after {deleted} of {} entries were removed; \
                         the batch is not atomic and was not rolled back: {}",
                        target.raw_id,
                        targets.len(),
                        error.detail
                    ),
                )
            })?;
        deleted += 1;
    }

    Ok(Json(Mem0BatchAck {
        message: format!("Successfully deleted {deleted} memories"),
    }))
}
