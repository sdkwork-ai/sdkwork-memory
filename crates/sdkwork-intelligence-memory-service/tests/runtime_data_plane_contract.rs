use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_intelligence_memory_service::{
    MemoryRuntimeDataPlane, MemoryRuntimeDataPlaneError, PHASE1_HTTP_DATA_PLANE_PORTS,
};
use sdkwork_memory_plugin_reference_profiles::{
    build_reference_executable_runtime, reference_profiles_manifest, ReferenceMemoryRuntime,
    REFERENCE_PROFILES_PLUGIN_ID,
};
use sdkwork_memory_spi::{
    AppendMemoryOutboxCommand, ApproveMemoryCandidateCommand, AssembleMemoryContextCommand,
    CreateCanonicalMemoryCommand, CreateMemoryCandidateCommand, CreateMemoryRecordCommand,
    CreateMemorySpaceCommand, DeleteCanonicalMemoryCommand, DeleteMemoryRecordCommand,
    ListMemoryCandidatesQuery, MemoryCandidate, MemoryCandidateEvidenceLink,
    MemoryCandidateStorePort, MemoryCoreRuntime, MemoryDeletionReceipt, MemoryDeploymentMode,
    MemoryExecutablePluginRuntime, MemoryGovernanceAccessPort, MemoryImplementationKind,
    MemoryMutationJournal, MemoryPluginPorts, MemoryRecord, MemoryRecordStorePort,
    MemoryRetrieverKind, MemoryRetrieverPort, MemoryRetrieverResult, MemoryRuntimeProfileMetadata,
    MemoryScopeContext, MemorySensitivityReadScope, MemorySpaceStorePort, MemorySpiResult,
    PromoteMemoryCandidateAtomicCommand, PromoteMemoryCandidateAtomicWithJournalCommand,
    PromoteMemoryHabitCommand, RejectMemoryCandidateCommand, ResolveMemorySpaceGovernanceQuery,
    RetrieveCanonicalMemoryQuery, RetrieveMemoryCandidateDetailQuery, RetrieveMemoryCandidateQuery,
    RetrieveMemoryCandidatesCommand, RetrieveMemoryOutboxQuery, RetrieveMemoryRecordQuery,
    SearchMemoryCandidatesQuery, SupersedeCanonicalMemoryAtomicCommand, UpsertMemoryHabitCommand,
};

struct RecordStoreCapabilityFixture {
    atomic_supersede: bool,
}

#[async_trait]
impl MemoryRecordStorePort for RecordStoreCapabilityFixture {
    fn supports_canonical_atomic(&self) -> bool {
        true
    }

    fn supports_atomic_record_quota_admission(&self) -> bool {
        true
    }

    fn supports_atomic_supersede(&self) -> bool {
        self.atomic_supersede
    }

    async fn create(&self, command: CreateMemoryRecordCommand) -> MemorySpiResult<MemoryRecord> {
        Ok(MemoryRecord {
            memory_id: command.memory_id,
            content: command.content,
        })
    }

    async fn retrieve(
        &self,
        _query: RetrieveMemoryRecordQuery,
    ) -> MemorySpiResult<Option<MemoryRecord>> {
        Ok(None)
    }

    async fn mark_deleted(
        &self,
        command: DeleteMemoryRecordCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        Ok(MemoryDeletionReceipt {
            memory_id: command.memory_id,
            deleted: false,
            already_deleted: false,
        })
    }
}

struct UnqualifiedRetriever;

#[async_trait]
impl MemoryRetrieverPort for UnqualifiedRetriever {
    fn retriever_code(&self) -> &str {
        "unqualified"
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Ok(MemoryRetrieverResult {
            memory_ids: Vec::new(),
        })
    }
}

struct ClaimedButDefaultRetriever;

#[async_trait]
impl MemoryRetrieverPort for ClaimedButDefaultRetriever {
    fn retriever_code(&self) -> &str {
        "claimed_default"
    }

    fn supports_bounded_scoped_search(&self) -> bool {
        true
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Ok(MemoryRetrieverResult {
            memory_ids: Vec::new(),
        })
    }
}

struct UnqualifiedGovernance;

#[async_trait]
impl MemoryGovernanceAccessPort for UnqualifiedGovernance {}

struct ClaimedButDefaultGovernance;

#[async_trait]
impl MemoryGovernanceAccessPort for ClaimedButDefaultGovernance {
    fn supports_bounded_governance_access(&self) -> bool {
        true
    }
}

struct UnqualifiedSpaceStore;

#[async_trait]
impl MemorySpaceStorePort for UnqualifiedSpaceStore {}

struct ClaimedButDefaultSpaceStore;

#[async_trait]
impl MemorySpaceStorePort for ClaimedButDefaultSpaceStore {
    fn supports_atomic_user_space_quota_admission(&self) -> bool {
        true
    }
}

struct UnqualifiedCandidateStore;

#[async_trait]
impl MemoryCandidateStorePort for UnqualifiedCandidateStore {
    async fn create(
        &self,
        command: CreateMemoryCandidateCommand,
    ) -> MemorySpiResult<MemoryCandidate> {
        Ok(MemoryCandidate {
            candidate_id: command.candidate_id,
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
        })
    }

    async fn retrieve(
        &self,
        _query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        Ok(None)
    }

    async fn approve(
        &self,
        _command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        Ok(None)
    }

    async fn reject(
        &self,
        _command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        Ok(None)
    }
}

struct ClaimedButDefaultCandidateStore;

#[async_trait]
impl MemoryCandidateStorePort for ClaimedButDefaultCandidateStore {
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
        UnqualifiedCandidateStore.create(command).await
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.retrieve(query).await
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.approve(command).await
    }

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.reject(command).await
    }
}

struct AtomicWithoutJournalCandidateStore;

#[async_trait]
impl MemoryCandidateStorePort for AtomicWithoutJournalCandidateStore {
    fn supports_candidate_detail_lookup(&self) -> bool {
        true
    }

    fn supports_candidate_listing(&self) -> bool {
        true
    }

    fn supports_atomic_candidate_promotion(&self) -> bool {
        true
    }

    async fn create(
        &self,
        command: CreateMemoryCandidateCommand,
    ) -> MemorySpiResult<MemoryCandidate> {
        UnqualifiedCandidateStore.create(command).await
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.retrieve(query).await
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.approve(command).await
    }

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.reject(command).await
    }
}

struct AtomicJournalWithoutDetailCandidateStore;

#[async_trait]
impl MemoryCandidateStorePort for AtomicJournalWithoutDetailCandidateStore {
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
        UnqualifiedCandidateStore.create(command).await
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.retrieve(query).await
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.approve(command).await
    }

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.reject(command).await
    }
}

struct AtomicJournalWithoutListingCandidateStore;

#[async_trait]
impl MemoryCandidateStorePort for AtomicJournalWithoutListingCandidateStore {
    fn supports_candidate_detail_lookup(&self) -> bool {
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
        UnqualifiedCandidateStore.create(command).await
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.retrieve(query).await
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.approve(command).await
    }

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        UnqualifiedCandidateStore.reject(command).await
    }
}

fn mutation_journal(memory_id: &str, scope_tag: &str, action: &str) -> MemoryMutationJournal {
    MemoryMutationJournal {
        outbox_id: format!("outbox-{scope_tag}-{action}"),
        aggregate_type: "memory_record".to_string(),
        aggregate_id: memory_id.to_string(),
        event_type: format!("memory.record.{action}"),
        event_version: "1.0".to_string(),
        payload_json: "{}".to_string(),
        audit_id: format!("audit-{scope_tag}-{action}"),
        audit_action: format!("memory.record.{action}"),
        audit_resource_type: "memory_record".to_string(),
        audit_resource_id: memory_id.to_string(),
        audit_result: "accepted".to_string(),
    }
}

fn eval_runtime() -> MemoryRuntimeDataPlane {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let executable = build_reference_executable_runtime(reference);
    let manifest = reference_profiles_manifest();
    let metadata = MemoryRuntimeProfileMetadata {
        profile_id: "reference-eval-contract".to_string(),
        implementation_kind: MemoryImplementationKind::SearchFirst,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    };
    let mut core = MemoryCoreRuntime::new(metadata);
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .chain(["MemoryContextAssemblerPort"])
    {
        assert!(manifest
            .port_exports
            .iter()
            .any(|export| export.port == port));
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &executable)
            .expect("reference runtime port must bind");
    }
    MemoryRuntimeDataPlane::try_for_phase1_http(core)
        .expect("reference contract runtime exposes all phase-1 HTTP ports")
}

fn core_with_record_store(record_store: Arc<dyn MemoryRecordStorePort>) -> MemoryCoreRuntime {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let reference_executable = build_reference_executable_runtime(reference);
    let record_executable = MemoryExecutablePluginRuntime::new(
        MemoryPluginPorts::new().with_record_store(record_store),
    );
    let mut core = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "record-preflight-contract".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .filter(|port| *port != "MemoryRecordStorePort")
    {
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &reference_executable)
            .unwrap();
    }
    core.bind_port(
        "record-contract-plugin",
        "MemoryRecordStorePort",
        &record_executable,
    )
    .unwrap();
    core
}

fn core_with_retriever(retriever: Arc<dyn MemoryRetrieverPort>) -> MemoryCoreRuntime {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let reference_executable = build_reference_executable_runtime(reference);
    let retriever_executable =
        MemoryExecutablePluginRuntime::new(MemoryPluginPorts::new().with_retriever(retriever));
    let mut core = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "retriever-preflight-contract".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .filter(|port| *port != "MemoryRetrieverPort")
    {
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &reference_executable)
            .unwrap();
    }
    core.bind_port(
        "retriever-contract-plugin",
        "MemoryRetrieverPort",
        &retriever_executable,
    )
    .unwrap();
    core
}

fn core_with_governance(governance: Arc<dyn MemoryGovernanceAccessPort>) -> MemoryCoreRuntime {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let reference_executable = build_reference_executable_runtime(reference);
    let governance_executable = MemoryExecutablePluginRuntime::new(
        MemoryPluginPorts::new().with_governance_access(governance),
    );
    let mut core = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "governance-preflight-contract".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .filter(|port| *port != "MemoryGovernanceAccessPort")
    {
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &reference_executable)
            .unwrap();
    }
    core.bind_port(
        "governance-contract-plugin",
        "MemoryGovernanceAccessPort",
        &governance_executable,
    )
    .unwrap();
    core
}

fn core_with_candidate_store(
    candidate_store: Arc<dyn MemoryCandidateStorePort>,
) -> MemoryCoreRuntime {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let reference_executable = build_reference_executable_runtime(reference);
    let candidate_executable = MemoryExecutablePluginRuntime::new(
        MemoryPluginPorts::new().with_candidate_store(candidate_store),
    );
    let mut core = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "candidate-preflight-contract".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .filter(|port| *port != "MemoryCandidateStorePort")
    {
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &reference_executable)
            .unwrap();
    }
    core.bind_port(
        "candidate-contract-plugin",
        "MemoryCandidateStorePort",
        &candidate_executable,
    )
    .unwrap();
    core
}

fn core_with_space_store(space_store: Arc<dyn MemorySpaceStorePort>) -> MemoryCoreRuntime {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let reference_executable = build_reference_executable_runtime(reference);
    let space_executable =
        MemoryExecutablePluginRuntime::new(MemoryPluginPorts::new().with_space_store(space_store));
    let mut core = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "space-preflight-contract".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .filter(|port| *port != "MemorySpaceStorePort")
    {
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &reference_executable)
            .unwrap();
    }
    core.bind_port(
        "space-contract-plugin",
        "MemorySpaceStorePort",
        &space_executable,
    )
    .unwrap();
    core
}

#[test]
fn phase1_data_plane_rejects_missing_required_port() {
    let runtime = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "incomplete-eval".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });

    match MemoryRuntimeDataPlane::try_for_phase1_http(runtime) {
        Err(MemoryRuntimeDataPlaneError::MissingRequiredPort { profile_id, port }) => {
            assert_eq!(profile_id, "incomplete-eval");
            assert_eq!(port, "MemoryRecordStorePort");
        }
        other => panic!("expected missing required port, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_record_store_without_atomic_supersede() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_record_store(Arc::new(
        RecordStoreCapabilityFixture {
            atomic_supersede: false,
        },
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "record-preflight-contract");
            assert_eq!(capability, "atomic_canonical_supersede");
        }
        other => panic!("expected atomic supersede capability failure, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_missing_governance_port() {
    let reference = Arc::new(ReferenceMemoryRuntime::new());
    let executable = build_reference_executable_runtime(reference);
    let mut core = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "missing-governance-contract".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: REFERENCE_PROFILES_PLUGIN_ID.to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });
    for port in PHASE1_HTTP_DATA_PLANE_PORTS
        .iter()
        .copied()
        .filter(|port| *port != "MemoryGovernanceAccessPort")
    {
        core.bind_port(REFERENCE_PROFILES_PLUGIN_ID, port, &executable)
            .unwrap();
    }
    match MemoryRuntimeDataPlane::try_for_phase1_http(core) {
        Err(MemoryRuntimeDataPlaneError::MissingRequiredPort { port, .. }) => {
            assert_eq!(port, "MemoryGovernanceAccessPort");
        }
        other => panic!("expected missing governance port, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_governance_without_bounded_capability() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_governance(Arc::new(
        UnqualifiedGovernance,
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "governance-preflight-contract");
            assert_eq!(capability, "bounded_tenant_scoped_governance_access");
        }
        other => panic!("expected governance capability failure, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_candidate_store_without_atomic_promotion() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        UnqualifiedCandidateStore,
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "candidate-preflight-contract");
            assert_eq!(capability, "atomic_candidate_promotion");
        }
        other => panic!("expected candidate promotion capability failure, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_candidate_promotion_without_atomic_journal() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        AtomicWithoutJournalCandidateStore,
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "candidate-preflight-contract");
            assert_eq!(capability, "atomic_candidate_promotion_journal");
        }
        other => panic!("expected candidate journal capability failure, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_candidate_store_without_listing_capability() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        AtomicJournalWithoutListingCandidateStore,
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "candidate-preflight-contract");
            assert_eq!(capability, "tenant_scoped_candidate_listing");
        }
        other => panic!("expected candidate listing capability failure, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_candidate_store_without_detail_projection() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        AtomicJournalWithoutDetailCandidateStore,
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "candidate-preflight-contract");
            assert_eq!(capability, "tenant_scoped_candidate_detail_lookup");
        }
        other => panic!("expected candidate detail capability failure, got {other:?}"),
    }
}

#[test]
fn phase1_data_plane_rejects_space_store_without_atomic_quota_admission() {
    match MemoryRuntimeDataPlane::try_for_phase1_http(core_with_space_store(Arc::new(
        UnqualifiedSpaceStore,
    ))) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "space-preflight-contract");
            assert_eq!(capability, "atomic_user_space_quota_admission");
        }
        other => panic!("expected space quota capability failure, got {other:?}"),
    }
}

#[tokio::test]
async fn claimed_space_quota_capability_still_fails_closed_without_mutation() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_space_store(Arc::new(
        ClaimedButDefaultSpaceStore,
    )))
    .expect("capability probe intentionally claims atomic space quota admission");
    let error = plane
        .create_space_atomic_with_quota(
            CreateMemorySpaceCommand {
                tenant_id: 1,
                space_id: 1,
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "owner-1".to_string(),
                space_type: "personal".to_string(),
                display_name: "Personal memory".to_string(),
                default_scope: "user".to_string(),
            },
            1,
        )
        .await
        .expect_err("default space mutation method must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}

#[tokio::test]
async fn claimed_atomic_supersede_capability_still_fails_closed_without_mutation() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_record_store(Arc::new(
        RecordStoreCapabilityFixture {
            atomic_supersede: true,
        },
    )))
    .expect("capability probe intentionally claims atomic supersede");
    let error = plane
        .supersede_canonical_memory_atomic_with_quota(
            SupersedeCanonicalMemoryAtomicCommand {
                scope: MemoryScopeContext::for_test(1, 1),
                old_memory_id: "memory-old".to_string(),
                new_memory_id: "memory-new".to_string(),
                scope_label: "user".to_string(),
                memory_type: "semantic".to_string(),
                subject: Some("account".to_string()),
                predicate: Some("prefers".to_string()),
                object_text: "new value".to_string(),
                canonical_text: "new value".to_string(),
                sensitivity_level: "internal".to_string(),
                expires_at: None,
                created_journal: mutation_journal("memory-new", "supersede", "created"),
                superseded_journal: mutation_journal("memory-old", "supersede", "superseded"),
                metadata_json: None,
            },
            2,
        )
        .await
        .expect_err("default supersede mutation method must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}

#[tokio::test]
async fn claimed_candidate_capability_still_fails_closed_without_promotion() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        ClaimedButDefaultCandidateStore,
    )))
    .expect("capability probe intentionally claims candidate promotion");
    let error = plane
        .promote_candidate_atomic_with_quota(
            PromoteMemoryCandidateAtomicCommand {
                scope: MemoryScopeContext::for_test(1, 1),
                candidate_id: "candidate-1".to_string(),
                memory_id: "memory-1".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "candidate".to_string(),
                evidence_links: vec![MemoryCandidateEvidenceLink {
                    source_id: "source-1".to_string(),
                    event_id: "event-1".to_string(),
                    confidence_delta: None,
                }],
                decided_by: None,
            },
            1,
        )
        .await
        .expect_err("default promotion method must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");

    let journal_error = plane
        .promote_candidate_atomic_with_quota_and_journal(
            PromoteMemoryCandidateAtomicWithJournalCommand {
                promotion: PromoteMemoryCandidateAtomicCommand {
                    scope: MemoryScopeContext::for_test(1, 1),
                    candidate_id: "candidate-1".to_string(),
                    memory_id: "memory-1".to_string(),
                    memory_type: "semantic".to_string(),
                    proposed_text: "candidate".to_string(),
                    evidence_links: Vec::new(),
                    decided_by: None,
                },
                journal: mutation_journal("memory-1", "candidate", "promoted"),
            },
            1,
        )
        .await
        .expect_err("default journaled promotion method must fail closed");
    assert_eq!(journal_error.code, "storage_error");
    assert_eq!(journal_error.detail, "internal storage error");
}

#[tokio::test]
async fn claimed_candidate_detail_capability_still_fails_closed_without_lookup() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        ClaimedButDefaultCandidateStore,
    )))
    .expect("capability probe intentionally claims candidate detail lookup");
    let error = plane
        .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
            tenant_id: 1,
            candidate_id: "candidate-1".to_string(),
        })
        .await
        .expect_err("default candidate detail lookup must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}

#[tokio::test]
async fn claimed_candidate_listing_capability_still_fails_closed_without_listing() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_candidate_store(Arc::new(
        ClaimedButDefaultCandidateStore,
    )))
    .expect("capability probe intentionally claims candidate listing");
    let error = plane
        .list_candidates(ListMemoryCandidatesQuery {
            tenant_id: 1,
            space_id: Some(1),
            page_size: 20,
            cursor: None,
        })
        .await
        .expect_err("default candidate listing must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}

#[tokio::test]
async fn claimed_governance_capability_still_fails_closed_without_resolution() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_governance(Arc::new(
        ClaimedButDefaultGovernance,
    )))
    .expect("capability probe intentionally claims bounded governance");
    let error = plane
        .resolve_space_governance(ResolveMemorySpaceGovernanceQuery {
            scope: MemoryScopeContext::for_test(1, 1),
            actor: None,
            capability_code: None,
            fact_limit: 1,
        })
        .await
        .expect_err("default governance resolution must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}

#[test]
fn phase1_data_plane_rejects_retriever_without_bounded_scoped_capability() {
    let core = core_with_retriever(Arc::new(UnqualifiedRetriever));
    match MemoryRuntimeDataPlane::try_for_phase1_http(core) {
        Err(MemoryRuntimeDataPlaneError::RequiredCapabilityMissing {
            profile_id,
            capability,
        }) => {
            assert_eq!(profile_id, "retriever-preflight-contract");
            assert_eq!(capability, "bounded_scope_aware_retrieval");
        }
        other => panic!("expected bounded retriever capability failure, got {other:?}"),
    }
}

#[tokio::test]
async fn claimed_retriever_capability_still_fails_closed_without_search_implementation() {
    let plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_with_retriever(Arc::new(
        ClaimedButDefaultRetriever,
    )))
    .expect("capability probe intentionally claims bounded search");
    let error = plane
        .search_candidates_scoped(SearchMemoryCandidatesQuery {
            scope: MemoryScopeContext::for_test(1, 1),
            query: "needle".to_string(),
            limit: 1,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: None,
            include_expired: false,
        })
        .await
        .expect_err("default search implementation must fail closed");
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}

#[tokio::test]
async fn canonical_record_retrieval_context_and_delete_are_scope_aware() {
    let plane = eval_runtime();
    let tenant_one = MemoryScopeContext::for_test(100, 10);
    let tenant_two = MemoryScopeContext::for_test(200, 10);

    plane
        .create_canonical_memory_atomic(CreateCanonicalMemoryCommand {
            scope: tenant_one.clone(),
            memory_id: "memory-1".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: Some("tenant-one".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: "tenant one preference".to_string(),
            canonical_text: "tenant one preference".to_string(),
            sensitivity_level: "internal".to_string(),
            expires_at: None,
            journal: mutation_journal("memory-1", "tenant-one", "created"),
            metadata_json: None,
        })
        .await
        .unwrap();
    plane
        .create_canonical_memory_atomic(CreateCanonicalMemoryCommand {
            scope: tenant_two.clone(),
            memory_id: "memory-1".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: Some("tenant-two".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: "tenant two preference".to_string(),
            canonical_text: "tenant two preference".to_string(),
            sensitivity_level: "internal".to_string(),
            expires_at: None,
            journal: mutation_journal("memory-1", "tenant-two", "created"),
            metadata_json: None,
        })
        .await
        .unwrap();

    assert_eq!(
        plane
            .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
                scope: tenant_one.clone(),
                memory_id: "memory-1".to_string(),
            })
            .await
            .unwrap()
            .unwrap()
            .canonical_text,
        "tenant one preference"
    );
    assert_eq!(
        plane
            .retrieve_candidates_scoped(
                tenant_one.clone(),
                RetrieveMemoryCandidatesCommand {
                read_scope: MemorySensitivityReadScope::Public,
                limit: sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES,
                    query: "preference".to_string(),
                },
            )
            .await
            .unwrap()
            .memory_ids,
        vec!["memory-1"]
    );

    let context = plane
        .assemble_context_scoped(
            tenant_one.clone(),
            AssembleMemoryContextCommand {
                memory_ids: vec!["memory-1".to_string()],
            },
        )
        .await
        .unwrap();
    assert_eq!(context.memory_ids, vec!["memory-1"]);
    assert_eq!(context.context_text, "tenant one preference");

    plane
        .delete_canonical_memory_atomic(DeleteCanonicalMemoryCommand {
            scope: tenant_one.clone(),
            memory_id: "memory-1".to_string(),
            journal: mutation_journal("memory-1", "tenant-one", "deleted"),
        })
        .await
        .unwrap();
    assert!(plane
        .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
            scope: tenant_one.clone(),
            memory_id: "memory-1".to_string(),
        })
        .await
        .unwrap()
        .is_none());
    assert!(plane
        .retrieve_candidates_scoped(
            tenant_one,
            RetrieveMemoryCandidatesCommand {
                read_scope: MemorySensitivityReadScope::Public,
                limit: sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES,
                query: "preference".to_string(),
            },
        )
        .await
        .unwrap()
        .memory_ids
        .is_empty());
    assert_eq!(
        plane
            .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
                scope: tenant_two,
                memory_id: "memory-1".to_string(),
            })
            .await
            .unwrap()
            .unwrap()
            .canonical_text,
        "tenant two preference"
    );
}

#[tokio::test]
async fn outbox_candidates_and_habits_do_not_cross_scope_boundaries() {
    let plane = eval_runtime();
    let first = MemoryScopeContext::for_test(1, 11);
    let second = MemoryScopeContext::for_test(2, 22);

    for scope in [first.clone(), second.clone()] {
        plane
            .append_outbox(AppendMemoryOutboxCommand {
                scope,
                outbox_id: "outbox-1".to_string(),
                aggregate_type: "memory_record".to_string(),
                aggregate_id: "memory-1".to_string(),
                event_type: "memory.record.created".to_string(),
                event_version: "1.0".to_string(),
                payload_json: "{}".to_string(),
            })
            .await
            .unwrap();
    }
    assert!(plane
        .retrieve_outbox(RetrieveMemoryOutboxQuery {
            scope: first.clone(),
            outbox_id: "outbox-1".to_string(),
        })
        .await
        .unwrap()
        .is_some());
    assert!(plane
        .retrieve_outbox(RetrieveMemoryOutboxQuery {
            scope: second.clone(),
            outbox_id: "outbox-1".to_string(),
        })
        .await
        .unwrap()
        .is_some());

    plane
        .create_candidate(CreateMemoryCandidateCommand {
            scope: first.clone(),
            candidate_id: "candidate-1".to_string(),
            candidate_type: "extraction".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "first candidate".to_string(),
            proposed_payload_json: None,
            evidence_json: None,
            confidence: 0.9,
        })
        .await
        .unwrap();
    plane
        .create_candidate(CreateMemoryCandidateCommand {
            scope: second.clone(),
            candidate_id: "candidate-1".to_string(),
            candidate_type: "extraction".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "second candidate".to_string(),
            proposed_payload_json: None,
            evidence_json: None,
            confidence: 0.8,
        })
        .await
        .unwrap();
    plane
        .reject_candidate(RejectMemoryCandidateCommand {
            scope: first.clone(),
            candidate_id: "candidate-1".to_string(),
            decision_reason: Some("not stable".to_string()),
            decided_by: Some(1),
        })
        .await
        .unwrap();
    assert_eq!(
        plane
            .retrieve_candidate(RetrieveMemoryCandidateQuery {
                scope: first.clone(),
                candidate_id: "candidate-1".to_string(),
            })
            .await
            .unwrap()
            .unwrap()
            .decision_state,
        "rejected"
    );
    assert_eq!(
        plane
            .retrieve_candidate(RetrieveMemoryCandidateQuery {
                scope: second.clone(),
                candidate_id: "candidate-1".to_string(),
            })
            .await
            .unwrap()
            .unwrap()
            .decision_state,
        "pending"
    );

    for (scope, description) in [(first.clone(), "light"), (second.clone(), "dark")] {
        plane
            .upsert_habit(UpsertMemoryHabitCommand {
                scope,
                habit_id: "habit-1".to_string(),
                user_id: 9,
                habit_key: "editor.theme".to_string(),
                habit_type: "preference".to_string(),
                description: description.to_string(),
                stage: "candidate".to_string(),
                strength: 0.4,
                confidence: 0.8,
                support_count: 1,
                metadata_json: None,
            })
            .await
            .unwrap();
    }
    plane
        .promote_habit(PromoteMemoryHabitCommand {
            scope: first.clone(),
            user_id: 9,
            habit_key: "editor.theme".to_string(),
            promoted_memory_id: Some("memory-1".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(
        plane
            .retrieve_habit(sdkwork_memory_spi::RetrieveMemoryHabitQuery {
                scope: first,
                user_id: 9,
                habit_key: "editor.theme".to_string(),
            })
            .await
            .unwrap()
            .unwrap()
            .stage,
        "promoted"
    );
    assert_eq!(
        plane
            .retrieve_habit(sdkwork_memory_spi::RetrieveMemoryHabitQuery {
                scope: second,
                user_id: 9,
                habit_key: "editor.theme".to_string(),
            })
            .await
            .unwrap()
            .unwrap()
            .stage,
        "candidate"
    );
}

#[tokio::test]
async fn unimplemented_external_bridge_is_not_exported_by_the_reference_runtime() {
    let plane = eval_runtime();
    let error = match plane.external_memory_bridge() {
        Ok(_) => panic!("reference runtime must not export an unimplemented provider bridge"),
        Err(error) => error,
    };
    assert_eq!(error.code, "storage_error");
    assert_eq!(error.detail, "internal storage error");
}
