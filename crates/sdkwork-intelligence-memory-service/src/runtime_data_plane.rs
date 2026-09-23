use std::sync::Arc;

use sdkwork_memory_contract::{MemoryServiceError, MemoryServiceResult};
use sdkwork_memory_spi::{
    AppendMemoryAuditCommand, AppendMemoryOutboxCommand, AppendMemoryRetrievalTraceCommand,
    ApproveMemoryCandidateCommand, AssembleMemoryContextCommand, CountActiveMemoryRecordsQuery,
    CountUserOwnedMemorySpacesQuery, CreateCanonicalMemoryCommand, CreateMemoryCandidateCommand,
    CreateMemoryRecordCommand, CreateMemorySpaceCommand, DecayMemoryHabitCommand,
    DeleteAllCanonicalMemoryCommand, DeleteCanonicalMemoryCommand, DeleteMemoryRecordCommand,
    ExternalMemoryBridgePort, ListMemoryCandidatesQuery, ListMemoryRetrievalTracesQuery,
    ListPendingMemoryOutboxQuery, MarkMemoryOutboxFailedCommand, MarkMemoryOutboxPublishedCommand,
    MemoryAuditRecord, MemoryBulkDeletionReceipt, MemoryCandidate, MemoryCandidateDetail,
    MemoryCandidatePage, MemoryCandidatePromotion, MemoryCanonicalRecord,
    MemoryContextAssemblerPort, MemoryContextPackDraft, MemoryCoreRuntime, MemoryDeletionReceipt,
    MemoryGovernanceAccessPort, MemoryHabit, MemoryOutboxEvent, MemoryRecord,
    MemoryRecordQuotaAdmission, MemoryRetrieverPort, MemoryRetrieverResult,
    MemoryRetrieverSearchResult, MemoryRuntimeProfileMetadata, MemorySpaceGovernanceFacts,
    MemorySpaceQuotaAdmission, MemorySpaceRecord, MemorySpaceStorePort, MemorySpiError,
    PromoteMemoryCandidateAtomicCommand, PromoteMemoryCandidateAtomicWithJournalCommand,
    PromoteMemoryHabitCommand, RejectMemoryCandidateCommand, ResolveMemorySpaceGovernanceQuery,
    RetrieveCanonicalMemoryQuery, RetrieveMemoryAuditQuery, RetrieveMemoryCandidateDetailQuery,
    RetrieveMemoryCandidateQuery, RetrieveMemoryCandidatesCommand, RetrieveMemoryHabitQuery,
    RetrieveMemoryOutboxQuery, RetrieveMemoryRecordQuery,
    RetrieveMemoryRetrievalTraceForTenantQuery, RetrieveMemoryRetrievalTraceQuery,
    ScopedMemoryRetrievalTrace, SearchMemoryCandidatesQuery, SupersedeCanonicalMemoryAtomicCommand,
    UpdateCanonicalMemoryCommand, UpsertMemoryHabitCommand,
};
use thiserror::Error;

use crate::store_error::map_memory_spi_error;

pub const PHASE1_HTTP_DATA_PLANE_PORTS: &[&str] = &[
    "MemoryRecordStorePort",
    "MemoryEventStorePort",
    "MemoryAuditStorePort",
    "MemoryOutboxStorePort",
    "MemoryCandidateStorePort",
    "MemoryHabitStorePort",
    "MemoryRetrievalTraceStorePort",
    "MemoryGovernanceAccessPort",
    "MemorySpaceStorePort",
    "MemoryRetrieverPort",
];

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MemoryRuntimeDataPlaneError {
    #[error("memory runtime profile {profile_id} is missing required data-plane port {port}")]
    MissingRequiredPort { profile_id: String, port: String },
    #[error(
        "memory runtime profile {profile_id} does not support required capability {capability}"
    )]
    RequiredCapabilityMissing {
        profile_id: String,
        capability: String,
    },
}

/// Service-facing facade over the typed executable ports selected by a runtime profile.
///
/// The facade keeps SQL/provider implementations out of use-case call sites and centralizes
/// fail-closed dispatch and SPI error mapping. Production HTTP composition validates the complete
/// Phase-1 port set once during startup; evaluation profiles may construct the facade without that
/// qualification and exercise only the ports they declare.
#[derive(Debug, Clone)]
pub struct MemoryRuntimeDataPlane {
    runtime: MemoryCoreRuntime,
}

impl MemoryRuntimeDataPlane {
    pub fn from_core_runtime(runtime: MemoryCoreRuntime) -> Self {
        Self { runtime }
    }

    pub fn try_for_phase1_http(
        runtime: MemoryCoreRuntime,
    ) -> Result<Self, MemoryRuntimeDataPlaneError> {
        for port in PHASE1_HTTP_DATA_PLANE_PORTS {
            if !runtime.has_port(port) {
                return Err(MemoryRuntimeDataPlaneError::MissingRequiredPort {
                    profile_id: runtime.profile().profile_id.clone(),
                    port: (*port).to_string(),
                });
            }
        }
        let record_store = runtime
            .record_store()
            .expect("record store was checked above");
        if !record_store.supports_canonical_atomic() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "canonical_memory_atomic_mutation".to_string(),
            });
        }
        if !record_store.supports_atomic_record_quota_admission() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "canonical_memory_atomic_quota_admission".to_string(),
            });
        }
        if !record_store.supports_atomic_supersede() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "atomic_canonical_supersede".to_string(),
            });
        }
        let candidate_store = runtime
            .candidate_store()
            .expect("candidate store was checked above");
        if !candidate_store.supports_atomic_candidate_promotion() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "atomic_candidate_promotion".to_string(),
            });
        }
        if !candidate_store.supports_candidate_detail_lookup() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "tenant_scoped_candidate_detail_lookup".to_string(),
            });
        }
        if !candidate_store.supports_candidate_listing() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "tenant_scoped_candidate_listing".to_string(),
            });
        }
        if !candidate_store.supports_atomic_candidate_promotion_journal() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "atomic_candidate_promotion_journal".to_string(),
            });
        }
        let retriever = runtime.retriever().expect("retriever was checked above");
        if !retriever.supports_bounded_scoped_search() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "bounded_scope_aware_retrieval".to_string(),
            });
        }
        let trace_store = runtime
            .retrieval_trace_store()
            .expect("retrieval trace store was checked above");
        if !trace_store.supports_tenant_trace_lookup() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "tenant_scoped_retrieval_trace_lookup".to_string(),
            });
        }
        let governance_access = runtime
            .governance_access()
            .expect("governance access port was checked above");
        if !governance_access.supports_bounded_governance_access() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "bounded_tenant_scoped_governance_access".to_string(),
            });
        }
        let space_store = runtime
            .space_store()
            .expect("space store was checked above");
        if !space_store.supports_atomic_user_space_quota_admission() {
            return Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
                profile_id: runtime.profile().profile_id.clone(),
                capability: "atomic_user_space_quota_admission".to_string(),
            });
        }
        Ok(Self { runtime })
    }

    pub fn profile(&self) -> &MemoryRuntimeProfileMetadata {
        self.runtime.profile()
    }

    pub fn core_runtime(&self) -> &MemoryCoreRuntime {
        &self.runtime
    }

    pub async fn create_record(
        &self,
        command: CreateMemoryRecordCommand,
    ) -> MemoryServiceResult<MemoryRecord> {
        self.require_record_store()?
            .create(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_record(
        &self,
        query: RetrieveMemoryRecordQuery,
    ) -> MemoryServiceResult<Option<MemoryRecord>> {
        self.require_record_store()?
            .retrieve(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn delete_record(
        &self,
        command: DeleteMemoryRecordCommand,
    ) -> MemoryServiceResult<MemoryDeletionReceipt> {
        self.require_record_store()?
            .mark_deleted(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn create_canonical_memory_atomic(
        &self,
        command: CreateCanonicalMemoryCommand,
    ) -> MemoryServiceResult<MemoryCanonicalRecord> {
        self.require_record_store()?
            .create_canonical_atomic(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn create_canonical_memory_atomic_with_quota(
        &self,
        command: CreateCanonicalMemoryCommand,
        max_active_records: u64,
    ) -> MemoryServiceResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        self.require_record_store()?
            .create_canonical_atomic_with_quota(command, max_active_records)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn supersede_canonical_memory_atomic_with_quota(
        &self,
        command: SupersedeCanonicalMemoryAtomicCommand,
        max_active_records: u64,
    ) -> MemoryServiceResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        self.require_record_store()?
            .supersede_canonical_atomic_with_quota(command, max_active_records)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_canonical_memory(
        &self,
        query: RetrieveCanonicalMemoryQuery,
    ) -> MemoryServiceResult<Option<MemoryCanonicalRecord>> {
        self.require_record_store()?
            .retrieve_canonical(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn update_canonical_memory_atomic(
        &self,
        command: UpdateCanonicalMemoryCommand,
    ) -> MemoryServiceResult<Option<MemoryCanonicalRecord>> {
        self.require_record_store()?
            .update_canonical_atomic(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn delete_canonical_memory_atomic(
        &self,
        command: DeleteCanonicalMemoryCommand,
    ) -> MemoryServiceResult<MemoryDeletionReceipt> {
        self.require_record_store()?
            .delete_canonical_atomic(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn delete_all_canonical_memory_atomic(
        &self,
        command: DeleteAllCanonicalMemoryCommand,
    ) -> MemoryServiceResult<MemoryBulkDeletionReceipt> {
        self.require_record_store()?
            .delete_all_canonical_atomic(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn append_audit(
        &self,
        command: AppendMemoryAuditCommand,
    ) -> MemoryServiceResult<MemoryAuditRecord> {
        self.require_audit_store()?
            .append(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_audit(
        &self,
        query: RetrieveMemoryAuditQuery,
    ) -> MemoryServiceResult<Option<MemoryAuditRecord>> {
        self.require_audit_store()?
            .retrieve(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn append_outbox(
        &self,
        command: AppendMemoryOutboxCommand,
    ) -> MemoryServiceResult<MemoryOutboxEvent> {
        self.require_outbox_store()?
            .append(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_outbox(
        &self,
        query: RetrieveMemoryOutboxQuery,
    ) -> MemoryServiceResult<Option<MemoryOutboxEvent>> {
        self.require_outbox_store()?
            .retrieve(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn list_pending_outbox(
        &self,
        query: ListPendingMemoryOutboxQuery,
    ) -> MemoryServiceResult<Vec<MemoryOutboxEvent>> {
        self.require_outbox_store()?
            .list_pending(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn mark_outbox_published(
        &self,
        command: MarkMemoryOutboxPublishedCommand,
    ) -> MemoryServiceResult<Option<MemoryOutboxEvent>> {
        self.require_outbox_store()?
            .mark_published(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn mark_outbox_failed(
        &self,
        command: MarkMemoryOutboxFailedCommand,
    ) -> MemoryServiceResult<Option<MemoryOutboxEvent>> {
        self.require_outbox_store()?
            .mark_failed(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn create_candidate(
        &self,
        command: CreateMemoryCandidateCommand,
    ) -> MemoryServiceResult<MemoryCandidate> {
        self.require_candidate_store()?
            .create(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_candidate(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemoryServiceResult<Option<MemoryCandidate>> {
        self.require_candidate_store()?
            .retrieve(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_candidate_detail(
        &self,
        query: RetrieveMemoryCandidateDetailQuery,
    ) -> MemoryServiceResult<Option<MemoryCandidateDetail>> {
        self.require_candidate_store()?
            .retrieve_detail(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn list_candidates(
        &self,
        query: ListMemoryCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidatePage> {
        self.require_candidate_store()?
            .list_candidates(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn promote_candidate_atomic_with_quota(
        &self,
        command: PromoteMemoryCandidateAtomicCommand,
        max_active_records: u64,
    ) -> MemoryServiceResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        self.require_candidate_store()?
            .promote_atomic_with_quota(command, max_active_records)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn promote_candidate_atomic_with_quota_and_journal(
        &self,
        command: PromoteMemoryCandidateAtomicWithJournalCommand,
        max_active_records: u64,
    ) -> MemoryServiceResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        self.require_candidate_store()?
            .promote_atomic_with_quota_and_journal(command, max_active_records)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn approve_candidate(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemoryServiceResult<Option<MemoryCandidate>> {
        self.require_candidate_store()?
            .approve(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn reject_candidate(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemoryServiceResult<Option<MemoryCandidate>> {
        self.require_candidate_store()?
            .reject(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn upsert_habit(
        &self,
        command: UpsertMemoryHabitCommand,
    ) -> MemoryServiceResult<MemoryHabit> {
        self.require_habit_store()?
            .upsert(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_habit(
        &self,
        query: RetrieveMemoryHabitQuery,
    ) -> MemoryServiceResult<Option<MemoryHabit>> {
        self.require_habit_store()?
            .retrieve(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn promote_habit(
        &self,
        command: PromoteMemoryHabitCommand,
    ) -> MemoryServiceResult<Option<MemoryHabit>> {
        self.require_habit_store()?
            .promote(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn decay_habit(
        &self,
        command: DecayMemoryHabitCommand,
    ) -> MemoryServiceResult<Option<MemoryHabit>> {
        self.require_habit_store()?
            .decay(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn append_retrieval_trace(
        &self,
        command: AppendMemoryRetrievalTraceCommand,
    ) -> MemoryServiceResult<sdkwork_memory_spi::MemoryRetrievalTrace> {
        self.require_retrieval_trace_store()?
            .append(command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_retrieval_trace(
        &self,
        query: RetrieveMemoryRetrievalTraceQuery,
    ) -> MemoryServiceResult<Option<sdkwork_memory_spi::MemoryRetrievalTrace>> {
        self.require_retrieval_trace_store()?
            .retrieve(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_retrieval_trace_for_tenant(
        &self,
        query: RetrieveMemoryRetrievalTraceForTenantQuery,
    ) -> MemoryServiceResult<Option<ScopedMemoryRetrievalTrace>> {
        self.require_retrieval_trace_store()?
            .retrieve_for_tenant(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn list_recent_retrieval_traces(
        &self,
        query: ListMemoryRetrievalTracesQuery,
    ) -> MemoryServiceResult<Vec<sdkwork_memory_spi::MemoryRetrievalTrace>> {
        self.require_retrieval_trace_store()?
            .list_recent(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn resolve_space_governance(
        &self,
        query: ResolveMemorySpaceGovernanceQuery,
    ) -> MemoryServiceResult<MemorySpaceGovernanceFacts> {
        self.require_governance_access()?
            .resolve_space_governance(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn count_active_records(
        &self,
        query: CountActiveMemoryRecordsQuery,
    ) -> MemoryServiceResult<u64> {
        self.require_governance_access()?
            .count_active_records(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn count_user_owned_spaces(
        &self,
        query: CountUserOwnedMemorySpacesQuery,
    ) -> MemoryServiceResult<u64> {
        self.require_governance_access()?
            .count_user_owned_spaces(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn create_space_atomic_with_quota(
        &self,
        command: CreateMemorySpaceCommand,
        max_active_spaces: u64,
    ) -> MemoryServiceResult<MemorySpaceQuotaAdmission<MemorySpaceRecord>> {
        self.require_space_store()?
            .create_space_atomic_with_quota(command, max_active_spaces)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn retrieve_candidates_scoped(
        &self,
        scope: sdkwork_memory_spi::MemoryScopeContext,
        command: RetrieveMemoryCandidatesCommand,
    ) -> MemoryServiceResult<MemoryRetrieverResult> {
        self.require_retriever()?
            .retrieve_scoped(scope, command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn search_candidates_scoped(
        &self,
        query: SearchMemoryCandidatesQuery,
    ) -> MemoryServiceResult<MemoryRetrieverSearchResult> {
        self.require_retriever()?
            .search_scoped(query)
            .await
            .map_err(map_memory_spi_error)
    }

    pub async fn assemble_context_scoped(
        &self,
        scope: sdkwork_memory_spi::MemoryScopeContext,
        command: AssembleMemoryContextCommand,
    ) -> MemoryServiceResult<MemoryContextPackDraft> {
        self.require_context_assembler()?
            .assemble_scoped(scope, command)
            .await
            .map_err(map_memory_spi_error)
    }

    pub fn external_memory_bridge(&self) -> MemoryServiceResult<Arc<dyn ExternalMemoryBridgePort>> {
        self.runtime
            .external_memory_bridge()
            .ok_or_else(|| self.missing_runtime_port("ExternalMemoryBridgePort"))
    }

    fn require_record_store(
        &self,
    ) -> MemoryServiceResult<Arc<dyn sdkwork_memory_spi::MemoryRecordStorePort>> {
        self.runtime
            .record_store()
            .ok_or_else(|| self.missing_runtime_port("MemoryRecordStorePort"))
    }

    fn require_audit_store(
        &self,
    ) -> MemoryServiceResult<Arc<dyn sdkwork_memory_spi::MemoryAuditStorePort>> {
        self.runtime
            .audit_store()
            .ok_or_else(|| self.missing_runtime_port("MemoryAuditStorePort"))
    }

    fn require_outbox_store(
        &self,
    ) -> MemoryServiceResult<Arc<dyn sdkwork_memory_spi::MemoryOutboxStorePort>> {
        self.runtime
            .outbox_store()
            .ok_or_else(|| self.missing_runtime_port("MemoryOutboxStorePort"))
    }

    fn require_candidate_store(
        &self,
    ) -> MemoryServiceResult<Arc<dyn sdkwork_memory_spi::MemoryCandidateStorePort>> {
        self.runtime
            .candidate_store()
            .ok_or_else(|| self.missing_runtime_port("MemoryCandidateStorePort"))
    }

    fn require_habit_store(
        &self,
    ) -> MemoryServiceResult<Arc<dyn sdkwork_memory_spi::MemoryHabitStorePort>> {
        self.runtime
            .habit_store()
            .ok_or_else(|| self.missing_runtime_port("MemoryHabitStorePort"))
    }

    fn require_retrieval_trace_store(
        &self,
    ) -> MemoryServiceResult<Arc<dyn sdkwork_memory_spi::MemoryRetrievalTraceStorePort>> {
        self.runtime
            .retrieval_trace_store()
            .ok_or_else(|| self.missing_runtime_port("MemoryRetrievalTraceStorePort"))
    }

    fn require_retriever(&self) -> MemoryServiceResult<Arc<dyn MemoryRetrieverPort>> {
        self.runtime
            .retriever()
            .ok_or_else(|| self.missing_runtime_port("MemoryRetrieverPort"))
    }

    fn require_governance_access(
        &self,
    ) -> MemoryServiceResult<Arc<dyn MemoryGovernanceAccessPort>> {
        self.runtime
            .governance_access()
            .ok_or_else(|| self.missing_runtime_port("MemoryGovernanceAccessPort"))
    }

    fn require_space_store(&self) -> MemoryServiceResult<Arc<dyn MemorySpaceStorePort>> {
        self.runtime
            .space_store()
            .ok_or_else(|| self.missing_runtime_port("MemorySpaceStorePort"))
    }

    fn require_context_assembler(
        &self,
    ) -> MemoryServiceResult<Arc<dyn MemoryContextAssemblerPort>> {
        self.runtime
            .context_assembler()
            .ok_or_else(|| self.missing_runtime_port("MemoryContextAssemblerPort"))
    }

    fn missing_runtime_port(&self, port: &'static str) -> MemoryServiceError {
        let error = MemorySpiError::ExecutablePortMissing {
            plugin_id: self.runtime.profile().primary_plugin_id.clone(),
            port: port.to_string(),
        };
        map_memory_spi_error(error)
    }
}
