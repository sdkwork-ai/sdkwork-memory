use async_trait::async_trait;

use crate::{MemoryRetrieverKind, MemorySpiError, MemorySpiResult, MetadataFilterExpression};

pub trait MemoryRuntimePlugin: Send + Sync {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryScopeContext {
    pub tenant_id: i64,
    pub space_id: i64,
    pub organization_id: Option<i64>,
    pub user_id: Option<i64>,
}

impl MemoryScopeContext {
    pub fn for_test(tenant_id: i64, space_id: i64) -> Self {
        Self {
            tenant_id,
            space_id,
            organization_id: None,
            user_id: Some(space_id),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemorySensitivityReadScope {
    Public,
    Elevated,
    Owner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMemoryRecordCommand {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    pub memory_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCanonicalRecord {
    pub memory_id: String,
    pub space_id: i64,
    pub user_id: Option<i64>,
    pub scope_label: String,
    pub memory_type: String,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_text: String,
    pub canonical_text: String,
    pub confidence: f64,
    pub evidence_count: i32,
    pub contradiction_count: i32,
    pub status: String,
    pub sensitivity_level: String,
    pub supersedes_memory_id: Option<String>,
    pub superseded_by_memory_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
    /// Instant after which the record is hidden from retrieval paths, mirroring
    /// the contract-declared `expiresAt` field on the canonical record schemas.
    pub expires_at: Option<String>,
    /// Caller metadata as stored JSON (contract `metadata` field).
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMutationJournal {
    pub outbox_id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub event_version: String,
    pub payload_json: String,
    pub audit_id: String,
    pub audit_action: String,
    pub audit_resource_type: String,
    pub audit_resource_id: String,
    pub audit_result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCanonicalMemoryCommand {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
    pub scope_label: String,
    pub memory_type: String,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_text: String,
    pub canonical_text: String,
    pub sensitivity_level: String,
    /// Optional contract-declared expiration instant persisted with the record.
    pub expires_at: Option<String>,
    /// Optional caller metadata, serialized JSON; persisted into
    /// `ai_record.metadata_json` where metadata filters evaluate.
    pub metadata_json: Option<String>,
    pub journal: MemoryMutationJournal,
}

/// Atomically replaces one active canonical memory with a new version in the
/// same space. Providers must serialize the space, admit the replacement
/// against the configured active-record quota, link both records, and persist
/// both mutation journals before committing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupersedeCanonicalMemoryAtomicCommand {
    pub scope: MemoryScopeContext,
    pub old_memory_id: String,
    pub new_memory_id: String,
    pub scope_label: String,
    pub memory_type: String,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_text: String,
    pub canonical_text: String,
    pub sensitivity_level: String,
    /// Optional contract-declared expiration instant persisted with the replacement.
    pub expires_at: Option<String>,
    /// Optional caller metadata, serialized JSON; persisted into
    /// `ai_record.metadata_json` where metadata filters evaluate.
    pub metadata_json: Option<String>,
    pub created_journal: MemoryMutationJournal,
    pub superseded_journal: MemoryMutationJournal,
}

/// Result of a record mutation whose space quota is admitted in the same
/// atomic boundary as the mutation itself.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryRecordQuotaAdmission<T> {
    Admitted(T),
    QuotaExceeded {
        active_records: u64,
        max_active_records: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveCanonicalMemoryQuery {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCanonicalMemoryCommand {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
    pub canonical_text: Option<String>,
    pub subject: Option<String>,
    /// Replacement metadata (merged by the caller); serialized JSON persisted
    /// into `ai_record.metadata_json`.
    pub metadata_json: Option<String>,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteCanonicalMemoryCommand {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
    pub journal: MemoryMutationJournal,
}

/// Bulk-deletion scope for the `delete_all` analogue. `user_id` optionally
/// narrows the sweep to one owner inside the space, mirroring the way mem0's
/// `delete_all(user_id=...)` narrows by identity axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteAllCanonicalMemoryCommand {
    pub scope: MemoryScopeContext,
    pub user_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MemoryBulkDeletionReceipt {
    pub deleted_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryRecordQuery {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteMemoryRecordCommand {
    pub scope: MemoryScopeContext,
    pub memory_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDeletionReceipt {
    pub memory_id: String,
    pub deleted: bool,
    pub already_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendMemoryEventCommand {
    pub scope: MemoryScopeContext,
    pub event_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEvent {
    pub event_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryEventQuery {
    pub scope: MemoryScopeContext,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryAuditRecord {
    pub audit_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendMemoryAuditCommand {
    pub scope: MemoryScopeContext,
    pub audit_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryAuditQuery {
    pub scope: MemoryScopeContext,
    pub audit_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryOutboxEvent {
    pub outbox_id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub event_version: String,
    pub payload_json: String,
    pub publish_state: String,
    pub published_at: Option<String>,
    pub retry_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendMemoryOutboxCommand {
    pub scope: MemoryScopeContext,
    pub outbox_id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub event_version: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryOutboxQuery {
    pub scope: MemoryScopeContext,
    pub outbox_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPendingMemoryOutboxQuery {
    pub scope: MemoryScopeContext,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkMemoryOutboxPublishedCommand {
    pub scope: MemoryScopeContext,
    pub outbox_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkMemoryOutboxFailedCommand {
    pub scope: MemoryScopeContext,
    pub outbox_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateMemoryCandidateCommand {
    pub scope: MemoryScopeContext,
    pub candidate_id: String,
    pub candidate_type: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub proposed_payload_json: Option<String>,
    pub evidence_json: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryCandidateQuery {
    pub scope: MemoryScopeContext,
    pub candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApproveMemoryCandidateCommand {
    pub scope: MemoryScopeContext,
    pub candidate_id: String,
    pub decision_reason: Option<String>,
    pub decided_by: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectMemoryCandidateCommand {
    pub scope: MemoryScopeContext,
    pub candidate_id: String,
    pub decision_reason: Option<String>,
    pub decided_by: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCandidateEvidenceLink {
    pub source_id: String,
    pub event_id: String,
    pub confidence_delta: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromoteMemoryCandidateAtomicCommand {
    pub scope: MemoryScopeContext,
    pub candidate_id: String,
    pub memory_id: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub evidence_links: Vec<MemoryCandidateEvidenceLink>,
    pub decided_by: Option<i64>,
}

/// Journal-aware candidate promotion command. The base promotion command stays
/// source-compatible for evaluation and legacy callers; production HTTP uses
/// this additive form so the provider can commit its outbox/audit entries with
/// the canonical record mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct PromoteMemoryCandidateAtomicWithJournalCommand {
    pub promotion: PromoteMemoryCandidateAtomicCommand,
    pub journal: MemoryMutationJournal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCandidatePromotion {
    pub candidate_id: String,
    pub memory_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCandidate {
    pub candidate_id: String,
    pub candidate_type: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub proposed_payload_json: Option<String>,
    pub evidence_json: Option<String>,
    pub confidence: f64,
    pub decision_state: String,
    pub decision_reason: Option<String>,
    pub decided_by: Option<i64>,
    pub decided_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrieveMemoryCandidateDetailQuery {
    pub tenant_id: i64,
    pub candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMemoryCandidatesQuery {
    pub tenant_id: i64,
    pub space_id: Option<i64>,
    pub page_size: u32,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCandidateSummary {
    pub candidate_id: String,
    pub space_id: i64,
    pub candidate_type: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub confidence: f64,
    pub decision_state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCandidatePage {
    pub items: Vec<MemoryCandidateSummary>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

/// Provider-neutral candidate projection used by promotion workflows. The
/// service must not depend on a provider's SQL row type to recover evidence or
/// an existing promotion target.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryCandidateDetail {
    pub candidate_id: String,
    pub space_id: i64,
    pub candidate_type: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub evidence_json: Option<String>,
    pub confidence: f64,
    pub decision_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub target_memory_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpsertMemoryHabitCommand {
    pub scope: MemoryScopeContext,
    pub habit_id: String,
    pub user_id: i64,
    pub habit_key: String,
    pub habit_type: String,
    pub description: String,
    pub stage: String,
    pub strength: f64,
    pub confidence: f64,
    pub support_count: i64,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryHabitQuery {
    pub scope: MemoryScopeContext,
    pub user_id: i64,
    pub habit_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromoteMemoryHabitCommand {
    pub scope: MemoryScopeContext,
    pub user_id: i64,
    pub habit_key: String,
    pub promoted_memory_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecayMemoryHabitCommand {
    pub scope: MemoryScopeContext,
    pub user_id: i64,
    pub habit_key: String,
    pub strength_delta: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryHabit {
    pub habit_id: String,
    pub user_id: i64,
    pub habit_key: String,
    pub habit_type: String,
    pub description: String,
    pub stage: String,
    pub strength: f64,
    pub confidence: f64,
    pub support_count: i64,
    pub last_signal_at: Option<String>,
    pub promoted_memory_id: Option<String>,
    pub decay_after: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRetrievalHitDraft {
    pub hit_id: String,
    pub memory_id: Option<String>,
    pub space_id: Option<i64>,
    pub retriever_name: String,
    pub result_rank: i64,
    pub raw_score: Option<f64>,
    pub fused_score: Option<f64>,
    pub explanation_json: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryContextPackSnapshot {
    pub context_pack_id: String,
    pub pack_json: String,
    pub estimated_tokens: i64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppendMemoryRetrievalTraceCommand {
    pub scope: MemoryScopeContext,
    pub trace_id: String,
    pub actor_id: Option<String>,
    pub query_text: Option<String>,
    pub query_hash: String,
    pub retrievers_json: Option<String>,
    pub latency_ms: Option<i64>,
    pub degraded: bool,
    pub metadata_json: Option<String>,
    pub hits: Vec<MemoryRetrievalHitDraft>,
    pub context_pack: Option<MemoryContextPackSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryRetrievalTraceQuery {
    pub scope: MemoryScopeContext,
    pub trace_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryRetrievalTraceForTenantQuery {
    pub tenant_id: i64,
    pub trace_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScopedMemoryRetrievalTrace {
    pub scope: MemoryScopeContext,
    pub trace: MemoryRetrievalTrace,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListMemoryRetrievalTracesQuery {
    pub scope: MemoryScopeContext,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRetrievalTrace {
    pub trace_id: String,
    pub actor_id: Option<String>,
    pub query_text: Option<String>,
    pub query_hash: String,
    pub retrievers_json: Option<String>,
    pub latency_ms: Option<i64>,
    pub result_count: i64,
    pub degraded: bool,
    pub metadata_json: Option<String>,
    pub hits: Vec<MemoryRetrievalHitDraft>,
    pub context_pack: Option<MemoryContextPackSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPolicy {
    pub policy_code: String,
}

pub const MAX_MEMORY_GOVERNANCE_FACTS: u32 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryGovernanceActor {
    /// Trusted subject type when the request context provides one.
    /// `None` preserves compatibility with the current actor-id-only HTTP context; providers
    /// must fail closed when that identifier is ambiguous across subject namespaces.
    pub subject_type: Option<String>,
    pub subject_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveMemorySpaceGovernanceQuery {
    pub scope: MemoryScopeContext,
    pub actor: Option<MemoryGovernanceActor>,
    pub capability_code: Option<String>,
    pub fact_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySpaceGovernanceFact {
    pub space_id: i64,
    pub organization_id: Option<i64>,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub lifecycle_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryActorSpaceBindingFact {
    pub binding_id: String,
    pub binding_kind: String,
    pub binding_role: String,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCapabilityBindingFact {
    pub binding_id: String,
    pub capability_code: String,
    pub mode: String,
    pub priority: i32,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
}

/// Complete, bounded governance facts for one tenant-scoped memory space.
///
/// Providers set `complete` to false when either fact collection exceeds the requested bound.
/// Service policy must fail closed instead of authorizing from a truncated fact set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySpaceGovernanceFacts {
    pub space: Option<MemorySpaceGovernanceFact>,
    pub actor_bindings: Vec<MemoryActorSpaceBindingFact>,
    pub capability_bindings: Vec<MemoryCapabilityBindingFact>,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountActiveMemoryRecordsQuery {
    pub scope: MemoryScopeContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountUserOwnedMemorySpacesQuery {
    pub tenant_id: i64,
    pub owner_subject_id: String,
}

/// Input for a provider-owned memory-space creation mutation.
///
/// Space identifiers are allocated by the service, while the provider owns the
/// transaction that admits the owner against its configured quota and persists
/// the row. Providers must validate the tenant and owner fields again at this
/// boundary; callers cannot use this command to widen a request scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateMemorySpaceCommand {
    pub tenant_id: i64,
    pub space_id: i64,
    pub organization_id: Option<i64>,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub space_type: String,
    pub display_name: String,
    pub default_scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySpaceRecord {
    pub space_id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub organization_id: Option<i64>,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub space_type: String,
    pub display_name: String,
    pub default_scope: String,
    pub lifecycle_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

/// Result of a space mutation whose user-owned-space quota is admitted in the
/// same serialization boundary as the insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySpaceQuotaAdmission<T> {
    Admitted(T),
    QuotaExceeded {
        active_spaces: u64,
        max_active_spaces: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrieveMemoryCandidatesCommand {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRetrieverResult {
    pub memory_ids: Vec<String>,
}

pub const MAX_MEMORY_RETRIEVAL_CANDIDATES: u32 = 200;

/// Bounded, scope-aware retrieval input used by the application retrieval pipeline.
///
/// The plugin receives only the search scope and the enabled retriever kinds. It must never
/// infer tenant or space context from ambient state, and it must honor the limit at the
/// authoritative index/store boundary.
///
/// `metadata_filter` is the caller's metadata predicate, already parsed and validated by
/// [`crate::parse_metadata_filter`]. A store must apply it **inside** its query rather than
/// to a broad read, per `PAGINATION_SPEC.md` section 5.1, and must refuse the query when it
/// cannot express an operator exactly rather than returning unfiltered rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMemoryCandidatesQuery {
    pub scope: MemoryScopeContext,
    pub query: String,
    pub limit: u32,
    pub retriever_kinds: Vec<MemoryRetrieverKind>,
    pub memory_types: Vec<String>,
    pub read_scope: MemorySensitivityReadScope,
    pub metadata_filter: Option<MetadataFilterExpression>,
    /// When `false` (the default in every caller-facing surface), records whose
    /// `expires_at` has passed are hidden from search and list results, mirroring
    /// mem0's `show_expired` semantics. Single-record retrieval ignores this.
    pub include_expired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRetrievalRecordCandidate {
    pub memory_id: String,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_text: String,
    pub canonical_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRetrievalEventCandidate {
    pub memory_id: String,
    pub event_id: String,
    pub payload_text: String,
    pub created_at: String,
}

/// Candidate projections are not canonical truth. The service must rehydrate every candidate
/// through `MemoryRecordStorePort` before returning it or assembling context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRetrieverSearchResult {
    pub records: Vec<MemoryRetrievalRecordCandidate>,
    pub events: Vec<MemoryRetrievalEventCandidate>,
    pub degraded: bool,
    pub unavailable_retriever_kinds: Vec<MemoryRetrieverKind>,
    pub degradation_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryIndexReceipt {
    pub memory_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageModelCommand {
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingCommand {
    pub input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankMemoryHitsCommand {
    pub memory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RerankMemoryHitsResult {
    pub memory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryImportCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryImportResult {
    pub imported_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryExportCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryExportResult {
    pub exported_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryDeleteCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryDeleteReceipt {
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryShadowReadCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalMemoryShadowReadResult {
    pub comparable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembleMemoryContextCommand {
    pub memory_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryContextPackDraft {
    pub memory_ids: Vec<String>,
    pub context_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunMemoryEvalCommand {
    pub eval_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEvalRunResult {
    pub eval_type: String,
}

#[async_trait]
pub trait MemoryRecordStorePort: Send + Sync {
    fn supports_canonical_atomic(&self) -> bool {
        false
    }

    /// Whether canonical record creation can serialize quota admission with
    /// the record mutation for one space.
    fn supports_atomic_record_quota_admission(&self) -> bool {
        false
    }

    /// Whether superseding a canonical record is admitted and committed as one
    /// provider transaction, including both record links and mutation journals.
    fn supports_atomic_supersede(&self) -> bool {
        false
    }

    async fn create(&self, command: CreateMemoryRecordCommand) -> MemorySpiResult<MemoryRecord>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryRecordQuery,
    ) -> MemorySpiResult<Option<MemoryRecord>>;

    async fn mark_deleted(
        &self,
        command: DeleteMemoryRecordCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt>;

    async fn create_canonical_atomic(
        &self,
        _command: CreateCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryCanonicalRecord> {
        Err(atomic_record_operation_required("create_canonical_atomic"))
    }

    /// Creates a canonical record only when the space remains below the
    /// supplied active-record limit at the mutation's serialization point.
    /// A zero limit disables quota rejection but still uses the atomic path.
    async fn create_canonical_atomic_with_quota(
        &self,
        _command: CreateCanonicalMemoryCommand,
        _max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        Err(atomic_record_operation_required(
            "create_canonical_atomic_with_quota",
        ))
    }

    async fn supersede_canonical_atomic_with_quota(
        &self,
        _command: SupersedeCanonicalMemoryAtomicCommand,
        _max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        Err(atomic_record_operation_required(
            "supersede_canonical_atomic_with_quota",
        ))
    }

    async fn retrieve_canonical(
        &self,
        _query: RetrieveCanonicalMemoryQuery,
    ) -> MemorySpiResult<Option<MemoryCanonicalRecord>> {
        Err(atomic_record_operation_required("retrieve_canonical"))
    }

    async fn update_canonical_atomic(
        &self,
        _command: UpdateCanonicalMemoryCommand,
    ) -> MemorySpiResult<Option<MemoryCanonicalRecord>> {
        Err(atomic_record_operation_required("update_canonical_atomic"))
    }

    async fn delete_canonical_atomic(
        &self,
        _command: DeleteCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        Err(atomic_record_operation_required("delete_canonical_atomic"))
    }

    /// Bulk-delete every active canonical record in the scope (optionally
    /// narrowed to one owner), the analogue of mem0's `delete_all`.
    ///
    /// Each deleted record gets its own outbox+audit journal entries inside the
    /// same transaction; journal ids are derived deterministically from the
    /// record id, so a repeat call over an already-cleared scope deletes
    /// nothing and stays idempotent.
    async fn delete_all_canonical_atomic(
        &self,
        _command: DeleteAllCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryBulkDeletionReceipt> {
        Err(atomic_record_operation_required(
            "delete_all_canonical_atomic",
        ))
    }
}

#[async_trait]
pub trait MemoryEventStorePort: Send + Sync {
    async fn append(&self, command: AppendMemoryEventCommand) -> MemorySpiResult<MemoryEvent>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryEventQuery,
    ) -> MemorySpiResult<Option<MemoryEvent>>;
}

#[async_trait]
pub trait MemoryAuditStorePort: Send + Sync {
    async fn append(&self, command: AppendMemoryAuditCommand)
        -> MemorySpiResult<MemoryAuditRecord>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryAuditQuery,
    ) -> MemorySpiResult<Option<MemoryAuditRecord>>;
}

#[async_trait]
pub trait MemoryOutboxStorePort: Send + Sync {
    async fn append(
        &self,
        command: AppendMemoryOutboxCommand,
    ) -> MemorySpiResult<MemoryOutboxEvent>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryOutboxQuery,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>>;

    async fn list_pending(
        &self,
        query: ListPendingMemoryOutboxQuery,
    ) -> MemorySpiResult<Vec<MemoryOutboxEvent>>;

    async fn mark_published(
        &self,
        command: MarkMemoryOutboxPublishedCommand,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>>;

    async fn mark_failed(
        &self,
        command: MarkMemoryOutboxFailedCommand,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>>;
}

#[async_trait]
pub trait MemoryCandidateStorePort: Send + Sync {
    /// Whether the provider exposes the complete, tenant-scoped candidate
    /// projection required by promotion workflows.
    fn supports_candidate_detail_lookup(&self) -> bool {
        false
    }

    /// Whether the provider can execute the bounded, cursor-based candidate
    /// listing used by the HTTP surfaces.
    fn supports_candidate_listing(&self) -> bool {
        false
    }

    fn supports_atomic_candidate_promotion(&self) -> bool {
        false
    }

    /// Whether candidate promotion also persists its mutation journal in the
    /// same transaction as record/source/target/approval/index writes.
    fn supports_atomic_candidate_promotion_journal(&self) -> bool {
        false
    }

    async fn create(
        &self,
        command: CreateMemoryCandidateCommand,
    ) -> MemorySpiResult<MemoryCandidate>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>>;

    async fn retrieve_detail(
        &self,
        _query: RetrieveMemoryCandidateDetailQuery,
    ) -> MemorySpiResult<Option<MemoryCandidateDetail>> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryCandidateStorePort".to_string(),
            message: "provider-neutral candidate detail lookup is not implemented".to_string(),
        })
    }

    async fn list_candidates(
        &self,
        _query: ListMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryCandidatePage> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryCandidateStorePort".to_string(),
            message: "provider-neutral candidate listing is not implemented".to_string(),
        })
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>>;

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>>;

    async fn promote_atomic_with_quota(
        &self,
        _command: PromoteMemoryCandidateAtomicCommand,
        _max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryCandidateStorePort".to_string(),
            message:
                "atomic candidate promotion is not implemented; refusing a non-atomic fallback"
                    .to_string(),
        })
    }

    async fn promote_atomic_with_quota_and_journal(
        &self,
        _command: PromoteMemoryCandidateAtomicWithJournalCommand,
        _max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryCandidateStorePort".to_string(),
            message: "journaled atomic candidate promotion is not implemented; refusing a non-atomic fallback"
                .to_string(),
        })
    }
}

#[async_trait]
pub trait MemoryHabitStorePort: Send + Sync {
    async fn upsert(&self, command: UpsertMemoryHabitCommand) -> MemorySpiResult<MemoryHabit>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryHabitQuery,
    ) -> MemorySpiResult<Option<MemoryHabit>>;

    async fn promote(
        &self,
        command: PromoteMemoryHabitCommand,
    ) -> MemorySpiResult<Option<MemoryHabit>>;

    async fn decay(&self, command: DecayMemoryHabitCommand)
        -> MemorySpiResult<Option<MemoryHabit>>;
}

#[async_trait]
pub trait MemoryRetrievalTraceStorePort: Send + Sync {
    fn supports_tenant_trace_lookup(&self) -> bool {
        false
    }

    async fn append(
        &self,
        command: AppendMemoryRetrievalTraceCommand,
    ) -> MemorySpiResult<MemoryRetrievalTrace>;

    async fn retrieve(
        &self,
        query: RetrieveMemoryRetrievalTraceQuery,
    ) -> MemorySpiResult<Option<MemoryRetrievalTrace>>;

    async fn retrieve_for_tenant(
        &self,
        _query: RetrieveMemoryRetrievalTraceForTenantQuery,
    ) -> MemorySpiResult<Option<ScopedMemoryRetrievalTrace>> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryRetrievalTraceStorePort".to_string(),
            message: "tenant-scoped retrieval trace lookup is not implemented".to_string(),
        })
    }

    async fn list_recent(
        &self,
        query: ListMemoryRetrievalTracesQuery,
    ) -> MemorySpiResult<Vec<MemoryRetrievalTrace>>;
}

#[async_trait]
pub trait MemoryPolicyStorePort: Send + Sync {
    async fn resolve_policy(&self, policy_code: String) -> MemorySpiResult<MemoryPolicy>;
}

#[async_trait]
pub trait MemoryGovernanceAccessPort: Send + Sync {
    /// Whether the implementation resolves tenant-scoped governance facts with a hard bound.
    /// Production HTTP composition fails closed when this capability is absent.
    fn supports_bounded_governance_access(&self) -> bool {
        false
    }

    async fn resolve_space_governance(
        &self,
        _query: ResolveMemorySpaceGovernanceQuery,
    ) -> MemorySpiResult<MemorySpaceGovernanceFacts> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryGovernanceAccessPort".to_string(),
            message: "bounded tenant-scoped governance resolution is not implemented".to_string(),
        })
    }

    async fn count_active_records(
        &self,
        _query: CountActiveMemoryRecordsQuery,
    ) -> MemorySpiResult<u64> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryGovernanceAccessPort".to_string(),
            message: "tenant-scoped active memory count is not implemented".to_string(),
        })
    }

    async fn count_user_owned_spaces(
        &self,
        _query: CountUserOwnedMemorySpacesQuery,
    ) -> MemorySpiResult<u64> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryGovernanceAccessPort".to_string(),
            message: "tenant-scoped user-owned space count is not implemented".to_string(),
        })
    }
}

/// Mutation owner for memory-space lifecycle writes.
///
/// This is intentionally separate from `MemoryGovernanceAccessPort`: governance
/// facts and quota observations are read-only evidence, while this port owns
/// the transaction that reserves a user-owned space slot and inserts the row.
#[async_trait]
pub trait MemorySpaceStorePort: Send + Sync {
    /// Whether user-owned-space quota admission is serialized with insertion.
    fn supports_atomic_user_space_quota_admission(&self) -> bool {
        false
    }

    /// Create a space with quota admission at the provider's serialization
    /// point. A zero limit disables rejection but still requires the atomic
    /// mutation path.
    async fn create_space_atomic_with_quota(
        &self,
        _command: CreateMemorySpaceCommand,
        _max_active_spaces: u64,
    ) -> MemorySpiResult<MemorySpaceQuotaAdmission<MemorySpaceRecord>> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemorySpaceStorePort".to_string(),
            message:
                "atomic memory-space creation is not implemented; refusing a non-atomic fallback"
                    .to_string(),
        })
    }
}

#[async_trait]
pub trait MemoryRetrieverPort: Send + Sync {
    fn retriever_code(&self) -> &str;

    /// Whether the implementation supports the bounded, scope-aware search contract.
    /// Production HTTP composition fails closed when this capability is absent.
    fn supports_bounded_scoped_search(&self) -> bool {
        false
    }

    async fn retrieve(
        &self,
        command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult>;

    async fn retrieve_scoped(
        &self,
        _scope: MemoryScopeContext,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryRetrieverPort".to_string(),
            message: "scope-aware retrieval is not implemented; refusing an unscoped fallback"
                .to_string(),
        })
    }

    async fn search_scoped(
        &self,
        _query: SearchMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryRetrieverSearchResult> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryRetrieverPort".to_string(),
            message:
                "bounded scope-aware retrieval is not implemented; refusing an unbounded fallback"
                    .to_string(),
        })
    }
}

#[async_trait]
pub trait MemoryIndexPort: Send + Sync {
    fn index_kind(&self) -> &str;

    async fn index(&self, memory_id: String) -> MemorySpiResult<MemoryIndexReceipt>;
}

#[async_trait]
pub trait LanguageModelPort: Send + Sync {
    fn provider_code(&self) -> &str;

    async fn generate(&self, command: LanguageModelCommand) -> MemorySpiResult<String>;
}

#[async_trait]
pub trait EmbeddingModelPort: Send + Sync {
    fn provider_code(&self) -> &str;

    fn dimensions(&self) -> usize;

    async fn embed(&self, command: EmbeddingCommand) -> MemorySpiResult<Vec<f32>>;
}

#[async_trait]
pub trait RerankModelPort: Send + Sync {
    fn provider_code(&self) -> &str;

    async fn rerank(
        &self,
        command: RerankMemoryHitsCommand,
    ) -> MemorySpiResult<RerankMemoryHitsResult>;
}

#[async_trait]
pub trait ExternalMemoryBridgePort: Send + Sync {
    fn provider_code(&self) -> &str;

    async fn import(
        &self,
        command: ExternalMemoryImportCommand,
    ) -> MemorySpiResult<ExternalMemoryImportResult>;

    async fn export(
        &self,
        command: ExternalMemoryExportCommand,
    ) -> MemorySpiResult<ExternalMemoryExportResult>;

    async fn delete(
        &self,
        command: ExternalMemoryDeleteCommand,
    ) -> MemorySpiResult<ExternalMemoryDeleteReceipt>;

    async fn shadow_read(
        &self,
        command: ExternalMemoryShadowReadCommand,
    ) -> MemorySpiResult<ExternalMemoryShadowReadResult>;
}

#[async_trait]
pub trait MemoryContextAssemblerPort: Send + Sync {
    async fn assemble(
        &self,
        command: AssembleMemoryContextCommand,
    ) -> MemorySpiResult<MemoryContextPackDraft>;

    async fn assemble_scoped(
        &self,
        _scope: MemoryScopeContext,
        _command: AssembleMemoryContextCommand,
    ) -> MemorySpiResult<MemoryContextPackDraft> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryContextAssemblerPort".to_string(),
            message:
                "scope-aware context assembly is not implemented; refusing an unscoped fallback"
                    .to_string(),
        })
    }
}

#[async_trait]
pub trait MemoryEvaluationPort: Send + Sync {
    async fn run(&self, command: RunMemoryEvalCommand) -> MemorySpiResult<MemoryEvalRunResult>;
}

fn atomic_record_operation_required(operation: &str) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: "MemoryRecordStorePort".to_string(),
        message: format!(
            "atomic canonical memory operation {operation} is not implemented; refusing non-atomic fallback"
        ),
    }
}
