use sdkwork_memory_spi::{
    AppendMemoryRetrievalTraceCommand, ApproveMemoryCandidateCommand, CreateMemoryCandidateCommand,
    CreateMemorySpaceCommand, DecayMemoryHabitCommand, MemoryCandidate,
    MemoryCandidateEvidenceLink, MemoryCandidatePromotion, MemoryCandidateStorePort,
    MemoryContextPackSnapshot, MemoryGovernanceAccessPort, MemoryGovernanceActor, MemoryHabit,
    MemoryHabitStorePort, MemoryRecordQuotaAdmission, MemoryRetrievalHitDraft,
    MemoryRetrievalTrace, MemoryRetrievalTraceStorePort, MemoryScopeContext,
    MemorySpaceQuotaAdmission, MemorySpaceRecord, MemorySpaceStorePort,
    PromoteMemoryCandidateAtomicCommand, PromoteMemoryHabitCommand, RejectMemoryCandidateCommand,
    ResolveMemorySpaceGovernanceQuery, RetrieveMemoryCandidateQuery, RetrieveMemoryHabitQuery,
    RetrieveMemoryRetrievalTraceQuery, UpsertMemoryHabitCommand, MAX_MEMORY_GOVERNANCE_FACTS,
};

#[test]
fn candidate_lifecycle_port_contract_types_are_public_and_scoped() {
    accept_candidate_port_object(None);

    let scope = MemoryScopeContext::for_test(1, 10);
    let create = CreateMemoryCandidateCommand {
        scope: scope.clone(),
        candidate_id: "cand-1".to_string(),
        candidate_type: "observation".to_string(),
        memory_type: "semantic".to_string(),
        proposed_text: "User prefers concise answers".to_string(),
        proposed_payload_json: Some(r#"{"preference":"concise"}"#.to_string()),
        evidence_json: Some(r#"{"source":"event"}"#.to_string()),
        confidence: 0.91,
        learning_job_uuid: None,
    };
    let retrieve = RetrieveMemoryCandidateQuery {
        scope: scope.clone(),
        candidate_id: create.candidate_id.clone(),
    };
    let approve = ApproveMemoryCandidateCommand {
        scope: scope.clone(),
        candidate_id: create.candidate_id.clone(),
        decision_reason: Some("confirmed by user".to_string()),
        decided_by: Some(7),
    };
    let reject = RejectMemoryCandidateCommand {
        scope,
        candidate_id: create.candidate_id.clone(),
        decision_reason: Some("stale signal".to_string()),
        decided_by: Some(8),
    };
    let candidate = MemoryCandidate {
        candidate_id: create.candidate_id,
        candidate_type: create.candidate_type,
        memory_type: create.memory_type,
        proposed_text: create.proposed_text,
        proposed_payload_json: create.proposed_payload_json,
        evidence_json: create.evidence_json,
        confidence: create.confidence,
        decision_state: "pending".to_string(),
        decision_reason: None,
        decided_by: None,
        decided_at: None,
    };
    let promotion = PromoteMemoryCandidateAtomicCommand {
        scope: approve.scope.clone(),
        candidate_id: candidate.candidate_id.clone(),
        memory_id: "memory-1".to_string(),
        memory_type: candidate.memory_type.clone(),
        proposed_text: candidate.proposed_text.clone(),
        evidence_links: vec![MemoryCandidateEvidenceLink {
            source_id: "source-1".to_string(),
            event_id: "event-1".to_string(),
            confidence_delta: Some(0.91),
        }],
        decided_by: approve.decided_by,
    };
    let promotion_outcome = MemoryRecordQuotaAdmission::Admitted(MemoryCandidatePromotion {
        candidate_id: promotion.candidate_id.clone(),
        memory_id: promotion.memory_id.clone(),
    });

    assert_eq!(retrieve.candidate_id, "cand-1");
    assert_eq!(approve.decided_by, Some(7));
    assert_eq!(reject.decided_by, Some(8));
    assert_eq!(candidate.decision_state, "pending");
    assert!(matches!(
        promotion_outcome,
        MemoryRecordQuotaAdmission::Admitted(_)
    ));
}

#[test]
fn habit_learning_port_contract_types_are_public_and_user_scoped() {
    accept_habit_port_object(None);

    let scope = MemoryScopeContext::for_test(1, 10);
    let upsert = UpsertMemoryHabitCommand {
        scope: scope.clone(),
        habit_id: "habit-1".to_string(),
        user_id: 42,
        habit_key: "answer_style:concise".to_string(),
        habit_type: "preference".to_string(),
        description: "Prefers concise answers".to_string(),
        stage: "candidate".to_string(),
        strength: 0.4,
        confidence: 0.8,
        support_count: 2,
        metadata_json: Some(r#"{"source":"signals"}"#.to_string()),
    };
    let retrieve = RetrieveMemoryHabitQuery {
        scope: scope.clone(),
        user_id: upsert.user_id,
        habit_key: upsert.habit_key.clone(),
    };
    let promote = PromoteMemoryHabitCommand {
        scope: scope.clone(),
        user_id: upsert.user_id,
        habit_key: upsert.habit_key.clone(),
        promoted_memory_id: Some("rec-1".to_string()),
    };
    let decay = DecayMemoryHabitCommand {
        scope,
        user_id: upsert.user_id,
        habit_key: upsert.habit_key.clone(),
        strength_delta: 0.1,
    };
    let habit = MemoryHabit {
        habit_id: upsert.habit_id,
        user_id: upsert.user_id,
        habit_key: upsert.habit_key,
        habit_type: upsert.habit_type,
        description: upsert.description,
        stage: upsert.stage,
        strength: upsert.strength,
        confidence: upsert.confidence,
        support_count: upsert.support_count,
        last_signal_at: Some("2026-06-10T00:00:00Z".to_string()),
        promoted_memory_id: None,
        decay_after: None,
        metadata_json: upsert.metadata_json,
    };

    assert_eq!(retrieve.user_id, 42);
    assert_eq!(promote.promoted_memory_id.as_deref(), Some("rec-1"));
    assert_eq!(decay.strength_delta, 0.1);
    assert_eq!(habit.habit_key, "answer_style:concise");
}

#[test]
fn retrieval_trace_port_contract_types_are_public_and_bounded() {
    accept_retrieval_trace_port_object(None);

    let scope = MemoryScopeContext::for_test(1, 10);
    let hit = MemoryRetrievalHitDraft {
        hit_id: "hit-1".to_string(),
        memory_id: Some("rec-1".to_string()),
        space_id: Some(scope.space_id),
        retriever_name: "native_sql".to_string(),
        result_rank: 1,
        raw_score: Some(0.75),
        fused_score: Some(0.9),
        explanation_json: Some(r#"{"match":"keyword"}"#.to_string()),
        status: "selected".to_string(),
    };
    let context_pack = MemoryContextPackSnapshot {
        context_pack_id: "pack-1".to_string(),
        pack_json: r#"{"memoryIds":["rec-1"]}"#.to_string(),
        estimated_tokens: 12,
        truncated: false,
    };
    let append = AppendMemoryRetrievalTraceCommand {
        scope: scope.clone(),
        trace_id: "trace-1".to_string(),
        actor_id: Some("user-42".to_string()),
        query_text: Some("concise answer preference".to_string()),
        query_hash: "hash:trace-1".to_string(),
        retrievers_json: Some(r#"["native_sql"]"#.to_string()),
        latency_ms: Some(17),
        degraded: false,
        metadata_json: Some(r#"{"profile":"native"}"#.to_string()),
        hits: vec![hit],
        context_pack: Some(context_pack),
    };
    let retrieve = RetrieveMemoryRetrievalTraceQuery {
        scope,
        trace_id: append.trace_id.clone(),
    };
    let trace = MemoryRetrievalTrace {
        trace_id: append.trace_id,
        actor_id: append.actor_id,
        query_text: append.query_text,
        query_hash: append.query_hash,
        retrievers_json: append.retrievers_json,
        latency_ms: append.latency_ms,
        result_count: 1,
        degraded: append.degraded,
        metadata_json: append.metadata_json,
        hits: append.hits,
        context_pack: append.context_pack,
        created_at: "2026-09-30T00:00:00.000Z".to_string(),
        space_id: Some(10),
    };

    assert_eq!(retrieve.trace_id, "trace-1");
    assert_eq!(trace.hits.len(), 1);
    assert_eq!(trace.result_count, 1);
    assert!(!trace.degraded);
    assert_eq!(trace.space_id, Some(10));
}

#[test]
fn governance_access_port_contract_is_scoped_bounded_and_actor_typed() {
    accept_governance_port_object(None);

    let query = ResolveMemorySpaceGovernanceQuery {
        scope: MemoryScopeContext::for_test(1, 10),
        actor: Some(MemoryGovernanceActor {
            subject_type: Some("user".to_string()),
            subject_id: "42".to_string(),
        }),
        capability_code: Some("memory.retrieve".to_string()),
        fact_limit: MAX_MEMORY_GOVERNANCE_FACTS,
    };

    assert_eq!(query.scope.tenant_id, 1);
    assert_eq!(query.scope.space_id, 10);
    assert_eq!(query.actor.unwrap().subject_type.as_deref(), Some("user"));
    assert_eq!(query.fact_limit, 32);
}

#[test]
fn space_store_quota_contract_types_are_public_and_tenant_scoped() {
    accept_space_store_port_object(None);
    let command = CreateMemorySpaceCommand {
        tenant_id: 7,
        space_id: 11,
        organization_id: Some(3),
        owner_subject_type: "user".to_string(),
        owner_subject_id: "42".to_string(),
        space_type: "personal".to_string(),
        display_name: "Personal memory".to_string(),
        default_scope: "user".to_string(),
    };
    let record = MemorySpaceRecord {
        space_id: command.space_id,
        uuid: "space-11".to_string(),
        tenant_id: command.tenant_id,
        organization_id: command.organization_id,
        owner_subject_type: command.owner_subject_type,
        owner_subject_id: command.owner_subject_id,
        space_type: command.space_type,
        display_name: command.display_name,
        default_scope: command.default_scope,
        lifecycle_status: "active".to_string(),
        created_at: "2026-07-12T00:00:00Z".to_string(),
        updated_at: "2026-07-12T00:00:00Z".to_string(),
        version: 0,
    };
    let admission = MemorySpaceQuotaAdmission::Admitted(record);

    assert!(matches!(
        admission,
        MemorySpaceQuotaAdmission::Admitted(MemorySpaceRecord {
            tenant_id: 7,
            space_id: 11,
            ..
        })
    ));
}

fn accept_candidate_port_object(_port: Option<&dyn MemoryCandidateStorePort>) {}

fn accept_habit_port_object(_port: Option<&dyn MemoryHabitStorePort>) {}

fn accept_retrieval_trace_port_object(_port: Option<&dyn MemoryRetrievalTraceStorePort>) {}

fn accept_governance_port_object(_port: Option<&dyn MemoryGovernanceAccessPort>) {}

fn accept_space_store_port_object(_port: Option<&dyn MemorySpaceStorePort>) {}
