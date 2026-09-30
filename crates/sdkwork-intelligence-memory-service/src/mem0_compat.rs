//! mem0 platform compatibility wire support.
//!
//! `sdkwork-specs/API_SPEC.md` section 4.5.2 sanctions `open-api` operations
//! that mirror an upstream third-party wire verbatim. The `mem0-platform`
//! protocol surface is the mem0 platform REST dialect spoken by the official
//! `mem0ai` Python and TypeScript clients (`external/mem0/mem0/client/main.py`,
//! `mem0-ts/src/client/mem0.ts`).
//!
//! Only one translation needs storage access, and it is owned here: mem0 has no
//! `space_id`, while every SDKWork memory operation is scoped to a memory space.
//! Each authenticated principal therefore gets one dedicated space for the mem0
//! wire, marked with [`MEM0_SPACE_DEFAULT_SCOPE`] so it is distinguishable from
//! the principal's ordinary spaces.
//!
//! Two reads also live here because the route layer cannot reach them: the
//! resource-scoped audit history behind the mem0 `history` operation
//! ([`OpenMemoryService::mem0_memory_history`]), and entity-scope registration
//! behind `GET /v1/entities/` ([`OpenMemoryService::mem0_register_entity_scope`]).
//! The wire-shaped translation — mem0 JSON in, mem0 JSON out — stays in the
//! route layer (`sdkwork-routes-memory-open-api::mem0`), which reuses the
//! existing `MemoryOpenApi` operations for everything else. In particular the
//! mem0 `id` is the canonical record uuid (`MemoryRecord.uuid`), which the store
//! already uses as its external identity, so the internal numeric row id never
//! crosses the mem0 boundary.

use sdkwork_memory_contract::{
    MemoryOpenApiRequestContext, MemoryServiceError, MemoryServiceErrorKind, MemoryServiceResult,
};
use sdkwork_memory_spi::{
    CountActiveMemoryRecordsQuery, CreateGraphEntityCommand, CreateMemorySpaceCommand,
    ListMemoryAuditHistoryQuery, MemorySpaceQuotaAdmission, RetrieveGraphEntityQuery,
};

use crate::platform;
use crate::store_error::{map_memory_spi_error, map_native_sql_store_error};
use crate::OpenMemoryService;

/// `default_scope` marking the space that backs the mem0 compatibility wire.
/// Distinct from a user's ordinary spaces so the compatibility surface never
/// silently absorbs, or is absorbed by, them.
pub const MEM0_SPACE_DEFAULT_SCOPE: &str = "mem0";

const MEM0_SPACE_DISPLAY_NAME: &str = "mem0 compatibility";

/// `space_type` of the compatibility space.
///
/// `ai_space` carries `uk_ai_space_owner_type ON (tenant_id,
/// owner_subject_type, owner_subject_id, space_type)`
/// (`database/ddl/baseline/postgres/0001_memory_baseline.sql`), so a principal
/// may own **one space per type**. Reusing `personal` would therefore put the
/// compatibility space in direct competition with the principal's own personal
/// space and fail the insert with a unique violation the moment that space
/// exists — which is the ordinary case, not an edge case. A dedicated type gives
/// the wire its own space without occupying a kind the principal's other traffic
/// already uses. `space_type` is an open column (no `CHECK` constraint) and no
/// authorization decision reads it, so the additional kind is free.
pub const MEM0_SPACE_TYPE: &str = "mem0";

/// `resource_type` the canonical memory mutation journal records for a memory
/// record (`OpenMemoryService::memory_mutation_journal`). The history read
/// resolves by this exact value, so it is named once here.
pub const MEM0_MEMORY_AUDIT_RESOURCE_TYPE: &str = "memory_record";

/// Sensitivity of a derived entity scope. Entities created by the
/// compatibility wire carry no content of their own — just an identifier the
/// caller already supplied — so they never inherit a record's sensitivity.
const MEM0_ENTITY_SENSITIVITY_LEVEL: &str = "internal";

/// One memory's mutation history, shaped for the mem0 `history` wire.
///
/// The record's own mem0 entity scope travels with the events because the wire
/// reports `user_id` per history entry and only the record carries it. It is
/// read from record metadata, i.e. the value the memory was actually filed
/// under, never from the request that asks for the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mem0MemoryHistory {
    /// The record's mem0 `user_id` scope as filed at write time; `None` when the
    /// memory was filed under a different entity scope only.
    pub user_id: Option<String>,
    /// Mutation events, newest first. Never longer than the `page_size` the
    /// caller asked for.
    pub events: Vec<Mem0MemoryMutationEvent>,
    /// `true` when the record's journal holds more entries than `page_size`, so
    /// `events` is a prefix, not the whole history. mem0's `history` is defined
    /// as the memory's *whole* history, and a partial log is indistinguishable
    /// from a complete one, so callers refuse (`501`) on this flag instead of
    /// answering with a subset.
    ///
    /// Conservative at the store clamp ceiling: every list read caps its SQL
    /// `LIMIT` at [`sdkwork_utils_rust::MAX_LIST_PAGE_SIZE`], so a history that
    /// fills that ceiling is reported truncated even if it ends exactly there —
    /// refusing a boundary-length history is preferred over silently serving a
    /// possibly partial one.
    pub truncated: bool,
}

/// One entry of a memory's mutation history.
///
/// Shaped for the mem0 `history` wire but deliberately content-free: the
/// canonical journal records *that* a mutation was accepted and by whom, not
/// the memory text. Copying record text into the audit trail would duplicate
/// personal data into a second store with its own retention and erasure
/// semantics, so `old_memory`/`new_memory` are reported as absent rather than
/// fabricated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mem0MemoryMutationEvent {
    pub audit_id: String,
    /// Journal action, e.g. `memory.record.created`.
    pub action: String,
    pub result: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub created_at: String,
}

impl OpenMemoryService {
    /// Resolve, creating on first use, the memory space that backs the mem0
    /// compatibility wire for one authenticated principal.
    ///
    /// The space is owned by the authenticated actor, so the existing
    /// user-owned-space authorization rules apply to mem0 traffic unchanged.
    pub async fn mem0_space_id(
        &self,
        context: &MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<u64> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let actor_id = context.actor_id.ok_or_else(|| {
            MemoryServiceError::validation(
                "mem0 compatibility requires an authenticated actor to own its memory space",
            )
        })?;
        let actor = actor_id.to_string();

        if let Some(space_id) = self.mem0_existing_space_id(tenant_id, &actor).await? {
            return Ok(space_id);
        }

        let space_id = i64::try_from(self.next_id()?)
            .map_err(|_| MemoryServiceError::storage("generated mem0 space id out of range"))?;
        let quota_limits =
            crate::tenant_quota::resolve_quota_limits(&self.store, tenant_id).await?;
        let admission = self
            .runtime_data_plane
            .create_space_atomic_with_quota(
                CreateMemorySpaceCommand {
                    tenant_id,
                    space_id,
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: actor.clone(),
                    space_type: MEM0_SPACE_TYPE.to_string(),
                    display_name: MEM0_SPACE_DISPLAY_NAME.to_string(),
                    default_scope: MEM0_SPACE_DEFAULT_SCOPE.to_string(),
                },
                quota_limits.max_spaces_per_user,
            )
            .await;

        match admission {
            Ok(MemorySpaceQuotaAdmission::Admitted(_)) => u64::try_from(space_id)
                .map_err(|_| MemoryServiceError::storage("generated mem0 space id out of range")),
            // Reported rather than swallowed: the caller asked for a memory and
            // the space that would hold it was refused, so answering with the
            // unwritten id would look like a successful write to a space that
            // does not exist.
            Ok(MemorySpaceQuotaAdmission::QuotaExceeded {
                active_spaces,
                max_active_spaces,
            }) => Err(MemoryServiceError::quota_exceeded(format!(
                "this principal owns {active_spaces} memory spaces and the limit is \
                 {max_active_spaces}; no space is available for the mem0 compatibility wire"
            ))),
            Err(error) => {
                // A concurrent first call can win `uk_ai_space_owner_type` between
                // the lookup above and this insert. The space the caller needs then
                // exists, which is not a caller-visible failure — so the lookup is
                // repeated and only a still-absent space is reported.
                match self.mem0_existing_space_id(tenant_id, &actor).await? {
                    Some(space_id) => Ok(space_id),
                    None => Err(error),
                }
            }
        }
    }

    /// The principal's existing compatibility space, if it has one.
    ///
    /// An exact index seek on `uk_ai_space_owner_type` keyed by the space type:
    /// a paged scan of the principal's spaces would miss the compatibility
    /// space for any principal owning more spaces than one page, and then fail
    /// on every subsequent call against the unique index.
    async fn mem0_existing_space_id(
        &self,
        tenant_id: i64,
        actor: &str,
    ) -> MemoryServiceResult<Option<u64>> {
        let space_id = self
            .store
            .find_space_id_by_owner_and_type(tenant_id, "user", actor, MEM0_SPACE_TYPE)
            .await
            .map_err(map_native_sql_store_error)?;
        space_id
            .map(|space_id| {
                u64::try_from(space_id).map_err(|_| {
                    MemoryServiceError::storage("mem0 space id does not fit in an unsigned integer")
                })
            })
            .transpose()
    }

    /// Register one mem0 entity scope (`user`/`agent`/`run`/`app`) as a graph
    /// entity in the compatibility space, so `GET /v1/entities/` can list the
    /// scopes memories actually exist for.
    ///
    /// Idempotent by construction: the entity uuid is derived from
    /// `(space, kind, reference)`, so a repeat call resolves to the row the
    /// first call wrote. The space participates in the derivation because graph
    /// entity uuids are unique per **tenant**, not per space — deriving without
    /// it would make the same reference collide across two spaces of one tenant.
    pub async fn mem0_register_entity_scope(
        &self,
        context: &MemoryOpenApiRequestContext,
        space_id: u64,
        entity_kind: &str,
        entity_ref: &str,
    ) -> MemoryServiceResult<()> {
        let entity_ref = entity_ref.trim();
        if entity_ref.is_empty() {
            return Ok(());
        }
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id_i64 = platform::space_id_i64(space_id)?;
        let entity_id = mem0_entity_uuid(space_id, entity_kind, entity_ref);

        if self
            .graph
            .retrieve_entity(RetrieveGraphEntityQuery {
                tenant_id,
                entity_id: entity_id.clone(),
            })
            .await
            .map_err(map_memory_spi_error)?
            .is_some()
        {
            return Ok(());
        }

        let scope = crate::commercial_api::commercial_mutation_scope(context, tenant_id, space_id_i64);
        let journal =
            crate::commercial_api::commercial_mutation_journal("entity", &entity_id, "created")?;
        let created = self
            .graph
            .create_entity(CreateGraphEntityCommand {
                scope,
                entity_id: entity_id.clone(),
                entity_type: entity_kind.to_string(),
                canonical_name: entity_ref.to_string(),
                aliases_json: None,
                attributes_json: None,
                sensitivity_level: MEM0_ENTITY_SENSITIVITY_LEVEL.to_string(),
                journal,
            })
            .await;

        match created {
            Ok(()) => Ok(()),
            // A concurrent add can insert the same scope between the lookup and
            // the insert; the unique index then rejects the second writer. That
            // is not a caller-visible failure — the entity the caller needs now
            // exists — so it is only an error when the re-read still finds
            // nothing.
            Err(error) => {
                let present = self
                    .graph
                    .retrieve_entity(RetrieveGraphEntityQuery { tenant_id, entity_id })
                    .await
                    .map_err(map_memory_spi_error)?
                    .is_some();
                if present {
                    Ok(())
                } else {
                    Err(map_memory_spi_error(error))
                }
            }
        }
    }

    /// Number of active records in the compatibility space, for mem0's required
    /// `count` field.
    ///
    /// Exact for this surface rather than approximate: the mem0 space is created
    /// and owned by one principal through this wire alone, and the wire never
    /// writes `expires_at`. The canonical counter counts every row whose status
    /// is not `deleted` (it does not apply the listing's expiry predicate), so
    /// for a space this surface writes, the two are the same set.
    pub async fn mem0_record_count(
        &self,
        context: &MemoryOpenApiRequestContext,
        space_id: u64,
    ) -> MemoryServiceResult<u64> {
        crate::access::assert_actor_can_access_space(&self.runtime_data_plane, context, space_id)
            .await?;
        let scope = Self::scope(context, space_id)?;
        self.runtime_data_plane
            .count_active_records(CountActiveMemoryRecordsQuery { scope })
            .await
    }

    /// Mutation history of one canonical memory, newest first.
    ///
    /// Authorization is the space read (the space survives every single-memory
    /// mutation), so the history of a deleted memory stays readable: verifying
    /// a deletion through the audit trail is the primary client flow for this
    /// operation. The mem0 `user_id` scope comes from the live record's
    /// metadata and is therefore absent once the record is deleted.
    ///
    /// `page_size` is the caller's page bound; the returned `events` never
    /// exceed it and `truncated` reports whether the journal holds more. The
    /// store read probes one entry past the bound (`page_size + 1` sentinel).
    /// Every store list read clamps its SQL `LIMIT` at
    /// [`sdkwork_utils_rust::MAX_LIST_PAGE_SIZE`], so at the ceiling the probe
    /// cannot look past one full clamp: a history filling the clamp is reported
    /// truncated even if it ends exactly there (conservative — see
    /// [`Mem0MemoryHistory::truncated`]).
    pub async fn mem0_memory_history(
        &self,
        context: &MemoryOpenApiRequestContext,
        space_id: u64,
        memory_id: u64,
        page_size: i32,
    ) -> MemoryServiceResult<Mem0MemoryHistory> {
        crate::access::assert_actor_can_access_space(&self.runtime_data_plane, context, space_id)
            .await?;
        let user_id = match self.load_scoped_record(context, space_id, memory_id).await {
            Ok(record) => record
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("user_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            // A deleted (or never-existing) record still serves its audit
            // trail; only the filed entity scope is unavailable.
            Err(error) if error.kind == MemoryServiceErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let scope = Self::scope(context, space_id)?;
        let requested = page_size.max(1);
        let clamp_ceiling = usize::try_from(sdkwork_utils_rust::MAX_LIST_PAGE_SIZE).unwrap_or(200);
        let page_size_usize = usize::try_from(requested).unwrap_or(clamp_ceiling);
        let entries = self
            .runtime_data_plane
            .list_audit_history(ListMemoryAuditHistoryQuery {
                scope,
                resource_type: MEM0_MEMORY_AUDIT_RESOURCE_TYPE.to_string(),
                resource_id: memory_id.to_string(),
                page_size: requested.saturating_add(1),
            })
            .await?;
        // Below the clamp the sentinel is exact: more rows than the bound means
        // truncated. At the clamp the store cannot return a (bound + 1)-th row,
        // so a full clamp is treated as truncated — the wire refuses rather
        // than risk an incomplete history.
        let truncated = if page_size_usize < clamp_ceiling {
            entries.len() > page_size_usize
        } else {
            entries.len() >= clamp_ceiling
        };
        Ok(Mem0MemoryHistory {
            user_id,
            events: entries
                .into_iter()
                .take(page_size_usize)
                .map(|entry| Mem0MemoryMutationEvent {
                    audit_id: entry.audit_id,
                    action: entry.action,
                    result: entry.result,
                    actor_type: entry.actor_type,
                    actor_id: entry.actor_id,
                    created_at: entry.created_at,
                })
                .collect(),
            truncated,
        })
    }
}

/// Deterministic graph-entity uuid for one mem0 entity scope.
///
/// SHA-256 of `mem0/<space>/<kind>/<reference>`; 64 hex characters, which is
/// exactly the width of `ai_entity.uuid`.
fn mem0_entity_uuid(space_id: u64, entity_kind: &str, entity_ref: &str) -> String {
    sdkwork_utils_rust::sha256_hash(format!("mem0/{space_id}/{entity_kind}/{entity_ref}").as_bytes())
}
