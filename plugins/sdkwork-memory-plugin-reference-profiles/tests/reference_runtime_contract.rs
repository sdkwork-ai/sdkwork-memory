use sdkwork_memory_plugin_reference_profiles::ReferenceMemoryRuntime;
use sdkwork_memory_spi::{
    AppendMemoryAuditCommand, AppendMemoryEventCommand, AppendMemoryOutboxCommand,
    AppendMemoryRetrievalTraceCommand, ApproveMemoryCandidateCommand, AssembleMemoryContextCommand,
    CreateCanonicalMemoryCommand, CreateMemoryCandidateCommand, CreateMemoryRecordCommand,
    DecayMemoryHabitCommand, DeleteCanonicalMemoryCommand, ExternalMemoryBridgePort,
    ExternalMemoryImportCommand, ListMemoryRetrievalTracesQuery, ListPendingMemoryOutboxQuery,
    MarkMemoryOutboxPublishedCommand, MemoryAuditStorePort, MemoryCandidateEvidenceLink,
    MemoryCandidateStorePort, MemoryContextAssemblerPort, MemoryContextPackSnapshot,
    MemoryEvaluationPort, MemoryEventStorePort, MemoryHabitStorePort, MemoryIndexPort,
    MemoryMutationJournal, MemoryOutboxStorePort, MemoryRecordQuotaAdmission,
    MemoryRecordStorePort, MemoryRetrievalHitDraft, MemoryRetrievalTraceStorePort,
    MemoryRetrieverKind, MemoryRetrieverPort, MemoryScopeContext, MemorySensitivityReadScope,
    MemorySpiError, PromoteMemoryCandidateAtomicCommand,
    PromoteMemoryCandidateAtomicWithJournalCommand, PromoteMemoryHabitCommand,
    RejectMemoryCandidateCommand, RetrieveCanonicalMemoryQuery, RetrieveMemoryAuditQuery,
    RetrieveMemoryCandidateDetailQuery, RetrieveMemoryCandidateQuery,
    RetrieveMemoryCandidatesCommand, RetrieveMemoryEventQuery, RetrieveMemoryHabitQuery,
    RetrieveMemoryOutboxQuery, RetrieveMemoryRecordQuery, RetrieveMemoryRetrievalTraceQuery,
    RunMemoryEvalCommand, SearchMemoryCandidatesQuery, SupersedeCanonicalMemoryAtomicCommand,
    UpsertMemoryHabitCommand, MAX_MEMORY_RETRIEVAL_CANDIDATES,
};

fn mutation_journal(memory_id: &str, action: &str) -> MemoryMutationJournal {
    MemoryMutationJournal {
        outbox_id: format!("outbox-{memory_id}-{action}"),
        aggregate_type: "memory_record".to_string(),
        aggregate_id: memory_id.to_string(),
        event_type: format!("memory.record.{action}"),
        event_version: "1.0".to_string(),
        payload_json: "{}".to_string(),
        audit_id: format!("audit-{memory_id}-{action}"),
        audit_action: format!("memory.record.{action}"),
        audit_resource_type: "memory_record".to_string(),
        audit_resource_id: memory_id.to_string(),
        audit_result: "accepted".to_string(),
    }
}

#[tokio::test]
async fn reference_runtime_round_trips_core_ports_and_retrieves_by_keyword() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryRecordStorePort::create(
        &runtime,
        CreateMemoryRecordCommand {
            scope: scope.clone(),
            memory_id: "rec-reference".to_string(),
            content: "reference memory supports keyword lookup".to_string(),
        },
    )
    .await
    .unwrap();
    MemoryEventStorePort::append(
        &runtime,
        AppendMemoryEventCommand {
            scope: scope.clone(),
            event_id: "evt-reference".to_string(),
            content: "event sourced baseline".to_string(),
        },
    )
    .await
    .unwrap();
    MemoryAuditStorePort::append(
        &runtime,
        AppendMemoryAuditCommand {
            scope: scope.clone(),
            audit_id: "aud-reference".to_string(),
            action: "memory.reference.checked".to_string(),
            resource_type: "ai_record".to_string(),
            resource_id: "rec-reference".to_string(),
            result: "success".to_string(),
        },
    )
    .await
    .unwrap();
    MemoryOutboxStorePort::append(
        &runtime,
        AppendMemoryOutboxCommand {
            scope: scope.clone(),
            outbox_id: "out-reference".to_string(),
            aggregate_type: "ai_record".to_string(),
            aggregate_id: "rec-reference".to_string(),
            event_type: "memory.record.created".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"memoryId":"rec-reference"}"#.to_string(),
        },
    )
    .await
    .unwrap();

    let record = MemoryRecordStorePort::retrieve(
        &runtime,
        RetrieveMemoryRecordQuery {
            scope: scope.clone(),
            memory_id: "rec-reference".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let event = MemoryEventStorePort::retrieve(
        &runtime,
        RetrieveMemoryEventQuery {
            scope: scope.clone(),
            event_id: "evt-reference".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let audit = MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "aud-reference".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let hits = MemoryRetrieverPort::retrieve_scoped(
        &runtime,
        scope.clone(),
        RetrieveMemoryCandidatesCommand {
            query: "keyword".to_string(),
        },
    )
    .await
    .unwrap();
    let index_error = MemoryIndexPort::index(&runtime, "rec-reference".to_string())
        .await
        .unwrap_err();

    assert_eq!(record.content, "reference memory supports keyword lookup");
    assert_eq!(event.content, "event sourced baseline");
    assert_eq!(audit.result, "success");
    assert_eq!(hits.memory_ids, vec!["rec-reference".to_string()]);
    assert!(index_error
        .to_string()
        .contains("no independently materialized index"));
}

#[tokio::test]
async fn reference_runtime_outbox_context_eval_and_bridge_fail_closed_are_deterministic() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryRecordStorePort::create(
        &runtime,
        CreateMemoryRecordCommand {
            scope: scope.clone(),
            memory_id: "rec-context".to_string(),
            content: "context line".to_string(),
        },
    )
    .await
    .unwrap();
    MemoryOutboxStorePort::append(
        &runtime,
        AppendMemoryOutboxCommand {
            scope: scope.clone(),
            outbox_id: "out-context".to_string(),
            aggregate_type: "ai_record".to_string(),
            aggregate_id: "rec-context".to_string(),
            event_type: "memory.record.created".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"memoryId":"rec-context"}"#.to_string(),
        },
    )
    .await
    .unwrap();

    let pending = MemoryOutboxStorePort::list_pending(
        &runtime,
        ListPendingMemoryOutboxQuery {
            scope: scope.clone(),
            limit: 10,
        },
    )
    .await
    .unwrap();
    let published = MemoryOutboxStorePort::mark_published(
        &runtime,
        MarkMemoryOutboxPublishedCommand {
            scope: scope.clone(),
            outbox_id: "out-context".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let context = MemoryContextAssemblerPort::assemble_scoped(
        &runtime,
        scope.clone(),
        AssembleMemoryContextCommand {
            memory_ids: vec!["rec-context".to_string()],
        },
    )
    .await
    .unwrap();
    let eval_error = MemoryEvaluationPort::run(
        &runtime,
        RunMemoryEvalCommand {
            eval_type: "baseline".to_string(),
        },
    )
    .await
    .unwrap_err();
    let bridge_error = ExternalMemoryBridgePort::import(&runtime, ExternalMemoryImportCommand)
        .await
        .unwrap_err();

    assert_eq!(pending.len(), 1);
    assert_eq!(published.publish_state, "published");
    assert!(published
        .published_at
        .as_deref()
        .is_some_and(|value| value.ends_with('Z')));
    assert_eq!(context.context_text, "context line");
    assert!(eval_error
        .to_string()
        .contains("no golden dataset evaluation engine"));
    assert!(bridge_error
        .to_string()
        .contains("fail-closed until a reviewed provider adapter is configured"));
}

#[tokio::test]
async fn reference_runtime_round_trips_learning_and_trace_ports_by_scope() {
    let runtime = ReferenceMemoryRuntime::new();
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 2);
    let wrong_space = MemoryScopeContext::for_test(1, 2);

    MemoryRecordStorePort::create(
        &runtime,
        CreateMemoryRecordCommand {
            scope: tenant_one.clone(),
            memory_id: "rec-trace".to_string(),
            content: "traceable reference memory".to_string(),
        },
    )
    .await
    .unwrap();

    let created_candidate = MemoryCandidateStorePort::create(
        &runtime,
        CreateMemoryCandidateCommand {
            scope: tenant_one.clone(),
            candidate_id: "cand-reference".to_string(),
            candidate_type: "observation".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "User prefers concise answers".to_string(),
            proposed_payload_json: Some(r#"{"preference":"concise"}"#.to_string()),
            evidence_json: Some(r#"{"source":"event"}"#.to_string()),
            confidence: 0.91,
        },
    )
    .await
    .unwrap();
    MemoryCandidateStorePort::create(
        &runtime,
        CreateMemoryCandidateCommand {
            scope: tenant_two.clone(),
            candidate_id: "cand-reference".to_string(),
            candidate_type: "observation".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "Tenant two candidate".to_string(),
            proposed_payload_json: None,
            evidence_json: None,
            confidence: 0.51,
        },
    )
    .await
    .unwrap();
    let approved = MemoryCandidateStorePort::approve(
        &runtime,
        ApproveMemoryCandidateCommand {
            scope: tenant_one.clone(),
            candidate_id: "cand-reference".to_string(),
            decision_reason: Some("confirmed".to_string()),
            decided_by: Some(7),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let rejected = MemoryCandidateStorePort::reject(
        &runtime,
        RejectMemoryCandidateCommand {
            scope: tenant_two.clone(),
            candidate_id: "cand-reference".to_string(),
            decision_reason: Some("stale".to_string()),
            decided_by: Some(8),
        },
    )
    .await
    .unwrap()
    .unwrap();

    let inserted_habit = MemoryHabitStorePort::upsert(
        &runtime,
        UpsertMemoryHabitCommand {
            scope: tenant_one.clone(),
            habit_id: "habit-reference".to_string(),
            user_id: 42,
            habit_key: "answer_style:concise".to_string(),
            habit_type: "preference".to_string(),
            description: "Prefers concise answers".to_string(),
            stage: "candidate".to_string(),
            strength: 0.4,
            confidence: 0.8,
            support_count: 2,
            metadata_json: Some(r#"{"source":"signals"}"#.to_string()),
        },
    )
    .await
    .unwrap();
    let promoted = MemoryHabitStorePort::promote(
        &runtime,
        PromoteMemoryHabitCommand {
            scope: tenant_one.clone(),
            user_id: 42,
            habit_key: "answer_style:concise".to_string(),
            promoted_memory_id: Some("rec-trace".to_string()),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let decayed = MemoryHabitStorePort::decay(
        &runtime,
        DecayMemoryHabitCommand {
            scope: tenant_one.clone(),
            user_id: 42,
            habit_key: "answer_style:concise".to_string(),
            strength_delta: 0.1,
        },
    )
    .await
    .unwrap()
    .unwrap();

    let trace = MemoryRetrievalTraceStorePort::append(
        &runtime,
        AppendMemoryRetrievalTraceCommand {
            scope: tenant_one.clone(),
            trace_id: "trace-reference".to_string(),
            actor_id: Some("user-42".to_string()),
            query_text: Some("traceable".to_string()),
            query_hash: "hash:trace-reference".to_string(),
            retrievers_json: Some(r#"["reference_keyword"]"#.to_string()),
            latency_ms: Some(3),
            degraded: false,
            metadata_json: Some(r#"{"profile":"reference"}"#.to_string()),
            hits: vec![MemoryRetrievalHitDraft {
                hit_id: "hit-reference".to_string(),
                memory_id: Some("rec-trace".to_string()),
                space_id: Some(tenant_one.space_id),
                retriever_name: "reference_keyword".to_string(),
                result_rank: 1,
                raw_score: Some(0.9),
                fused_score: Some(0.95),
                explanation_json: Some(r#"{"match":"keyword"}"#.to_string()),
                status: "selected".to_string(),
            }],
            context_pack: Some(MemoryContextPackSnapshot {
                context_pack_id: "pack-reference".to_string(),
                pack_json: r#"{"memoryIds":["rec-trace"]}"#.to_string(),
                estimated_tokens: 9,
                truncated: false,
            }),
        },
    )
    .await
    .unwrap();
    let retrieved_trace = MemoryRetrievalTraceStorePort::retrieve(
        &runtime,
        RetrieveMemoryRetrievalTraceQuery {
            scope: tenant_one.clone(),
            trace_id: "trace-reference".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let recent = MemoryRetrievalTraceStorePort::list_recent(
        &runtime,
        ListMemoryRetrievalTracesQuery {
            scope: tenant_one.clone(),
            limit: 1,
        },
    )
    .await
    .unwrap();

    assert_eq!(created_candidate.decision_state, "pending");
    assert_eq!(approved.decision_state, "approved");
    assert_eq!(approved.decided_by, Some(7));
    assert_eq!(rejected.decision_state, "rejected");
    assert_eq!(inserted_habit.strength, 0.4);
    assert_eq!(promoted.promoted_memory_id.as_deref(), Some("rec-trace"));
    assert_eq!(decayed.stage, "decayed");
    assert!((decayed.strength - 0.3).abs() < f64::EPSILON);
    assert_eq!(trace.result_count, 1);
    assert_eq!(
        retrieved_trace.hits[0].memory_id.as_deref(),
        Some("rec-trace")
    );
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].trace_id, "trace-reference");
    assert!(MemoryCandidateStorePort::retrieve(
        &runtime,
        RetrieveMemoryCandidateQuery {
            scope: wrong_space.clone(),
            candidate_id: "cand-reference".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryHabitStorePort::retrieve(
        &runtime,
        RetrieveMemoryHabitQuery {
            scope: wrong_space.clone(),
            user_id: 42,
            habit_key: "answer_style:concise".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryRetrievalTraceStorePort::retrieve(
        &runtime,
        RetrieveMemoryRetrievalTraceQuery {
            scope: wrong_space,
            trace_id: "trace-reference".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn reference_retrieval_and_context_assembly_are_isolated_by_tenant_and_space() {
    let runtime = ReferenceMemoryRuntime::new();
    let tenant_one = MemoryScopeContext::for_test(1, 10);
    let tenant_two = MemoryScopeContext::for_test(2, 10);
    let tenant_one_other_space = MemoryScopeContext::for_test(1, 20);

    for (scope, memory_id, content) in [
        (
            tenant_one.clone(),
            "rec-tenant-one",
            "shared isolation keyword tenant one",
        ),
        (
            tenant_two.clone(),
            "rec-tenant-two",
            "shared isolation keyword tenant two",
        ),
        (
            tenant_one_other_space.clone(),
            "rec-other-space",
            "shared isolation keyword other space",
        ),
    ] {
        MemoryRecordStorePort::create(
            &runtime,
            CreateMemoryRecordCommand {
                scope,
                memory_id: memory_id.to_string(),
                content: content.to_string(),
            },
        )
        .await
        .unwrap();
    }

    let tenant_one_hits = MemoryRetrieverPort::retrieve_scoped(
        &runtime,
        tenant_one.clone(),
        RetrieveMemoryCandidatesCommand {
            query: "isolation keyword".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(tenant_one_hits.memory_ids, vec!["rec-tenant-one"]);

    let tenant_two_hits = MemoryRetrieverPort::retrieve_scoped(
        &runtime,
        tenant_two.clone(),
        RetrieveMemoryCandidatesCommand {
            query: "isolation keyword".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(tenant_two_hits.memory_ids, vec!["rec-tenant-two"]);

    let other_space_hits = MemoryRetrieverPort::retrieve_scoped(
        &runtime,
        tenant_one_other_space.clone(),
        RetrieveMemoryCandidatesCommand {
            query: "isolation keyword".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(other_space_hits.memory_ids, vec!["rec-other-space"]);

    let tenant_one_context = MemoryContextAssemblerPort::assemble_scoped(
        &runtime,
        tenant_one,
        AssembleMemoryContextCommand {
            memory_ids: vec![
                "rec-tenant-one".to_string(),
                "rec-tenant-two".to_string(),
                "rec-other-space".to_string(),
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        tenant_one_context.context_text,
        "shared isolation keyword tenant one"
    );
    assert_eq!(tenant_one_context.memory_ids, vec!["rec-tenant-one"]);

    let tenant_two_context = MemoryContextAssemblerPort::assemble_scoped(
        &runtime,
        tenant_two,
        AssembleMemoryContextCommand {
            memory_ids: vec![
                "rec-tenant-one".to_string(),
                "rec-tenant-two".to_string(),
                "rec-other-space".to_string(),
            ],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        tenant_two_context.context_text,
        "shared isolation keyword tenant two"
    );
    assert_eq!(tenant_two_context.memory_ids, vec!["rec-tenant-two"]);

    let unscoped_retrieval_error = MemoryRetrieverPort::retrieve(
        &runtime,
        RetrieveMemoryCandidatesCommand {
            query: "isolation keyword".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert!(unscoped_retrieval_error
        .to_string()
        .contains("use retrieve_scoped"));

    let unscoped_context_error = MemoryContextAssemblerPort::assemble(
        &runtime,
        AssembleMemoryContextCommand {
            memory_ids: vec!["rec-tenant-one".to_string()],
        },
    )
    .await
    .unwrap_err();
    assert!(unscoped_context_error
        .to_string()
        .contains("use assemble_scoped"));
}

#[tokio::test]
async fn reference_record_quota_admission_is_atomic_and_releases_deleted_slots() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(10, 100);
    let first = CreateCanonicalMemoryCommand {
        scope: scope.clone(),
        memory_id: "quota-first".to_string(),
        scope_label: "user".to_string(),
        memory_type: "semantic".to_string(),
        subject: None,
        predicate: None,
        object_text: "first".to_string(),
        canonical_text: "first".to_string(),
        sensitivity_level: "internal".to_string(),
        expires_at: None,
        journal: mutation_journal("quota-first", "created"),
        metadata_json: None,
    };
    assert!(matches!(
        MemoryRecordStorePort::create_canonical_atomic_with_quota(&runtime, first, 1)
            .await
            .unwrap(),
        MemoryRecordQuotaAdmission::Admitted(_)
    ));

    let rejected = MemoryRecordStorePort::create_canonical_atomic_with_quota(
        &runtime,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "quota-rejected".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: None,
            predicate: None,
            object_text: "rejected".to_string(),
            canonical_text: "rejected".to_string(),
            sensitivity_level: "internal".to_string(),
            expires_at: None,
            journal: mutation_journal("quota-rejected", "created"),
            metadata_json: None,
        },
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        rejected,
        MemoryRecordQuotaAdmission::QuotaExceeded {
            active_records: 1,
            max_active_records: 1,
        }
    );
    assert!(MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "quota-rejected".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-quota-rejected-created".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-quota-rejected-created".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());

    MemoryRecordStorePort::delete_canonical_atomic(
        &runtime,
        DeleteCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "quota-first".to_string(),
            journal: mutation_journal("quota-first", "deleted"),
        },
    )
    .await
    .unwrap();
    let admitted = MemoryRecordStorePort::create_canonical_atomic_with_quota(
        &runtime,
        CreateCanonicalMemoryCommand {
            scope,
            memory_id: "quota-reused".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: None,
            predicate: None,
            object_text: "reused".to_string(),
            canonical_text: "reused".to_string(),
            sensitivity_level: "internal".to_string(),
            expires_at: None,
            journal: mutation_journal("quota-reused", "created"),
            metadata_json: None,
        },
        1,
    )
    .await
    .unwrap();
    assert!(matches!(admitted, MemoryRecordQuotaAdmission::Admitted(_)));
}

#[tokio::test]
async fn reference_candidate_promotion_is_quota_atomic_and_retry_idempotent() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(10, 100);
    for candidate_id in ["candidate-first", "candidate-second"] {
        MemoryCandidateStorePort::create(
            &runtime,
            CreateMemoryCandidateCommand {
                scope: scope.clone(),
                candidate_id: candidate_id.to_string(),
                candidate_type: "observation".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: format!("proposal for {candidate_id}"),
                proposed_payload_json: None,
                evidence_json: None,
                confidence: 0.9,
            },
        )
        .await
        .unwrap();
    }

    let promoted = MemoryCandidateStorePort::promote_atomic_with_quota_and_journal(
        &runtime,
        PromoteMemoryCandidateAtomicWithJournalCommand {
            promotion: PromoteMemoryCandidateAtomicCommand {
                scope: scope.clone(),
                candidate_id: "candidate-first".to_string(),
                memory_id: "promoted-first".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "promoted reference memory".to_string(),
                evidence_links: vec![MemoryCandidateEvidenceLink {
                    source_id: "source-first".to_string(),
                    event_id: "event-first".to_string(),
                    confidence_delta: Some(0.9),
                }],
                decided_by: Some(7),
            },
            journal: mutation_journal("promoted-first", "candidate-promoted"),
        },
        1,
    )
    .await
    .unwrap();
    let MemoryRecordQuotaAdmission::Admitted(promoted) = promoted else {
        panic!("first candidate promotion must be admitted");
    };
    assert_eq!(promoted.memory_id, "promoted-first");
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-promoted-first-candidate-promoted".to_string(),
        },
    )
    .await
    .unwrap()
    .is_some());
    assert!(MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-promoted-first-candidate-promoted".to_string(),
        },
    )
    .await
    .unwrap()
    .is_some());

    let retry = MemoryCandidateStorePort::promote_atomic_with_quota_and_journal(
        &runtime,
        PromoteMemoryCandidateAtomicWithJournalCommand {
            promotion: PromoteMemoryCandidateAtomicCommand {
                scope: scope.clone(),
                candidate_id: "candidate-first".to_string(),
                memory_id: "promoted-duplicate".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "retry payload".to_string(),
                evidence_links: Vec::new(),
                decided_by: Some(8),
            },
            journal: mutation_journal("promoted-duplicate", "candidate-retry"),
        },
        1,
    )
    .await
    .unwrap();
    let MemoryRecordQuotaAdmission::Admitted(retry) = retry else {
        panic!("candidate retry must return its existing target");
    };
    assert_eq!(retry.memory_id, "promoted-first");
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-promoted-duplicate-candidate-retry".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());

    let rejected = MemoryCandidateStorePort::promote_atomic_with_quota_and_journal(
        &runtime,
        PromoteMemoryCandidateAtomicWithJournalCommand {
            promotion: PromoteMemoryCandidateAtomicCommand {
                scope: scope.clone(),
                candidate_id: "candidate-second".to_string(),
                memory_id: "promoted-second".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "second reference memory".to_string(),
                evidence_links: Vec::new(),
                decided_by: Some(9),
            },
            journal: mutation_journal("promoted-second", "candidate-rejected-by-quota"),
        },
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        rejected,
        MemoryRecordQuotaAdmission::QuotaExceeded {
            active_records: 1,
            max_active_records: 1,
        }
    );
    let pending = MemoryCandidateStorePort::retrieve(
        &runtime,
        RetrieveMemoryCandidateQuery {
            scope: scope.clone(),
            candidate_id: "candidate-second".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(pending.decision_state, "pending");
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-promoted-second-candidate-rejected-by-quota".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope,
            memory_id: "promoted-second".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn reference_candidate_detail_preserves_timestamps_target_and_tenant_scope() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(21, 210);
    MemoryCandidateStorePort::create(
        &runtime,
        CreateMemoryCandidateCommand {
            scope: scope.clone(),
            candidate_id: "candidate-detail".to_string(),
            candidate_type: "observation".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "Reference detail candidate".to_string(),
            proposed_payload_json: Some(r#"{"preference":"detail"}"#.to_string()),
            evidence_json: Some(r#"{"eventId":"event-detail"}"#.to_string()),
            confidence: 0.93,
        },
    )
    .await
    .unwrap();

    let initial = MemoryCandidateStorePort::retrieve_detail(
        &runtime,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(initial.space_id, scope.space_id);
    assert_eq!(
        initial.evidence_json.as_deref(),
        Some(r#"{"eventId":"event-detail"}"#)
    );
    assert!(initial.created_at.ends_with('Z'));
    assert_eq!(initial.updated_at, initial.created_at);
    assert!(initial.target_memory_id.is_none());

    let promoted = MemoryCandidateStorePort::promote_atomic_with_quota_and_journal(
        &runtime,
        PromoteMemoryCandidateAtomicWithJournalCommand {
            promotion: PromoteMemoryCandidateAtomicCommand {
                scope: scope.clone(),
                candidate_id: "candidate-detail".to_string(),
                memory_id: "candidate-detail-target".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "Reference detail candidate".to_string(),
                evidence_links: Vec::new(),
                decided_by: Some(7),
            },
            journal: mutation_journal("candidate-detail-target", "promoted"),
        },
        10,
    )
    .await
    .unwrap();
    assert!(matches!(promoted, MemoryRecordQuotaAdmission::Admitted(_)));

    let updated = MemoryCandidateStorePort::retrieve_detail(
        &runtime,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let candidate = MemoryCandidateStorePort::retrieve(
        &runtime,
        RetrieveMemoryCandidateQuery {
            scope: scope.clone(),
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.created_at, initial.created_at);
    assert!(updated.updated_at.ends_with('Z'));
    assert!(updated.updated_at >= updated.created_at);
    assert_eq!(
        candidate.decided_at.as_deref(),
        Some(updated.updated_at.as_str())
    );
    assert_eq!(
        updated.target_memory_id.as_deref(),
        Some("candidate-detail-target")
    );

    let cross_tenant = MemoryCandidateStorePort::retrieve_detail(
        &runtime,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id + 1,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(cross_tenant.is_none());

    let duplicate = MemoryCandidateStorePort::create(
        &runtime,
        CreateMemoryCandidateCommand {
            scope: scope.clone(),
            candidate_id: "candidate-detail".to_string(),
            candidate_type: "observation".to_string(),
            memory_type: "semantic".to_string(),
            proposed_text: "must not overwrite the promoted candidate".to_string(),
            proposed_payload_json: None,
            evidence_json: None,
            confidence: 0.1,
        },
    )
    .await
    .expect_err("same-space duplicate candidate must be rejected");
    assert!(matches!(
        duplicate,
        MemorySpiError::IdempotencyConflict { ref idempotency_key }
            if idempotency_key == "candidate-detail"
    ));
    let after_duplicate = MemoryCandidateStorePort::retrieve_detail(
        &runtime,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(after_duplicate.decision_state, "approved");
    assert_eq!(
        after_duplicate.target_memory_id.as_deref(),
        Some("candidate-detail-target")
    );

    MemoryRecordStorePort::delete_canonical_atomic(
        &runtime,
        DeleteCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "candidate-detail-target".to_string(),
            journal: mutation_journal("candidate-detail-target", "deleted"),
        },
    )
    .await
    .unwrap();
    let after_delete = MemoryCandidateStorePort::retrieve_detail(
        &runtime,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(after_delete.target_memory_id, None);
    assert!(matches!(
        MemoryCandidateStorePort::promote_atomic_with_quota(
            &runtime,
            PromoteMemoryCandidateAtomicCommand {
                scope,
                candidate_id: "candidate-detail".to_string(),
                memory_id: "candidate-detail-target".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "Reference detail candidate".to_string(),
                evidence_links: Vec::new(),
                decided_by: Some(7),
            },
            10,
        )
        .await,
        Err(MemorySpiError::PortOperationFailed { ref port, ref message })
            if port == "MemoryCandidateStorePort" && message.contains("missing or deleted target")
    ));
}

#[tokio::test]
async fn reference_candidate_detail_fails_closed_when_id_is_ambiguous_across_spaces() {
    let runtime = ReferenceMemoryRuntime::new();
    for space_id in [301, 302] {
        MemoryCandidateStorePort::create(
            &runtime,
            CreateMemoryCandidateCommand {
                scope: MemoryScopeContext::for_test(31, space_id),
                candidate_id: "ambiguous-candidate".to_string(),
                candidate_type: "observation".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: format!("candidate in space {space_id}"),
                proposed_payload_json: None,
                evidence_json: None,
                confidence: 0.8,
            },
        )
        .await
        .unwrap();
    }

    let error = MemoryCandidateStorePort::retrieve_detail(
        &runtime,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: 31,
            candidate_id: "ambiguous-candidate".to_string(),
        },
    )
    .await
    .expect_err("ambiguous tenant candidate id must fail closed");
    assert!(matches!(
        error,
        MemorySpiError::PortOperationFailed { ref port, ref message }
            if port == "MemoryCandidateStorePort"
                && message.contains("ambiguous across tenant spaces")
    ));
}

#[tokio::test]
async fn reference_rich_retrieval_is_bounded_filtered_and_fail_closed() {
    let runtime = ReferenceMemoryRuntime::new();
    let primary = MemoryScopeContext::for_test(10, 100);
    let other_space = MemoryScopeContext::for_test(10, 101);
    let other_tenant = MemoryScopeContext::for_test(11, 100);

    for (scope, memory_id, memory_type, sensitivity) in [
        (primary.clone(), "999-allowed", "semantic", "internal"),
        (primary.clone(), "000-sensitive", "semantic", "sensitive"),
        (primary.clone(), "001-wrong-type", "episodic", "internal"),
        (other_space, "002-other-space", "semantic", "internal"),
        (other_tenant, "003-other-tenant", "semantic", "internal"),
    ] {
        MemoryRecordStorePort::create_canonical_atomic(
            &runtime,
            CreateCanonicalMemoryCommand {
                scope,
                memory_id: memory_id.to_string(),
                scope_label: "user".to_string(),
                memory_type: memory_type.to_string(),
                subject: Some("retrieval".to_string()),
                predicate: Some("matches".to_string()),
                object_text: "needle reference memory".to_string(),
                canonical_text: "needle reference memory".to_string(),
                sensitivity_level: sensitivity.to_string(),
                expires_at: None,
                journal: mutation_journal(memory_id, "created"),
                metadata_json: None,
            },
        )
        .await
        .unwrap();
    }

    let result = MemoryRetrieverPort::search_scoped(
        &runtime,
        SearchMemoryCandidatesQuery {
            scope: primary.clone(),
            query: "needle".to_string(),
            limit: 1,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: vec!["semantic".to_string()],
            read_scope: MemorySensitivityReadScope::Public,
            metadata_filter: None,
            include_expired: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].memory_id, "999-allowed");

    let degraded = MemoryRetrieverPort::search_scoped(
        &runtime,
        SearchMemoryCandidatesQuery {
            scope: primary.clone(),
            query: "needle".to_string(),
            limit: 10,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword, MemoryRetrieverKind::Event],
            memory_types: vec!["semantic".to_string()],
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: None,
            include_expired: false,
        },
    )
    .await
    .unwrap();
    assert!(degraded.degraded);
    assert_eq!(
        degraded.unavailable_retriever_kinds,
        vec![MemoryRetrieverKind::Event]
    );
    assert_eq!(
        degraded.degradation_codes,
        vec!["retriever_kind_unavailable"]
    );
    assert_eq!(degraded.records.len(), 2);

    MemoryRecordStorePort::delete_canonical_atomic(
        &runtime,
        DeleteCanonicalMemoryCommand {
            scope: primary.clone(),
            memory_id: "999-allowed".to_string(),
            journal: mutation_journal("999-allowed", "deleted"),
        },
    )
    .await
    .unwrap();
    let after_delete = MemoryRetrieverPort::search_scoped(
        &runtime,
        SearchMemoryCandidatesQuery {
            scope: primary.clone(),
            query: "needle".to_string(),
            limit: 10,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: vec!["semantic".to_string()],
            read_scope: MemorySensitivityReadScope::Public,
            metadata_filter: None,
            include_expired: false,
        },
    )
    .await
    .unwrap();
    assert!(after_delete.records.is_empty());

    for query in [
        SearchMemoryCandidatesQuery {
            scope: primary.clone(),
            query: "   ".to_string(),
            limit: 1,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: None,
            include_expired: false,
        },
        SearchMemoryCandidatesQuery {
            scope: primary.clone(),
            query: "needle".to_string(),
            limit: 0,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: None,
            include_expired: false,
        },
        SearchMemoryCandidatesQuery {
            scope: primary,
            query: "needle".to_string(),
            limit: MAX_MEMORY_RETRIEVAL_CANDIDATES + 1,
            retriever_kinds: Vec::new(),
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: None,
            include_expired: false,
        },
    ] {
        assert!(MemoryRetrieverPort::search_scoped(&runtime, query)
            .await
            .is_err());
    }
}

#[tokio::test]
async fn reference_supersede_atomic_chain_persists_dual_journals_and_retry_is_idempotent() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(1, 1);
    MemoryRecordStorePort::create_canonical_atomic(
        &runtime,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "supersede-old".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: Some("account".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: "old canonical value".to_string(),
            canonical_text: "User prefers the old canonical value".to_string(),
            sensitivity_level: "internal".to_string(),
            expires_at: None,
            journal: mutation_journal("supersede-old", "created"),
            metadata_json: None,
        },
    )
    .await
    .unwrap();

    let command = SupersedeCanonicalMemoryAtomicCommand {
        scope: scope.clone(),
        old_memory_id: "supersede-old".to_string(),
        new_memory_id: "supersede-new".to_string(),
        scope_label: "user".to_string(),
        memory_type: "semantic".to_string(),
        subject: Some("account".to_string()),
        predicate: Some("prefers".to_string()),
        object_text: "new canonical value".to_string(),
        canonical_text: "User prefers the new canonical value".to_string(),
        sensitivity_level: "internal".to_string(),
        expires_at: None,
        created_journal: mutation_journal("supersede-new", "supersede-created"),
        superseded_journal: mutation_journal("supersede-old", "supersede-superseded"),
        metadata_json: None,
    };

    let first =
        MemoryRecordStorePort::supersede_canonical_atomic_with_quota(&runtime, command.clone(), 2)
            .await
            .unwrap();
    let admitted = match first {
        MemoryRecordQuotaAdmission::Admitted(record) => record,
        MemoryRecordQuotaAdmission::QuotaExceeded { .. } => {
            panic!("supersede unexpectedly rejected with available capacity")
        }
    };
    assert_eq!(admitted.memory_id, "supersede-new");
    assert_eq!(
        admitted.supersedes_memory_id.as_deref(),
        Some("supersede-old")
    );
    assert_eq!(admitted.status, "active");

    let old = MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-old".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseded source remains readable for lifecycle inspection");
    assert_eq!(old.status, "superseded");
    assert_eq!(
        old.superseded_by_memory_id.as_deref(),
        Some("supersede-new")
    );

    let new = MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-new".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseding target must be readable");
    assert_eq!(new.status, "active");
    assert_eq!(new.supersedes_memory_id.as_deref(), Some("supersede-old"));
    assert_eq!(new.superseded_by_memory_id, None);

    let superseded_outbox = MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-old-supersede-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseded journal outbox must commit with the chain");
    assert_eq!(superseded_outbox.aggregate_id, "supersede-old");
    let superseded_audit = MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-old-supersede-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseded journal audit must commit with the chain");
    assert_eq!(superseded_audit.resource_id, "supersede-old");

    let created_outbox = MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-new-supersede-created".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("created journal outbox must commit with the chain");
    assert_eq!(created_outbox.aggregate_id, "supersede-new");
    let created_audit = MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-new-supersede-created".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("created journal audit must commit with the chain");
    assert_eq!(created_audit.resource_id, "supersede-new");

    let retry = MemoryRecordStorePort::supersede_canonical_atomic_with_quota(&runtime, command, 2)
        .await
        .unwrap();
    assert_eq!(retry, MemoryRecordQuotaAdmission::Admitted(admitted));

    let superseded_outbox_retry = MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-old-supersede-superseded".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(superseded_outbox_retry.is_some());
    let created_audit_retry = MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope,
            audit_id: "audit-supersede-new-supersede-created".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(created_audit_retry.is_some());

    let changed_payload = SupersedeCanonicalMemoryAtomicCommand {
        scope: MemoryScopeContext::for_test(1, 1),
        old_memory_id: "supersede-old".to_string(),
        new_memory_id: "supersede-new".to_string(),
        scope_label: "user".to_string(),
        memory_type: "semantic".to_string(),
        subject: Some("account".to_string()),
        predicate: Some("prefers".to_string()),
        object_text: "new canonical value".to_string(),
        canonical_text: "different retry payload".to_string(),
        sensitivity_level: "internal".to_string(),
        expires_at: None,
        created_journal: mutation_journal("supersede-new", "supersede-created"),
        superseded_journal: mutation_journal("supersede-old", "supersede-superseded"),
        metadata_json: None,
    };
    assert!(matches!(
        MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
            &runtime,
            changed_payload,
            2,
        )
        .await,
        Err(MemorySpiError::IdempotencyConflict { ref idempotency_key })
            if idempotency_key == "supersede-new"
    ));

    let changed_journal = SupersedeCanonicalMemoryAtomicCommand {
        scope: MemoryScopeContext::for_test(1, 1),
        old_memory_id: "supersede-old".to_string(),
        new_memory_id: "supersede-new".to_string(),
        scope_label: "user".to_string(),
        memory_type: "semantic".to_string(),
        subject: Some("account".to_string()),
        predicate: Some("prefers".to_string()),
        object_text: "new canonical value".to_string(),
        canonical_text: "User prefers the new canonical value".to_string(),
        sensitivity_level: "internal".to_string(),
        expires_at: None,
        created_journal: mutation_journal("supersede-new", "different-journal"),
        superseded_journal: mutation_journal("supersede-old", "supersede-superseded"),
        metadata_json: None,
    };
    assert!(matches!(
        MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
            &runtime,
            changed_journal,
            2,
        )
        .await,
        Err(MemorySpiError::IdempotencyConflict { ref idempotency_key })
            if idempotency_key == "supersede-new"
    ));

    let unchanged = MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: MemoryScopeContext::for_test(1, 1),
            memory_id: "supersede-new".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        unchanged.canonical_text,
        "User prefers the new canonical value"
    );
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: MemoryScopeContext::for_test(1, 1),
            outbox_id: "outbox-supersede-new-different-journal".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: MemoryScopeContext::for_test(1, 1),
            audit_id: "audit-supersede-new-different-journal".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn reference_supersede_quota_rejection_keeps_chain_and_journals_unchanged() {
    let runtime = ReferenceMemoryRuntime::new();
    let scope = MemoryScopeContext::for_test(1, 1);
    for (memory_id, text) in [
        ("supersede-quota-old", "old value"),
        ("supersede-quota-blocker", "blocking value"),
    ] {
        MemoryRecordStorePort::create_canonical_atomic(
            &runtime,
            CreateCanonicalMemoryCommand {
                scope: scope.clone(),
                memory_id: memory_id.to_string(),
                scope_label: "user".to_string(),
                memory_type: "semantic".to_string(),
                subject: Some("account".to_string()),
                predicate: Some("prefers".to_string()),
                object_text: text.to_string(),
                canonical_text: text.to_string(),
                sensitivity_level: "internal".to_string(),
                expires_at: None,
                journal: mutation_journal(memory_id, "created"),
                metadata_json: None,
            },
        )
        .await
        .unwrap();
    }

    let admission = MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
        &runtime,
        SupersedeCanonicalMemoryAtomicCommand {
            scope: scope.clone(),
            old_memory_id: "supersede-quota-old".to_string(),
            new_memory_id: "supersede-quota-new".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: Some("account".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: "must not be written".to_string(),
            canonical_text: "must not be written".to_string(),
            sensitivity_level: "internal".to_string(),
            expires_at: None,
            created_journal: mutation_journal("supersede-quota-new", "quota-created"),
            superseded_journal: mutation_journal("supersede-quota-old", "quota-superseded"),
            metadata_json: None,
        },
        2,
    )
    .await
    .unwrap();
    assert_eq!(
        admission,
        MemoryRecordQuotaAdmission::QuotaExceeded {
            active_records: 2,
            max_active_records: 2,
        }
    );

    let old = MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-quota-old".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("quota rejection must preserve source record");
    assert_eq!(old.status, "active");
    assert_eq!(old.superseded_by_memory_id, None);
    let blocker = MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-quota-blocker".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("quota rejection must preserve the record occupying the final slot");
    assert_eq!(blocker.status, "active");
    assert_eq!(blocker.superseded_by_memory_id, None);
    assert!(MemoryRecordStorePort::retrieve_canonical(
        &runtime,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-quota-new".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-quota-new-quota-created".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryOutboxStorePort::retrieve(
        &runtime,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-quota-old-quota-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-quota-new-quota-created".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &runtime,
        RetrieveMemoryAuditQuery {
            scope,
            audit_id: "audit-supersede-quota-old-quota-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}
