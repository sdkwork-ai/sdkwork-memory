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
    MemoryRetrievalRequest, MemoryServiceError, MemoryServiceErrorKind, MemoryType,
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
    // Upstream declares no count cap, but an unbounded `Vec` of one-line
    // messages would let the join fan the request into one giant record under
    // this surface's own control; the body limit already bounds bytes, and
    // this bounds the entry count alongside it. 1000 entries is far above any
    // real conversation the official clients assemble.
    const MAX_ADD_MESSAGES: usize = 1000;
    if messages.len() > MAX_ADD_MESSAGES {
        return Err(Mem0Error::invalid_parameter(format!(
            "messages must hold at most {MAX_ADD_MESSAGES} entries, received {}",
            messages.len()
        )));
    }
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
fn add_metadata(
    request: &Mem0AddRequest,
    entity_references: &[(&'static str, &'static str, String)],
) -> Result<Option<Value>, Mem0Error> {
    let mut merged = match request.metadata.clone() {
        None => Map::new(),
        Some(Value::Object(map)) => map,
        Some(_) => {
            return Err(Mem0Error::invalid_parameter(
                "metadata must be a JSON object",
            ))
        }
    };
    for (_, metadata_key, reference) in entity_references {
        merged.insert(
            metadata_key.to_string(),
            Value::String(reference.clone()),
        );
    }
    if merged.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(merged)))
    }
}

/// The identity axes this add is filed under, from both spellings: the
/// top-level v1/v2 names and the v3 `filters` object. For one axis given in
/// both, the top-level value wins, matching how the write merges caller
/// metadata (entity identifiers are written after it).
///
/// Keys inside `filters` that are not identity axes are refused: an unknown
/// key has no canonical meaning on this surface, and dropping it silently
/// would repeat the exact false success this wire refuses to produce.
fn add_entity_references(
    request: &Mem0AddRequest,
) -> Result<Vec<(&'static str, &'static str, String)>, Mem0Error> {
    if let Some(filters) = &request.filters {
        let object = filters.as_object().ok_or_else(|| {
            Mem0Error::invalid_parameter("filters must be a JSON object")
        })?;
        for key in object.keys() {
            if !MEM0_ENTITY_SCOPES.iter().any(|(_, metadata_key)| *metadata_key == key.as_str()) {
                return Err(Mem0Error::invalid_parameter(format!(
                    "filters key `{key}` is not an identity axis on this surface; file caller \
                     metadata through `metadata` and scope the memory with the identity filters"
                )));
            }
        }
    }

    let mut references: Vec<(&'static str, &'static str, String)> = Vec::new();
    for (entity_kind, metadata_key) in MEM0_ENTITY_SCOPES {
        if let Some(Value::Object(map)) = &request.filters {
            if let Some(value) = map.get(metadata_key).and_then(Value::as_str) {
                references.push((entity_kind, metadata_key, value.to_string()));
            }
        }
        if let Some(reference) = entity_value(metadata_key, request) {
            references.retain(|(existing_kind, _, _)| *existing_kind != entity_kind);
            references.push((entity_kind, metadata_key, reference.to_string()));
        }
    }
    Ok(references)
}

/// Refuses a request field this surface cannot honour, by name.
///
/// The field is declared on the request DTO precisely so presence is
/// observable; answering 200 with the field dropped would be a false success.
fn refuse_unsupported_field(
    present: Option<&Value>,
    field: &str,
    reason: &str,
) -> Result<(), Mem0Error> {
    if present.is_some() {
        return Err(Mem0Error::unsupported(field, reason.to_string()));
    }
    Ok(())
}

/// Merges explicit `filters` with any top-level entity parameters.
///
/// mem0's filter language is a conjunction at the root, and `AND` groups are
/// flattened into it, so every axis present in either spelling stays in force.
/// When one axis arrives in both spellings with different values, the
/// `filters` value wins (`or_insert_with` keeps the entry that is already
/// there); the official clients send an axis through one spelling only, so
/// the precedence resolves a caller contradiction rather than silently
/// dropping a constraint the caller asked for twice.
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
        // The JavaScript client requires this literal; see `Mem0PingResponse`.
        status: "ok",
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

    // Fields this surface cannot honour are refused by name, never dropped:
    // answering 200 with a field silently discarded is a false success.
    refuse_unsupported_field(
        request.timestamp.as_ref(),
        "timestamp",
        "the canonical record keeps the store's own creation and update instants and does not \
         accept a caller-supplied timestamp",
    )?;
    refuse_unsupported_field(
        request.custom_categories.as_ref(),
        "custom_categories",
        "category vocabularies are deployment configuration, not a per-request switch",
    )?;
    refuse_unsupported_field(
        request.custom_instructions.as_ref(),
        "custom_instructions",
        "extraction instructions are deployment configuration, and this surface files the \
         literal text without an extraction step",
    )?;
    refuse_unsupported_field(
        request.agent_custom_instructions.as_ref(),
        "agent_custom_instructions",
        "extraction instructions are deployment configuration, and this surface files the \
         literal text without an extraction step",
    )?;
    refuse_unsupported_field(
        request.structured_data_schema.as_ref(),
        "structured_data_schema",
        "structured extraction is not a write shape on this surface; the literal conversation \
         text is what gets filed",
    )?;
    refuse_unsupported_field(
        request.immutable.as_ref(),
        "immutable",
        "this surface has no immutability flag; canonical records are rewritten by the ordinary \
         update operation",
    )?;
    refuse_unsupported_field(
        request.includes.as_ref(),
        "includes",
        "extraction instructions are deployment configuration, and this surface files the \
         literal text without an extraction step",
    )?;
    refuse_unsupported_field(
        request.excludes.as_ref(),
        "excludes",
        "extraction instructions are deployment configuration, and this surface files the \
         literal text without an extraction step",
    )?;
    refuse_unsupported_field(
        request.enable_graph.as_ref(),
        "enable_graph",
        "the canonical record has no graph-memory variant; entity scopes are registered on \
         every write",
    )?;
    refuse_unsupported_field(
        request.output_format.as_ref(),
        "output_format",
        "a response-shape switch would change the response contract, and the mem0 memory shape \
         on this surface is fixed",
    )?;
    refuse_unsupported_field(
        request.prompt_profile_id.as_ref(),
        "prompt_profile_id",
        "the extraction model profile is deployment configuration, not a per-request switch",
    )?;
    refuse_unsupported_field(
        request.temporal_reasoning.as_ref(),
        "temporal_reasoning",
        "temporal reasoning derives caller-supplied instants, and the canonical record keeps \
         the store's own creation and update instants",
    )?;
    refuse_unsupported_field(
        request.timezone.as_ref(),
        "timezone",
        "a timezone only matters for interpreting caller-supplied instants, and this surface \
         stamps its own instead",
    )?;
    refuse_unsupported_field(
        request.observation_datetime.as_ref(),
        "observation_datetime",
        "the canonical record keeps the store's own creation and update instants and does not \
         accept a caller-supplied observation instant",
    )?;
    refuse_unsupported_field(
        request.observation_date.as_ref(),
        "observation_date",
        "the canonical record keeps the store's own creation and update instants and does not \
         accept a caller-supplied observation instant",
    )?;
    let expiration_date = match request.expiration_date.as_ref() {
        None => None,
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.clone()),
        Some(Value::String(_)) => {
            return Err(Mem0Error::invalid_parameter(
                "expiration_date must be a non-empty ISO-8601 string",
            ))
        }
        Some(_) => {
            return Err(Mem0Error::invalid_parameter(
                "expiration_date must be an ISO-8601 string",
            ))
        }
    };

    let entity_references = add_entity_references(&request)?;
    let metadata = add_metadata(&request, &entity_references)?;

    // Entity scopes are registered before the record so a failure here cannot
    // leave a memory that `GET /v1/entities/` does not account for. They are
    // derived metadata, so an entity without a memory is the harmless direction.
    for (entity_kind, _, reference) in &entity_references {
        product
            .mem0_register_entity_scope(&context, space_id, entity_kind, reference)
            .await
            .map_err(Mem0Error::from)?;
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
                expires_at: expiration_date,
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

    // Fields this surface cannot honour are refused by name, never dropped.
    refuse_unsupported_field(
        request.fields.as_ref(),
        "fields",
        "a projection changes the response contract, and the mem0 memory shape on this surface \
         is fixed",
    )?;
    refuse_unsupported_field(
        request.categories.as_ref(),
        "categories",
        "mem0's category model has no counterpart in the canonical record; the response reports \
         `categories` as absent",
    )?;
    refuse_unsupported_field(
        request.reference_date.as_ref(),
        "reference_date",
        "relative-to-a-reference-date expiry semantics are undefined on this surface",
    )?;
    if request.latest_only == Some(true) {
        return Err(Mem0Error::unsupported(
            "latest_only",
            "latest-version-only retrieval is a retrieval-profile decision, not a per-request \
             switch",
        ));
    }
    if request.keyword_search.is_some() {
        return Err(Mem0Error::unsupported(
            "keyword_search",
            "retriever-kind selection is the retrieval profile's decision; lexical matching is \
             already part of the default ranking",
        ));
    }

    let mut filters = entity_filter(
        &request.filters,
        [
            ("user_id", request.user_id.as_deref()),
            ("agent_id", request.agent_id.as_deref()),
            ("run_id", request.run_id.as_deref()),
            ("app_id", request.app_id.as_deref()),
        ],
    )?;
    // `metadata` conditions are exact matches against record metadata — the
    // same thing the canonical filter conjunction expresses with a flat
    // `{key: value}` entry — so they merge into it instead of being dropped.
    if let Some(metadata) = &request.metadata {
        let object = metadata
            .as_object()
            .ok_or_else(|| Mem0Error::invalid_parameter("metadata must be a JSON object"))?;
        let mut merged = filters.take().and_then(|value| value.as_object().cloned()).unwrap_or_default();
        for (key, value) in object {
            if matches!(key.as_str(), "AND" | "OR" | "NOT") {
                return Err(Mem0Error::invalid_parameter(format!(
                    "metadata key `{key}` collides with the filter grammar; express that \
                     condition through `filters` instead"
                )));
            }
            merged.insert(key.clone(), value.clone());
        }
        if !merged.is_empty() {
            filters = Some(Value::Object(merged));
        }
    }

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

    // Fields this surface cannot honour are refused by name, never dropped: a
    // silently ignored date window or category filter would report a smaller
    // result set as complete.
    for (field, value) in [
        ("start_date", request.start_date.as_ref()),
        ("end_date", request.end_date.as_ref()),
        ("categories", request.categories.as_ref()),
        ("keywords", request.keywords.as_ref()),
    ] {
        refuse_unsupported_field(
            value,
            field,
            "the canonical listing has no date, category, or keyword narrowing; use POST \
             /v3/memories/search/ with `filters`, which applies conditions to candidate selection",
        )?;
    }
    refuse_unsupported_field(
        request.fields.as_ref(),
        "fields",
        "a projection changes the response contract, and the mem0 memory shape on this surface \
         is fixed",
    )?;
    if request.latest_only == Some(true) {
        return Err(Mem0Error::unsupported(
            "latest_only",
            "latest-version-only listing is a retrieval-profile decision, not a per-request switch",
        ));
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
                show_expired: request.show_expired,
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
    // would delete far more than the caller asked for, so any filter is refused
    // — in **every** spelling the clients put on the wire, not only the declared
    // one. `MemoryClient.delete_all(filters={...})` sends a single `filters`
    // query parameter whose value is a `str()` of the dict; reading only the four
    // top-level names is what let that call delete the whole space and report
    // success.
    if let Some((key, value)) = params.requested_filter() {
        let received: String = value.chars().take(200).collect();
        return Err(Mem0Error::unsupported(
            "filtered delete_all",
            format!(
                "the `{key}` filter cannot be applied by the canonical bulk deletion, which is \
                 space-scoped; delete the matching memories individually (received `{received}`)"
            ),
        ));
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
///
/// The same completeness rule refuses a *truncated* trail: the compatibility
/// history is a single bounded page, and a journal longer than that bound means
/// the page is a prefix, not the history. Like `GET /v1/entities/`, the response
/// is refused by name instead of reporting a subset as complete.
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

    if history.truncated {
        return Err(Mem0Error::unsupported(
            "memory history",
            format!(
                "this memory's journal holds more events than the {MEM0_HISTORY_PAGE_SIZE}-entry \
                 history bound, and mem0's `history` is defined as the memory's whole event log \
                 rather than a page of it, so a truncated log cannot be answered. The full \
                 journal for this record is available on the canonical surface at \
                 /mem/v3/api/memory/events"
            ),
        ));
    }

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

    // Refused by name, mirroring `POST /v3/memories/?page=`: the canonical
    // listing is cursor-ordered and only its first page is addressable.
    if let Some(page) = params.page {
        if page != 1 {
            return Err(Mem0Error::unsupported(
                "page-based pagination",
                format!(
                    "page {page} was requested; this surface returns the first page only, because \
                     the canonical listing is cursor-ordered and its cursor is not an ordinal"
                ),
            ));
        }
    }

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

/// Validates a whole batch's shape **before** any of it is applied.
///
/// Upstream's 200 carries only `{"message": "Successfully updated N memories"}`
/// — there is no per-item result channel. Everything that can be checked
/// without touching the store is therefore checked first:
///
/// * every `memory_id` must name an identity the canonical store can address;
/// * an update entry must carry `text` or `metadata` (the canonical update
///   refuses an empty patch);
/// * duplicate `memory_id`s are collapsed to their first occurrence. Repeating
///   an id is not an upstream error, and without collapsing, a repeated delete
///   would fail *after* the first occurrence had already removed the record.
///
/// Existence is proven by the service layer's batched precheck read inside
/// [`OpenMemoryService::update_memories_batch`], which applies the resolvable
/// entries atomically (each with its own mutation journal) and reports a
/// per-item `NotFound` for the rest, results in input order. The handler turns
/// the first failing slot into the batch's single error, naming the entry as
/// the caller sent it.
fn resolve_batch_targets(
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

    Ok(targets)
}

/// The batch's single error for its first failing slot, in the wire's own
/// dialect.
///
/// `NotFound` keeps the spelling the per-entry pre-flight used to answer with:
/// 404 naming the id exactly as the caller sent it. Every other cause (storage
/// outage, authorization, quota) keeps its own kind and the count-honest
/// conflict message, instead of masquerading as a vanished memory. `detail` is
/// the caller-safe authored text (`MemoryServiceError` docs); raw store causes
/// are masked before they ever reach it.
fn batch_slot_error(
    target: &Mem0BatchTarget,
    error: MemoryServiceError,
    verb: &str,
    participle: &str,
    applied: usize,
    total: usize,
) -> Mem0Error {
    match error.kind {
        MemoryServiceErrorKind::NotFound => Mem0Error::new(
            StatusCode::NOT_FOUND,
            format!("memory {} not found", target.raw_id),
        ),
        _ => Mem0Error::new(
            StatusCode::CONFLICT,
            format!(
                "memory {} failed to {verb} after {applied} of {total} entries were {participle}; \
                 the batch is not atomic and was not rolled back: {}",
                target.raw_id, error.detail
            ),
        ),
    }
}

/// `PUT /v1/batch/`.
///
/// One service-level batched call does the whole thing: a single space
/// authorization, one batched precheck read of every addressed record, and one
/// atomic (individually journalled) update per entry, results in input order.
/// The first failing slot becomes the batch's single error — 404 naming the
/// entry the caller sent when the store has no such memory, the count-honest
/// conflict message for every other cause — and the entries before it stay
/// applied, exactly as the message says.
pub(crate) async fn batch_update_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0BatchRequest>,
) -> Result<Json<Mem0BatchAck>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;

    let targets = resolve_batch_targets(request.memories, true)?;

    let updates = targets
        .iter()
        .map(|target| {
            (
                target.id,
                MemoryRecordPatch {
                    canonical_text: target.text.clone(),
                    subject: None,
                    summary_text: None,
                    metadata: target.metadata.clone(),
                },
            )
        })
        .collect();

    let mut updated = 0usize;
    for (target, result) in targets.iter().zip(
        product
            .update_memories_batch(context.clone(), space_id, updates)
            .await,
    ) {
        match result {
            Ok(_) => updated += 1,
            Err(error) => {
                return Err(batch_slot_error(
                    target,
                    error,
                    "update",
                    "written",
                    updated,
                    targets.len(),
                ))
            }
        }
    }

    Ok(Json(Mem0BatchAck {
        message: format!("Successfully updated {updated} memories"),
    }))
}

/// `DELETE /v1/batch/`.
///
/// Deletion has no service-level batch, so the entries are removed one by one
/// with the same per-item error mapping as the update batch: the first failing
/// slot is the batch's single error, and the entries before it stay removed.
pub(crate) async fn batch_delete_memories(
    Extension(state): Extension<OpenState>,
    context: Option<Extension<MemoryOpenApiRequestContext>>,
    Mem0Json(request): Mem0Json<Mem0BatchRequest>,
) -> Result<Json<Mem0BatchAck>, Mem0Error> {
    let product = product(&state)?;
    let context = require_context(context)?;
    let space_id = space_id(&product, &context).await?;

    let targets = resolve_batch_targets(request.memories, false)?;

    let mut deleted = 0usize;
    for target in &targets {
        match product
            .delete_memory(context.clone(), target.id, space_id)
            .await
        {
            Ok(()) => deleted += 1,
            Err(error) => {
                return Err(batch_slot_error(
                    target,
                    error,
                    "delete",
                    "removed",
                    deleted,
                    targets.len(),
                ))
            }
        }
    }

    Ok(Json(Mem0BatchAck {
        message: format!("Successfully deleted {deleted} memories"),
    }))
}

// ---------------------------------------------------------------------------
// `/v2/` refusals
//
// Each of these is a path the official clients build verbatim and this surface
// does not implement. They are registered for two reasons, and both are load
// bearing:
//
// * A declared prefix has to carry real paths
//   (`mem0_wire_context_selector_contract`), or `/v2/` would be a declaration
//   that exempts a path space nobody serves.
// * A caller reaching them must get the mem0 failure dialect. Before `/v2/` was
//   declared, these calls were answered by the framework's surface classifier
//   *ahead of* both mem0 bridges: `401` with `application/problem+json`, which
//   the Python client surfaces as raw text because `mem0/client/utils.py` only
//   unwraps `detail` when the content type starts with `application/json`.
//
// Every handler below takes no body and reads no parameter. That is deliberate,
// not an omission: the answer does not depend on the request, and parsing it
// would suggest a partial honour that does not exist.
// ---------------------------------------------------------------------------

/// `DELETE /v2/entities/{entity_type}/{entity_id}/`.
///
/// The Python client's `delete_users` reaches this once per user scope. It is
/// not a single-record delete: mem0 erases the *scope* — the entity together
/// with the memories filed under it. This service has no entity-scope deletion
/// at all (the canonical entity route is read/patch only), so the call could only
/// be faked by deleting the scope's memories and leaving the scope behind, or by
/// deleting nothing and reporting success. Either is indistinguishable from an
/// erasure that did not happen, which is the one outcome a delete must never
/// report.
pub(crate) async fn refuse_v2_entity_delete() -> Mem0Error {
    Mem0Error::unsupported(
        "entity deletion",
        "mem0 deletes an entity scope together with the memories filed under it, and this service \
         has no entity-scope deletion (the canonical entity route is read/patch only). Delete the \
         scope's memories individually on the canonical surface \
         (/mem/v3/api/memory/memories/{memoryId}) instead of calling an operation that would have \
         to report an erasure it did not perform",
    )
}

/// `GET /v2/entities/{entity_type}/{entity_id}/profile/`.
///
/// mem0's "profile" is text mem0's platform *derives* from a scope's memories and
/// stores beside them. This service stores records, retrievals, and context
/// packs; it neither generates nor persists a derived profile. Returning the
/// scope's raw memories under a `profile` key would present source records as
/// derived text, and nothing in the response would let a caller tell them apart.
pub(crate) async fn refuse_v2_entity_profile() -> Mem0Error {
    Mem0Error::unsupported(
        "entity profile retrieval",
        "mem0's profile is text derived from a scope's memories and stored by mem0's platform; \
         this service does not generate or store profiles. The records such a profile would \
         summarise are readable on the canonical surface (/mem/v3/api/memory/memories)",
    )
}

/// `POST /v2/profiles/jobs/`.
///
/// Both `generate_profile` and `sample_profiles` post here — they differ in the
/// filters they send, not in the resource they create. This service generates no
/// profiles, so a job would have nothing to run; queuing one would leave the
/// caller holding an id that can never become an answer.
pub(crate) async fn refuse_v2_profile_job_create() -> Mem0Error {
    Mem0Error::unsupported(
        "profile generation job",
        "this service does not generate profiles, so a generation job would have nothing to run; \
         the request is refused instead of being queued into a job that never completes",
    )
}

/// `GET /v2/profiles/jobs/{job_id}/`.
///
/// The read half of the pair above. A job id this service never issued has no
/// state to report, and `404` would not be honest about why: it reads as "that
/// job is gone" rather than "no job is ever created here". The named refusal is
/// the only answer that does not misattribute the id space to this service.
pub(crate) async fn refuse_v2_profile_job_retrieve() -> Mem0Error {
    Mem0Error::unsupported(
        "profile job lookup",
        "no profile job is ever created on this surface, so a job id has no state to report here, \
         and a `not found` would read as a job that expired rather than one that never existed",
    )
}

/// `GET /v2/profiles/settings/`.
///
/// mem0's profile settings are **project-level** — which model writes a profile
/// and when. This service is not the owner of a mem0 project's configuration:
/// project and organisation administration is mem0's platform-account plane,
/// which this surface does not serve (the same reason the `/api/v1/orgs/...` and
/// `/api/v1/webhooks/...` calls the official clients make are not served here).
/// A settings document derived from this service's own space scope would be a
/// different object published under the same name.
pub(crate) async fn refuse_v2_profile_settings_read() -> Mem0Error {
    Mem0Error::unsupported(
        "profile settings retrieval",
        "mem0's profile settings are project-level configuration owned by mem0's platform-account \
         plane, which this surface does not serve; a settings document scoped to this service's \
         own space would be a different object under the same name",
    )
}

/// `POST /v2/profiles/settings/`.
///
/// The write half of the pair above, and the more dangerous of the two: a
/// refusal is recoverable, but accepting the write would report a configuration
/// change that took effect nowhere.
pub(crate) async fn refuse_v2_profile_settings_update() -> Mem0Error {
    Mem0Error::unsupported(
        "profile settings update",
        "mem0's profile settings are project-level configuration owned by mem0's platform-account \
         plane, which this surface does not serve; accepting the write would report a \
         configuration change that took effect nowhere",
    )
}

#[cfg(test)]
mod add_field_tests {
    use super::*;
    use crate::mem0::dto::Mem0Message;

    fn add_request(messages: Vec<Mem0Message>) -> Mem0AddRequest {
        Mem0AddRequest {
            messages,
            user_id: None,
            agent_id: None,
            run_id: None,
            app_id: None,
            filters: None,
            metadata: None,
            timestamp: None,
            expiration_date: None,
            custom_categories: None,
            custom_instructions: None,
            agent_custom_instructions: None,
            structured_data_schema: None,
            infer: None,
            immutable: None,
            includes: None,
            excludes: None,
            enable_graph: None,
            output_format: None,
            prompt_profile_id: None,
            temporal_reasoning: None,
            timezone: None,
            observation_datetime: None,
            observation_date: None,
        }
    }

    #[test]
    fn v3_filters_file_the_identity_axes() {
        let mut request = add_request(vec![Mem0Message {
            role: "user".to_owned(),
            content: "hello".to_owned(),
        }]);
        request.filters = Some(serde_json::json!({"user_id": "alice", "agent_id": "planner"}));
        let references = add_entity_references(&request).expect("references");
        assert!(references.contains(&("user", "user_id", "alice".to_owned())));
        assert!(references.contains(&("agent", "agent_id", "planner".to_owned())));
    }

    #[test]
    fn the_top_level_spelling_wins_for_one_axis() {
        let mut request = add_request(vec![Mem0Message {
            role: "user".to_owned(),
            content: "hello".to_owned(),
        }]);
        request.filters = Some(serde_json::json!({"user_id": "from-filters"}));
        request.user_id = Some("from-top-level".to_owned());
        let references = add_entity_references(&request).expect("references");
        assert_eq!(
            references,
            vec![("user", "user_id", "from-top-level".to_owned())],
            "one axis given in both spellings resolves to the top-level value"
        );
    }

    #[test]
    fn an_unknown_filters_key_is_refused_not_dropped() {
        let mut request = add_request(vec![Mem0Message {
            role: "user".to_owned(),
            content: "hello".to_owned(),
        }]);
        request.filters = Some(serde_json::json!({"user_id": "alice", "category": "tools"}));
        let error = add_entity_references(&request).expect_err("unknown key");
        assert!(
            error.detail().contains("category"),
            "the refusal must name the offending key, got: {}",
            error.detail()
        );
    }

    #[test]
    fn a_present_unsupported_field_is_refused_and_an_absent_one_is_not() {
        let mut request = add_request(vec![Mem0Message {
            role: "user".to_owned(),
            content: "hello".to_owned(),
        }]);
        request.timestamp = Some(serde_json::json!(1735689600));
        assert!(refuse_unsupported_field(request.timestamp.as_ref(), "timestamp", "why").is_err());
        assert!(refuse_unsupported_field(None, "timestamp", "why").is_ok());
    }
}
