use std::collections::{BTreeSet, HashMap};
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sdkwork_memory_spi::{
    AppendMemoryAuditCommand, AppendMemoryEventCommand, AppendMemoryOutboxCommand,
    AppendMemoryRetrievalTraceCommand, ApproveMemoryCandidateCommand, AssembleMemoryContextCommand,
    CountActiveMemoryRecordsQuery, CountUserOwnedMemorySpacesQuery, CreateCanonicalMemoryCommand,
    CreateMemoryCandidateCommand, CreateMemoryRecordCommand, CreateMemorySpaceCommand,
    DecayMemoryHabitCommand, DeleteAllCanonicalMemoryCommand, DeleteCanonicalMemoryCommand,
    DeleteMemoryRecordCommand, ExternalMemoryBridgePort, ExternalMemoryDeleteCommand,
    ExternalMemoryDeleteReceipt, ExternalMemoryExportCommand, ExternalMemoryExportResult,
    ExternalMemoryImportCommand, ExternalMemoryImportResult, ExternalMemoryShadowReadCommand,
    ExternalMemoryShadowReadResult, ListMemoryCandidatesQuery, ListMemoryRetrievalTracesQuery,
    ListPendingMemoryOutboxQuery, MarkMemoryOutboxFailedCommand, MarkMemoryOutboxPublishedCommand,
    MemoryAuditRecord, MemoryAuditStorePort, MemoryBulkDeletionReceipt, MemoryCandidate,
    MemoryCandidateDetail, MemoryCandidatePage, MemoryCandidatePromotion, MemoryCandidateStorePort,
    MemoryCandidateSummary, MemoryCanonicalRecord, MemoryContextAssemblerPort,
    MemoryContextPackDraft, MemoryDeletionReceipt, MemoryEvalRunResult, MemoryEvaluationPort,
    MemoryEvent, MemoryEventStorePort, MemoryGovernanceAccessPort, MemoryGovernanceActor,
    MemoryHabit, MemoryHabitStorePort, MemoryIndexPort, MemoryIndexReceipt, MemoryMutationJournal,
    MemoryOutboxEvent, MemoryOutboxStorePort, MemoryPluginPorts, MemoryRecord,
    MemoryRecordQuotaAdmission, MemoryRecordStorePort, MemoryRetrievalEventCandidate,
    MemoryRetrievalRecordCandidate, MemoryRetrievalTrace, MemoryRetrievalTraceStorePort,
    MemoryRetrieverKind, MemoryRetrieverPort, MemoryRetrieverResult, MemoryRetrieverSearchResult,
    MemoryScopeContext, MemorySensitivityReadScope, MemorySpaceGovernanceFact,
    MemorySpaceGovernanceFacts, MemorySpaceQuotaAdmission, MemorySpaceRecord, MemorySpaceStorePort,
    MemorySpiError, MemorySpiResult, PromoteMemoryCandidateAtomicCommand,
    PromoteMemoryCandidateAtomicWithJournalCommand, PromoteMemoryHabitCommand,
    RejectMemoryCandidateCommand, ResolveMemorySpaceGovernanceQuery, RetrieveCanonicalMemoryQuery,
    RetrieveMemoryAuditQuery, RetrieveMemoryCandidateDetailQuery, RetrieveMemoryCandidateQuery,
    RetrieveMemoryCandidatesCommand, RetrieveMemoryEventQuery, RetrieveMemoryHabitQuery,
    RetrieveMemoryOutboxQuery, RetrieveMemoryRecordQuery,
    RetrieveMemoryRetrievalTraceForTenantQuery, RetrieveMemoryRetrievalTraceQuery,
    RunMemoryEvalCommand, ScopedMemoryRetrievalTrace, SearchMemoryCandidatesQuery,
    SupersedeCanonicalMemoryAtomicCommand, UpdateCanonicalMemoryCommand, UpsertMemoryHabitCommand,
    MAX_MEMORY_GOVERNANCE_FACTS, MAX_MEMORY_RETRIEVAL_CANDIDATES,
};
use serde_json::Value;

#[derive(Debug, Default)]
pub struct ReferenceMemoryRuntime {
    records: Mutex<HashMap<ScopedId, MemoryRecordState>>,
    events: Mutex<HashMap<ScopedId, MemoryEvent>>,
    audits: Mutex<HashMap<ScopedId, MemoryAuditRecord>>,
    outbox: Mutex<HashMap<ScopedId, MemoryOutboxEvent>>,
    candidates: Mutex<HashMap<ScopedId, MemoryCandidate>>,
    candidate_timestamps: Mutex<HashMap<ScopedId, (String, String)>>,
    candidate_targets: Mutex<HashMap<ScopedId, String>>,
    candidate_listing_index: Mutex<BTreeSet<TenantCandidateListKey>>,
    candidate_listing_ambiguous_tenants: Mutex<BTreeSet<i64>>,
    candidate_space_listing_index: Mutex<BTreeSet<SpaceCandidateListKey>>,
    habits: Mutex<HashMap<ScopedHabitKey, MemoryHabit>>,
    retrieval_traces: Mutex<HashMap<ScopedId, MemoryRetrievalTrace>>,
    governance_spaces: Mutex<HashMap<GovernanceSpaceKey, MemorySpaceGovernanceFact>>,
    governance_bindings: Mutex<
        HashMap<GovernanceActorSpaceKey, Vec<sdkwork_memory_spi::MemoryActorSpaceBindingFact>>,
    >,
    governance_capabilities: Mutex<
        HashMap<GovernanceCapabilityKey, Vec<sdkwork_memory_spi::MemoryCapabilityBindingFact>>,
    >,
}

impl ReferenceMemoryRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed_governance_space(
        &self,
        tenant_id: i64,
        fact: MemorySpaceGovernanceFact,
    ) -> MemorySpiResult<()> {
        self.governance_spaces
            .lock()
            .map_err(lock_error)?
            .insert(GovernanceSpaceKey::new(tenant_id, fact.space_id), fact);
        Ok(())
    }

    pub fn seed_actor_space_binding(
        &self,
        scope: &MemoryScopeContext,
        actor: &MemoryGovernanceActor,
        fact: sdkwork_memory_spi::MemoryActorSpaceBindingFact,
    ) -> MemorySpiResult<()> {
        self.governance_bindings
            .lock()
            .map_err(lock_error)?
            .entry(GovernanceActorSpaceKey::new(scope, actor))
            .or_default()
            .push(fact);
        Ok(())
    }

    pub fn seed_capability_binding(
        &self,
        scope: &MemoryScopeContext,
        fact: sdkwork_memory_spi::MemoryCapabilityBindingFact,
    ) -> MemorySpiResult<()> {
        let key = GovernanceCapabilityKey::new(scope, &fact.capability_code);
        self.governance_capabilities
            .lock()
            .map_err(lock_error)?
            .entry(key)
            .or_default()
            .push(fact);
        Ok(())
    }
}

pub fn build_reference_executable_runtime(
    runtime: Arc<ReferenceMemoryRuntime>,
) -> sdkwork_memory_spi::MemoryExecutablePluginRuntime {
    sdkwork_memory_spi::MemoryExecutablePluginRuntime::new(
        MemoryPluginPorts::new()
            .with_record_store(runtime.clone())
            .with_event_store(runtime.clone())
            .with_audit_store(runtime.clone())
            .with_outbox_store(runtime.clone())
            .with_candidate_store(runtime.clone())
            .with_habit_store(runtime.clone())
            .with_retrieval_trace_store(runtime.clone())
            .with_governance_access(runtime.clone())
            .with_space_store(runtime.clone())
            .with_retriever(runtime.clone())
            .with_context_assembler(runtime),
    )
}

#[async_trait]
impl MemoryGovernanceAccessPort for ReferenceMemoryRuntime {
    fn supports_bounded_governance_access(&self) -> bool {
        true
    }

    async fn resolve_space_governance(
        &self,
        query: ResolveMemorySpaceGovernanceQuery,
    ) -> MemorySpiResult<MemorySpaceGovernanceFacts> {
        let fact_limit = validate_governance_fact_limit(query.fact_limit)?;
        let space_key = GovernanceSpaceKey::new(query.scope.tenant_id, query.scope.space_id);
        let space = self
            .governance_spaces
            .lock()
            .map_err(lock_error)?
            .get(&space_key)
            .cloned();

        let mut complete = true;
        let actor_bindings = if let Some(actor) = &query.actor {
            let mut facts = self
                .governance_bindings
                .lock()
                .map_err(lock_error)?
                .get(&GovernanceActorSpaceKey::new(&query.scope, actor))
                .cloned()
                .unwrap_or_default();
            facts.sort_by(|left, right| left.binding_id.cmp(&right.binding_id));
            if facts.len() > fact_limit {
                complete = false;
                facts.truncate(fact_limit);
            }
            facts
        } else {
            Vec::new()
        };

        let capability_bindings = if let Some(capability_code) = &query.capability_code {
            let mut facts = self
                .governance_capabilities
                .lock()
                .map_err(lock_error)?
                .get(&GovernanceCapabilityKey::new(&query.scope, capability_code))
                .cloned()
                .unwrap_or_default();
            facts.sort_by(|left, right| {
                right
                    .priority
                    .cmp(&left.priority)
                    .then_with(|| left.binding_id.cmp(&right.binding_id))
            });
            if facts.len() > fact_limit {
                complete = false;
                facts.truncate(fact_limit);
            }
            facts
        } else {
            Vec::new()
        };

        Ok(MemorySpaceGovernanceFacts {
            space,
            actor_bindings,
            capability_bindings,
            complete,
        })
    }

    async fn count_active_records(
        &self,
        query: CountActiveMemoryRecordsQuery,
    ) -> MemorySpiResult<u64> {
        let records = self.records.lock().map_err(lock_error)?;
        Ok(records
            .iter()
            .filter(|(key, state)| key.matches_scope(&query.scope) && !state.deleted)
            .count() as u64)
    }

    async fn count_user_owned_spaces(
        &self,
        query: CountUserOwnedMemorySpacesQuery,
    ) -> MemorySpiResult<u64> {
        let spaces = self.governance_spaces.lock().map_err(lock_error)?;
        Ok(spaces
            .iter()
            .filter(|(key, fact)| {
                key.tenant_id == query.tenant_id
                    && fact.owner_subject_type == "user"
                    && fact.owner_subject_id == query.owner_subject_id
                    && fact.lifecycle_status != "deleted"
            })
            .count() as u64)
    }
}

#[async_trait]
impl MemorySpaceStorePort for ReferenceMemoryRuntime {
    fn supports_atomic_user_space_quota_admission(&self) -> bool {
        true
    }

    async fn create_space_atomic_with_quota(
        &self,
        command: CreateMemorySpaceCommand,
        max_active_spaces: u64,
    ) -> MemorySpiResult<MemorySpaceQuotaAdmission<MemorySpaceRecord>> {
        validate_space_command(&command)?;
        let key = GovernanceSpaceKey::new(command.tenant_id, command.space_id);
        let mut spaces = self.governance_spaces.lock().map_err(lock_error)?;
        let active_spaces = if command.owner_subject_type == "user" {
            spaces
                .iter()
                .filter(|(space_key, fact)| {
                    space_key.tenant_id == command.tenant_id
                        && fact.owner_subject_type == "user"
                        && fact.owner_subject_id == command.owner_subject_id
                        && fact.lifecycle_status != "deleted"
                })
                .count() as u64
        } else {
            0
        };
        if command.owner_subject_type == "user"
            && max_active_spaces > 0
            && active_spaces >= max_active_spaces
        {
            return Ok(MemorySpaceQuotaAdmission::QuotaExceeded {
                active_spaces,
                max_active_spaces,
            });
        }
        if spaces.contains_key(&key) {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemorySpaceStorePort".to_string(),
                message: format!(
                    "memory space {} already exists for tenant {}",
                    command.space_id, command.tenant_id
                ),
            });
        }

        let timestamp = now_text();
        spaces.insert(
            key,
            MemorySpaceGovernanceFact {
                space_id: command.space_id,
                organization_id: command.organization_id,
                owner_subject_type: command.owner_subject_type.clone(),
                owner_subject_id: command.owner_subject_id.clone(),
                lifecycle_status: "active".to_string(),
            },
        );
        Ok(MemorySpaceQuotaAdmission::Admitted(MemorySpaceRecord {
            space_id: command.space_id,
            uuid: format!("space-{}", command.space_id),
            tenant_id: command.tenant_id,
            organization_id: command.organization_id,
            owner_subject_type: command.owner_subject_type,
            owner_subject_id: command.owner_subject_id,
            space_type: command.space_type,
            display_name: command.display_name,
            default_scope: command.default_scope,
            lifecycle_status: "active".to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            version: 0,
        }))
    }
}

#[async_trait]
impl MemoryRecordStorePort for ReferenceMemoryRuntime {
    fn supports_canonical_atomic(&self) -> bool {
        true
    }

    fn supports_atomic_record_quota_admission(&self) -> bool {
        true
    }

    fn supports_atomic_supersede(&self) -> bool {
        true
    }

    async fn create(&self, command: CreateMemoryRecordCommand) -> MemorySpiResult<MemoryRecord> {
        let record = MemoryRecord {
            memory_id: command.memory_id.clone(),
            content: command.content,
        };
        let key = ScopedId::new(&command.scope, command.memory_id);
        self.records
            .lock()
            .map_err(lock_error)?
            .insert(key, MemoryRecordState::active(record.clone()));

        Ok(record)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryRecordQuery,
    ) -> MemorySpiResult<Option<MemoryRecord>> {
        let key = ScopedId::new(&query.scope, query.memory_id);
        let records = self.records.lock().map_err(lock_error)?;
        Ok(records
            .get(&key)
            .and_then(MemoryRecordState::visible_record))
    }

    async fn mark_deleted(
        &self,
        command: DeleteMemoryRecordCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        let key = ScopedId::new(&command.scope, command.memory_id.clone());
        let mut records = self.records.lock().map_err(lock_error)?;

        let Some(record) = records.get_mut(&key) else {
            return Ok(MemoryDeletionReceipt {
                memory_id: command.memory_id,
                deleted: false,
                already_deleted: false,
            });
        };

        let already_deleted = record.deleted;
        record.deleted = true;

        Ok(MemoryDeletionReceipt {
            memory_id: command.memory_id,
            deleted: true,
            already_deleted,
        })
    }

    async fn create_canonical_atomic(
        &self,
        command: CreateCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryCanonicalRecord> {
        match self.create_canonical_atomic_with_quota(command, 0).await? {
            MemoryRecordQuotaAdmission::Admitted(record) => Ok(record),
            MemoryRecordQuotaAdmission::QuotaExceeded { .. } => {
                Err(MemorySpiError::PortOperationFailed {
                    port: "MemoryRecordStorePort".to_string(),
                    message: "unlimited canonical memory mutation was rejected by quota admission"
                        .to_string(),
                })
            }
        }
    }

    async fn create_canonical_atomic_with_quota(
        &self,
        command: CreateCanonicalMemoryCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        validate_memory_journal(&command.memory_id, &command.journal)?;
        let timestamp = now_text();
        let canonical = MemoryCanonicalRecord {
            memory_id: command.memory_id.clone(),
            space_id: command.scope.space_id,
            user_id: command.scope.user_id,
            scope_label: command.scope_label,
            memory_type: command.memory_type,
            subject: command.subject,
            predicate: command.predicate.or_else(|| Some("is".to_string())),
            object_text: command.object_text.clone(),
            canonical_text: command.canonical_text,
            confidence: 1.0,
            evidence_count: 1,
            contradiction_count: 0,
            status: "active".to_string(),
            sensitivity_level: command.sensitivity_level,
            supersedes_memory_id: None,
            superseded_by_memory_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            version: 1,
            expires_at: command.expires_at,
            metadata_json: command.metadata_json,
        };
        let key = ScopedId::new(&command.scope, command.memory_id);
        let (outbox_key, outbox, audit_key, audit) =
            reference_journal_entries(&command.scope, command.journal);

        let mut records = self.records.lock().map_err(lock_error)?;
        let active_records = records
            .iter()
            .filter(|(record_key, state)| {
                record_key.matches_scope(&command.scope) && !state.deleted
            })
            .count() as u64;
        if max_active_records > 0 && active_records >= max_active_records {
            return Ok(MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            });
        }
        let mut outbox_store = self.outbox.lock().map_err(lock_error)?;
        let mut audit_store = self.audits.lock().map_err(lock_error)?;
        records.insert(key, MemoryRecordState::active_canonical(canonical.clone()));
        outbox_store.insert(outbox_key, outbox);
        audit_store.insert(audit_key, audit);
        Ok(MemoryRecordQuotaAdmission::Admitted(canonical))
    }

    async fn supersede_canonical_atomic_with_quota(
        &self,
        command: SupersedeCanonicalMemoryAtomicCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        validate_memory_journal(&command.new_memory_id, &command.created_journal)?;
        validate_memory_journal(&command.old_memory_id, &command.superseded_journal)?;
        if command.created_journal.outbox_id == command.superseded_journal.outbox_id
            || command.created_journal.audit_id == command.superseded_journal.audit_id
        {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRecordStorePort".to_string(),
                message: "supersede journals must use distinct outbox and audit ids".to_string(),
            });
        }
        if command.old_memory_id == command.new_memory_id {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRecordStorePort".to_string(),
                message: "supersede source and target memory ids must differ".to_string(),
            });
        }

        let old_key = ScopedId::new(&command.scope, command.old_memory_id.clone());
        let new_key = ScopedId::new(&command.scope, command.new_memory_id.clone());
        let mut records = self.records.lock().map_err(lock_error)?;
        let Some(old_state) = records.get(&old_key) else {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRecordStorePort".to_string(),
                message: format!(
                    "supersede source memory {} not found",
                    command.old_memory_id
                ),
            });
        };
        let Some(old_canonical) = old_state.canonical.as_ref() else {
            return Err(atomic_record_state_missing());
        };
        if let Some(existing_state) = records.get(&new_key) {
            if let Some(existing) = existing_state.canonical.as_ref() {
                if !existing_state.deleted
                    && existing.status == "active"
                    && existing.supersedes_memory_id.as_deref()
                        == Some(command.old_memory_id.as_str())
                    && existing.superseded_by_memory_id.is_none()
                    && !old_state.deleted
                    && old_canonical.status == "superseded"
                    && old_canonical.superseded_by_memory_id.as_deref()
                        == Some(command.new_memory_id.as_str())
                {
                    if !reference_supersede_target_matches(existing, &command)
                        || !reference_supersede_journals_match(
                            self,
                            &command.scope,
                            &command.created_journal,
                            &command.superseded_journal,
                        )?
                    {
                        return Err(MemorySpiError::IdempotencyConflict {
                            idempotency_key: command.new_memory_id,
                        });
                    }
                    return Ok(MemoryRecordQuotaAdmission::Admitted(existing.clone()));
                }
            }
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRecordStorePort".to_string(),
                message: format!(
                    "supersede target memory {} already exists with an incompatible chain",
                    command.new_memory_id
                ),
            });
        }
        if old_state.deleted || old_canonical.status != "active" {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRecordStorePort".to_string(),
                message: format!(
                    "supersede source memory {} is not active",
                    command.old_memory_id
                ),
            });
        }
        let active_records = records
            .iter()
            .filter(|(key, state)| key.matches_scope(&command.scope) && !state.deleted)
            .count() as u64;
        if max_active_records > 0 && active_records >= max_active_records {
            return Ok(MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            });
        }

        let timestamp = now_text();
        let canonical = MemoryCanonicalRecord {
            memory_id: command.new_memory_id.clone(),
            space_id: command.scope.space_id,
            user_id: command.scope.user_id,
            scope_label: command.scope_label,
            memory_type: command.memory_type,
            subject: command.subject,
            predicate: command.predicate.or_else(|| Some("is".to_string())),
            object_text: command.object_text,
            canonical_text: command.canonical_text,
            confidence: 1.0,
            evidence_count: 1,
            contradiction_count: 0,
            status: "active".to_string(),
            sensitivity_level: command.sensitivity_level,
            supersedes_memory_id: Some(command.old_memory_id.clone()),
            superseded_by_memory_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            version: 1,
            expires_at: command.expires_at,
            metadata_json: command.metadata_json,
        };
        let (created_outbox_key, created_outbox, created_audit_key, created_audit) =
            reference_journal_entries(&command.scope, command.created_journal);
        let (superseded_outbox_key, superseded_outbox, superseded_audit_key, superseded_audit) =
            reference_journal_entries(&command.scope, command.superseded_journal);
        let mut outbox_store = self.outbox.lock().map_err(lock_error)?;
        let mut audit_store = self.audits.lock().map_err(lock_error)?;
        if outbox_store.contains_key(&created_outbox_key)
            || outbox_store.contains_key(&superseded_outbox_key)
            || audit_store.contains_key(&created_audit_key)
            || audit_store.contains_key(&superseded_audit_key)
        {
            return Err(MemorySpiError::IdempotencyConflict {
                idempotency_key: command.new_memory_id,
            });
        }

        let old_state = records
            .get_mut(&old_key)
            .ok_or_else(atomic_record_state_missing)?;
        let old_canonical = old_state
            .canonical
            .as_mut()
            .ok_or_else(atomic_record_state_missing)?;
        old_canonical.status = "superseded".to_string();
        old_canonical.superseded_by_memory_id = Some(command.new_memory_id.clone());
        old_canonical.updated_at = timestamp.clone();
        old_canonical.version += 1;
        records.insert(
            new_key,
            MemoryRecordState::active_canonical(canonical.clone()),
        );
        outbox_store.insert(superseded_outbox_key, superseded_outbox);
        audit_store.insert(superseded_audit_key, superseded_audit);
        outbox_store.insert(created_outbox_key, created_outbox);
        audit_store.insert(created_audit_key, created_audit);
        Ok(MemoryRecordQuotaAdmission::Admitted(canonical))
    }

    async fn retrieve_canonical(
        &self,
        query: RetrieveCanonicalMemoryQuery,
    ) -> MemorySpiResult<Option<MemoryCanonicalRecord>> {
        let key = ScopedId::new(&query.scope, query.memory_id);
        Ok(self
            .records
            .lock()
            .map_err(lock_error)?
            .get(&key)
            .and_then(MemoryRecordState::visible_canonical))
    }

    async fn update_canonical_atomic(
        &self,
        command: UpdateCanonicalMemoryCommand,
    ) -> MemorySpiResult<Option<MemoryCanonicalRecord>> {
        validate_memory_journal(&command.memory_id, &command.journal)?;
        let key = ScopedId::new(&command.scope, command.memory_id);
        let (outbox_key, outbox, audit_key, audit) =
            reference_journal_entries(&command.scope, command.journal);

        let mut records = self.records.lock().map_err(lock_error)?;
        let Some(state) = records.get_mut(&key) else {
            return Ok(None);
        };
        if state.deleted {
            return Ok(None);
        }
        let Some(canonical) = state.canonical.as_mut() else {
            return Err(atomic_record_state_missing());
        };
        if let Some(text) = command.canonical_text {
            canonical.canonical_text = text.clone();
            canonical.object_text = text.clone();
            state.record.content = text;
        }
        if let Some(subject) = command.subject {
            canonical.subject = Some(subject);
        }
        canonical.updated_at = now_text();
        canonical.version += 1;
        let updated = canonical.clone();

        self.outbox
            .lock()
            .map_err(lock_error)?
            .insert(outbox_key, outbox);
        self.audits
            .lock()
            .map_err(lock_error)?
            .insert(audit_key, audit);
        Ok(Some(updated))
    }

    async fn delete_canonical_atomic(
        &self,
        command: DeleteCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        validate_memory_journal(&command.memory_id, &command.journal)?;
        let key = ScopedId::new(&command.scope, command.memory_id.clone());
        let (outbox_key, outbox, audit_key, audit) =
            reference_journal_entries(&command.scope, command.journal);

        let mut records = self.records.lock().map_err(lock_error)?;
        let Some(state) = records.get_mut(&key) else {
            return Ok(MemoryDeletionReceipt {
                memory_id: command.memory_id,
                deleted: false,
                already_deleted: false,
            });
        };
        if state.deleted {
            return Ok(MemoryDeletionReceipt {
                memory_id: command.memory_id,
                deleted: true,
                already_deleted: true,
            });
        }
        state.deleted = true;
        if let Some(canonical) = state.canonical.as_mut() {
            canonical.status = "deleted".to_string();
            canonical.updated_at = now_text();
            canonical.version += 1;
        }
        self.outbox
            .lock()
            .map_err(lock_error)?
            .insert(outbox_key, outbox);
        self.audits
            .lock()
            .map_err(lock_error)?
            .insert(audit_key, audit);
        Ok(MemoryDeletionReceipt {
            memory_id: command.memory_id,
            deleted: true,
            already_deleted: false,
        })
    }

    async fn delete_all_canonical_atomic(
        &self,
        command: DeleteAllCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryBulkDeletionReceipt> {
        let matches: Vec<(MemoryScopeContext, String)> = {
            let records = self.records.lock().map_err(lock_error)?;
            records
                .iter()
                .filter(|(record_key, state)| {
                    record_key.matches_scope(&command.scope)
                        && !state.deleted
                        && command
                            .user_id
                            .is_none_or(|user_id| state.user_id() == Some(user_id))
                })
                .map(|(record_key, _)| {
                    (
                        MemoryScopeContext {
                            tenant_id: record_key.tenant_id,
                            space_id: record_key.space_id,
                            organization_id: None,
                            user_id: None,
                        },
                        record_key.id.clone(),
                    )
                })
                .collect()
        };
        let mut deleted_ids = Vec::with_capacity(matches.len());
        for (scope, memory_id) in matches {
            let journal = MemoryMutationJournal {
                outbox_id: format!("outbox-memory-record-deleted-all-{memory_id}"),
                aggregate_type: "memory_record".to_string(),
                aggregate_id: memory_id.clone(),
                event_type: "memory.record.deleted".to_string(),
                event_version: "1.0".to_string(),
                payload_json: format!(r#"{{"memoryId":"{memory_id}"}}"#),
                audit_id: format!("audit-memory-record-deleted-all-{memory_id}"),
                audit_action: "memory.record.delete".to_string(),
                audit_resource_type: "memory_record".to_string(),
                audit_resource_id: memory_id.clone(),
                audit_result: "accepted".to_string(),
            };
            let receipt = MemoryRecordStorePort::delete_canonical_atomic(
                self,
                DeleteCanonicalMemoryCommand {
                    scope,
                    memory_id: memory_id.clone(),
                    journal,
                },
            )
            .await?;
            if receipt.deleted {
                deleted_ids.push(memory_id);
            }
        }
        Ok(MemoryBulkDeletionReceipt { deleted_ids })
    }
}

#[async_trait]
impl MemoryEventStorePort for ReferenceMemoryRuntime {
    async fn append(&self, command: AppendMemoryEventCommand) -> MemorySpiResult<MemoryEvent> {
        let event = MemoryEvent {
            event_id: command.event_id.clone(),
            content: command.content,
        };
        let key = ScopedId::new(&command.scope, command.event_id);
        self.events
            .lock()
            .map_err(lock_error)?
            .insert(key, event.clone());

        Ok(event)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryEventQuery,
    ) -> MemorySpiResult<Option<MemoryEvent>> {
        let key = ScopedId::new(&query.scope, query.event_id);
        Ok(self.events.lock().map_err(lock_error)?.get(&key).cloned())
    }
}

#[async_trait]
impl MemoryAuditStorePort for ReferenceMemoryRuntime {
    async fn append(
        &self,
        command: AppendMemoryAuditCommand,
    ) -> MemorySpiResult<MemoryAuditRecord> {
        let audit = MemoryAuditRecord {
            audit_id: command.audit_id.clone(),
            action: command.action,
            resource_type: command.resource_type,
            resource_id: command.resource_id,
            result: command.result,
        };
        let key = ScopedId::new(&command.scope, command.audit_id);
        self.audits
            .lock()
            .map_err(lock_error)?
            .insert(key, audit.clone());

        Ok(audit)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryAuditQuery,
    ) -> MemorySpiResult<Option<MemoryAuditRecord>> {
        let key = ScopedId::new(&query.scope, query.audit_id);
        Ok(self.audits.lock().map_err(lock_error)?.get(&key).cloned())
    }
}

#[async_trait]
impl MemoryOutboxStorePort for ReferenceMemoryRuntime {
    async fn append(
        &self,
        command: AppendMemoryOutboxCommand,
    ) -> MemorySpiResult<MemoryOutboxEvent> {
        let outbox = MemoryOutboxEvent {
            outbox_id: command.outbox_id.clone(),
            aggregate_type: command.aggregate_type,
            aggregate_id: command.aggregate_id,
            event_type: command.event_type,
            event_version: command.event_version,
            payload_json: command.payload_json,
            publish_state: "pending".to_string(),
            published_at: None,
            retry_count: 0,
        };
        let key = ScopedId::new(&command.scope, command.outbox_id);
        self.outbox
            .lock()
            .map_err(lock_error)?
            .insert(key, outbox.clone());

        Ok(outbox)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryOutboxQuery,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        let key = ScopedId::new(&query.scope, query.outbox_id);
        Ok(self.outbox.lock().map_err(lock_error)?.get(&key).cloned())
    }

    async fn list_pending(
        &self,
        query: ListPendingMemoryOutboxQuery,
    ) -> MemorySpiResult<Vec<MemoryOutboxEvent>> {
        let outbox = self.outbox.lock().map_err(lock_error)?;
        let mut pending = outbox
            .iter()
            .filter(|(key, event)| {
                key.matches_scope(&query.scope) && event.publish_state == "pending"
            })
            .map(|(_, event)| event.clone())
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| left.outbox_id.cmp(&right.outbox_id));
        pending.truncate(query.limit as usize);
        Ok(pending)
    }

    async fn mark_published(
        &self,
        command: MarkMemoryOutboxPublishedCommand,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        self.update_outbox_state(
            &command.scope,
            command.outbox_id,
            "published",
            Some(now_text()),
        )
    }

    async fn mark_failed(
        &self,
        command: MarkMemoryOutboxFailedCommand,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        let key = ScopedId::new(&command.scope, command.outbox_id);
        let mut outbox = self.outbox.lock().map_err(lock_error)?;
        let Some(event) = outbox.get_mut(&key) else {
            return Ok(None);
        };
        event.publish_state = "failed".to_string();
        event.retry_count += 1;
        Ok(Some(event.clone()))
    }
}

impl ReferenceMemoryRuntime {
    fn update_outbox_state(
        &self,
        scope: &MemoryScopeContext,
        outbox_id: String,
        publish_state: &str,
        published_at: Option<String>,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        let key = ScopedId::new(scope, outbox_id);
        let mut outbox = self.outbox.lock().map_err(lock_error)?;
        let Some(event) = outbox.get_mut(&key) else {
            return Ok(None);
        };
        event.publish_state = publish_state.to_string();
        event.published_at = published_at;
        Ok(Some(event.clone()))
    }
}

#[async_trait]
impl MemoryCandidateStorePort for ReferenceMemoryRuntime {
    fn supports_candidate_detail_lookup(&self) -> bool {
        true
    }

    fn supports_candidate_listing(&self) -> bool {
        true
    }

    fn supports_atomic_candidate_promotion(&self) -> bool {
        true
    }

    fn supports_atomic_candidate_promotion_journal(&self) -> bool {
        true
    }

    async fn create(
        &self,
        command: CreateMemoryCandidateCommand,
    ) -> MemorySpiResult<MemoryCandidate> {
        let candidate = MemoryCandidate {
            candidate_id: command.candidate_id.clone(),
            candidate_type: command.candidate_type,
            memory_type: command.memory_type,
            proposed_text: command.proposed_text,
            proposed_payload_json: command.proposed_payload_json,
            evidence_json: command.evidence_json,
            confidence: command.confidence,
            decision_state: "pending".to_string(),
            decision_reason: None,
            decided_by: None,
            decided_at: None,
        };
        let key = ScopedId::new(&command.scope, command.candidate_id);
        let tenant_index_key = TenantCandidateListKey {
            tenant_id: command.scope.tenant_id,
            candidate_id: candidate.candidate_id.clone(),
            space_id: command.scope.space_id,
        };
        let space_index_key = SpaceCandidateListKey {
            tenant_id: command.scope.tenant_id,
            space_id: command.scope.space_id,
            candidate_id: candidate.candidate_id.clone(),
        };
        let timestamp = now_text();
        let mut candidates = self.candidates.lock().map_err(lock_error)?;
        if candidates.contains_key(&key) {
            return Err(MemorySpiError::IdempotencyConflict {
                idempotency_key: candidate.candidate_id,
            });
        }
        let mut timestamps = self.candidate_timestamps.lock().map_err(lock_error)?;
        let mut tenant_index = self.candidate_listing_index.lock().map_err(lock_error)?;
        let duplicate_across_space = tenant_index
            .range((
                Included(TenantCandidateListKey {
                    tenant_id: command.scope.tenant_id,
                    candidate_id: candidate.candidate_id.clone(),
                    space_id: i64::MIN,
                }),
                Included(TenantCandidateListKey {
                    tenant_id: command.scope.tenant_id,
                    candidate_id: candidate.candidate_id.clone(),
                    space_id: i64::MAX,
                }),
            ))
            .any(|existing| existing.space_id != command.scope.space_id);
        let mut ambiguous_tenants = self
            .candidate_listing_ambiguous_tenants
            .lock()
            .map_err(lock_error)?;
        let mut space_index = self
            .candidate_space_listing_index
            .lock()
            .map_err(lock_error)?;
        candidates.insert(key.clone(), candidate.clone());
        timestamps.insert(key, (timestamp.clone(), timestamp));
        tenant_index.insert(tenant_index_key);
        if duplicate_across_space {
            ambiguous_tenants.insert(command.scope.tenant_id);
        }
        space_index.insert(space_index_key);

        Ok(candidate)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        let key = ScopedId::new(&query.scope, query.candidate_id);
        Ok(self
            .candidates
            .lock()
            .map_err(lock_error)?
            .get(&key)
            .cloned())
    }

    async fn retrieve_detail(
        &self,
        query: RetrieveMemoryCandidateDetailQuery,
    ) -> MemorySpiResult<Option<MemoryCandidateDetail>> {
        let index = self.candidate_listing_index.lock().map_err(lock_error)?;
        let lower_bound = Included(TenantCandidateListKey {
            tenant_id: query.tenant_id,
            candidate_id: query.candidate_id.clone(),
            space_id: i64::MIN,
        });
        let upper_bound = Included(TenantCandidateListKey {
            tenant_id: query.tenant_id,
            candidate_id: query.candidate_id.clone(),
            space_id: i64::MAX,
        });
        let mut matches = index.range((lower_bound, upper_bound));
        let Some(first_match) = matches.next() else {
            return Ok(None);
        };
        let key = ScopedId {
            tenant_id: first_match.tenant_id,
            space_id: first_match.space_id,
            id: first_match.candidate_id.clone(),
        };
        if matches.next().is_some() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryCandidateStorePort".to_string(),
                message: "candidate detail id is ambiguous across tenant spaces".to_string(),
            });
        }
        drop(index);
        let candidates = self.candidates.lock().map_err(lock_error)?;
        let candidate =
            candidates
                .get(&key)
                .ok_or_else(|| MemorySpiError::PortOperationFailed {
                    port: "MemoryCandidateStorePort".to_string(),
                    message: "candidate detail index points to a missing candidate".to_string(),
                })?;
        let target_memory_id = self
            .candidate_targets
            .lock()
            .map_err(lock_error)?
            .get(&key)
            .cloned();
        let target_memory_id = match target_memory_id {
            Some(memory_id) => {
                let target_key = ScopedId {
                    tenant_id: key.tenant_id,
                    space_id: key.space_id,
                    id: memory_id.clone(),
                };
                self.records
                    .lock()
                    .map_err(lock_error)?
                    .get(&target_key)
                    .filter(|state| !state.deleted)
                    .map(|_| memory_id)
            }
            None => None,
        };
        let (created_at, updated_at) = self
            .candidate_timestamps
            .lock()
            .map_err(lock_error)?
            .get(&key)
            .cloned()
            .ok_or_else(|| MemorySpiError::PortOperationFailed {
                port: "MemoryCandidateStorePort".to_string(),
                message: "candidate detail index points to missing timestamps".to_string(),
            })?;
        Ok(Some(MemoryCandidateDetail {
            candidate_id: candidate.candidate_id.clone(),
            space_id: key.space_id,
            candidate_type: candidate.candidate_type.clone(),
            memory_type: candidate.memory_type.clone(),
            proposed_text: candidate.proposed_text.clone(),
            evidence_json: candidate.evidence_json.clone(),
            confidence: candidate.confidence,
            decision_state: candidate.decision_state.clone(),
            created_at,
            updated_at,
            target_memory_id,
        }))
    }

    async fn list_candidates(
        &self,
        query: ListMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryCandidatePage> {
        let page_size = query
            .page_size
            .clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE as u32)
            as usize;
        let cursor = query.cursor.unwrap_or_default();
        let mut keys = if let Some(space_id) = query.space_id {
            let index = self
                .candidate_space_listing_index
                .lock()
                .map_err(lock_error)?;
            let lower_bound = if cursor.is_empty() {
                Included(SpaceCandidateListKey {
                    tenant_id: query.tenant_id,
                    space_id,
                    candidate_id: String::new(),
                })
            } else {
                Excluded(SpaceCandidateListKey {
                    tenant_id: query.tenant_id,
                    space_id,
                    candidate_id: cursor.clone(),
                })
            };
            let mut keys = Vec::with_capacity(page_size.saturating_add(1));
            for key in index.range((lower_bound, Unbounded)) {
                if key.tenant_id != query.tenant_id || key.space_id != space_id {
                    break;
                }
                keys.push(ScopedId {
                    tenant_id: key.tenant_id,
                    space_id: key.space_id,
                    id: key.candidate_id.clone(),
                });
                if keys.len() > page_size {
                    break;
                }
            }
            keys
        } else {
            let index = self.candidate_listing_index.lock().map_err(lock_error)?;
            let ambiguous_tenants = self
                .candidate_listing_ambiguous_tenants
                .lock()
                .map_err(lock_error)?;
            if ambiguous_tenants.contains(&query.tenant_id) {
                return Err(MemorySpiError::PortOperationFailed {
                    port: "MemoryCandidateStorePort".to_string(),
                    message: "candidate listing cursor is ambiguous across tenant spaces"
                        .to_string(),
                });
            }
            let lower_bound = if cursor.is_empty() {
                Included(TenantCandidateListKey {
                    tenant_id: query.tenant_id,
                    candidate_id: String::new(),
                    space_id: i64::MIN,
                })
            } else {
                Excluded(TenantCandidateListKey {
                    tenant_id: query.tenant_id,
                    candidate_id: cursor.clone(),
                    space_id: i64::MAX,
                })
            };
            let mut keys = Vec::with_capacity(page_size.saturating_add(1));
            let mut previous_candidate_id: Option<&str> = None;
            for key in index.range((lower_bound, Unbounded)) {
                if key.tenant_id != query.tenant_id {
                    break;
                }
                if previous_candidate_id == Some(key.candidate_id.as_str()) {
                    return Err(MemorySpiError::PortOperationFailed {
                        port: "MemoryCandidateStorePort".to_string(),
                        message: "candidate listing cursor is ambiguous across tenant spaces"
                            .to_string(),
                    });
                }
                previous_candidate_id = Some(&key.candidate_id);
                keys.push(ScopedId {
                    tenant_id: key.tenant_id,
                    space_id: key.space_id,
                    id: key.candidate_id.clone(),
                });
                if keys.len() > page_size {
                    break;
                }
            }
            keys
        };
        let has_more = keys.len() > page_size;
        keys.truncate(page_size);
        let candidates = self.candidates.lock().map_err(lock_error)?;
        let timestamps = self.candidate_timestamps.lock().map_err(lock_error)?;
        let mut items = Vec::with_capacity(keys.len());
        for key in keys {
            let candidate =
                candidates
                    .get(&key)
                    .ok_or_else(|| MemorySpiError::PortOperationFailed {
                        port: "MemoryCandidateStorePort".to_string(),
                        message: "candidate listing index points to a missing candidate"
                            .to_string(),
                    })?;
            let (created_at, updated_at) = timestamps.get(&key).cloned().ok_or_else(|| {
                MemorySpiError::PortOperationFailed {
                    port: "MemoryCandidateStorePort".to_string(),
                    message: "candidate listing index points to missing timestamps".to_string(),
                }
            })?;
            items.push(MemoryCandidateSummary {
                candidate_id: candidate.candidate_id.clone(),
                space_id: key.space_id,
                candidate_type: candidate.candidate_type.clone(),
                memory_type: candidate.memory_type.clone(),
                proposed_text: candidate.proposed_text.clone(),
                confidence: candidate.confidence,
                decision_state: candidate.decision_state.clone(),
                created_at,
                updated_at,
            });
        }
        let next_cursor = if has_more {
            items.last().map(|item| item.candidate_id.clone())
        } else {
            None
        };
        Ok(MemoryCandidatePage {
            items,
            has_more,
            next_cursor,
        })
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        self.decide_candidate(
            &command.scope,
            command.candidate_id,
            "approved",
            command.decision_reason,
            command.decided_by,
        )
    }

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        self.decide_candidate(
            &command.scope,
            command.candidate_id,
            "rejected",
            command.decision_reason,
            command.decided_by,
        )
    }

    async fn promote_atomic_with_quota(
        &self,
        command: PromoteMemoryCandidateAtomicCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        self.promote_candidate_atomic_inner(command, max_active_records, None)
    }

    async fn promote_atomic_with_quota_and_journal(
        &self,
        command: PromoteMemoryCandidateAtomicWithJournalCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        validate_memory_journal(&command.promotion.memory_id, &command.journal)?;
        self.promote_candidate_atomic_inner(
            command.promotion,
            max_active_records,
            Some(command.journal),
        )
    }
}

impl ReferenceMemoryRuntime {
    fn promote_candidate_atomic_inner(
        &self,
        command: PromoteMemoryCandidateAtomicCommand,
        max_active_records: u64,
        journal: Option<MemoryMutationJournal>,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        let candidate_key = ScopedId::new(&command.scope, command.candidate_id.clone());
        let mut candidates = self.candidates.lock().map_err(lock_error)?;
        let candidate = candidates.get_mut(&candidate_key).ok_or_else(|| {
            MemorySpiError::PortOperationFailed {
                port: "MemoryCandidateStorePort".to_string(),
                message: format!("candidate {} does not exist", command.candidate_id),
            }
        })?;
        let mut candidate_targets = self.candidate_targets.lock().map_err(lock_error)?;
        if let Some(memory_id) = candidate_targets.get(&candidate_key).cloned() {
            if candidate.decision_state != "approved" {
                return Err(MemorySpiError::PortOperationFailed {
                    port: "MemoryCandidateStorePort".to_string(),
                    message: format!(
                        "candidate {} has a target reference with invalid decision state {}",
                        command.candidate_id, candidate.decision_state
                    ),
                });
            }
            let target_key = ScopedId::new(&command.scope, memory_id.clone());
            let target_is_visible = self
                .records
                .lock()
                .map_err(lock_error)?
                .get(&target_key)
                .is_some_and(|state| !state.deleted);
            if !target_is_visible {
                return Err(MemorySpiError::PortOperationFailed {
                    port: "MemoryCandidateStorePort".to_string(),
                    message: format!(
                        "approved candidate {} references a missing or deleted target",
                        command.candidate_id
                    ),
                });
            }
            return Ok(MemoryRecordQuotaAdmission::Admitted(
                MemoryCandidatePromotion {
                    candidate_id: command.candidate_id,
                    memory_id,
                },
            ));
        }
        if candidate.decision_state != "pending" {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryCandidateStorePort".to_string(),
                message: format!(
                    "target-less candidate {} has invalid decision state {}",
                    command.candidate_id, candidate.decision_state
                ),
            });
        }

        let mut records = self.records.lock().map_err(lock_error)?;
        let active_records = records
            .iter()
            .filter(|(record_key, state)| {
                record_key.matches_scope(&command.scope) && !state.deleted
            })
            .count() as u64;
        if max_active_records > 0 && active_records >= max_active_records {
            return Ok(MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            });
        }
        let record_key = ScopedId::new(&command.scope, command.memory_id.clone());
        if records.contains_key(&record_key) {
            return Err(MemorySpiError::IdempotencyConflict {
                idempotency_key: command.memory_id,
            });
        }

        let journal_entries = journal
            .as_ref()
            .map(|journal| reference_journal_entries(&command.scope, journal.clone()));
        let mut outbox_store = if journal_entries.is_some() {
            Some(self.outbox.lock().map_err(lock_error)?)
        } else {
            None
        };
        let mut audit_store = if journal_entries.is_some() {
            Some(self.audits.lock().map_err(lock_error)?)
        } else {
            None
        };

        let timestamp = now_text();
        let canonical = MemoryCanonicalRecord {
            memory_id: command.memory_id.clone(),
            space_id: command.scope.space_id,
            user_id: command.scope.user_id,
            scope_label: "user".to_string(),
            memory_type: command.memory_type,
            subject: None,
            predicate: Some("is".to_string()),
            object_text: command.proposed_text.clone(),
            canonical_text: command.proposed_text,
            confidence: 1.0,
            evidence_count: 1,
            contradiction_count: 0,
            status: "active".to_string(),
            sensitivity_level: "internal".to_string(),
            supersedes_memory_id: None,
            superseded_by_memory_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            version: 1,
            expires_at: None,
            metadata_json: None,
        };
        records.insert(record_key, MemoryRecordState::active_canonical(canonical));
        candidate_targets.insert(candidate_key.clone(), command.memory_id.clone());
        if let Some((outbox_key, outbox, audit_key, audit)) = journal_entries {
            outbox_store
                .as_mut()
                .expect("journal outbox lock is present")
                .insert(outbox_key, outbox);
            audit_store
                .as_mut()
                .expect("journal audit lock is present")
                .insert(audit_key, audit);
        }
        candidate.decision_state = "approved".to_string();
        candidate.decision_reason = None;
        candidate.decided_by = command.decided_by;
        candidate.decided_at = Some(now_text());
        if let Some(timestamps) = self
            .candidate_timestamps
            .lock()
            .map_err(lock_error)?
            .get_mut(&candidate_key)
        {
            timestamps.1 = candidate.decided_at.clone().unwrap_or_default();
        }

        Ok(MemoryRecordQuotaAdmission::Admitted(
            MemoryCandidatePromotion {
                candidate_id: command.candidate_id,
                memory_id: command.memory_id,
            },
        ))
    }
}

impl ReferenceMemoryRuntime {
    fn decide_candidate(
        &self,
        scope: &MemoryScopeContext,
        candidate_id: String,
        decision_state: &str,
        decision_reason: Option<String>,
        decided_by: Option<i64>,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        let key = ScopedId::new(scope, candidate_id);
        let mut candidates = self.candidates.lock().map_err(lock_error)?;
        let Some(candidate) = candidates.get_mut(&key) else {
            return Ok(None);
        };

        candidate.decision_state = decision_state.to_string();
        candidate.decision_reason = decision_reason;
        candidate.decided_by = decided_by;
        candidate.decided_at = Some(now_text());
        if let Some(timestamps) = self
            .candidate_timestamps
            .lock()
            .map_err(lock_error)?
            .get_mut(&key)
        {
            timestamps.1 = candidate.decided_at.clone().unwrap_or_default();
        }

        Ok(Some(candidate.clone()))
    }
}

#[async_trait]
impl MemoryHabitStorePort for ReferenceMemoryRuntime {
    async fn upsert(&self, command: UpsertMemoryHabitCommand) -> MemorySpiResult<MemoryHabit> {
        let key = ScopedHabitKey::new(&command.scope, command.user_id, command.habit_key.clone());
        let habit = MemoryHabit {
            habit_id: command.habit_id,
            user_id: command.user_id,
            habit_key: command.habit_key,
            habit_type: command.habit_type,
            description: command.description,
            stage: command.stage,
            strength: command.strength,
            confidence: command.confidence,
            support_count: command.support_count,
            last_signal_at: Some(now_text()),
            promoted_memory_id: None,
            decay_after: None,
            metadata_json: command.metadata_json,
        };

        self.habits
            .lock()
            .map_err(lock_error)?
            .insert(key, habit.clone());

        Ok(habit)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryHabitQuery,
    ) -> MemorySpiResult<Option<MemoryHabit>> {
        let key = ScopedHabitKey::new(&query.scope, query.user_id, query.habit_key);
        Ok(self.habits.lock().map_err(lock_error)?.get(&key).cloned())
    }

    async fn promote(
        &self,
        command: PromoteMemoryHabitCommand,
    ) -> MemorySpiResult<Option<MemoryHabit>> {
        let key = ScopedHabitKey::new(&command.scope, command.user_id, command.habit_key);
        let mut habits = self.habits.lock().map_err(lock_error)?;
        let Some(habit) = habits.get_mut(&key) else {
            return Ok(None);
        };

        habit.stage = "promoted".to_string();
        habit.promoted_memory_id = command.promoted_memory_id;

        Ok(Some(habit.clone()))
    }

    async fn decay(
        &self,
        command: DecayMemoryHabitCommand,
    ) -> MemorySpiResult<Option<MemoryHabit>> {
        let key = ScopedHabitKey::new(&command.scope, command.user_id, command.habit_key);
        let mut habits = self.habits.lock().map_err(lock_error)?;
        let Some(habit) = habits.get_mut(&key) else {
            return Ok(None);
        };

        habit.stage = "decayed".to_string();
        habit.strength = (habit.strength - command.strength_delta).max(0.0);

        Ok(Some(habit.clone()))
    }
}

#[async_trait]
impl MemoryRetrievalTraceStorePort for ReferenceMemoryRuntime {
    fn supports_tenant_trace_lookup(&self) -> bool {
        true
    }

    async fn append(
        &self,
        command: AppendMemoryRetrievalTraceCommand,
    ) -> MemorySpiResult<MemoryRetrievalTrace> {
        let trace = MemoryRetrievalTrace {
            trace_id: command.trace_id.clone(),
            actor_id: command.actor_id,
            query_text: command.query_text,
            query_hash: command.query_hash,
            retrievers_json: command.retrievers_json,
            latency_ms: command.latency_ms,
            result_count: command.hits.len() as i64,
            degraded: command.degraded,
            metadata_json: command.metadata_json,
            hits: command.hits,
            context_pack: command.context_pack,
        };
        let key = ScopedId::new(&command.scope, command.trace_id);
        self.retrieval_traces
            .lock()
            .map_err(lock_error)?
            .insert(key, trace.clone());

        Ok(trace)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryRetrievalTraceQuery,
    ) -> MemorySpiResult<Option<MemoryRetrievalTrace>> {
        let key = ScopedId::new(&query.scope, query.trace_id);
        Ok(self
            .retrieval_traces
            .lock()
            .map_err(lock_error)?
            .get(&key)
            .cloned())
    }

    async fn retrieve_for_tenant(
        &self,
        query: RetrieveMemoryRetrievalTraceForTenantQuery,
    ) -> MemorySpiResult<Option<ScopedMemoryRetrievalTrace>> {
        let traces = self.retrieval_traces.lock().map_err(lock_error)?;
        let mut matches = traces
            .iter()
            .filter(|(key, _trace)| key.tenant_id == query.tenant_id && key.id == query.trace_id)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRetrievalTraceStorePort".to_string(),
                message: "tenant-scoped retrieval trace id is ambiguous across spaces".to_string(),
            });
        }
        Ok(matches
            .pop()
            .map(|(key, trace)| ScopedMemoryRetrievalTrace {
                scope: MemoryScopeContext {
                    tenant_id: key.tenant_id,
                    space_id: key.space_id,
                    organization_id: None,
                    user_id: None,
                },
                trace: trace.clone(),
                created_at: None,
            }))
    }

    async fn list_recent(
        &self,
        query: ListMemoryRetrievalTracesQuery,
    ) -> MemorySpiResult<Vec<MemoryRetrievalTrace>> {
        let traces = self.retrieval_traces.lock().map_err(lock_error)?;
        let mut recent = traces
            .iter()
            .filter(|(key, _trace)| key.matches_scope(&query.scope))
            .map(|(_, trace)| trace.clone())
            .collect::<Vec<_>>();
        recent.sort_by(|left, right| right.trace_id.cmp(&left.trace_id));
        recent.truncate(query.limit as usize);
        Ok(recent)
    }
}

#[async_trait]
impl MemoryRetrieverPort for ReferenceMemoryRuntime {
    fn retriever_code(&self) -> &str {
        "reference_keyword"
    }

    fn supports_bounded_scoped_search(&self) -> bool {
        true
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Err(scope_required_error(
            "MemoryRetrieverPort",
            "retrieve_scoped",
        ))
    }

    async fn retrieve_scoped(
        &self,
        scope: MemoryScopeContext,
        command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        let query = command.query.to_ascii_lowercase();
        let records = self.records.lock().map_err(lock_error)?;
        let mut memory_ids = records
            .iter()
            .filter(|(key, _state)| key.matches_scope(&scope))
            .filter_map(|(_key, state)| {
                state
                    .visible_record()
                    .filter(|record| record.content.to_ascii_lowercase().contains(&query))
                    .map(|record| record.memory_id)
            })
            .collect::<Vec<_>>();
        memory_ids.sort();
        memory_ids.truncate(MAX_MEMORY_RETRIEVAL_CANDIDATES as usize);
        Ok(MemoryRetrieverResult { memory_ids })
    }

    async fn search_scoped(
        &self,
        query: SearchMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryRetrieverSearchResult> {
        let normalized_query = query.query.trim().to_lowercase();
        if normalized_query.is_empty() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRetrieverPort".to_string(),
                message: "memory retrieval query must not be blank".to_string(),
            });
        }
        if query.retriever_kinds.is_empty() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRetrieverPort".to_string(),
                message: "memory retrieval must select at least one retriever".to_string(),
            });
        }
        if query.limit == 0 || query.limit > MAX_MEMORY_RETRIEVAL_CANDIDATES {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRetrieverPort".to_string(),
                message: format!(
                    "memory retrieval candidate limit must be between 1 and {MAX_MEMORY_RETRIEVAL_CANDIDATES}"
                ),
            });
        }

        let supported_record_search = query
            .retriever_kinds
            .contains(&MemoryRetrieverKind::Keyword);
        let unavailable_retriever_kinds = query
            .retriever_kinds
            .iter()
            .filter(|kind| **kind != MemoryRetrieverKind::Keyword)
            .cloned()
            .collect::<Vec<_>>();
        if !supported_record_search {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemoryRetrieverPort".to_string(),
                message: "selected memory retrievers are not supported by the reference runtime"
                    .to_string(),
            });
        }

        let records = self.records.lock().map_err(lock_error)?;
        let mut candidates = records
            .iter()
            .filter(|(key, _state)| key.matches_scope(&query.scope))
            .filter_map(|(_key, state)| state.visible_canonical())
            .filter(|record| {
                query.memory_types.is_empty()
                    || query
                        .memory_types
                        .iter()
                        .any(|memory_type| memory_type == &record.memory_type)
            })
            .filter(|record| match query.read_scope {
                MemorySensitivityReadScope::Owner => true,
                MemorySensitivityReadScope::Elevated => record.sensitivity_level != "restricted",
                MemorySensitivityReadScope::Public => {
                    matches!(record.sensitivity_level.as_str(), "public" | "internal")
                }
            })
            .filter(|record| {
                let searchable = format!(
                    "{} {} {} {}",
                    record.subject.as_deref().unwrap_or_default(),
                    record.predicate.as_deref().unwrap_or_default(),
                    record.object_text,
                    record.canonical_text
                )
                .to_lowercase();
                searchable.contains(&normalized_query)
            })
            .map(|record| MemoryRetrievalRecordCandidate {
                memory_id: record.memory_id,
                subject: record.subject,
                predicate: record.predicate,
                object_text: record.object_text,
                canonical_text: record.canonical_text,
                created_at: record.created_at,
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| left.memory_id.cmp(&right.memory_id))
        });
        candidates.truncate(query.limit as usize);

        Ok(MemoryRetrieverSearchResult {
            records: candidates,
            events: Vec::<MemoryRetrievalEventCandidate>::new(),
            degraded: !unavailable_retriever_kinds.is_empty(),
            degradation_codes: if unavailable_retriever_kinds.is_empty() {
                Vec::new()
            } else {
                vec!["retriever_kind_unavailable".to_string()]
            },
            unavailable_retriever_kinds,
        })
    }
}

#[async_trait]
impl MemoryIndexPort for ReferenceMemoryRuntime {
    fn index_kind(&self) -> &str {
        "reference_index_unavailable"
    }

    async fn index(&self, _memory_id: String) -> MemorySpiResult<MemoryIndexReceipt> {
        Err(reference_capability_unavailable(
            "MemoryIndexPort",
            "the reference runtime has no independently materialized index",
        ))
    }
}

#[async_trait]
impl ExternalMemoryBridgePort for ReferenceMemoryRuntime {
    fn provider_code(&self) -> &str {
        "reference_external_bridge_unconfigured"
    }

    async fn import(
        &self,
        _command: ExternalMemoryImportCommand,
    ) -> MemorySpiResult<ExternalMemoryImportResult> {
        Err(external_bridge_unconfigured())
    }

    async fn export(
        &self,
        _command: ExternalMemoryExportCommand,
    ) -> MemorySpiResult<ExternalMemoryExportResult> {
        Err(external_bridge_unconfigured())
    }

    async fn delete(
        &self,
        _command: ExternalMemoryDeleteCommand,
    ) -> MemorySpiResult<ExternalMemoryDeleteReceipt> {
        Err(external_bridge_unconfigured())
    }

    async fn shadow_read(
        &self,
        _command: ExternalMemoryShadowReadCommand,
    ) -> MemorySpiResult<ExternalMemoryShadowReadResult> {
        Err(external_bridge_unconfigured())
    }
}

#[async_trait]
impl MemoryContextAssemblerPort for ReferenceMemoryRuntime {
    async fn assemble(
        &self,
        _command: AssembleMemoryContextCommand,
    ) -> MemorySpiResult<MemoryContextPackDraft> {
        Err(scope_required_error(
            "MemoryContextAssemblerPort",
            "assemble_scoped",
        ))
    }

    async fn assemble_scoped(
        &self,
        scope: MemoryScopeContext,
        command: AssembleMemoryContextCommand,
    ) -> MemorySpiResult<MemoryContextPackDraft> {
        let records = self.records.lock().map_err(lock_error)?;
        let mut memory_ids = Vec::new();
        let mut context_lines = Vec::new();
        for memory_id in command.memory_ids {
            let key = ScopedId::new(&scope, memory_id.clone());
            if let Some(record) = records
                .get(&key)
                .and_then(MemoryRecordState::visible_record)
            {
                memory_ids.push(memory_id);
                context_lines.push(record.content);
            }
        }

        Ok(MemoryContextPackDraft {
            memory_ids,
            context_text: context_lines.join("\n"),
        })
    }
}

#[async_trait]
impl MemoryEvaluationPort for ReferenceMemoryRuntime {
    async fn run(&self, _command: RunMemoryEvalCommand) -> MemorySpiResult<MemoryEvalRunResult> {
        Err(reference_capability_unavailable(
            "MemoryEvaluationPort",
            "the reference runtime has no golden dataset evaluation engine",
        ))
    }
}

#[derive(Debug, Clone)]
struct MemoryRecordState {
    record: MemoryRecord,
    canonical: Option<MemoryCanonicalRecord>,
    deleted: bool,
}

impl MemoryRecordState {
    fn active(record: MemoryRecord) -> Self {
        Self {
            record,
            canonical: None,
            deleted: false,
        }
    }

    fn user_id(&self) -> Option<i64> {
        self.canonical
            .as_ref()
            .and_then(|canonical| canonical.user_id)
    }

    fn active_canonical(canonical: MemoryCanonicalRecord) -> Self {
        Self {
            record: MemoryRecord {
                memory_id: canonical.memory_id.clone(),
                content: canonical.object_text.clone(),
            },
            canonical: Some(canonical),
            deleted: false,
        }
    }

    fn visible_record(&self) -> Option<MemoryRecord> {
        if self.deleted {
            None
        } else {
            Some(self.record.clone())
        }
    }

    fn visible_canonical(&self) -> Option<MemoryCanonicalRecord> {
        if self.deleted {
            None
        } else {
            self.canonical.clone()
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ScopedId {
    tenant_id: i64,
    space_id: i64,
    id: String,
}

impl ScopedId {
    fn new(scope: &MemoryScopeContext, id: String) -> Self {
        Self {
            tenant_id: scope.tenant_id,
            space_id: scope.space_id,
            id,
        }
    }

    fn matches_scope(&self, scope: &MemoryScopeContext) -> bool {
        self.tenant_id == scope.tenant_id && self.space_id == scope.space_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TenantCandidateListKey {
    tenant_id: i64,
    candidate_id: String,
    space_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SpaceCandidateListKey {
    tenant_id: i64,
    space_id: i64,
    candidate_id: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ScopedHabitKey {
    tenant_id: i64,
    space_id: i64,
    user_id: i64,
    habit_key: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GovernanceSpaceKey {
    tenant_id: i64,
    space_id: i64,
}

impl GovernanceSpaceKey {
    fn new(tenant_id: i64, space_id: i64) -> Self {
        Self {
            tenant_id,
            space_id,
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GovernanceActorSpaceKey {
    tenant_id: i64,
    space_id: i64,
    subject_type: Option<String>,
    subject_id: String,
}

impl GovernanceActorSpaceKey {
    fn new(scope: &MemoryScopeContext, actor: &MemoryGovernanceActor) -> Self {
        Self {
            tenant_id: scope.tenant_id,
            space_id: scope.space_id,
            subject_type: actor.subject_type.clone(),
            subject_id: actor.subject_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct GovernanceCapabilityKey {
    tenant_id: i64,
    space_id: i64,
    capability_code: String,
}

impl GovernanceCapabilityKey {
    fn new(scope: &MemoryScopeContext, capability_code: &str) -> Self {
        Self {
            tenant_id: scope.tenant_id,
            space_id: scope.space_id,
            capability_code: capability_code.to_string(),
        }
    }
}

impl ScopedHabitKey {
    fn new(scope: &MemoryScopeContext, user_id: i64, habit_key: String) -> Self {
        Self {
            tenant_id: scope.tenant_id,
            space_id: scope.space_id,
            user_id,
            habit_key,
        }
    }
}

fn external_bridge_unconfigured() -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: "ExternalMemoryBridgePort".to_string(),
        message: "reference external memory bridge is fail-closed until a reviewed provider adapter is configured".to_string(),
    }
}

fn reference_capability_unavailable(port: &str, reason: &str) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: port.to_string(),
        message: format!("reference capability is unavailable and fails closed: {reason}"),
    }
}

fn reference_supersede_target_matches(
    existing: &MemoryCanonicalRecord,
    command: &SupersedeCanonicalMemoryAtomicCommand,
) -> bool {
    existing.space_id == command.scope.space_id
        && existing.user_id == command.scope.user_id
        && existing.scope_label == command.scope_label
        && existing.memory_type == command.memory_type
        && existing.subject == command.subject
        && existing.predicate.as_deref() == Some(command.predicate.as_deref().unwrap_or("is"))
        && existing.object_text == command.object_text
        && existing.canonical_text == command.canonical_text
        && existing.sensitivity_level == command.sensitivity_level
}

fn reference_supersede_journals_match(
    runtime: &ReferenceMemoryRuntime,
    scope: &MemoryScopeContext,
    created: &MemoryMutationJournal,
    superseded: &MemoryMutationJournal,
) -> MemorySpiResult<bool> {
    let outbox_store = runtime.outbox.lock().map_err(lock_error)?;
    let audit_store = runtime.audits.lock().map_err(lock_error)?;
    for journal in [created, superseded] {
        let (outbox_key, expected_outbox, audit_key, expected_audit) =
            reference_journal_entries(scope, journal.clone());
        let Some(actual_outbox) = outbox_store.get(&outbox_key) else {
            return Ok(false);
        };
        if actual_outbox.aggregate_type != expected_outbox.aggregate_type
            || actual_outbox.aggregate_id != expected_outbox.aggregate_id
            || actual_outbox.event_type != expected_outbox.event_type
            || actual_outbox.event_version != expected_outbox.event_version
            || !reference_journal_payload_matches(
                &actual_outbox.payload_json,
                &expected_outbox.payload_json,
            )
        {
            return Ok(false);
        }
        let Some(actual_audit) = audit_store.get(&audit_key) else {
            return Ok(false);
        };
        if actual_audit != &expected_audit {
            return Ok(false);
        }
    }
    Ok(true)
}

fn reference_journal_payload_matches(stored: &str, expected: &str) -> bool {
    match (
        serde_json::from_str::<Value>(stored),
        serde_json::from_str::<Value>(expected),
    ) {
        (Ok(stored), Ok(expected)) => stored == expected,
        _ => stored == expected,
    }
}

fn validate_memory_journal(
    memory_id: &str,
    journal: &MemoryMutationJournal,
) -> MemorySpiResult<()> {
    if journal.aggregate_id != memory_id || journal.audit_resource_id != memory_id {
        return Err(MemorySpiError::PortOperationFailed {
            port: "MemoryRecordStorePort".to_string(),
            message: "memory mutation journal resource ids must match the canonical memory id"
                .to_string(),
        });
    }
    Ok(())
}

fn reference_journal_entries(
    scope: &MemoryScopeContext,
    journal: MemoryMutationJournal,
) -> (ScopedId, MemoryOutboxEvent, ScopedId, MemoryAuditRecord) {
    let outbox_key = ScopedId::new(scope, journal.outbox_id.clone());
    let audit_key = ScopedId::new(scope, journal.audit_id.clone());
    let outbox = MemoryOutboxEvent {
        outbox_id: journal.outbox_id,
        aggregate_type: journal.aggregate_type,
        aggregate_id: journal.aggregate_id,
        event_type: journal.event_type,
        event_version: journal.event_version,
        payload_json: journal.payload_json,
        publish_state: "pending".to_string(),
        published_at: None,
        retry_count: 0,
    };
    let audit = MemoryAuditRecord {
        audit_id: journal.audit_id,
        action: journal.audit_action,
        resource_type: journal.audit_resource_type,
        resource_id: journal.audit_resource_id,
        result: journal.audit_result,
    };
    (outbox_key, outbox, audit_key, audit)
}

fn atomic_record_state_missing() -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: "MemoryRecordStorePort".to_string(),
        message: "reference record was not created through the canonical atomic path".to_string(),
    }
}

fn scope_required_error(port: &str, scoped_method: &str) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: port.to_string(),
        message: format!(
            "reference runtime requires explicit tenant and space scope; use {scoped_method}"
        ),
    }
}

fn validate_governance_fact_limit(fact_limit: u32) -> MemorySpiResult<usize> {
    if fact_limit == 0 || fact_limit > MAX_MEMORY_GOVERNANCE_FACTS {
        return Err(MemorySpiError::PortOperationFailed {
            port: "MemoryGovernanceAccessPort".to_string(),
            message: format!(
                "governance fact limit must be between 1 and {MAX_MEMORY_GOVERNANCE_FACTS}"
            ),
        });
    }
    Ok(fact_limit as usize)
}

fn validate_space_command(command: &CreateMemorySpaceCommand) -> MemorySpiResult<()> {
    if command.tenant_id < 0 || command.space_id < 0 {
        return Err(MemorySpiError::PortOperationFailed {
            port: "MemorySpaceStorePort".to_string(),
            message: "memory-space tenant and space identifiers must be non-negative".to_string(),
        });
    }
    for (field, value) in [
        ("owner subject type", command.owner_subject_type.as_str()),
        ("owner subject id", command.owner_subject_id.as_str()),
        ("space type", command.space_type.as_str()),
        ("display name", command.display_name.as_str()),
        ("default scope", command.default_scope.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(MemorySpiError::PortOperationFailed {
                port: "MemorySpaceStorePort".to_string(),
                message: format!("memory-space {field} must not be blank"),
            });
        }
    }
    Ok(())
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: "ReferenceMemoryRuntime".to_string(),
        message: "reference runtime lock is poisoned".to_string(),
    }
}

fn now_text() -> String {
    sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None)
}
