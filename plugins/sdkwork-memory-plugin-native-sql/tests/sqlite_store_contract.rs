// WORKSPACE-PATH:allow-fixture: fixtures name a foreign checkout root, drive, or home directory to exercise path handling, so the literal is the value under assertion rather than a binding this build resolves
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};
use sdkwork_memory_plugin_native_sql::{
    StaleRequeueCounts,
    build_native_sql_candidate_store, build_native_sql_habit_store,
    build_native_sql_retrieval_trace_store, ConsolidateDuplicateRecordsCommand,
    FinishLearningJobCommand, InsertEdgeCommand, InsertEntityCommand, InsertLearningJobCommand,
    InsertMemoryEvalRunCommand, InsertPolicyCommand, InsertSubjectCommand,
    NativeSqlAppendOutboxEventCommand, NativeSqlCreateSpaceCommand, NativeSqlMemoryStore,
    NativeSqlStoreError, NativeSqlUsageDailyRow, PromoteApprovedCandidateCommand,
    UpdateEntityCommand, UpdateEvalRunStateCommand, MAX_ENTITY_LINK_RESULTS,
    SENSITIVITY_READ_OWNER,
};
use sdkwork_memory_spi::{
    AppendMemoryAuditCommand, AppendMemoryEventCommand, AppendMemoryOutboxCommand,
    AppendMemoryRetrievalTraceCommand, ApproveMemoryCandidateCommand, CreateCanonicalMemoryCommand,
    CreateMemoryCandidateCommand, CreateMemoryRecordCommand, CreateMemorySpaceCommand,
    DecayMemoryHabitCommand, DeleteAllCanonicalMemoryCommand, DeleteCanonicalMemoryCommand,
    DeleteMemoryRecordCommand, ListMemoryRetrievalTracesQuery, ListPendingMemoryOutboxQuery,
    MarkMemoryOutboxFailedCommand, MarkMemoryOutboxPublishedCommand, MemoryAuditStorePort,
    MemoryCandidateStorePort, MemoryContextPackSnapshot, MemoryEventStorePort,
    MemoryHabitStorePort, MemoryMutationJournal, MemoryOutboxStorePort, MemoryRecordQuotaAdmission,
    MemoryRecordStorePort, MemoryRetrievalHitDraft, MemoryRetrievalTraceStorePort,
    MemoryRetrieverKind, MemoryRetrieverPort, MemoryScopeContext, MemorySensitivityReadScope,
    MemorySpaceQuotaAdmission, MemorySpaceStorePort, MemorySpiError,
    PromoteMemoryCandidateAtomicWithJournalCommand, PromoteMemoryHabitCommand,
    RejectMemoryCandidateCommand, RetrieveCanonicalMemoryQuery, RetrieveMemoryAuditQuery,
    RetrieveMemoryCandidateDetailQuery, RetrieveMemoryCandidateQuery, RetrieveMemoryEventQuery,
    RetrieveMemoryHabitQuery, RetrieveMemoryOutboxQuery, RetrieveMemoryRecordQuery,
    RetrieveMemoryRetrievalTraceQuery, SearchMemoryCandidatesQuery,
    SupersedeCanonicalMemoryAtomicCommand, UpdateCanonicalMemoryCommand, UpsertMemoryHabitCommand,
    MAX_MEMORY_RETRIEVAL_CANDIDATES,
};
use tokio::sync::Barrier;

fn mutation_journal(memory_id: &str, suffix: &str) -> MemoryMutationJournal {
    MemoryMutationJournal {
        outbox_id: format!("outbox-{suffix}"),
        aggregate_type: "memory_record".to_string(),
        aggregate_id: memory_id.to_string(),
        event_type: format!("memory.record.{suffix}"),
        event_version: "1.0".to_string(),
        payload_json: format!(r#"{{"memoryId":"{memory_id}"}}"#),
        audit_id: format!("audit-{suffix}"),
        audit_action: format!("memory.record.{suffix}"),
        audit_resource_type: "memory_record".to_string(),
        audit_resource_id: memory_id.to_string(),
        audit_result: "accepted".to_string(),
    }
}

#[tokio::test]
async fn sqlite_canonical_schema_readiness_rejects_unmigrated_database() {
    let config = DatabaseConfig {
        engine: DatabaseEngine::Sqlite,
        url: "sqlite::memory:".to_owned(),
        ..DatabaseConfig::default()
    };
    let store = NativeSqlMemoryStore::open_pool(&config, false)
        .await
        .expect("connect to unmigrated SQLite database");

    assert!(store.verify_canonical_schema().await.is_err());
}

#[tokio::test]
async fn sqlite_canonical_schema_readiness_accepts_current_database() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("create migrated SQLite database");

    store
        .verify_canonical_schema()
        .await
        .expect("current canonical schema must be ready");
}

#[tokio::test]
async fn sqlite_provider_health_scan_uses_stable_bounded_keyset_pages() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("create SQLite provider-health store");

    for tenant_id in [10_i64, 20, 30, 40] {
        store
            .insert_mem_provider_binding(
                tenant_id,
                &format!("binding-{tenant_id}-1"),
                "embedding",
                "test",
                "Test provider",
                "[]",
                None,
                None,
                None,
                None,
                "unknown",
                None,
            )
            .await
            .expect("insert provider binding");
    }

    let first_tenant_page = store
        .list_tenant_ids_with_provider_bindings_page(None, 2)
        .await
        .expect("first tenant page");
    assert_eq!(first_tenant_page, vec![10, 20, 30]);
    let second_tenant_page = store
        .list_tenant_ids_with_provider_bindings_page(Some(first_tenant_page[1]), 2)
        .await
        .expect("second tenant page");
    assert_eq!(second_tenant_page, vec![30, 40]);

    for suffix in [2, 3] {
        store
            .insert_mem_provider_binding(
                10,
                &format!("binding-10-{suffix}"),
                "embedding",
                &format!("test-{suffix}"),
                "Test provider",
                "[]",
                None,
                None,
                None,
                None,
                "unknown",
                None,
            )
            .await
            .expect("insert paged provider binding");
    }
    let first_binding_page = store
        .list_mem_provider_bindings_for_tenant(10, 2, None)
        .await
        .expect("first provider binding page");
    assert_eq!(first_binding_page.len(), 3);
    let second_binding_page = store
        .list_mem_provider_bindings_for_tenant(10, 2, Some(&first_binding_page[1].binding_uuid))
        .await
        .expect("second provider binding page");
    assert_eq!(second_binding_page.len(), 1);
    assert_eq!(second_binding_page[0].binding_uuid, "binding-10-3");
}

#[tokio::test]
async fn sqlite_graph_mutation_rolls_back_when_atomic_journal_fails() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("create SQLite graph journal store");
    store
        .create_space_record(
            1,
            7,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "tenant".to_string(),
                owner_subject_id: "1".to_string(),
                space_type: "workspace".to_string(),
                display_name: "Graph journal space".to_string(),
                default_scope: "tenant".to_string(),
            },
        )
        .await
        .expect("create graph journal space");
    let scope = MemoryScopeContext {
        tenant_id: 1,
        space_id: 7,
        organization_id: None,
        user_id: Some(9),
    };
    let created = MemoryMutationJournal {
        outbox_id: "graph-outbox-created".to_string(),
        aggregate_type: "ai_entity".to_string(),
        aggregate_id: "graph-entity-1".to_string(),
        event_type: "memory.entity.created".to_string(),
        event_version: "1".to_string(),
        payload_json: r#"{"resourceId":"graph-entity-1"}"#.to_string(),
        audit_id: "graph-audit-created".to_string(),
        audit_action: "memory.entity.created".to_string(),
        audit_resource_type: "entity".to_string(),
        audit_resource_id: "graph-entity-1".to_string(),
        audit_result: "accepted".to_string(),
    };
    store
        .insert_entity_with_journal(
            InsertEntityCommand {
                id: 701,
                uuid: "graph-entity-1",
                tenant_id: 1,
                space_id: 7,
                entity_type: "person",
                canonical_name: "Before",
                aliases_json: None,
                attributes_json: None,
                sensitivity_level: "internal",
            },
            &scope,
            &created,
        )
        .await
        .expect("journaled entity insert");

    let conflicting_update = MemoryMutationJournal {
        outbox_id: created.outbox_id.clone(),
        aggregate_type: "ai_entity".to_string(),
        aggregate_id: "graph-entity-1".to_string(),
        event_type: "memory.entity.updated".to_string(),
        event_version: "1".to_string(),
        payload_json: r#"{"resourceId":"graph-entity-1"}"#.to_string(),
        audit_id: "graph-audit-update".to_string(),
        audit_action: "memory.entity.updated".to_string(),
        audit_resource_type: "entity".to_string(),
        audit_resource_id: "graph-entity-1".to_string(),
        audit_result: "accepted".to_string(),
    };
    assert!(store
        .update_entity_with_journal(
            1,
            "graph-entity-1",
            UpdateEntityCommand {
                canonical_name: Some("After"),
                aliases_json: None,
                attributes_json: None,
                sensitivity_level: None,
                status: None,
            },
            &scope,
            &conflicting_update,
        )
        .await
        .is_err());

    let entity = store
        .retrieve_entity(1, "graph-entity-1")
        .await
        .expect("retrieve graph entity")
        .expect("graph entity remains after rollback");
    assert_eq!(entity.canonical_name, "Before");
    assert_eq!(entity.version, 1);
    let outbox_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_outbox_event WHERE aggregate_id = ?")
            .bind("graph-entity-1")
            .fetch_one(store.pool())
            .await
            .expect("count graph outbox");
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_audit_log WHERE resource_id = ?")
            .bind("graph-entity-1")
            .fetch_one(store.pool())
            .await
            .expect("count graph audit");
    assert_eq!(outbox_count, 1);
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn sqlite_job_history_uses_store_level_cursor_and_tenant_filters() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    for (tenant_id, space_id) in [(77, 10), (77, 20), (88, 30)] {
        let admitted = MemorySpaceStorePort::create_space_atomic_with_quota(
            &store,
            CreateMemorySpaceCommand {
                tenant_id,
                space_id,
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: format!("owner-{tenant_id}-{space_id}"),
                space_type: "personal".to_string(),
                display_name: format!("Space {space_id}"),
                default_scope: "user".to_string(),
            },
            10,
        )
        .await
        .unwrap();
        assert!(matches!(admitted, MemorySpaceQuotaAdmission::Admitted(_)));
    }
    for (job_uuid, tenant_id, job_type, space_id) in [
        ("101", 77, "extraction", Some(10)),
        ("102", 77, "extraction", Some(10)),
        ("103", 77, "extraction", Some(20)),
        ("104", 77, "consolidation", Some(10)),
        ("105", 88, "extraction", Some(30)),
    ] {
        store
            .insert_learning_job(InsertLearningJobCommand {
                tenant_id,
                job_uuid,
                space_id,
                job_type,
                state: "queued",
                priority: 0,
                idempotency_key: None,
                input_json: None,
            })
            .await
            .unwrap();
    }

    let first = store
        .list_learning_jobs_for_tenant(77, "extraction", None, 2, None)
        .await
        .unwrap();
    assert_eq!(
        first
            .iter()
            .map(|row| row.job_uuid.as_str())
            .collect::<Vec<_>>(),
        vec!["103", "102", "101"]
    );

    let second = store
        .list_learning_jobs_for_tenant(77, "extraction", None, 2, Some("102"))
        .await
        .unwrap();
    assert_eq!(
        second
            .iter()
            .map(|row| row.job_uuid.as_str())
            .collect::<Vec<_>>(),
        vec!["101"]
    );

    let scoped = store
        .list_learning_jobs_for_tenant(77, "extraction", Some(10), 20, None)
        .await
        .unwrap();
    assert_eq!(
        scoped
            .iter()
            .map(|row| row.job_uuid.as_str())
            .collect::<Vec<_>>(),
        vec!["102", "101"]
    );
}

#[tokio::test]
async fn sqlite_privacy_export_rejects_payloads_over_the_byte_budget() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .create_record(
            &scope,
            "export-byte-limit-record",
            "export",
            &"sensitive export content ".repeat(32),
        )
        .await
        .unwrap();

    let error = store
        .collect_export_payload_for_spaces(1, &[1], false, SENSITIVITY_READ_OWNER, 128)
        .await
        .expect_err("export must reject content above its byte budget");
    assert!(error.to_string().contains("byte limit exceeded"));
}

#[tokio::test]
async fn sqlite_learning_job_completion_is_fenced_by_execution_lease() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    store
        .insert_learning_job(InsertLearningJobCommand {
            tenant_id: 77,
            job_uuid: "lease-job-1",
            space_id: None,
            job_type: "retention",
            state: "queued",
            priority: 0,
            idempotency_key: None,
            input_json: Some(r#"{"dryRun":true}"#),
        })
        .await
        .unwrap();

    let first = store
        .claim_queued_learning_jobs(1, "job-worker-a", "job-lease-a", 30)
        .await
        .unwrap();
    assert_eq!(first.len(), 1);
    assert!(store
        .finish_learning_job(FinishLearningJobCommand {
            tenant_id: 77,
            job_uuid: "lease-job-1",
            lease_owner: "job-worker-a",
            lease_token: "wrong-token",
            state: "succeeded",
            result_json: Some(r#"{"status":"wrong"}"#),
            error_json: None,
        })
        .await
        .unwrap()
        .is_none());
    assert!(store
        .renew_learning_job_lease(77, "lease-job-1", "job-worker-a", "job-lease-a", 30)
        .await
        .unwrap());

    sqlx::query(
        "UPDATE ai_learning_job SET lease_expires_at = '1970-01-01T00:00:00.000Z' WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(77_i64)
    .bind("lease-job-1")
    .execute(store.pool())
    .await
    .unwrap();
    assert_eq!(
        store.requeue_stale_running_learning_jobs(30).await.unwrap(),
        StaleRequeueCounts { requeued: 1, dead: 0 }
    );
    let replacement = store
        .claim_queued_learning_jobs(1, "job-worker-b", "job-lease-b", 30)
        .await
        .unwrap();
    assert_eq!(replacement.len(), 1);
    assert!(store
        .finish_learning_job(FinishLearningJobCommand {
            tenant_id: 77,
            job_uuid: "lease-job-1",
            lease_owner: "job-worker-a",
            lease_token: "job-lease-a",
            state: "succeeded",
            result_json: Some(r#"{"status":"stale"}"#),
            error_json: None,
        })
        .await
        .unwrap()
        .is_none());
    let completed = store
        .finish_learning_job(FinishLearningJobCommand {
            tenant_id: 77,
            job_uuid: "lease-job-1",
            lease_owner: "job-worker-b",
            lease_token: "job-lease-b",
            state: "succeeded",
            result_json: Some(r#"{"status":"current"}"#),
            error_json: None,
        })
        .await
        .unwrap()
        .expect("current lease must complete learning job");
    assert_eq!(completed.state, "succeeded");
}

#[tokio::test]
async fn sqlite_learning_job_idempotency_key_replay_resolves_to_the_stored_job() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    let command = |job_uuid: &'static str| InsertLearningJobCommand {
        tenant_id: 77,
        job_uuid,
        space_id: None,
        job_type: "extraction",
        state: "queued",
        priority: 0,
        idempotency_key: Some("http-idempotency-key-1"),
        input_json: Some(r#"{"spaceId":"10"}"#),
    };
    store.insert_learning_job(command("keyed-job-1")).await.unwrap();
    // A concurrent replica enqueued the same logical job first: the partial
    // unique index fires, and the conflict recovery must resolve to the
    // stored job instead of surfacing a constraint error.
    store.insert_learning_job(command("keyed-job-replay")).await.unwrap();

    let stored = store
        .retrieve_learning_job_for_tenant(77, "keyed-job-1")
        .await
        .unwrap()
        .expect("the first job row must win the replay");
    assert_eq!(stored.job_type, "extraction");
    assert!(store
        .retrieve_learning_job_for_tenant(77, "keyed-job-replay")
        .await
        .unwrap()
        .is_none());
    let keyed_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_learning_job WHERE tenant_id = ?")
            .bind(77_i64)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(keyed_rows, 1);
}

#[tokio::test]
async fn sqlite_learning_job_idempotency_key_reuse_with_different_payload_conflicts() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    store
        .insert_learning_job(InsertLearningJobCommand {
            tenant_id: 77,
            job_uuid: "keyed-job-1",
            space_id: None,
            job_type: "extraction",
            state: "queued",
            priority: 0,
            idempotency_key: Some("http-idempotency-key-1"),
            input_json: Some(r#"{"spaceId":"10"}"#),
        })
        .await
        .unwrap();

    // One idempotency key maps to one logical request: reusing it for a
    // different payload is a client contract violation, not a replay.
    let error = store
        .insert_learning_job(InsertLearningJobCommand {
            tenant_id: 77,
            job_uuid: "keyed-job-2",
            space_id: None,
            job_type: "extraction",
            state: "queued",
            priority: 0,
            idempotency_key: Some("http-idempotency-key-1"),
            input_json: Some(r#"{"spaceId":"20"}"#),
        })
        .await
        .expect_err("key reuse with a different payload must conflict");
    assert!(matches!(
        error,
        NativeSqlStoreError::IdempotencyConflict { ref idempotency_key }
            if idempotency_key == "http-idempotency-key-1"
    ));
    assert!(store
        .retrieve_learning_job_for_tenant(77, "keyed-job-2")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_governance_job_history_filters_actor_before_pagination() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    let scope = MemoryScopeContext {
        tenant_id: 77,
        space_id: 1,
        organization_id: None,
        user_id: Some(501),
    };
    for (job_id, resource_type, actor_id) in [
        ("201", "forget_job", Some("501")),
        ("202", "forget_job", Some("999")),
        ("203", "export_job", Some("501")),
        ("204", "forget_job", Some("501")),
    ] {
        store
            .append_audit_with_metadata(
                &scope,
                job_id,
                "job.create",
                resource_type,
                job_id,
                "accepted",
                r#"{"state":"succeeded"}"#,
                actor_id,
            )
            .await
            .unwrap();
    }

    let first = store
        .list_governance_jobs_for_tenant(77, "forget_job", Some("501"), 1, None)
        .await
        .unwrap();
    assert_eq!(
        first
            .iter()
            .map(|row| row.job_id.as_str())
            .collect::<Vec<_>>(),
        vec!["204", "201"]
    );

    let second = store
        .list_governance_jobs_for_tenant(77, "forget_job", Some("501"), 1, Some("204"))
        .await
        .unwrap();
    assert_eq!(
        second
            .iter()
            .map(|row| row.job_id.as_str())
            .collect::<Vec<_>>(),
        vec!["201"]
    );
}

#[tokio::test]
async fn sqlite_default_implementation_profile_matches_the_live_store() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    store
        .ensure_default_implementation_profile_for_tenant(77)
        .await
        .unwrap();
    let profile = store
        .retrieve_mem_implementation_profile_for_tenant(77, "1")
        .await
        .unwrap()
        .expect("default implementation profile must exist");

    assert_eq!(profile.name, "local-embedded-phase1");
    assert_eq!(profile.implementation_kind, "local_embedded");
    assert_eq!(profile.role, "primary");
    assert!(profile.capability_json.contains("productionQualified"));
}

#[tokio::test]
async fn sqlite_eval_run_persists_dataset_profile_config_and_lifecycle_timestamps() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    store
        .insert_mem_eval_run_request(InsertMemoryEvalRunCommand {
            tenant_id: 77,
            eval_run_uuid: "501",
            eval_type: "retrieval_quality",
            state: "accepted",
            dataset_ref: Some("golden-v1"),
            profile_ref: Some("42"),
            config_json: Some(
                r#"{"cases":[{"spaceId":"1","query":"q","expectedMemoryIds":["9"]}]}"#,
            ),
        })
        .await
        .unwrap();

    let claimed = store
        .claim_queued_eval_runs(1, "eval-worker", "eval-lease", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].tenant_id, 77);
    assert_eq!(claimed[0].eval_run_uuid, "501");
    assert_eq!(claimed[0].eval_type, "retrieval_quality");
    let running = store
        .retrieve_mem_eval_run_for_tenant(77, "501")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(running.dataset_ref.as_deref(), Some("golden-v1"));
    assert_eq!(running.profile_ref.as_deref(), Some("42"));
    assert!(running
        .result_json
        .as_deref()
        .is_some_and(|value| value.contains("expectedMemoryIds")));
    assert_utc_timestamp(running.started_at.as_deref());
    assert!(running.finished_at.is_none());

    store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id: 77,
            eval_run_uuid: "501",
            lease_owner: "eval-worker",
            lease_token: "eval-lease",
            state: "succeeded",
            metrics_json: Some(r#"{"recallAtK":1.0}"#),
            result_json: Some(r#"{"status":"completed"}"#),
        })
        .await
        .unwrap();
    let completed = store
        .retrieve_mem_eval_run_for_tenant(77, "501")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        completed.metrics_json.as_deref(),
        Some(r#"{"recallAtK":1.0}"#)
    );
    assert_eq!(
        completed.result_json.as_deref(),
        Some(r#"{"status":"completed"}"#)
    );
    assert_utc_timestamp(completed.finished_at.as_deref());
}

/// G2 contract: `requeue_failed_learning_job` requeues below the attempt
/// ceiling (with backoff and a version charge), dead-letters at the ceiling,
/// and rejects a fenced lease without writing anything.
#[tokio::test]
async fn sqlite_requeue_failed_learning_job_covers_fence_backoff_and_dead_letter() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    store
        .insert_learning_job(InsertLearningJobCommand {
            tenant_id: 77,
            job_uuid: "requeue-job-1",
            space_id: None,
            job_type: "extract",
            state: "queued",
            priority: 0,
            idempotency_key: None,
            input_json: Some(r#"{"spaceId":"1"}"#),
        })
        .await
        .unwrap();
    let claimed = store
        .claim_queued_learning_jobs(1, "job-worker-a", "job-lease-a", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);

    // Fenced lease: a wrong token is rejected with None and writes nothing.
    let version_before = store
        .retrieve_learning_job_for_tenant(77, "requeue-job-1")
        .await
        .unwrap()
        .unwrap()
        .version;
    let fenced = store
        .requeue_failed_learning_job(
            77,
            "requeue-job-1",
            "job-worker-a",
            "wrong-token",
            2,
            30,
            r#"{"error":"boom"}"#,
        )
        .await
        .unwrap();
    assert!(fenced.is_none(), "a wrong lease token must be rejected");
    let untouched = store
        .retrieve_learning_job_for_tenant(77, "requeue-job-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.state, "running");
    assert_eq!(untouched.version, version_before, "a fenced requeue must not charge");

    // Below the ceiling: back to 'queued' with backoff and version + 1.
    let requeued = store
        .requeue_failed_learning_job(
            77,
            "requeue-job-1",
            "job-worker-a",
            "job-lease-a",
            2,
            30,
            r#"{"error":"boom"}"#,
        )
        .await
        .unwrap()
        .expect("below-ceiling failure requeues the job");
    assert_eq!(requeued, "queued");
    let row = store
        .retrieve_learning_job_for_tenant(77, "requeue-job-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.version,
        version_before + 1,
        "a requeue must charge version + 1 like the terminal finish does"
    );
    let next_attempt_at: Option<String> =
        sqlx::query_scalar("SELECT next_attempt_at FROM ai_learning_job WHERE tenant_id = ? AND uuid = ?")
            .bind(77_i64)
            .bind("requeue-job-1")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(
        next_attempt_at.is_some(),
        "a below-ceiling requeue must schedule a backoff window"
    );

    // Fast-forward the backoff, re-claim, then exhaust the ceiling.
    sqlx::query(
        "UPDATE ai_learning_job SET next_attempt_at = '1970-01-01T00:00:00.000Z' WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(77_i64)
    .bind("requeue-job-1")
    .execute(store.pool())
    .await
    .unwrap();
    let reclaimed = store
        .claim_queued_learning_jobs(1, "job-worker-b", "job-lease-b", 30)
        .await
        .unwrap();
    assert_eq!(reclaimed.len(), 1);
    let dead = store
        .requeue_failed_learning_job(
            77,
            "requeue-job-1",
            "job-worker-b",
            "job-lease-b",
            2,
            30,
            r#"{"error":"boom again"}"#,
        )
        .await
        .unwrap()
        .expect("ceiling-exhausting failure dead-letters the job");
    assert_eq!(dead, "failed");
    let terminal = store
        .retrieve_learning_job_for_tenant(77, "requeue-job-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(terminal.state, "failed");
    let next_attempt_after_dead: Option<String> =
        sqlx::query_scalar("SELECT next_attempt_at FROM ai_learning_job WHERE tenant_id = ? AND uuid = ?")
            .bind(77_i64)
            .bind("requeue-job-1")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert!(next_attempt_after_dead.is_none());
}

/// G2 contract (B8): `update_eval_run_state` charges `version + 1` on the
/// terminal transition, matching `finish_learning_job`, and stays fenced.
#[tokio::test]
async fn sqlite_update_eval_run_state_bumps_version_and_stays_fenced() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite().await.unwrap();
    store
        .insert_mem_eval_run_request(InsertMemoryEvalRunCommand {
            tenant_id: 77,
            eval_run_uuid: "601",
            eval_type: "retrieval_quality",
            state: "accepted",
            dataset_ref: None,
            profile_ref: None,
            config_json: None,
        })
        .await
        .unwrap();
    let claimed = store
        .claim_queued_eval_runs(1, "eval-worker", "eval-lease", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);

    let wrong_token = store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id: 77,
            eval_run_uuid: "601",
            lease_owner: "eval-worker",
            lease_token: "wrong-token",
            state: "succeeded",
            metrics_json: None,
            result_json: Some(r#"{"status":"fenced"}"#),
        })
        .await
        .unwrap();
    assert!(!wrong_token, "a wrong lease token must be rejected");

    let updated = store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id: 77,
            eval_run_uuid: "601",
            lease_owner: "eval-worker",
            lease_token: "eval-lease",
            state: "succeeded",
            metrics_json: Some(r#"{"recallAtK":1.0}"#),
            result_json: Some(r#"{"status":"completed"}"#),
        })
        .await
        .unwrap();
    assert!(updated);
    let version: i64 = sqlx::query_scalar("SELECT version FROM ai_eval_run WHERE tenant_id = ? AND uuid = ?")
        .bind(77_i64)
        .bind("601")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(version, 1, "the terminal transition must charge version + 1");
}

/// C5: the extraction-to-candidate linkage persists so a candidate stays
/// traceable to the learning job that produced it.
#[tokio::test]
async fn sqlite_candidate_persists_learning_job_linkage() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryCandidateStorePort::create(
        &store,
        CreateMemoryCandidateCommand {
            learning_job_uuid: Some("9001".to_string()),
            ..candidate_command(scope.clone(), "cand-linked")
        },
    )
    .await
    .unwrap();
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "cand-unlinked"),
    )
    .await
    .unwrap();

    let linked: Option<String> = sqlx::query_scalar(
        "SELECT learning_job_uuid FROM ai_candidate WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(1_i64)
    .bind("cand-linked")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(linked.as_deref(), Some("9001"));

    let unlinked: Option<String> = sqlx::query_scalar(
        "SELECT learning_job_uuid FROM ai_candidate WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(1_i64)
    .bind("cand-unlinked")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(unlinked.is_none());
}

/// B11 regression: a cursor whose row was purged ends the window with an
/// empty page (hasMore=false downstream) instead of restarting the list, on
/// both cursor directions.
#[tokio::test]
async fn sqlite_purged_cursor_rows_end_list_windows_fail_closed() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    // DESC keyset list: retrieval traces newest-first.
    for trace_id in ["trace-cursor-1", "trace-cursor-2", "trace-cursor-3"] {
        store
            .append_retrieval_trace(&AppendMemoryRetrievalTraceCommand {
                scope: scope.clone(),
                trace_id: trace_id.to_string(),
                actor_id: None,
                query_text: Some("cursor probe".to_string()),
                query_hash: format!("sha256:{trace_id}"),
                retrievers_json: None,
                latency_ms: Some(1),
                degraded: false,
                metadata_json: None,
                hits: Vec::new(),
                context_pack: None,
            })
            .await
            .unwrap();
    }
    let first_page = store
        .list_retrieval_traces_for_tenant(1, Some(1), 2, None)
        .await
        .unwrap();
    // The store returns page_size + 1 rows so callers can detect has_more.
    assert_eq!(first_page.len(), 3);
    let cursor_trace = first_page[1].trace_id.clone();
    // Purge the cursor row out from under the paginator.
    sqlx::query("DELETE FROM ai_retrieval_trace WHERE tenant_id = ? AND uuid = ?")
        .bind(1_i64)
        .bind(&cursor_trace)
        .execute(store.pool())
        .await
        .unwrap();
    let purged_cursor_page = store
        .list_retrieval_traces_for_tenant(1, Some(1), 2, Some(&cursor_trace))
        .await
        .unwrap();
    assert!(
        purged_cursor_page.is_empty(),
        "a purged DESC cursor row must end the window (empty page) instead of restarting the list"
    );
    // The first page (no cursor) is unchanged by the fail-closed rule.
    let fresh_first_page = store
        .list_retrieval_traces_for_tenant(1, Some(1), 2, None)
        .await
        .unwrap();
    assert_eq!(fresh_first_page.len(), 2);

    // ASC keyset list: eval runs oldest-first.
    for eval_run_uuid in ["eval-cursor-1", "eval-cursor-2", "eval-cursor-3"] {
        store
            .insert_mem_eval_run_request(InsertMemoryEvalRunCommand {
                tenant_id: 1,
                eval_run_uuid,
                eval_type: "retrieval_quality",
                state: "accepted",
                dataset_ref: None,
                profile_ref: None,
                config_json: None,
            })
            .await
            .unwrap();
    }
    let first_eval_page = store
        .list_mem_eval_runs_for_tenant(1, 2, None)
        .await
        .unwrap();
    assert_eq!(first_eval_page.len(), 3);
    let eval_cursor = first_eval_page[1].eval_run_uuid.clone();
    sqlx::query("DELETE FROM ai_eval_run WHERE tenant_id = ? AND uuid = ?")
        .bind(1_i64)
        .bind(&eval_cursor)
        .execute(store.pool())
        .await
        .unwrap();
    let purged_eval_page = store
        .list_mem_eval_runs_for_tenant(1, 2, Some(&eval_cursor))
        .await
        .unwrap();
    assert!(
        purged_eval_page.is_empty(),
        "a purged ASC cursor row must end the window (empty page) instead of restarting the list"
    );
    let fresh_eval_first_page = store
        .list_mem_eval_runs_for_tenant(1, 2, None)
        .await
        .unwrap();
    assert_eq!(fresh_eval_first_page.len(), 2);
}

/// D10: a space a memory binding grants the actor appears in the actor-filtered
/// space list next to the actor's own spaces; an unbound actor does not see it.
#[tokio::test]
async fn sqlite_list_spaces_includes_binding_shared_spaces() {
    let store = new_contract_store().await;
    // Space 60 is owned by another principal.
    store
        .create_space_record(
            1,
            60,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "999".to_string(),
                space_type: "workspace".to_string(),
                display_name: "Shared Space".to_string(),
                default_scope: "tenant".to_string(),
            },
        )
        .await
        .unwrap();
    // Space 61 stays owned by the actor themself.
    store
        .create_space_record(
            1,
            61,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "501".to_string(),
                space_type: "workspace".to_string(),
                display_name: "Own Space".to_string(),
                default_scope: "tenant".to_string(),
            },
        )
        .await
        .unwrap();
    store
        .insert_subject(InsertSubjectCommand {
            id: 9100,
            uuid: "subject-shared-actor",
            tenant_id: 1,
            organization_id: None,
            subject_type: "user",
            subject_ref: "501",
            display_name: "Shared actor",
            default_space_id: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    // Grant the actor access to space 60 through an active binding.
    sqlx::query(
        r#"
        INSERT INTO ai_memory_binding (
          id, uuid, tenant_id, binding_kind, source_subject_id,
          target_space_id, binding_role, status, created_at, updated_at, version
        )
        VALUES (9200, 'binding-shared-1', 1, 'access', 9100, 60, 'reader', 'active',
                '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 1)
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();

    let actor_spaces = store
        .list_spaces_for_tenant(1, 10, 0, Some("501"))
        .await
        .unwrap();
    let shared_space_ids = actor_spaces
        .iter()
        .map(|row| row.space_id)
        .collect::<Vec<_>>();
    assert!(
        shared_space_ids.contains(&60),
        "a binding-shared space must appear in the actor-filtered list, got {shared_space_ids:?}"
    );
    assert!(
        shared_space_ids.contains(&61),
        "the actor's own space must stay in the list"
    );

    let other_actor_spaces = store
        .list_spaces_for_tenant(1, 10, 0, Some("404"))
        .await
        .unwrap();
    assert!(
        !other_actor_spaces.iter().any(|row| row.space_id == 60),
        "an actor without a binding must not see the shared space"
    );
    // The unfiltered list keeps seeing every tenant-one space: the two seeded
    // contract spaces plus the two this test created.
    let all_spaces = store.list_spaces_for_tenant(1, 10, 0, None).await.unwrap();
    assert_eq!(all_spaces.len(), 4);
}

/// D10 (gate parity): the list's binding branch uses the same gates as the
/// governance evaluation — only access/share/ownership kinds authorize and the
/// binding window must be current — so a `reference`-kind or expired binding
/// never makes a space visible that access would refuse.
#[tokio::test]
async fn sqlite_list_spaces_binding_visibility_matches_the_governance_gates() {
    let store = new_contract_store().await;
    // Distinct space types: uk_ai_space_owner_type forbids one owner holding
    // two spaces of the same type.
    for (space_id, space_type, name) in [
        (70, "workspace", "Reference Space"),
        (71, "notes", "Expired Space"),
        (72, "archive", "Live Space"),
    ] {
        store
            .create_space_record(
                1,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: "999".to_string(),
                    space_type: space_type.to_string(),
                    display_name: name.to_string(),
                    default_scope: "tenant".to_string(),
                },
            )
            .await
            .unwrap();
    }
    store
        .insert_subject(InsertSubjectCommand {
            id: 9300,
            uuid: "subject-gate-actor",
            tenant_id: 1,
            organization_id: None,
            subject_type: "user",
            subject_ref: "601",
            display_name: "Gate actor",
            default_space_id: None,
            metadata_json: None,
        })
        .await
        .unwrap();
    // reference kind: never authorizes, so the space must not be listed.
    sqlx::query(
        r#"
        INSERT INTO ai_memory_binding (
          id, uuid, tenant_id, binding_kind, source_subject_id,
          target_space_id, binding_role, status, created_at, updated_at, version
        )
        VALUES (9400, 'binding-gate-ref', 1, 'reference', 9300, 70, 'reader', 'active',
                '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 1)
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    // Expired window: status is still 'active' but validity has lapsed.
    sqlx::query(
        r#"
        INSERT INTO ai_memory_binding (
          id, uuid, tenant_id, binding_kind, source_subject_id,
          target_space_id, binding_role, status, valid_from, valid_to,
          created_at, updated_at, version
        )
        VALUES (9401, 'binding-gate-expired', 1, 'access', 9300, 71, 'reader', 'active',
                '2026-01-01T00:00:00.000Z', '2026-06-01T00:00:00.000Z',
                '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 1)
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    // Live access binding: the space must be listed.
    sqlx::query(
        r#"
        INSERT INTO ai_memory_binding (
          id, uuid, tenant_id, binding_kind, source_subject_id,
          target_space_id, binding_role, status, valid_from, valid_to,
          created_at, updated_at, version
        )
        VALUES (9402, 'binding-gate-live', 1, 'access', 9300, 72, 'reader', 'active',
                '2026-01-01T00:00:00.000Z', '2099-01-01T00:00:00.000Z',
                '2026-01-01T00:00:00.000Z', '2026-01-01T00:00:00.000Z', 1)
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();

    let actor_spaces = store
        .list_spaces_for_tenant(1, 10, 0, Some("601"))
        .await
        .unwrap();
    let listed = actor_spaces
        .iter()
        .map(|row| row.space_id)
        .collect::<Vec<_>>();
    assert!(
        !listed.contains(&70),
        "a reference-kind binding must not surface the space, got {listed:?}"
    );
    assert!(
        !listed.contains(&71),
        "an expired binding must not surface the space, got {listed:?}"
    );
    assert!(
        listed.contains(&72),
        "a live access binding must surface the space, got {listed:?}"
    );
}

/// C5/R5b: a requeued extraction removes only the previous attempt's pending
/// candidates for its own job attribution — reviewed candidates and other
/// jobs' candidates survive.
#[tokio::test]
async fn sqlite_delete_pending_candidates_for_learning_job_is_scoped_to_the_job() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let command = |candidate_id: &str, job: Option<&str>| CreateMemoryCandidateCommand {
        scope: scope.clone(),
        candidate_id: candidate_id.to_string(),
        candidate_type: "extraction".to_string(),
        memory_type: "semantic".to_string(),
        proposed_text: format!("proposal {candidate_id}"),
        proposed_payload_json: None,
        evidence_json: None,
        confidence: 0.7,
        learning_job_uuid: job.map(str::to_string),
    };
    for (id, job) in [
        ("7100", Some("job-a")),
        ("7101", Some("job-a")),
        ("7102", Some("job-b")),
    ] {
        store.create_candidate(&command(id, job)).await.unwrap();
    }
    // Approve one of job-a's candidates directly: reviewed rows are never
    // reachable through the pending-only cleanup.
    sqlx::query("UPDATE ai_candidate SET decision_state = 'approved' WHERE uuid = '7100'")
        .execute(store.pool())
        .await
        .unwrap();

    let deleted = store
        .delete_pending_candidates_for_learning_job(1, "job-a")
        .await
        .unwrap();
    assert_eq!(deleted, 1, "only job-a's pending candidate is removed");

    let remaining: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT uuid, learning_job_uuid FROM ai_candidate ORDER BY uuid")
            .fetch_all(store.pool())
            .await
            .unwrap()
            .into_iter()
            .map(|row: (String, Option<String>)| row)
            .collect();
    assert_eq!(remaining.len(), 2);
    assert!(remaining.iter().any(|(uuid, _)| uuid == "7100"));
    assert!(remaining.iter().any(|(uuid, _)| uuid == "7102"));
}

#[tokio::test]
async fn sqlite_consolidation_atomically_preserves_evidence_journals_and_identity_boundaries() {
    let store = new_contract_store().await;
    let user_one = MemoryScopeContext {
        tenant_id: 1,
        space_id: 1,
        organization_id: None,
        user_id: Some(101),
    };
    let user_two = MemoryScopeContext {
        user_id: Some(202),
        ..user_one.clone()
    };
    for (scope, memory_id) in [
        (&user_one, "consolidation-winner"),
        (&user_one, "consolidation-loser"),
        (&user_two, "consolidate-user-two"),
    ] {
        store
            .create_record_open_api(
                scope,
                memory_id,
                "user",
                "semantic",
                Some("editor"),
                Some("prefers"),
                "uses a modal editor",
                "editor prefers modal",
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }

    sqlx::query("UPDATE ai_record SET evidence_count = 10 WHERE tenant_id = ? AND uuid = ?")
        .bind(user_one.tenant_id)
        .bind("consolidation-winner")
        .execute(store.pool())
        .await
        .unwrap();
    for event_id in [
        "consolidation-shared",
        "consolidation-winner-only",
        "consolidation-loser-only",
    ] {
        store
            .append_open_api_event(
                &user_one,
                event_id,
                "memory.evidence.observed",
                "contract_test",
                "2026-07-20T00:00:00Z",
                &serde_json::json!({ "content": event_id }),
                "internal",
            )
            .await
            .unwrap();
    }
    for (source_id, memory_id, event_id) in [
        (
            "consolidation-source-winner-shared",
            "consolidation-winner",
            "consolidation-shared",
        ),
        (
            "consolidation-source-winner-only",
            "consolidation-winner",
            "consolidation-winner-only",
        ),
        (
            "consolidation-source-loser-shared",
            "consolidation-loser",
            "consolidation-shared",
        ),
        (
            "consolidation-source-loser-only",
            "consolidation-loser",
            "consolidation-loser-only",
        ),
    ] {
        store
            .append_record_source_for_tenant(
                user_one.tenant_id,
                source_id,
                memory_id,
                event_id,
                "supporting",
                Some(0.1),
            )
            .await
            .unwrap();
    }
    store
        .rebuild_record_search_indexes_for_space(user_one.tenant_id, user_one.space_id)
        .await
        .unwrap();

    let all_users_scope = MemoryScopeContext {
        user_id: None,
        ..user_one.clone()
    };
    let operation_id = "consolidation-contract-operation";
    let consolidated = store
        .consolidate_duplicate_records_in_scope_detailed(ConsolidateDuplicateRecordsCommand {
            scope: &all_users_scope,
            operation_id,
        })
        .await
        .unwrap();
    assert_eq!(consolidated.superseded_records, 1);
    assert_eq!(consolidated.transferred_sources, 1);
    assert_eq!(consolidated.deduplicated_sources, 1);

    let winner = store
        .retrieve_record_detail(&user_one, "consolidation-winner")
        .await
        .unwrap()
        .unwrap();
    let superseded = store
        .retrieve_record_detail(&user_one, "consolidation-loser")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(winner.status, "active");
    assert_eq!(winner.evidence_count, Some(3));
    assert_eq!(superseded.status, "superseded");
    assert!(superseded.superseded_by_memory_id.is_some());

    let winner_source_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM ai_record_source source
        JOIN ai_record record ON record.id = source.memory_id
        WHERE source.tenant_id = ? AND record.uuid = ?
        "#,
    )
    .bind(user_one.tenant_id)
    .bind("consolidation-winner")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(winner_source_count, 3);
    let loser_source_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM ai_record_source source
        JOIN ai_record record ON record.id = source.memory_id
        WHERE source.tenant_id = ? AND record.uuid = ?
        "#,
    )
    .bind(user_one.tenant_id)
    .bind("consolidation-loser")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(loser_source_count, 0);

    let stale_fts_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(user_one.tenant_id)
    .bind(user_one.space_id)
    .bind("consolidation-loser")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(stale_fts_rows, 0);

    let outbox_payload: String = sqlx::query_scalar(
        "SELECT payload_json FROM ai_outbox_event WHERE tenant_id = ? AND event_type = ?",
    )
    .bind(user_one.tenant_id)
    .bind("memory.record.superseded")
    .fetch_one(store.pool())
    .await
    .unwrap();
    let outbox_payload: serde_json::Value = serde_json::from_str(&outbox_payload).unwrap();
    assert_eq!(outbox_payload["operationId"], operation_id);
    assert_eq!(outbox_payload["memoryId"], "consolidation-loser");
    assert_eq!(
        outbox_payload["supersededByMemoryId"],
        "consolidation-winner"
    );
    assert_eq!(outbox_payload["transferredSources"], 1);
    assert_eq!(outbox_payload["deduplicatedSources"], 1);
    let pending_outbox: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND event_type = ? AND publish_state = 'pending'",
    )
    .bind(user_one.tenant_id)
    .bind("memory.record.superseded")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(pending_outbox, 1);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND action = ? AND resource_id = ?",
    )
    .bind(user_one.tenant_id)
    .bind("memory.record.consolidate")
    .bind("consolidation-loser")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(audit_count, 1);

    let retried = store
        .consolidate_duplicate_records_in_scope_detailed(ConsolidateDuplicateRecordsCommand {
            scope: &all_users_scope,
            operation_id,
        })
        .await
        .unwrap();
    assert_eq!(retried, consolidated);
    let new_operation = store
        .consolidate_duplicate_records_in_scope_detailed(ConsolidateDuplicateRecordsCommand {
            scope: &all_users_scope,
            operation_id: "consolidation-contract-noop",
        })
        .await
        .unwrap();
    assert_eq!(new_operation.superseded_records, 0);

    let isolated = store
        .retrieve_record_detail(&user_two, "consolidate-user-two")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(isolated.status, "active");
    assert!(isolated.superseded_by_memory_id.is_none());
}

#[tokio::test]
async fn sqlite_consolidation_rolls_back_supersession_sources_and_outbox_on_journal_failure() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    for memory_id in ["rollback-winner", "rollback-loser"] {
        store
            .create_record_open_api(
                &scope,
                memory_id,
                "user",
                "semantic",
                Some("editor"),
                Some("prefers"),
                "uses a modal editor",
                "editor prefers modal",
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }
    sqlx::query("UPDATE ai_record SET evidence_count = 10 WHERE tenant_id = ? AND uuid = ?")
        .bind(scope.tenant_id)
        .bind("rollback-winner")
        .execute(store.pool())
        .await
        .unwrap();
    store
        .append_open_api_event(
            &scope,
            "rollback-event",
            "memory.evidence.observed",
            "contract_test",
            "2026-07-20T00:00:00Z",
            &serde_json::json!({ "content": "rollback evidence" }),
            "internal",
        )
        .await
        .unwrap();
    store
        .append_record_source_for_tenant(
            scope.tenant_id,
            "rollback-source",
            "rollback-loser",
            "rollback-event",
            "supporting",
            Some(0.1),
        )
        .await
        .unwrap();
    sqlx::query(
        r#"
        CREATE TRIGGER fail_consolidation_audit
        BEFORE INSERT ON ai_audit_log
        WHEN NEW.action = 'memory.record.consolidate'
        BEGIN
          SELECT RAISE(ABORT, 'forced consolidation journal failure');
        END
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();

    let result = store
        .consolidate_duplicate_records_in_scope_detailed(ConsolidateDuplicateRecordsCommand {
            scope: &scope,
            operation_id: "rollback-operation",
        })
        .await;
    assert!(result.is_err());

    let loser = store
        .retrieve_record_detail(&scope, "rollback-loser")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loser.status, "active");
    assert!(loser.superseded_by_memory_id.is_none());
    let loser_sources: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM ai_record_source source
        JOIN ai_record record ON record.id = source.memory_id
        WHERE source.tenant_id = ? AND record.uuid = ?
        "#,
    )
    .bind(scope.tenant_id)
    .bind("rollback-loser")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(loser_sources, 1);
    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND event_type = ?",
    )
    .bind(scope.tenant_id)
    .bind("memory.record.superseded")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn sqlite_open_api_event_with_audit_commits_event_and_audit_rows_together() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .append_open_api_event_with_audit(
            &scope,
            "audited-event",
            "memory.evidence.observed",
            "contract_test",
            "2026-07-20T00:00:00Z",
            &serde_json::json!({ "content": "audited evidence" }),
            "internal",
            "audited-event-audit",
            "memory.event.create",
            "memory_event",
            "audited-event",
            "accepted",
        )
        .await
        .expect("event and audit rows must commit together");

    let event = store
        .retrieve_open_api_event(&scope, "audited-event")
        .await
        .unwrap()
        .expect("audited event row");
    assert_eq!(event.event_type, "memory.evidence.observed");
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind("audited-event-audit")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn sqlite_open_api_event_with_audit_rolls_back_event_when_audit_insert_fails() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    sqlx::query(
        r#"
        CREATE TRIGGER fail_open_api_event_audit
        BEFORE INSERT ON ai_audit_log
        WHEN NEW.action = 'memory.event.create'
        BEGIN
          SELECT RAISE(ABORT, 'forced open api event audit failure');
        END
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();

    let result = store
        .append_open_api_event_with_audit(
            &scope,
            "orphan-event",
            "memory.evidence.observed",
            "contract_test",
            "2026-07-20T00:00:00Z",
            &serde_json::json!({ "content": "orphan evidence" }),
            "internal",
            "orphan-event-audit",
            "memory.event.create",
            "memory_event",
            "orphan-event",
            "accepted",
        )
        .await;
    assert!(result.is_err(), "a failed audit insert must fail the append");

    assert!(
        store
            .retrieve_open_api_event(&scope, "orphan-event")
            .await
            .unwrap()
            .is_none(),
        "the event row must roll back with its failed audit line"
    );
}

#[tokio::test]
async fn sqlite_open_api_event_with_audit_replay_writes_only_the_owed_audit_line() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    async fn append(
        store: &NativeSqlMemoryStore,
        scope: &MemoryScopeContext,
        audit_id: &str,
        payload: serde_json::Value,
    ) -> Result<(), NativeSqlStoreError> {
        store
            .append_open_api_event_with_audit(
                scope,
                "replayed-event",
                "memory.evidence.observed",
                "contract_test",
                "2026-07-20T00:00:00Z",
                &payload,
                "internal",
                audit_id,
                "memory.event.create",
                "memory_event",
                "replayed-event",
                "accepted",
            )
            .await
    }
    append(
        &store,
        &scope,
        "replayed-event-audit-1",
        serde_json::json!({ "content": "evidence" }),
    )
    .await
    .unwrap();
    // Same event id and payload: the idempotent replay must not duplicate the
    // event row, but the replayed request still earns its audit line.
    append(
        &store,
        &scope,
        "replayed-event-audit-2",
        serde_json::json!({ "content": "evidence" }),
    )
    .await
    .unwrap();

    let event_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_event WHERE tenant_id = ? AND uuid = ?")
            .bind(scope.tenant_id)
            .bind("replayed-event")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(event_count, 1);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND uuid LIKE ?",
    )
    .bind(scope.tenant_id)
    .bind("replayed-event-audit-%")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(audit_count, 2);

    // Same event id with a different payload is a conflict, not a replay: it
    // must fail and leave no audit row behind.
    let conflict = append(
        &store,
        &scope,
        "replayed-event-audit-3",
        serde_json::json!({ "content": "different" }),
    )
    .await
    .expect_err("conflicting replay must fail");
    assert!(matches!(conflict, NativeSqlStoreError::EventConflict { .. }));
    let audit_count_after_conflict: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind("replayed-event-audit-3")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(audit_count_after_conflict, 0);
}

fn assert_utc_timestamp(value: Option<&str>) {
    let Some(text) = value else {
        panic!("expected UTC timestamp");
    };
    assert!(text.ends_with('Z'), "timestamp must be UTC RFC3339: {text}");
}

fn outbox_command<'a>(
    scope: &'a MemoryScopeContext,
    outbox_id: &'a str,
    aggregate_id: &'a str,
    payload_json: &'a str,
) -> NativeSqlAppendOutboxEventCommand<'a> {
    NativeSqlAppendOutboxEventCommand {
        scope,
        outbox_id,
        aggregate_type: "ai_record",
        aggregate_id,
        event_type: "memory.record.created",
        event_version: "1",
        payload_json,
    }
}

fn candidate_command(
    scope: MemoryScopeContext,
    candidate_id: &str,
) -> CreateMemoryCandidateCommand {
    CreateMemoryCandidateCommand {
        scope: scope.clone(),
        candidate_id: candidate_id.to_string(),
        candidate_type: "observation".to_string(),
        memory_type: "semantic".to_string(),
        proposed_text: "User prefers concise answers".to_string(),
        proposed_payload_json: Some(r#"{"preference":"concise"}"#.to_string()),
        evidence_json: Some(r#"{"eventId":"evt-1"}"#.to_string()),
        confidence: 0.91,
        learning_job_uuid: None,
    }
}

fn habit_command(
    scope: MemoryScopeContext,
    habit_id: &str,
    user_id: i64,
) -> UpsertMemoryHabitCommand {
    UpsertMemoryHabitCommand {
        scope,
        habit_id: habit_id.to_string(),
        user_id,
        habit_key: "answer_style:concise".to_string(),
        habit_type: "preference".to_string(),
        description: "Prefers concise answers".to_string(),
        stage: "candidate".to_string(),
        strength: 0.4,
        confidence: 0.8,
        support_count: 2,
        metadata_json: Some(r#"{"source":"signals"}"#.to_string()),
    }
}

fn retrieval_trace_command(
    scope: MemoryScopeContext,
    trace_id: &str,
) -> AppendMemoryRetrievalTraceCommand {
    AppendMemoryRetrievalTraceCommand {
        scope: scope.clone(),
        trace_id: trace_id.to_string(),
        actor_id: Some("user-42".to_string()),
        query_text: Some("concise answer preference".to_string()),
        query_hash: format!("hash:{trace_id}"),
        retrievers_json: Some(r#"["native_sql"]"#.to_string()),
        latency_ms: Some(17),
        degraded: false,
        metadata_json: Some(r#"{"profile":"native_sql"}"#.to_string()),
        hits: vec![
            MemoryRetrievalHitDraft {
                hit_id: format!("{trace_id}-hit-1"),
                memory_id: Some("rec-trace-1".to_string()),
                space_id: Some(scope.space_id),
                retriever_name: "native_sql".to_string(),
                result_rank: 1,
                raw_score: Some(0.75),
                fused_score: Some(0.9),
                explanation_json: Some(r#"{"match":"keyword"}"#.to_string()),
                status: "selected".to_string(),
            },
            MemoryRetrievalHitDraft {
                hit_id: format!("{trace_id}-hit-2"),
                memory_id: None,
                space_id: None,
                retriever_name: "native_sql".to_string(),
                result_rank: 2,
                raw_score: Some(0.5),
                fused_score: Some(0.6),
                explanation_json: None,
                status: "candidate".to_string(),
            },
        ],
        context_pack: Some(MemoryContextPackSnapshot {
            context_pack_id: format!("{trace_id}-pack"),
            pack_json: r#"{"memoryIds":["rec-trace-1"]}"#.to_string(),
            estimated_tokens: 12,
            truncated: false,
        }),
    }
}

async fn seed_contract_spaces(store: &NativeSqlMemoryStore) {
    let spaces = [
        (1_i64, 1_i64, "tenant", "1", "workspace"),
        (1_i64, 2_i64, "tenant", "1", "shared"),
        (2_i64, 3_i64, "tenant", "2", "workspace"),
    ];
    for (tenant_id, space_id, owner_type, owner_id, space_type) in spaces {
        store
            .create_space_record(
                tenant_id,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: owner_type.to_string(),
                    owner_subject_id: owner_id.to_string(),
                    space_type: space_type.to_string(),
                    display_name: format!("Contract Space {space_id}"),
                    default_scope: "user".to_string(),
                },
            )
            .await
            .expect("seed contract space");
    }
}

async fn new_contract_store() -> NativeSqlMemoryStore {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("contract sqlite store must initialize");
    seed_contract_spaces(&store).await;
    store
}

fn space_command(space_id: i64, owner_id: &str, space_type: &str) -> CreateMemorySpaceCommand {
    CreateMemorySpaceCommand {
        tenant_id: 1,
        space_id,
        organization_id: Some(7),
        owner_subject_type: "user".to_string(),
        owner_subject_id: owner_id.to_string(),
        space_type: space_type.to_string(),
        display_name: format!("Space {space_id}"),
        default_scope: "user".to_string(),
    }
}

fn file_backed_sqlite_config(label: &str) -> (DatabaseConfig, PathBuf) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sdkwork-memory-{label}-{}-{nonce}.sqlite",
        std::process::id()
    ));
    let normalized = path.to_string_lossy().replace('\\', "/");
    let url = if cfg!(windows) {
        format!("sqlite:///{normalized}?mode=rwc")
    } else {
        format!("sqlite://{normalized}?mode=rwc")
    };
    (
        DatabaseConfig {
            engine: DatabaseEngine::Sqlite,
            url,
            max_connections: 1,
            min_connections: 1,
            ..DatabaseConfig::default()
        },
        path,
    )
}

fn remove_sqlite_test_artifacts(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
}

async fn create_canonical_fixture(
    store: &NativeSqlMemoryStore,
    scope: &MemoryScopeContext,
    memory_id: &str,
    memory_type: &str,
    canonical_text: &str,
    sensitivity_level: &str,
) {
    MemoryRecordStorePort::create_canonical_atomic(
        store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: memory_id.to_string(),
            scope_label: "user".to_string(),
            memory_type: memory_type.to_string(),
            subject: Some("retrieval".to_string()),
            predicate: Some("matches".to_string()),
            object_text: canonical_text.to_string(),
            canonical_text: canonical_text.to_string(),
            sensitivity_level: sensitivity_level.to_string(),
            journal: mutation_journal(memory_id, &format!("{memory_id}-created")),
            expires_at: None,
            metadata_json: None,
        },
    )
    .await
    .expect("canonical retrieval fixture must be created");
}

#[tokio::test]
async fn sqlite_canonical_atomic_mutations_journal_and_suppress_stale_fts() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    assert!(store.supports_canonical_atomic());

    let created = MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "canonical-1".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: Some("user".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: "dark mode".to_string(),
            canonical_text: "User prefers dark mode".to_string(),
            sensitivity_level: "internal".to_string(),
            journal: mutation_journal("canonical-1", "created"),
            expires_at: None,
            metadata_json: None,
        },
    )
    .await
    .expect("canonical create must commit");
    assert_eq!(created.version, 1);
    assert_eq!(created.canonical_text, "User prefers dark mode");

    let loaded = MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "canonical-1".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(loaded.memory_id, "canonical-1");
    assert_eq!(loaded.version, 1);

    let create_outbox = MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-created".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(create_outbox.aggregate_id, "canonical-1");
    let create_audit = MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-created".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(create_audit.resource_id, "canonical-1");

    let updated = MemoryRecordStorePort::update_canonical_atomic(
        &store,
        UpdateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "canonical-1".to_string(),
            canonical_text: Some("User prefers light mode".to_string()),
            subject: Some("account".to_string()),
            journal: mutation_journal("canonical-1", "updated"),
            metadata_json: None,
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.version, 2);
    assert_eq!(updated.subject.as_deref(), Some("account"));

    let updated_hits = store
        .search_record_details_fulltext(&scope, "light mode", 5)
        .await
        .unwrap();
    assert_eq!(updated_hits.len(), 1);

    let receipt = MemoryRecordStorePort::delete_canonical_atomic(
        &store,
        DeleteCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "canonical-1".to_string(),
            journal: mutation_journal("canonical-1", "deleted"),
        },
    )
    .await
    .unwrap();
    assert!(receipt.deleted);
    assert!(!receipt.already_deleted);
    assert!(MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "canonical-1".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(store
        .search_record_details_fulltext(&scope, "light mode", 5)
        .await
        .unwrap()
        .is_empty());
    let fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("canonical-1")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        fts_count, 0,
        "canonical delete must physically remove FTS state"
    );

    let delete_outbox = MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope,
            outbox_id: "outbox-deleted".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(delete_outbox.event_type, "memory.record.deleted");
}

#[tokio::test]
async fn sqlite_record_quota_admission_rejects_without_partial_side_effects() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    create_canonical_fixture(
        &store,
        &scope,
        "quota-existing",
        "semantic",
        "existing quota record",
        "internal",
    )
    .await;

    let admission = MemoryRecordStorePort::create_canonical_atomic_with_quota(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "quota-rejected".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: None,
            predicate: None,
            object_text: "must not be written".to_string(),
            canonical_text: "must not be written".to_string(),
            sensitivity_level: "internal".to_string(),
            journal: mutation_journal("quota-rejected", "quota-rejected"),
            expires_at: None,
            metadata_json: None,
        },
        1,
    )
    .await
    .expect("quota rejection is a successful admission outcome");
    assert_eq!(
        admission,
        MemoryRecordQuotaAdmission::QuotaExceeded {
            active_records: 1,
            max_active_records: 1,
        }
    );
    assert!(MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "quota-rejected".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-quota-rejected".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-quota-rejected".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    let fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("quota-rejected")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fts_count, 0);
}

#[tokio::test]
async fn sqlite_candidate_quota_admission_is_atomic_and_retry_idempotent() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    create_canonical_fixture(
        &store,
        &scope,
        "promotion-existing",
        "semantic",
        "existing promotion record",
        "internal",
    )
    .await;
    MemoryCandidateStorePort::create(&store, candidate_command(scope.clone(), "candidate-quota"))
        .await
        .expect("candidate fixture must be created");

    let rejected = store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-quota",
                memory_uuid: "promotion-rejected",
                memory_type: "semantic",
                proposed_text: "should remain pending",
                evidence_links: &[],
                decided_by: Some(7),
                create_record: true,
            },
            1,
        )
        .await
        .expect("quota rejection is a successful admission outcome");
    assert_eq!(
        rejected,
        MemoryRecordQuotaAdmission::QuotaExceeded {
            active_records: 1,
            max_active_records: 1,
        }
    );
    let pending = store
        .retrieve_candidate_detail_for_tenant(scope.tenant_id, "candidate-quota")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.decision_state, "pending");
    assert!(pending.target_memory_uuid.is_none());
    assert!(store
        .retrieve_record_detail(&scope, "promotion-rejected")
        .await
        .unwrap()
        .is_none());

    MemoryRecordStorePort::delete_canonical_atomic(
        &store,
        DeleteCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "promotion-existing".to_string(),
            journal: mutation_journal("promotion-existing", "promotion-existing-deleted"),
        },
    )
    .await
    .unwrap();

    let admitted = store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-quota",
                memory_uuid: "promotion-admitted",
                memory_type: "semantic",
                proposed_text: "promoted memory",
                evidence_links: &[],
                decided_by: Some(7),
                create_record: true,
            },
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        admitted,
        MemoryRecordQuotaAdmission::Admitted("promotion-admitted".to_string())
    );

    let retry = store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-quota",
                memory_uuid: "promotion-duplicate-request",
                memory_type: "semantic",
                proposed_text: "different retry payload",
                evidence_links: &[],
                decided_by: Some(8),
                create_record: true,
            },
            1,
        )
        .await
        .unwrap();
    assert_eq!(
        retry,
        MemoryRecordQuotaAdmission::Admitted("promotion-admitted".to_string())
    );
    let promoted = store
        .retrieve_candidate_detail_for_tenant(scope.tenant_id, "candidate-quota")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(promoted.decision_state, "approved");
    assert_eq!(
        promoted.target_memory_uuid.as_deref(),
        Some("promotion-admitted")
    );
    let active_count = store.count_active_records_for_scope(&scope).await.unwrap();
    assert_eq!(active_count, 1);
    let fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("promotion-admitted")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fts_count, 1);
}

#[tokio::test]
async fn sqlite_candidate_detail_projection_is_provider_neutral_and_tenant_scoped() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    MemoryCandidateStorePort::create(&store, candidate_command(scope.clone(), "candidate-detail"))
        .await
        .unwrap();

    let admitted = store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-detail",
                memory_uuid: "candidate-detail-target",
                memory_type: "semantic",
                proposed_text: "provider-neutral detail target",
                evidence_links: &[],
                decided_by: Some(7),
                create_record: true,
            },
            10,
        )
        .await
        .unwrap();
    assert_eq!(
        admitted,
        MemoryRecordQuotaAdmission::Admitted("candidate-detail-target".to_string())
    );

    let detail = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(detail.candidate_id, "candidate-detail");
    assert_eq!(detail.space_id, scope.space_id);
    assert_eq!(
        detail.evidence_json.as_deref(),
        Some(r#"{"eventId":"evt-1"}"#)
    );
    assert_eq!(
        detail.target_memory_id.as_deref(),
        Some("candidate-detail-target")
    );
    assert_eq!(detail.decision_state, "approved");

    let cross_tenant = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id + 1,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap();
    assert!(cross_tenant.is_none());

    store
        .mark_record_deleted(&scope, "candidate-detail-target")
        .await
        .unwrap();
    let after_delete = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-detail".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(after_delete.target_memory_id, None);
    assert!(store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-detail",
                memory_uuid: "candidate-detail-replacement",
                memory_type: "semantic",
                proposed_text: "must not recreate a deleted promotion target",
                evidence_links: &[],
                decided_by: Some(7),
                create_record: true,
            },
            10,
        )
        .await
        .is_err());
    assert!(store
        .retrieve_record(&scope, "candidate-detail-replacement")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_candidate_detail_does_not_leak_cross_space_target_memory() {
    let store = new_contract_store().await;
    let candidate_scope = MemoryScopeContext::for_test(1, 1);
    let target_scope = MemoryScopeContext::for_test(1, 2);

    MemoryCandidateStorePort::create(
        &store,
        candidate_command(candidate_scope.clone(), "candidate-cross-space-target"),
    )
    .await
    .unwrap();
    store
        .create_record_open_api(
            &target_scope,
            "cross-space-target",
            "user",
            "semantic",
            Some("account"),
            Some("prefers"),
            "other space value",
            "The other space value",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();

    // The legacy tenant-scoped helper must reject cross-space assignment.
    // Inject a historical corrupted FK directly to prove that the read model
    // still fails closed if an older database contains one.
    store
        .set_candidate_target_memory_for_tenant(
            candidate_scope.tenant_id,
            "candidate-cross-space-target",
            "cross-space-target",
        )
        .await
        .expect_err("cross-space candidate target must be rejected");
    sqlx::query(
        r#"
        UPDATE ai_candidate
        SET target_memory_id = (
          SELECT id
          FROM ai_record
          WHERE tenant_id = ? AND space_id = ? AND uuid = ?
        )
        WHERE tenant_id = ? AND space_id = ? AND uuid = ?
        "#,
    )
    .bind(target_scope.tenant_id)
    .bind(target_scope.space_id)
    .bind("cross-space-target")
    .bind(candidate_scope.tenant_id)
    .bind(candidate_scope.space_id)
    .bind("candidate-cross-space-target")
    .execute(store.pool())
    .await
    .unwrap();

    let detail = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: candidate_scope.tenant_id,
            candidate_id: "candidate-cross-space-target".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(detail.space_id, candidate_scope.space_id);
    assert_eq!(detail.target_memory_id, None);
}

#[tokio::test]
async fn sqlite_candidate_target_assignment_requires_live_same_space_record() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let other_scope = MemoryScopeContext::for_test(1, 2);
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "candidate-live-target"),
    )
    .await
    .unwrap();
    store
        .create_record_open_api(
            &scope,
            "candidate-live-memory",
            "user",
            "semantic",
            None,
            None,
            "live target",
            "live target",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .set_candidate_target_memory_for_tenant(
            scope.tenant_id,
            "candidate-live-target",
            "candidate-live-memory",
        )
        .await
        .unwrap();
    store
        .create_record_open_api(
            &other_scope,
            "candidate-other-space-memory",
            "user",
            "semantic",
            None,
            None,
            "other-space target",
            "other-space target",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    for invalid_target in ["candidate-other-space-memory", "missing-memory"] {
        assert!(store
            .set_candidate_target_memory_for_tenant(
                scope.tenant_id,
                "candidate-live-target",
                invalid_target,
            )
            .await
            .is_err());
    }
    let assigned = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-live-target".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        assigned.target_memory_id.as_deref(),
        Some("candidate-live-memory")
    );

    store
        .mark_record_deleted(&scope, "candidate-live-memory")
        .await
        .unwrap();
    assert!(store
        .set_candidate_target_memory_for_tenant(
            scope.tenant_id,
            "candidate-live-target",
            "candidate-live-memory",
        )
        .await
        .is_err());
    let after_delete = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-live-target".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(after_delete.target_memory_id, None);
}

#[tokio::test]
async fn sqlite_candidate_promotion_rejects_pending_target_reference_without_side_effects() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "candidate-pending-target"),
    )
    .await
    .unwrap();
    store
        .create_record_open_api(
            &scope,
            "candidate-pending-existing-target",
            "user",
            "semantic",
            None,
            None,
            "legacy target",
            "legacy target",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .set_candidate_target_memory_for_tenant(
            scope.tenant_id,
            "candidate-pending-target",
            "candidate-pending-existing-target",
        )
        .await
        .unwrap();

    let error = store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-pending-target",
                memory_uuid: "candidate-pending-new-target",
                memory_type: "semantic",
                proposed_text: "must not be promoted",
                evidence_links: &[],
                decided_by: Some(7),
                create_record: true,
            },
            10,
        )
        .await
        .expect_err("pending candidate with target reference must fail closed");
    assert!(matches!(
        error,
        NativeSqlStoreError::InvariantViolation { .. }
    ));
    let detail = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-pending-target".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(detail.decision_state, "pending");
    assert_eq!(
        detail.target_memory_id.as_deref(),
        Some("candidate-pending-existing-target")
    );
    assert!(store
        .retrieve_record(&scope, "candidate-pending-new-target")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_candidate_promotion_rejects_cross_space_evidence_without_creating_memory() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let other_scope = MemoryScopeContext::for_test(1, 2);
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "candidate-cross-space-evidence"),
    )
    .await
    .unwrap();
    MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope: other_scope,
            event_id: "event-other-space".to_string(),
            content: "must not become evidence in another space".to_string(),
        },
    )
    .await
    .unwrap();

    assert!(store
        .promote_and_approve_candidate_with_quota(
            PromoteApprovedCandidateCommand {
                scope: &scope,
                tenant_id: scope.tenant_id,
                candidate_id: "candidate-cross-space-evidence",
                memory_uuid: "candidate-cross-space-evidence-target",
                memory_type: "semantic",
                proposed_text: "must not persist",
                evidence_links: &[(
                    "cross-space-source".to_string(),
                    "event-other-space".to_string(),
                    None,
                )],
                decided_by: Some(7),
                create_record: true,
            },
            10,
        )
        .await
        .is_err());
    assert!(store
        .retrieve_record(&scope, "candidate-cross-space-evidence-target")
        .await
        .unwrap()
        .is_none());
    let detail = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "candidate-cross-space-evidence".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(detail.decision_state, "pending");
    assert_eq!(detail.target_memory_id, None);
}

#[tokio::test]
async fn sqlite_hard_delete_cleans_foreign_key_dependents_and_fts() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let now = "2026-07-12T00:00:00Z";
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(foreign_keys, 1, "native SQLite must enforce foreign keys");

    for memory_id in ["hard-delete-target", "hard-delete-sibling"] {
        store
            .create_record_open_api(
                &scope,
                memory_id,
                "user",
                "semantic",
                None,
                None,
                "privacy deletion fixture",
                "privacy deletion fixture",
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "hard-delete-candidate"),
    )
    .await
    .unwrap();
    store
        .set_candidate_target_memory_for_tenant(
            scope.tenant_id,
            "hard-delete-candidate",
            "hard-delete-target",
        )
        .await
        .unwrap();
    MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope: scope.clone(),
            event_id: "hard-delete-event".to_string(),
            content: "privacy deletion evidence".to_string(),
        },
    )
    .await
    .unwrap();
    let mut tx = store.begin_tx().await.unwrap();
    store
        .append_record_source_on_tx(
            &mut tx,
            sdkwork_memory_plugin_native_sql::NativeSqlAppendRecordSourceCommand {
                scope: &scope,
                source_id: "hard-delete-source",
                memory_uuid: "hard-delete-target",
                event_uuid: "hard-delete-event",
                source_role: "evidence",
                confidence_delta: Some(1.0),
            },
        )
        .await
        .unwrap();
    tx.commit().await.unwrap();
    MemoryHabitStorePort::upsert(&store, habit_command(scope.clone(), "hard-delete-habit", 1))
        .await
        .unwrap();

    sqlx::query(
        r#"
        UPDATE ai_habit
        SET promoted_memory_id = (
          SELECT id FROM ai_record WHERE tenant_id = ? AND uuid = ?
        )
        WHERE tenant_id = ? AND uuid = ?
        "#,
    )
    .bind(scope.tenant_id)
    .bind("hard-delete-target")
    .bind(scope.tenant_id)
    .bind("hard-delete-habit")
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_retrieval_trace (
          id, uuid, tenant_id, space_id, query_hash, created_at
        ) VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(910_001_i64)
    .bind("hard-delete-trace")
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("hard-delete-query-hash")
    .bind(now)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_retrieval_hit (
          id, uuid, tenant_id, retrieval_trace_id, memory_id, retriever_name,
          result_rank, explanation_json, status, created_at
        )
        SELECT ?, ?, ?, trace.id, record.id, ?, ?, ?, ?, ?
        FROM ai_retrieval_trace trace
        JOIN ai_record record ON record.tenant_id = trace.tenant_id
        WHERE trace.tenant_id = ? AND trace.uuid = ? AND record.uuid = ?
        "#,
    )
    .bind(910_002_i64)
    .bind("hard-delete-hit")
    .bind(scope.tenant_id)
    .bind("native_sql")
    .bind(1_i64)
    .bind(r#"{"memoryId":"hard-delete-target"}"#)
    .bind("selected")
    .bind(now)
    .bind(scope.tenant_id)
    .bind("hard-delete-trace")
    .bind("hard-delete-target")
    .execute(store.pool())
    .await
    .unwrap();
    for (row_id, entity_id, canonical_name) in [
        (910_003_i64, "hard-delete-source-entity", "source"),
        (910_004_i64, "hard-delete-target-entity", "target"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO ai_entity (
              id, uuid, tenant_id, space_id, entity_type, canonical_name,
              sensitivity_level, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row_id)
        .bind(entity_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind("person")
        .bind(canonical_name)
        .bind("internal")
        .bind("active")
        .bind(now)
        .bind(now)
        .execute(store.pool())
        .await
        .unwrap();
    }
    sqlx::query(
        r#"
        INSERT INTO ai_edge (
          id, uuid, tenant_id, space_id, source_entity_id, target_entity_id,
          relation_type, source_memory_id, status, created_at, updated_at
        )
        SELECT ?, ?, ?, ?, source.id, target.id, ?, record.id, ?, ?, ?
        FROM ai_entity source
        JOIN ai_entity target ON target.tenant_id = source.tenant_id
        JOIN ai_record record ON record.tenant_id = source.tenant_id
        WHERE source.tenant_id = ? AND source.uuid = ?
          AND target.uuid = ? AND record.uuid = ?
        "#,
    )
    .bind(910_005_i64)
    .bind("hard-delete-edge")
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("derived_from")
    .bind("active")
    .bind(now)
    .bind(now)
    .bind(scope.tenant_id)
    .bind("hard-delete-source-entity")
    .bind("hard-delete-target-entity")
    .bind("hard-delete-target")
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_memory_binding (
          id, uuid, tenant_id, binding_kind, source_memory_id, target_memory_id,
          binding_role, status, created_at, updated_at
        )
        SELECT ?, ?, ?, ?, record.id, record.id, ?, ?, ?, ?
        FROM ai_record record
        WHERE record.tenant_id = ? AND record.uuid = ?
        "#,
    )
    .bind(910_006_i64)
    .bind("hard-delete-binding")
    .bind(scope.tenant_id)
    .bind("derived")
    .bind("source")
    .bind("active")
    .bind(now)
    .bind(now)
    .bind(scope.tenant_id)
    .bind("hard-delete-target")
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        UPDATE ai_record
        SET supersedes_memory_id = (
              SELECT id FROM ai_record WHERE tenant_id = ? AND uuid = ?
            ),
            superseded_by_memory_id = (
              SELECT id FROM ai_record WHERE tenant_id = ? AND uuid = ?
            )
        WHERE tenant_id = ? AND uuid = ?
        "#,
    )
    .bind(scope.tenant_id)
    .bind("hard-delete-target")
    .bind(scope.tenant_id)
    .bind("hard-delete-target")
    .bind(scope.tenant_id)
    .bind("hard-delete-sibling")
    .execute(store.pool())
    .await
    .unwrap();

    let outcome = store
        .hard_delete_record_with_cleanup(&scope, "hard-delete-target")
        .await
        .unwrap();
    assert_eq!(
        outcome,
        sdkwork_memory_plugin_native_sql::NativeSqlHardDeleteRecordOutcome {
            deleted: true,
            rejected_candidates: 1,
        }
    );
    assert!(store
        .retrieve_record(&scope, "hard-delete-target")
        .await
        .unwrap()
        .is_none());
    let candidate = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "hard-delete-candidate".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(candidate.decision_state, "rejected");
    assert_eq!(candidate.target_memory_id, None);

    for (table, condition) in [
        ("ai_record_source", "uuid = 'hard-delete-source'"),
        ("ai_record_fts", "memory_uuid = 'hard-delete-target'"),
    ] {
        // `table` and `condition` are the two literal pairs in this loop; no
        // external input reaches the statement, so the audited escape hatch is
        // the correct way to satisfy sqlx 0.9's `SqlSafeStr` bound.
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {table} WHERE {condition}"
        )))
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(count, 0, "{table} must not retain the deleted record");
    }
    let habit_target: Option<i64> = sqlx::query_scalar(
        "SELECT promoted_memory_id FROM ai_habit WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind("hard-delete-habit")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(habit_target, None);
    let hit = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT memory_id FROM ai_retrieval_hit WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind("hard-delete-hit")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(hit, None);
    let hit_status: String =
        sqlx::query_scalar("SELECT status FROM ai_retrieval_hit WHERE tenant_id = ? AND uuid = ?")
            .bind(scope.tenant_id)
            .bind("hard-delete-hit")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(hit_status, "suppressed");
    let edge_target: Option<i64> =
        sqlx::query_scalar("SELECT source_memory_id FROM ai_edge WHERE tenant_id = ? AND uuid = ?")
            .bind(scope.tenant_id)
            .bind("hard-delete-edge")
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(edge_target, None);
    let binding_targets: (Option<i64>, Option<i64>, String) = sqlx::query_as(
        "SELECT source_memory_id, target_memory_id, status FROM ai_memory_binding WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind("hard-delete-binding")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(binding_targets.0, None);
    assert_eq!(binding_targets.1, None);
    assert_eq!(binding_targets.2, "deleted");
    let sibling_links: (Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT supersedes_memory_id, superseded_by_memory_id FROM ai_record WHERE tenant_id = ? AND uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind("hard-delete-sibling")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(sibling_links, (None, None));
    let foreign_key_errors = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(store.pool())
        .await
        .unwrap();
    assert!(
        foreign_key_errors.is_empty(),
        "foreign keys must remain valid"
    );
}

#[tokio::test]
async fn sqlite_hard_delete_rolls_back_dependent_cleanup_when_parent_delete_fails() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .create_record_open_api(
            &scope,
            "hard-delete-rollback-target",
            "user",
            "semantic",
            None,
            None,
            "transaction rollback fixture",
            "transaction rollback fixture",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "hard-delete-rollback-candidate"),
    )
    .await
    .unwrap();
    store
        .set_candidate_target_memory_for_tenant(
            scope.tenant_id,
            "hard-delete-rollback-candidate",
            "hard-delete-rollback-target",
        )
        .await
        .unwrap();
    let fts_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("hard-delete-rollback-target")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fts_before, 1);
    sqlx::query(
        r#"
        CREATE TRIGGER abort_hard_delete_rollback
        BEFORE DELETE ON ai_record
        WHEN OLD.uuid = 'hard-delete-rollback-target'
        BEGIN
          SELECT RAISE(ABORT, 'forced hard delete rollback');
        END
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();

    assert!(store
        .hard_delete_record_with_cleanup(&scope, "hard-delete-rollback-target")
        .await
        .is_err());
    assert!(store
        .retrieve_record(&scope, "hard-delete-rollback-target")
        .await
        .unwrap()
        .is_some());
    let candidate = MemoryCandidateStorePort::retrieve_detail(
        &store,
        RetrieveMemoryCandidateDetailQuery {
            tenant_id: scope.tenant_id,
            candidate_id: "hard-delete-rollback-candidate".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(candidate.decision_state, "pending");
    assert_eq!(
        candidate.target_memory_id.as_deref(),
        Some("hard-delete-rollback-target")
    );
    let fts_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("hard-delete-rollback-target")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fts_after, 1);
}

#[tokio::test]
async fn sqlite_user_space_forget_preserves_other_users_records_and_events() {
    let store = new_contract_store().await;
    let user_scope = MemoryScopeContext {
        tenant_id: 1,
        space_id: 1,
        organization_id: None,
        user_id: Some(101),
    };
    let other_user_scope = MemoryScopeContext {
        tenant_id: 1,
        space_id: 1,
        organization_id: None,
        user_id: Some(202),
    };
    for (scope, memory_id) in [
        (user_scope.clone(), "forget-user-memory"),
        (other_user_scope.clone(), "forget-other-memory"),
    ] {
        store
            .create_record_open_api(
                &scope,
                memory_id,
                "user",
                "semantic",
                None,
                None,
                "user forget fixture",
                "user forget fixture",
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }
    for (scope, event_id) in [
        (user_scope.clone(), "forget-user-event"),
        (other_user_scope.clone(), "forget-other-event"),
    ] {
        MemoryEventStorePort::append(
            &store,
            AppendMemoryEventCommand {
                scope,
                event_id: event_id.to_string(),
                content: "user forget event fixture".to_string(),
            },
        )
        .await
        .unwrap();
    }

    let stats = store
        .forget_records_for_user(1, 101, Some(1))
        .await
        .unwrap();
    assert_eq!(stats.deleted_records, 1);
    assert_eq!(stats.purged_events, 1);
    assert!(store
        .retrieve_record(&user_scope, "forget-user-memory")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .retrieve_record(&other_user_scope, "forget-other-memory")
        .await
        .unwrap()
        .is_some());
    assert!(MemoryEventStorePort::retrieve(
        &store,
        RetrieveMemoryEventQuery {
            scope: user_scope,
            event_id: "forget-user-event".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryEventStorePort::retrieve(
        &store,
        RetrieveMemoryEventQuery {
            scope: other_user_scope,
            event_id: "forget-other-event".to_string(),
        },
    )
    .await
    .unwrap()
    .is_some());
}

#[tokio::test]
async fn sqlite_candidate_promotion_journal_is_transactional_and_retry_idempotent() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "candidate-journal"),
    )
    .await
    .unwrap();

    let first = MemoryCandidateStorePort::promote_atomic_with_quota_and_journal(
        &store,
        PromoteMemoryCandidateAtomicWithJournalCommand {
            promotion: sdkwork_memory_spi::PromoteMemoryCandidateAtomicCommand {
                scope: scope.clone(),
                candidate_id: "candidate-journal".to_string(),
                memory_id: "journal-memory".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "journaled promotion".to_string(),
                evidence_links: Vec::new(),
                decided_by: Some(7),
            },
            journal: mutation_journal("journal-memory", "candidate-journal"),
        },
        1,
    )
    .await
    .unwrap();
    assert!(matches!(
        first,
        MemoryRecordQuotaAdmission::Admitted(ref promotion)
            if promotion.memory_id == "journal-memory"
    ));

    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND aggregate_id = ?",
    )
    .bind(1_i64)
    .bind("journal-memory")
    .fetch_one(store.pool())
    .await
    .unwrap();
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND resource_id = ?",
    )
    .bind(1_i64)
    .bind("journal-memory")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(outbox_count, 1);
    assert_eq!(audit_count, 1);

    let retry = MemoryCandidateStorePort::promote_atomic_with_quota_and_journal(
        &store,
        PromoteMemoryCandidateAtomicWithJournalCommand {
            promotion: sdkwork_memory_spi::PromoteMemoryCandidateAtomicCommand {
                scope,
                candidate_id: "candidate-journal".to_string(),
                memory_id: "different-retry-memory".to_string(),
                memory_type: "semantic".to_string(),
                proposed_text: "different retry payload".to_string(),
                evidence_links: Vec::new(),
                decided_by: Some(8),
            },
            journal: mutation_journal("different-retry-memory", "candidate-journal-retry"),
        },
        1,
    )
    .await
    .unwrap();
    assert!(matches!(
        retry,
        MemoryRecordQuotaAdmission::Admitted(ref promotion)
            if promotion.memory_id == "journal-memory"
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND aggregate_id = ?",
        )
        .bind(1_i64)
        .bind("journal-memory")
        .fetch_one(store.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND resource_id = ?",
        )
        .bind(1_i64)
        .bind("journal-memory")
        .fetch_one(store.pool())
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn sqlite_retriever_port_applies_scope_type_and_sensitivity_before_limit() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let other_space = MemoryScopeContext::for_test(1, 2);
    let other_tenant = MemoryScopeContext::for_test(2, 3);

    create_canonical_fixture(
        &store,
        &scope,
        "999-allowed",
        "semantic",
        "needle allowed memory",
        "internal",
    )
    .await;
    create_canonical_fixture(
        &store,
        &scope,
        "000-sensitive",
        "semantic",
        "needle sensitive memory",
        "sensitive",
    )
    .await;
    create_canonical_fixture(
        &store,
        &scope,
        "001-wrong-type",
        "episodic",
        "needle episodic memory",
        "internal",
    )
    .await;
    create_canonical_fixture(
        &store,
        &other_space,
        "002-other-space",
        "semantic",
        "needle other space",
        "internal",
    )
    .await;
    create_canonical_fixture(
        &store,
        &other_tenant,
        "003-other-tenant",
        "semantic",
        "needle other tenant",
        "internal",
    )
    .await;

    let public_result = MemoryRetrieverPort::search_scoped(
        &store,
        SearchMemoryCandidatesQuery {
            scope: scope.clone(),
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
    .expect("public filtered retrieval must succeed");
    assert_eq!(public_result.records.len() + public_result.events.len(), 1);
    assert_eq!(public_result.records[0].memory_id, "999-allowed");

    let owner_result = MemoryRetrieverPort::search_scoped(
        &store,
        SearchMemoryCandidatesQuery {
            scope,
            query: "needle".to_string(),
            limit: 10,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword, MemoryRetrieverKind::Vector],
            memory_types: vec!["semantic".to_string()],
            read_scope: MemorySensitivityReadScope::Owner,
            metadata_filter: None,
            include_expired: false,
        },
    )
    .await
    .expect("owner retrieval with optional unsupported kind must degrade");
    let owner_ids = owner_result
        .records
        .iter()
        .map(|candidate| candidate.memory_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(owner_ids, vec!["000-sensitive", "999-allowed"]);
    assert!(owner_result.degraded);
    assert_eq!(
        owner_result.unavailable_retriever_kinds,
        vec![MemoryRetrieverKind::Vector]
    );
    assert!(owner_result.records.len() + owner_result.events.len() <= 10);

    for invalid_limit in [0, MAX_MEMORY_RETRIEVAL_CANDIDATES + 1] {
        let error = MemoryRetrieverPort::search_scoped(
            &store,
            SearchMemoryCandidatesQuery {
                scope: MemoryScopeContext::for_test(1, 1),
                query: "needle".to_string(),
                limit: invalid_limit,
                retriever_kinds: vec![MemoryRetrieverKind::Keyword],
                memory_types: Vec::new(),
                read_scope: MemorySensitivityReadScope::Owner,
                metadata_filter: None,
                include_expired: false,
            },
        )
        .await
        .expect_err("out-of-range candidate limits must fail closed");
        assert!(error.to_string().contains("candidate limit"));
    }
}

#[tokio::test]
async fn sqlite_retriever_marks_known_fulltext_unavailability_as_degraded() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    create_canonical_fixture(
        &store,
        &scope,
        "fts-fallback-memory",
        "semantic",
        "recoverable fulltext fallback needle",
        "internal",
    )
    .await;
    sqlx::query("DROP TABLE ai_record_fts")
        .execute(store.pool())
        .await
        .unwrap();

    let result = MemoryRetrieverPort::search_scoped(
        &store,
        SearchMemoryCandidatesQuery {
            scope,
            query: "fallback needle".to_string(),
            limit: 5,
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            memory_types: vec!["semantic".to_string()],
            read_scope: MemorySensitivityReadScope::Public,
            metadata_filter: None,
            include_expired: false,
        },
    )
    .await
    .expect("missing optional FTS state must use bounded LIKE fallback");

    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].memory_id, "fts-fallback-memory");
    assert!(result.degraded);
    assert_eq!(result.degradation_codes, vec!["fulltext_fallback"]);
}

#[tokio::test]
async fn sqlite_event_retriever_returns_linked_canonical_memory_and_respects_total_limit() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    create_canonical_fixture(
        &store,
        &scope,
        "event-memory",
        "semantic",
        "canonical text does not contain the event term",
        "internal",
    )
    .await;
    store
        .append_open_api_event(
            &scope,
            "event-source",
            "conversation.observed",
            "conversation",
            "2026-07-12T00:00:00Z",
            &serde_json::json!({ "content": "linked-event-needle" }),
            "internal",
        )
        .await
        .unwrap();
    store
        .append_record_source_for_tenant(
            scope.tenant_id,
            "event-source-link",
            "event-memory",
            "event-source",
            "supporting",
            Some(0.2),
        )
        .await
        .unwrap();

    let result = MemoryRetrieverPort::search_scoped(
        &store,
        SearchMemoryCandidatesQuery {
            scope,
            query: "linked-event-needle".to_string(),
            limit: 1,
            retriever_kinds: vec![MemoryRetrieverKind::Event],
            memory_types: vec!["semantic".to_string()],
            read_scope: MemorySensitivityReadScope::Public,
            metadata_filter: None,
            include_expired: false,
        },
    )
    .await
    .unwrap();
    assert!(result.records.is_empty());
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].memory_id, "event-memory");
    assert_eq!(result.records.len() + result.events.len(), 1);
}

#[tokio::test]
async fn sqlite_store_rejects_duplicate_owner_space_type() {
    let store = new_contract_store().await;
    let err = store
        .create_space_record(
            1,
            99,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "tenant".to_string(),
                owner_subject_id: "1".to_string(),
                space_type: "workspace".to_string(),
                display_name: "Duplicate workspace".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, NativeSqlStoreError::Database(_)),
        "expected unique constraint violation, got {err:?}"
    );
}

#[tokio::test]
async fn sqlite_store_applies_phase1_migration_and_round_trips_event_and_record() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_event(&scope, "evt-1", "User prefers concise answers")
        .await
        .unwrap();
    store
        .create_record(&scope, "rec-1", "answer_style", "concise")
        .await
        .unwrap();

    let event = store
        .retrieve_event(&scope, "evt-1")
        .await
        .unwrap()
        .unwrap();
    let record = store
        .retrieve_record(&scope, "rec-1")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(event.event_id, "evt-1");
    assert_eq!(event.content, "User prefers concise answers");
    assert_eq!(record.memory_id, "rec-1");
    assert_eq!(record.content, "concise");
}

#[tokio::test]
async fn sqlite_store_preserves_event_content_with_json_sensitive_characters() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    let content = r#"User said "use C:\sdkwork\memory" for local tests"#;
    store
        .append_event(&scope, "evt-json", content)
        .await
        .unwrap();

    let event = store
        .retrieve_event(&scope, "evt-json")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(event.content, content);
}

#[tokio::test]
async fn sqlite_store_reads_event_payload_as_structured_json() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    let content = "line one\nline two";
    store
        .append_event(&scope, "evt-payload", content)
        .await
        .unwrap();

    let payload = store
        .retrieve_event_payload(&scope, "evt-payload")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(payload["content"].as_str(), Some(content));
}

#[tokio::test]
async fn sqlite_store_implements_record_and_event_store_spi_ports() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    let event = MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope: scope.clone(),
            event_id: "evt-spi".to_string(),
            content: "SPI event payload".to_string(),
        },
    )
    .await
    .unwrap();
    let record = MemoryRecordStorePort::create(
        &store,
        CreateMemoryRecordCommand {
            scope: scope.clone(),
            memory_id: "rec-spi".to_string(),
            content: "SPI record payload".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(event.event_id, "evt-spi");
    assert_eq!(event.content, "SPI event payload");
    assert_eq!(record.memory_id, "rec-spi");
    assert_eq!(record.content, "SPI record payload");
}

#[tokio::test]
async fn sqlite_store_keeps_records_and_events_isolated_by_tenant_and_space() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);
    let wrong_space = MemoryScopeContext::for_test(1, 2);

    store
        .append_event(&tenant_one, "evt-shared", "tenant one event")
        .await
        .unwrap();
    store
        .append_event(&tenant_two, "evt-shared", "tenant two event")
        .await
        .unwrap();
    store
        .create_record(&tenant_one, "rec-shared", "preference", "tenant one record")
        .await
        .unwrap();
    store
        .create_record(&tenant_two, "rec-shared", "preference", "tenant two record")
        .await
        .unwrap();

    let tenant_one_event = store
        .retrieve_event(&tenant_one, "evt-shared")
        .await
        .unwrap()
        .unwrap();
    let tenant_two_event = store
        .retrieve_event(&tenant_two, "evt-shared")
        .await
        .unwrap()
        .unwrap();
    let tenant_one_record = store
        .retrieve_record(&tenant_one, "rec-shared")
        .await
        .unwrap()
        .unwrap();
    let tenant_two_record = store
        .retrieve_record(&tenant_two, "rec-shared")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(tenant_one_event.content, "tenant one event");
    assert_eq!(tenant_two_event.content, "tenant two event");
    assert_eq!(tenant_one_record.content, "tenant one record");
    assert_eq!(tenant_two_record.content, "tenant two record");
    assert!(store
        .retrieve_event(&wrong_space, "evt-shared")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .retrieve_record(&wrong_space, "rec-shared")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_store_spi_retrieve_methods_require_matching_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);

    MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope: tenant_one.clone(),
            event_id: "evt-spi-scoped".to_string(),
            content: "tenant one event".to_string(),
        },
    )
    .await
    .unwrap();
    MemoryRecordStorePort::create(
        &store,
        CreateMemoryRecordCommand {
            scope: tenant_one.clone(),
            memory_id: "rec-spi-scoped".to_string(),
            content: "tenant one record".to_string(),
        },
    )
    .await
    .unwrap();

    assert!(MemoryEventStorePort::retrieve(
        &store,
        RetrieveMemoryEventQuery {
            scope: tenant_two.clone(),
            event_id: "evt-spi-scoped".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryRecordStorePort::retrieve(
        &store,
        RetrieveMemoryRecordQuery {
            scope: tenant_two,
            memory_id: "rec-spi-scoped".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn sqlite_store_soft_deletes_records_and_suppresses_retrieve() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .create_record(&scope, "rec-delete", "preference", "delete me")
        .await
        .unwrap();

    let receipt = store
        .mark_record_deleted(&scope, "rec-delete")
        .await
        .unwrap();
    let retrieved = store.retrieve_record(&scope, "rec-delete").await.unwrap();
    let lifecycle = store
        .retrieve_record_lifecycle(&scope, "rec-delete")
        .await
        .unwrap()
        .unwrap();

    assert!(receipt.deleted);
    assert!(!receipt.already_deleted);
    assert!(retrieved.is_none());
    assert_eq!(lifecycle.memory_id, "rec-delete");
    assert_eq!(lifecycle.status, "deleted");
    assert_utc_timestamp(lifecycle.deleted_at.as_deref());
}

#[tokio::test]
async fn sqlite_store_record_delete_is_idempotent_for_already_deleted_records() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .create_record(&scope, "rec-delete-repeat", "preference", "delete me")
        .await
        .unwrap();

    let first = store
        .mark_record_deleted(&scope, "rec-delete-repeat")
        .await
        .unwrap();
    let second = store
        .mark_record_deleted(&scope, "rec-delete-repeat")
        .await
        .unwrap();

    assert!(first.deleted);
    assert!(!first.already_deleted);
    assert!(second.deleted);
    assert!(second.already_deleted);
}

#[tokio::test]
async fn sqlite_store_record_delete_does_not_cross_tenant_or_space_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);
    let wrong_space = MemoryScopeContext::for_test(1, 2);

    store
        .create_record(&tenant_one, "rec-delete-scoped", "preference", "tenant one")
        .await
        .unwrap();
    store
        .create_record(&tenant_two, "rec-delete-scoped", "preference", "tenant two")
        .await
        .unwrap();

    let missing = store
        .mark_record_deleted(&wrong_space, "rec-delete-scoped")
        .await
        .unwrap();
    let deleted = store
        .mark_record_deleted(&tenant_one, "rec-delete-scoped")
        .await
        .unwrap();
    let tenant_two_record = store
        .retrieve_record(&tenant_two, "rec-delete-scoped")
        .await
        .unwrap()
        .unwrap();

    assert!(!missing.deleted);
    assert!(deleted.deleted);
    assert!(store
        .retrieve_record(&tenant_one, "rec-delete-scoped")
        .await
        .unwrap()
        .is_none());
    assert_eq!(tenant_two_record.content, "tenant two");
}

#[tokio::test]
async fn sqlite_store_implements_record_delete_spi_port() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryRecordStorePort::create(
        &store,
        CreateMemoryRecordCommand {
            scope: scope.clone(),
            memory_id: "rec-spi-delete".to_string(),
            content: "SPI delete payload".to_string(),
        },
    )
    .await
    .unwrap();

    let receipt = MemoryRecordStorePort::mark_deleted(
        &store,
        DeleteMemoryRecordCommand {
            scope: scope.clone(),
            memory_id: "rec-spi-delete".to_string(),
        },
    )
    .await
    .unwrap();
    let retrieved = MemoryRecordStorePort::retrieve(
        &store,
        RetrieveMemoryRecordQuery {
            scope,
            memory_id: "rec-spi-delete".to_string(),
        },
    )
    .await
    .unwrap();

    assert_eq!(receipt.memory_id, "rec-spi-delete");
    assert!(receipt.deleted);
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn sqlite_store_event_append_is_idempotent_for_same_scope_event_and_content() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_event(&scope, "evt-idempotent", "same content")
        .await
        .unwrap();
    store
        .append_event(&scope, "evt-idempotent", "same content")
        .await
        .unwrap();

    let event = store
        .retrieve_event(&scope, "evt-idempotent")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(event.content, "same content");
}

#[tokio::test]
async fn sqlite_store_event_append_rejects_same_scope_event_with_different_content() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_event(&scope, "evt-conflict", "alpha")
        .await
        .unwrap();
    let err = store
        .append_event(&scope, "evt-conflict", "omega")
        .await
        .unwrap_err();

    assert!(matches!(err, NativeSqlStoreError::EventConflict { .. }));
}

#[tokio::test]
async fn sqlite_store_event_append_rejects_same_tenant_event_reuse_in_different_space() {
    let store = new_contract_store().await;
    let first_space = MemoryScopeContext::for_test(1, 1);
    let second_space = MemoryScopeContext::for_test(1, 2);

    store
        .append_event(&first_space, "evt-space-conflict", "same content")
        .await
        .unwrap();
    let err = store
        .append_event(&second_space, "evt-space-conflict", "same content")
        .await
        .unwrap_err();

    assert!(matches!(err, NativeSqlStoreError::EventConflict { .. }));
    assert!(store
        .retrieve_event(&second_space, "evt-space-conflict")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_store_spi_event_append_maps_idempotency_conflict_to_spi_conflict() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope: scope.clone(),
            event_id: "evt-spi-conflict".to_string(),
            content: "alpha".to_string(),
        },
    )
    .await
    .unwrap();
    let err = MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope,
            event_id: "evt-spi-conflict".to_string(),
            content: "omega".to_string(),
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(err, MemorySpiError::IdempotencyConflict { .. }));
}

#[tokio::test]
async fn sqlite_store_appends_and_retrieves_audit_records_by_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);

    store
        .append_audit(
            &tenant_one,
            "aud-shared",
            "memory.record.created",
            "ai_record",
            "rec-1",
            "success",
        )
        .await
        .unwrap();
    store
        .append_audit(
            &tenant_two,
            "aud-shared",
            "memory.record.created",
            "ai_record",
            "rec-2",
            "success",
        )
        .await
        .unwrap();

    let tenant_one_audit = store
        .retrieve_audit(&tenant_one, "aud-shared")
        .await
        .unwrap()
        .unwrap();
    let tenant_two_audit = store
        .retrieve_audit(&tenant_two, "aud-shared")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(tenant_one_audit.action, "memory.record.created");
    assert_eq!(tenant_one_audit.resource_id, "rec-1");
    assert_eq!(tenant_two_audit.resource_id, "rec-2");
    assert!(store
        .retrieve_audit(&MemoryScopeContext::for_test(3, 3), "aud-shared")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_store_implements_audit_store_spi_port() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    let audit = MemoryAuditStorePort::append(
        &store,
        AppendMemoryAuditCommand {
            scope: scope.clone(),
            audit_id: "aud-spi".to_string(),
            action: "memory.event.appended".to_string(),
            resource_type: "ai_event".to_string(),
            resource_id: "evt-spi".to_string(),
            result: "success".to_string(),
        },
    )
    .await
    .unwrap();
    let retrieved = MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope,
            audit_id: "aud-spi".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(audit.audit_id, "aud-spi");
    assert_eq!(retrieved.action, "memory.event.appended");
    assert_eq!(retrieved.resource_type, "ai_event");
    assert_eq!(retrieved.resource_id, "evt-spi");
    assert_eq!(retrieved.result, "success");
}

#[tokio::test]
async fn sqlite_store_appends_and_retrieves_outbox_events_by_tenant_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);

    store
        .append_outbox_event(outbox_command(
            &tenant_one,
            "out-shared",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    store
        .append_outbox_event(outbox_command(
            &tenant_two,
            "out-shared",
            "rec-2",
            r#"{"memoryId":"rec-2"}"#,
        ))
        .await
        .unwrap();

    let tenant_one_outbox = store
        .retrieve_outbox_event(&tenant_one, "out-shared")
        .await
        .unwrap()
        .unwrap();
    let tenant_two_outbox = store
        .retrieve_outbox_event(&tenant_two, "out-shared")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(tenant_one_outbox.aggregate_id, "rec-1");
    assert_eq!(tenant_one_outbox.publish_state, "pending");
    assert_eq!(tenant_one_outbox.retry_count, 0);
    assert_eq!(tenant_two_outbox.aggregate_id, "rec-2");
    assert!(store
        .retrieve_outbox_event(&MemoryScopeContext::for_test(3, 3), "out-shared")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn sqlite_store_outbox_append_is_idempotent_for_same_tenant_event_and_payload() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-idempotent",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-idempotent",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();

    let outbox = store
        .retrieve_outbox_event(&scope, "out-idempotent")
        .await
        .unwrap()
        .unwrap();

    assert_eq!(outbox.aggregate_id, "rec-1");
}

#[tokio::test]
async fn sqlite_store_outbox_append_rejects_same_tenant_event_with_different_payload() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-conflict",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    let err = store
        .append_outbox_event(outbox_command(
            &scope,
            "out-conflict",
            "rec-1",
            r#"{"memoryId":"rec-other"}"#,
        ))
        .await
        .unwrap_err();

    assert!(matches!(err, NativeSqlStoreError::OutboxConflict { .. }));
}

#[tokio::test]
async fn sqlite_store_implements_outbox_store_spi_port() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    let outbox = MemoryOutboxStorePort::append(
        &store,
        AppendMemoryOutboxCommand {
            scope: scope.clone(),
            outbox_id: "out-spi".to_string(),
            aggregate_type: "ai_event".to_string(),
            aggregate_id: "evt-spi".to_string(),
            event_type: "memory.event.appended".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"eventId":"evt-spi"}"#.to_string(),
        },
    )
    .await
    .unwrap();
    let retrieved = MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope,
            outbox_id: "out-spi".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(outbox.outbox_id, "out-spi");
    assert_eq!(retrieved.aggregate_type, "ai_event");
    assert_eq!(retrieved.aggregate_id, "evt-spi");
    assert_eq!(retrieved.event_type, "memory.event.appended");
    assert_eq!(retrieved.event_version, "1");
    assert_eq!(retrieved.payload_json, r#"{"eventId":"evt-spi"}"#);
    assert_eq!(retrieved.publish_state, "pending");
}

#[tokio::test]
async fn sqlite_store_spi_outbox_append_maps_idempotency_conflict_to_spi_conflict() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryOutboxStorePort::append(
        &store,
        AppendMemoryOutboxCommand {
            scope: scope.clone(),
            outbox_id: "out-spi-conflict".to_string(),
            aggregate_type: "ai_record".to_string(),
            aggregate_id: "rec-1".to_string(),
            event_type: "memory.record.created".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"memoryId":"rec-1"}"#.to_string(),
        },
    )
    .await
    .unwrap();
    let err = MemoryOutboxStorePort::append(
        &store,
        AppendMemoryOutboxCommand {
            scope,
            outbox_id: "out-spi-conflict".to_string(),
            aggregate_type: "ai_record".to_string(),
            aggregate_id: "rec-1".to_string(),
            event_type: "memory.record.created".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"memoryId":"rec-other"}"#.to_string(),
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(err, MemorySpiError::IdempotencyConflict { .. }));
}

#[tokio::test]
async fn sqlite_store_lists_pending_outbox_events_by_tenant_scope_and_limit() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);

    store
        .append_outbox_event(outbox_command(
            &tenant_one,
            "out-pending-1",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    store
        .append_outbox_event(outbox_command(
            &tenant_one,
            "out-pending-2",
            "rec-2",
            r#"{"memoryId":"rec-2"}"#,
        ))
        .await
        .unwrap();
    store
        .append_outbox_event(outbox_command(
            &tenant_two,
            "out-pending-tenant-two",
            "rec-3",
            r#"{"memoryId":"rec-3"}"#,
        ))
        .await
        .unwrap();

    let pending = store
        .list_pending_outbox_events(&tenant_one, 1)
        .await
        .unwrap();

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].outbox_id, "out-pending-1");
    assert_eq!(pending[0].publish_state, "pending");
}

#[tokio::test]
async fn sqlite_store_marks_outbox_published_and_excludes_it_from_pending() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-publish",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();

    // Publishing is lease-fenced: claim first so this caller owns the event.
    let claimed = store
        .claim_global_pending_outbox_events(1, "publisher-a", "lease-a", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    let published = store
        .mark_outbox_published(&scope, "out-publish", "publisher-a", "lease-a")
        .await
        .unwrap()
        .unwrap();
    let retrieved = store
        .retrieve_outbox_event(&scope, "out-publish")
        .await
        .unwrap()
        .unwrap();
    let pending = store.list_pending_outbox_events(&scope, 10).await.unwrap();

    assert_eq!(published.publish_state, "published");
    assert_utc_timestamp(published.published_at.as_deref());
    assert_eq!(retrieved.publish_state, "published");
    assert!(pending.is_empty());
}

#[tokio::test]
async fn sqlite_store_claim_global_pending_outbox_events_publishes_once() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-claim-1",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-claim-2",
            "rec-2",
            r#"{"memoryId":"rec-2"}"#,
        ))
        .await
        .unwrap();

    let first = store
        .claim_global_pending_outbox_events(10, "publisher-a", "lease-a", 30)
        .await
        .unwrap();
    assert_eq!(first.len(), 2);
    assert!(first
        .iter()
        .all(|row| row.outbox.publish_state == "processing"));

    let second = store
        .claim_global_pending_outbox_events(10, "publisher-b", "lease-b", 30)
        .await
        .unwrap();
    assert!(second.is_empty());

    let fenced = store
        .ack_outbox_delivery_success(1, "out-claim-1", "publisher-a", "wrong-token")
        .await
        .unwrap();
    assert!(
        fenced.is_none(),
        "a stale lease token must not acknowledge a row"
    );

    for row in &first {
        let published = store
            .ack_outbox_delivery_success(
                row.tenant_id,
                &row.outbox.outbox_id,
                row.lease_owner.as_deref().unwrap(),
                row.lease_token.as_deref().unwrap(),
            )
            .await
            .unwrap()
            .expect("ack must return published row");
        assert_eq!(published.publish_state, "published");
        assert!(published.published_at.is_some());
    }

    let pending = store.list_pending_outbox_events(&scope, 10).await.unwrap();
    assert!(pending.is_empty());
}

#[tokio::test]
async fn sqlite_store_outbox_delivery_failure_requeues_until_max_retries() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-retry",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();

    let claimed = store
        .claim_global_pending_outbox_events(10, "publisher-a", "lease-1", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].outbox.publish_state, "processing");

    let first_failure = store
        .record_outbox_delivery_failure(1, "out-retry", "publisher-a", "lease-1", 3)
        .await
        .unwrap()
        .expect("first failure row");
    assert_eq!(first_failure.publish_state, "pending");
    assert_eq!(first_failure.retry_count, 1);

    // Exponential backoff prevents immediate re-claim after failure.
    // Fast-forward the explicit retry timestamp so the backoff window has elapsed.
    sqlx::query("UPDATE ai_outbox_event SET next_attempt_at = '1970-01-01T00:00:00.000Z' WHERE uuid = 'out-retry'")
        .execute(store.pool())
        .await
        .unwrap();

    let reclaimed = store
        .claim_global_pending_outbox_events(10, "publisher-a", "lease-2", 30)
        .await
        .unwrap();
    assert_eq!(reclaimed.len(), 1);
    let terminal_failure = store
        .record_outbox_delivery_failure(1, "out-retry", "publisher-a", "lease-2", 3)
        .await
        .unwrap()
        .expect("second failure row");
    assert_eq!(terminal_failure.publish_state, "pending");
    assert_eq!(terminal_failure.retry_count, 2);

    // Fast-forward again for the second retry's backoff window.
    sqlx::query("UPDATE ai_outbox_event SET next_attempt_at = '1970-01-01T00:00:00.000Z' WHERE uuid = 'out-retry'")
        .execute(store.pool())
        .await
        .unwrap();

    store
        .claim_global_pending_outbox_events(10, "publisher-a", "lease-3", 30)
        .await
        .unwrap();
    let failed = store
        .record_outbox_delivery_failure(1, "out-retry", "publisher-a", "lease-3", 3)
        .await
        .unwrap()
        .expect("terminal failure row");
    assert_eq!(failed.publish_state, "failed");
    assert_eq!(failed.retry_count, 3);
}

#[tokio::test]
async fn sqlite_store_marks_outbox_failed_increments_retry_and_excludes_it_from_pending() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-fail",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();

    // Failure marking is lease-fenced too: claim first.
    let claimed_for_failure = store
        .claim_global_pending_outbox_events(1, "publisher-a", "lease-a", 30)
        .await
        .unwrap();
    assert_eq!(claimed_for_failure.len(), 1);
    // max_retries = 1: the first charged attempt already reaches the ceiling,
    // so the event lands terminally 'failed' (the SPI mark_failed shape).
    let failed = store
        .mark_outbox_failed(&scope, "out-fail", "publisher-a", "lease-a", 1)
        .await
        .unwrap()
        .unwrap();
    let pending = store.list_pending_outbox_events(&scope, 10).await.unwrap();

    assert_eq!(failed.publish_state, "failed");
    assert_eq!(failed.retry_count, 1);
    assert!(failed.published_at.is_none());
    assert!(pending.is_empty());
}

/// B10 parity: `mark_outbox_failed` follows the same retry ladder as
/// `record_outbox_delivery_failure` — attempts below the ceiling return the
/// event to 'pending' with a backoff window instead of a terminal failure.
#[tokio::test]
async fn sqlite_store_marks_outbox_failed_requeues_below_the_retry_ceiling() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .append_outbox_event(outbox_command(
            &scope,
            "out-fail-retry",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    store
        .claim_global_pending_outbox_events(1, "publisher-a", "lease-a", 30)
        .await
        .unwrap();

    // Ceiling not reached: the event goes back to 'pending' with a future
    // next_attempt_at and its lease cleared, exactly like the delivery path.
    let requeued = store
        .mark_outbox_failed(&scope, "out-fail-retry", "publisher-a", "lease-a", 3)
        .await
        .unwrap()
        .expect("requeued failure row");
    assert_eq!(requeued.publish_state, "pending");
    assert_eq!(requeued.retry_count, 1);

    let next_attempt_at: Option<String> = sqlx::query_scalar(
        "SELECT next_attempt_at FROM ai_outbox_event WHERE uuid = 'out-fail-retry'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(
        next_attempt_at.is_some(),
        "a below-ceiling failure must schedule an exponential-backoff retry"
    );

    // The lease was cleared, so a fenced worker cannot charge a second time.
    let fenced = store
        .mark_outbox_failed(&scope, "out-fail-retry", "publisher-a", "lease-a", 3)
        .await
        .unwrap();
    assert!(fenced.is_none(), "a stale lease must fence the failure mark");
}

#[tokio::test]
async fn sqlite_store_outbox_delivery_lifecycle_does_not_cross_tenant_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);
    let missing_tenant = MemoryScopeContext::for_test(3, 3);

    store
        .append_outbox_event(outbox_command(
            &tenant_one,
            "out-scoped",
            "rec-1",
            r#"{"memoryId":"rec-1"}"#,
        ))
        .await
        .unwrap();
    store
        .append_outbox_event(outbox_command(
            &tenant_two,
            "out-scoped",
            "rec-2",
            r#"{"memoryId":"rec-2"}"#,
        ))
        .await
        .unwrap();

    // Claim under tenant one, then prove the fence blocks both a wrong
    // tenant scope and a wrong tenant's lease from publishing the event.
    let claimed = store
        .claim_global_pending_outbox_events(1, "publisher-a", "lease-a", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    let missing = store
        .mark_outbox_published(&missing_tenant, "out-scoped", "publisher-a", "lease-a")
        .await
        .unwrap();
    assert!(missing.is_none(), "wrong tenant must not publish");
    let tenant_one_published = store
        .mark_outbox_published(&tenant_one, "out-scoped", "publisher-a", "lease-a")
        .await
        .unwrap()
        .unwrap();
    let tenant_two_pending = store
        .list_pending_outbox_events(&tenant_two, 10)
        .await
        .unwrap();

    assert_eq!(tenant_one_published.publish_state, "published");
    assert_eq!(tenant_two_pending.len(), 1);
    assert_eq!(tenant_two_pending[0].aggregate_id, "rec-2");
}

#[tokio::test]
async fn sqlite_store_implements_outbox_delivery_lifecycle_spi_port() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryOutboxStorePort::append(
        &store,
        AppendMemoryOutboxCommand {
            scope: scope.clone(),
            outbox_id: "out-spi-lifecycle".to_string(),
            aggregate_type: "ai_record".to_string(),
            aggregate_id: "rec-1".to_string(),
            event_type: "memory.record.created".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"memoryId":"rec-1"}"#.to_string(),
        },
    )
    .await
    .unwrap();

    let pending = MemoryOutboxStorePort::list_pending(
        &store,
        ListPendingMemoryOutboxQuery {
            scope: scope.clone(),
            limit: 10,
        },
    )
    .await
    .unwrap();
    // The mark is lease-fenced: claim first so this caller owns the event.
    let claimed = store
        .claim_global_pending_outbox_events(1, "spi-worker", "spi-lease-a", 30)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 1);
    let published = MemoryOutboxStorePort::mark_published(
        &store,
        MarkMemoryOutboxPublishedCommand {
            scope: scope.clone(),
            outbox_id: "out-spi-lifecycle".to_string(),
            lease_owner: "spi-worker".to_string(),
            lease_token: "spi-lease-a".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let pending_after_publish = MemoryOutboxStorePort::list_pending(
        &store,
        ListPendingMemoryOutboxQuery {
            scope: scope.clone(),
            limit: 10,
        },
    )
    .await
    .unwrap();

    MemoryOutboxStorePort::append(
        &store,
        AppendMemoryOutboxCommand {
            scope: scope.clone(),
            outbox_id: "out-spi-failed".to_string(),
            aggregate_type: "ai_event".to_string(),
            aggregate_id: "evt-1".to_string(),
            event_type: "memory.event.appended".to_string(),
            event_version: "1".to_string(),
            payload_json: r#"{"eventId":"evt-1"}"#.to_string(),
        },
    )
    .await
    .unwrap();
    let claimed_for_failure = store
        .claim_global_pending_outbox_events(1, "spi-worker", "spi-lease-b", 30)
        .await
        .unwrap();
    assert_eq!(claimed_for_failure.len(), 1);
    let failed = MemoryOutboxStorePort::mark_failed(
        &store,
        MarkMemoryOutboxFailedCommand {
            scope,
            outbox_id: "out-spi-failed".to_string(),
            lease_owner: "spi-worker".to_string(),
            lease_token: "spi-lease-b".to_string(),
            max_retries: 1,
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].outbox_id, "out-spi-lifecycle");
    assert_eq!(published.publish_state, "published");
    assert_utc_timestamp(published.published_at.as_deref());
    assert!(pending_after_publish.is_empty());
    assert_eq!(failed.publish_state, "failed");
    assert_eq!(failed.retry_count, 1);
}

#[tokio::test]
async fn sqlite_store_creates_and_decides_candidates_by_tenant_and_space_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);
    let wrong_space = MemoryScopeContext::for_test(1, 2);

    let tenant_one_candidate = MemoryCandidateStorePort::create(
        &store,
        candidate_command(tenant_one.clone(), "cand-shared"),
    )
    .await
    .unwrap();
    let tenant_two_candidate = MemoryCandidateStorePort::create(
        &store,
        candidate_command(tenant_two.clone(), "cand-shared"),
    )
    .await
    .unwrap();

    let approved = MemoryCandidateStorePort::approve(
        &store,
        ApproveMemoryCandidateCommand {
            scope: tenant_one.clone(),
            candidate_id: "cand-shared".to_string(),
            decision_reason: Some("confirmed by user".to_string()),
            decided_by: Some(7),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let rejected = MemoryCandidateStorePort::reject(
        &store,
        RejectMemoryCandidateCommand {
            scope: tenant_two.clone(),
            candidate_id: "cand-shared".to_string(),
            decision_reason: Some("stale signal".to_string()),
            decided_by: Some(8),
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(tenant_one_candidate.decision_state, "pending");
    assert_eq!(tenant_two_candidate.decision_state, "pending");
    assert_eq!(approved.decision_state, "approved");
    assert_eq!(
        approved.decision_reason.as_deref(),
        Some("confirmed by user")
    );
    assert_eq!(approved.decided_by, Some(7));
    assert_utc_timestamp(approved.decided_at.as_deref());
    assert_eq!(rejected.decision_state, "rejected");
    assert_eq!(rejected.decision_reason.as_deref(), Some("stale signal"));
    assert!(MemoryCandidateStorePort::retrieve(
        &store,
        RetrieveMemoryCandidateQuery {
            scope: wrong_space,
            candidate_id: "cand-shared".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn sqlite_store_upserts_promotes_and_decays_habits_by_tenant_space_and_user_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);
    let wrong_user = 43;

    store
        .create_record(&tenant_one, "rec-promoted", "answer_style", "concise")
        .await
        .unwrap();
    let inserted =
        MemoryHabitStorePort::upsert(&store, habit_command(tenant_one.clone(), "habit-1", 42))
            .await
            .unwrap();
    let updated = MemoryHabitStorePort::upsert(
        &store,
        UpsertMemoryHabitCommand {
            strength: 0.7,
            support_count: 4,
            ..habit_command(tenant_one.clone(), "habit-1", 42)
        },
    )
    .await
    .unwrap();
    let tenant_two_habit =
        MemoryHabitStorePort::upsert(&store, habit_command(tenant_two.clone(), "habit-2", 42))
            .await
            .unwrap();
    let promoted = MemoryHabitStorePort::promote(
        &store,
        PromoteMemoryHabitCommand {
            scope: tenant_one.clone(),
            user_id: 42,
            habit_key: "answer_style:concise".to_string(),
            promoted_memory_id: Some("rec-promoted".to_string()),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let decayed = MemoryHabitStorePort::decay(
        &store,
        DecayMemoryHabitCommand {
            scope: tenant_one.clone(),
            user_id: 42,
            habit_key: "answer_style:concise".to_string(),
            strength_delta: 0.2,
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(inserted.strength, 0.4);
    assert_eq!(updated.strength, 0.7);
    assert_eq!(updated.support_count, 4);
    assert_eq!(tenant_two_habit.habit_id, "habit-2");
    assert_eq!(promoted.stage, "promoted");
    assert_eq!(promoted.promoted_memory_id.as_deref(), Some("rec-promoted"));
    assert_eq!(decayed.stage, "decayed");
    assert!((decayed.strength - 0.5).abs() < f64::EPSILON);
    assert!(MemoryHabitStorePort::retrieve(
        &store,
        RetrieveMemoryHabitQuery {
            scope: tenant_one,
            user_id: wrong_user,
            habit_key: "answer_style:concise".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn sqlite_store_appends_retrieval_trace_with_hits_and_context_pack_by_scope() {
    let store = new_contract_store().await;
    let tenant_one = MemoryScopeContext::for_test(1, 1);
    let tenant_two = MemoryScopeContext::for_test(2, 3);
    let wrong_space = MemoryScopeContext::for_test(1, 2);

    store
        .create_record(&tenant_one, "rec-trace-1", "answer_style", "concise")
        .await
        .unwrap();
    let appended = MemoryRetrievalTraceStorePort::append(
        &store,
        retrieval_trace_command(tenant_one.clone(), "trace-shared"),
    )
    .await
    .unwrap();
    MemoryRetrievalTraceStorePort::append(
        &store,
        AppendMemoryRetrievalTraceCommand {
            query_text: Some("tenant two query".to_string()),
            ..retrieval_trace_command(tenant_two.clone(), "trace-shared")
        },
    )
    .await
    .unwrap();

    let retrieved = MemoryRetrievalTraceStorePort::retrieve(
        &store,
        RetrieveMemoryRetrievalTraceQuery {
            scope: tenant_one.clone(),
            trace_id: "trace-shared".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let tenant_two_trace = MemoryRetrievalTraceStorePort::retrieve(
        &store,
        RetrieveMemoryRetrievalTraceQuery {
            scope: tenant_two,
            trace_id: "trace-shared".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let recent = MemoryRetrievalTraceStorePort::list_recent(
        &store,
        ListMemoryRetrievalTracesQuery {
            scope: tenant_one.clone(),
            limit: 1,
        },
    )
    .await
    .unwrap();

    assert_eq!(appended.trace_id, "trace-shared");
    assert_eq!(retrieved.query_hash, "hash:trace-shared");
    assert_eq!(retrieved.result_count, 2);
    assert_eq!(retrieved.hits.len(), 2);
    assert_eq!(retrieved.hits[0].hit_id, "trace-shared-hit-1");
    assert_eq!(retrieved.hits[0].memory_id.as_deref(), Some("rec-trace-1"));
    assert_eq!(retrieved.hits[1].memory_id, None);
    assert_eq!(
        retrieved
            .context_pack
            .as_ref()
            .map(|pack| pack.context_pack_id.as_str()),
        Some("trace-shared-pack")
    );
    assert_eq!(
        tenant_two_trace.query_text.as_deref(),
        Some("tenant two query")
    );
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].trace_id, "trace-shared");
    assert!(MemoryRetrievalTraceStorePort::retrieve(
        &store,
        RetrieveMemoryRetrievalTraceQuery {
            scope: wrong_space,
            trace_id: "trace-shared".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn sqlite_store_rolls_back_retrieval_trace_when_a_hit_insert_fails() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .create_record(&scope, "rec-trace-1", "answer_style", "concise")
        .await
        .unwrap();

    let mut command = retrieval_trace_command(scope.clone(), "trace-atomic");
    command.hits[1].hit_id = command.hits[0].hit_id.clone();
    MemoryRetrievalTraceStorePort::append(&store, command)
        .await
        .expect_err("duplicate hit ids must roll back the complete trace append");

    assert!(MemoryRetrievalTraceStorePort::retrieve(
        &store,
        RetrieveMemoryRetrievalTraceQuery {
            scope,
            trace_id: "trace-atomic".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[test]
fn native_sql_manifest_exports_candidate_habit_and_retrieval_trace_builders() {
    let candidate = build_native_sql_candidate_store();
    let habit = build_native_sql_habit_store();
    let retrieval_trace = build_native_sql_retrieval_trace_store();

    assert_eq!(candidate.port_name, "MemoryCandidateStorePort");
    assert_eq!(candidate.builder_name, "build_native_sql_candidate_store");
    assert!(candidate.ready);
    assert_eq!(habit.port_name, "MemoryHabitStorePort");
    assert_eq!(habit.builder_name, "build_native_sql_habit_store");
    assert!(habit.ready);
    assert_eq!(retrieval_trace.port_name, "MemoryRetrievalTraceStorePort");
    assert_eq!(
        retrieval_trace.builder_name,
        "build_native_sql_retrieval_trace_store"
    );
    assert!(retrieval_trace.ready);
}

#[tokio::test]
async fn sqlite_store_lists_candidates_with_cursor_pagination() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    for candidate_id in ["cand-a", "cand-b", "cand-c"] {
        MemoryCandidateStorePort::create(&store, candidate_command(scope.clone(), candidate_id))
            .await
            .unwrap();
    }

    let first_page = store
        .list_candidates_for_tenant(1, Some(1), 2, None)
        .await
        .unwrap();
    assert_eq!(first_page.len(), 3);
    let next_cursor = first_page[1].candidate_id.clone();

    let second_page = store
        .list_candidates_for_tenant(1, Some(1), 2, Some(next_cursor.as_str()))
        .await
        .unwrap();
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0].candidate_id, "cand-c");
}

#[tokio::test]
async fn sqlite_space_quota_rejection_has_no_insert_and_deleted_slots_are_reusable() {
    let store = new_contract_store().await;
    assert!(store.supports_atomic_user_space_quota_admission());

    let first = MemorySpaceStorePort::create_space_atomic_with_quota(
        &store,
        space_command(40, "quota-owner", "personal-a"),
        1,
    )
    .await
    .unwrap();
    assert!(matches!(first, MemorySpaceQuotaAdmission::Admitted(_)));

    let rejected = MemorySpaceStorePort::create_space_atomic_with_quota(
        &store,
        space_command(41, "quota-owner", "personal-b"),
        1,
    )
    .await
    .unwrap();
    assert_eq!(
        rejected,
        MemorySpaceQuotaAdmission::QuotaExceeded {
            active_spaces: 1,
            max_active_spaces: 1,
        }
    );
    assert!(store
        .retrieve_space_for_tenant(1, 41)
        .await
        .unwrap()
        .is_none());

    sqlx::query("UPDATE ai_space SET lifecycle_status = 'deleted' WHERE tenant_id = ? AND id = ?")
        .bind(1_i64)
        .bind(40_i64)
        .execute(store.pool())
        .await
        .unwrap();
    let reused = MemorySpaceStorePort::create_space_atomic_with_quota(
        &store,
        space_command(42, "quota-owner", "replacement"),
        1,
    )
    .await
    .unwrap();
    assert!(matches!(reused, MemorySpaceQuotaAdmission::Admitted(_)));
    assert_eq!(
        store
            .count_user_owned_spaces_for_tenant(1, "quota-owner")
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn sqlite_independent_pools_serialize_user_space_quota_admission() {
    let (config, database_path) = file_backed_sqlite_config("space-quota-race");
    let first_store = NativeSqlMemoryStore::connect(&config).await.unwrap();
    let second_store = NativeSqlMemoryStore::open_pool(&config, false)
        .await
        .unwrap();
    for store in [&first_store, &second_store] {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(store.pool())
            .await
            .unwrap();
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(store.pool())
            .await
            .unwrap();
    }

    let barrier = Arc::new(Barrier::new(2));
    let first = {
        let store = first_store.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            MemorySpaceStorePort::create_space_atomic_with_quota(
                &store,
                space_command(50, "race-owner", "personal-a"),
                1,
            )
            .await
        })
    };
    let second = {
        let store = second_store.clone();
        let barrier = barrier.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            MemorySpaceStorePort::create_space_atomic_with_quota(
                &store,
                space_command(51, "race-owner", "personal-b"),
                1,
            )
            .await
        })
    };
    let (first, second) = tokio::join!(first, second);
    let outcomes = [first.unwrap().unwrap(), second.unwrap().unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, MemorySpaceQuotaAdmission::Admitted(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, MemorySpaceQuotaAdmission::QuotaExceeded { .. }))
            .count(),
        1
    );
    assert_eq!(
        first_store
            .count_user_owned_spaces_for_tenant(1, "race-owner")
            .await
            .unwrap(),
        1
    );

    first_store.pool().close().await;
    second_store.pool().close().await;
    remove_sqlite_test_artifacts(&database_path);
}

#[tokio::test]
async fn sqlite_store_lists_spaces_with_cursor_pagination() {
    let store = new_contract_store().await;
    store
        .create_space_record(
            1,
            10,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "user-10".to_string(),
                space_type: "personal".to_string(),
                display_name: "Space 10".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .unwrap();

    let first_page = store.list_spaces_for_tenant(1, 2, 0, None).await.unwrap();
    assert_eq!(first_page.len(), 3);
    let next_cursor = first_page[1].space_id;

    let second_page = store
        .list_spaces_for_tenant(1, 2, next_cursor, None)
        .await
        .unwrap();
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0].space_id, 10);
}

#[tokio::test]
async fn sqlite_store_lists_spaces_scoped_to_actor_owner() {
    let store = new_contract_store().await;
    for (space_id, owner) in [(4_i64, "2001"), (5, "3002")] {
        store
            .create_space_record(
                1,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: owner.to_string(),
                    space_type: "personal".to_string(),
                    display_name: format!("Space {space_id}"),
                    default_scope: "user".to_string(),
                },
            )
            .await
            .unwrap();
    }

    let scoped = store
        .list_spaces_for_tenant(1, 10, 0, Some("2001"))
        .await
        .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].space_id, 4);
}

#[tokio::test]
async fn sqlite_store_lists_record_sources_with_cursor_pagination() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .create_record(&scope, "100", "user", "concise answers")
        .await
        .unwrap();
    for (source_id, event_id) in [("8101", "8001"), ("8102", "8002"), ("8103", "8003")] {
        store
            .append_open_api_event(
                &scope,
                event_id,
                "message.user",
                "chat",
                "2026-06-10T00:00:00Z",
                &serde_json::json!({ "text": "seed" }),
                "internal",
            )
            .await
            .unwrap();
        store
            .append_record_source_for_tenant(1, source_id, "100", event_id, "evidence", Some(0.1))
            .await
            .unwrap();
    }

    let first_page = store
        .list_record_sources_for_memory(1, "100", 2, None, None)
        .await
        .unwrap();
    assert_eq!(first_page.len(), 3);
    let next_cursor = first_page[1].source_uuid.clone();

    let second_page = store
        .list_record_sources_for_memory(1, "100", 2, Some(next_cursor.as_str()), None)
        .await
        .unwrap();
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0].source_uuid, "8101");
}

#[tokio::test]
async fn sqlite_rebuild_search_index_is_scoped_to_space() {
    let store = new_contract_store().await;
    let scope_a = MemoryScopeContext::for_test(1, 10);
    let scope_b = MemoryScopeContext::for_test(1, 20);
    store
        .create_space_record(
            1,
            10,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "user-10".to_string(),
                space_type: "personal".to_string(),
                display_name: "Space 10".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .unwrap();
    store
        .create_space_record(
            1,
            20,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "user-20".to_string(),
                space_type: "personal".to_string(),
                display_name: "Space 20".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .unwrap();
    store
        .create_record_open_api(
            &scope_a,
            "mem-a",
            "user",
            "semantic",
            Some("topic"),
            Some("located_in"),
            "alpha city",
            "alpha city",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .create_record_open_api(
            &scope_b,
            "mem-b",
            "user",
            "semantic",
            Some("topic"),
            Some("located_in"),
            "beta city",
            "beta city",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();

    store
        .rebuild_record_search_indexes_for_space(1, 10)
        .await
        .unwrap();

    let hits_a = store
        .search_record_details_fulltext(&scope_a, "alpha", 5)
        .await
        .unwrap();
    let hits_b = store
        .search_record_details_fulltext(&scope_b, "beta", 5)
        .await
        .unwrap();
    assert_eq!(hits_a.len(), 1);
    assert_eq!(hits_b.len(), 1);
}

#[tokio::test]
async fn sqlite_rebuild_search_index_tenant_scope_preserves_other_tenants() {
    let store = new_contract_store().await;
    let scope_t1 = MemoryScopeContext::for_test(1, 10);
    let scope_t2 = MemoryScopeContext::for_test(2, 20);
    for (tenant_id, space_id, owner) in [(1_i64, 10_i64, "user-t1"), (2, 20, "user-t2")] {
        store
            .create_space_record(
                tenant_id,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: owner.to_string(),
                    space_type: "personal".to_string(),
                    display_name: format!("Space {space_id}"),
                    default_scope: "user".to_string(),
                },
            )
            .await
            .unwrap();
    }
    store
        .create_record_open_api(
            &scope_t1,
            "mem-t1",
            "user",
            "semantic",
            Some("topic"),
            Some("located_in"),
            "tenant-one landmark",
            "tenant-one landmark",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .create_record_open_api(
            &scope_t2,
            "mem-t2",
            "user",
            "semantic",
            Some("topic"),
            Some("located_in"),
            "tenant-two landmark",
            "tenant-two landmark",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();

    store.rebuild_all_record_search_indexes(1).await.unwrap();

    let hits_t1 = store
        .search_record_details_fulltext(&scope_t1, "tenant-one", 5)
        .await
        .unwrap();
    let hits_t2 = store
        .search_record_details_fulltext(&scope_t2, "tenant-two", 5)
        .await
        .unwrap();
    assert_eq!(hits_t1.len(), 1);
    assert_eq!(hits_t2.len(), 1);
}

#[tokio::test]
async fn sqlite_fts_matches_predicate_field() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .create_record_open_api(
            &scope,
            "pred-mem",
            "user",
            "semantic",
            Some("Earth"),
            Some("orbits"),
            "Sun",
            "Earth orbits the Sun",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();

    let hits = store
        .search_record_details_fulltext(&scope, "orbits", 5)
        .await
        .unwrap();
    assert!(
        hits.iter().any(|row| row.memory_id == "pred-mem"),
        "FTS must match predicate column"
    );
}

#[tokio::test]
async fn sqlite_supersede_atomic_chain_persists_dual_journals_and_retry_is_idempotent() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    create_canonical_fixture(
        &store,
        &scope,
        "supersede-old",
        "semantic",
        "old canonical value",
        "internal",
    )
    .await;

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
        MemoryRecordStorePort::supersede_canonical_atomic_with_quota(&store, command.clone(), 2)
            .await
            .expect("supersede must commit while quota has capacity");
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
        &store,
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
        &store,
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
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseded journal outbox must commit with the chain");
    assert_eq!(superseded_outbox.aggregate_id, "supersede-old");
    let superseded_audit = MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseded journal audit must commit with the chain");
    assert_eq!(superseded_audit.resource_id, "supersede-old");

    let created_outbox = MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-created".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("created journal outbox must commit with the chain");
    assert_eq!(created_outbox.aggregate_id, "supersede-new");
    let created_audit = MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-created".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("created journal audit must commit with the chain");
    assert_eq!(created_audit.resource_id, "supersede-new");

    let old_fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("supersede-old")
    .fetch_one(store.pool())
    .await
    .unwrap();
    let new_fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("supersede-new")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        old_fts_count, 0,
        "superseded source must leave no stale FTS row"
    );
    assert_eq!(new_fts_count, 1, "active target must have one FTS row");

    let retry = MemoryRecordStorePort::supersede_canonical_atomic_with_quota(&store, command, 2)
        .await
        .expect("replaying the same supersede must be idempotent");
    assert_eq!(retry, MemoryRecordQuotaAdmission::Admitted(admitted));

    let supersede_outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND uuid IN (?, ?)",
    )
    .bind(scope.tenant_id)
    .bind("outbox-supersede-superseded")
    .bind("outbox-supersede-created")
    .fetch_one(store.pool())
    .await
    .unwrap();
    let supersede_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ? AND uuid IN (?, ?)",
    )
    .bind(scope.tenant_id)
    .bind("audit-supersede-superseded")
    .bind("audit-supersede-created")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        supersede_outbox_count, 2,
        "retry must not duplicate outbox rows"
    );
    assert_eq!(
        supersede_audit_count, 2,
        "retry must not duplicate audit rows"
    );

    let mut changed_payload = SupersedeCanonicalMemoryAtomicCommand {
        scope: scope.clone(),
        old_memory_id: "supersede-old".to_string(),
        new_memory_id: "supersede-new".to_string(),
        scope_label: "user".to_string(),
        memory_type: "semantic".to_string(),
        subject: Some("account".to_string()),
        predicate: Some("prefers".to_string()),
        object_text: "new canonical value".to_string(),
        canonical_text: "different retry payload".to_string(),
        sensitivity_level: "internal".to_string(),
        created_journal: mutation_journal("supersede-new", "supersede-created"),
        superseded_journal: mutation_journal("supersede-old", "supersede-superseded"),
        expires_at: None,
        metadata_json: None,
    };
    assert!(matches!(
        MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
            &store,
            changed_payload.clone(),
            2,
        )
        .await,
        Err(MemorySpiError::IdempotencyConflict { ref idempotency_key })
            if idempotency_key == "supersede-new"
    ));

    changed_payload.canonical_text = "User prefers the new canonical value".to_string();
    changed_payload.created_journal = mutation_journal("supersede-new", "supersede-created-retry");
    assert!(matches!(
        MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
            &store,
            changed_payload,
            2,
        )
        .await,
        Err(MemorySpiError::IdempotencyConflict { ref idempotency_key })
            if idempotency_key == "supersede-new"
    ));

    let unchanged = MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = ? AND uuid IN (?, ?)",
        )
        .bind(scope.tenant_id)
        .bind("outbox-supersede-superseded")
        .bind("outbox-supersede-created")
        .fetch_one(store.pool())
        .await
        .unwrap(),
        2
    );
    assert!(MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-created-retry".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope,
            audit_id: "audit-supersede-created-retry".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
}

#[tokio::test]
async fn sqlite_supersede_quota_rejection_keeps_chain_and_journals_unchanged() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    create_canonical_fixture(
        &store,
        &scope,
        "supersede-quota-old",
        "semantic",
        "old value",
        "internal",
    )
    .await;
    create_canonical_fixture(
        &store,
        &scope,
        "supersede-quota-blocker",
        "semantic",
        "blocking value",
        "internal",
    )
    .await;

    let admission = MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
        &store,
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
            created_journal: mutation_journal("supersede-quota-new", "supersede-quota-created"),
            superseded_journal: mutation_journal(
                "supersede-quota-old",
                "supersede-quota-superseded",
            ),
            expires_at: None,
            metadata_json: None,
        },
        2,
    )
    .await
    .expect("quota rejection is a successful admission outcome");
    assert_eq!(
        admission,
        MemoryRecordQuotaAdmission::QuotaExceeded {
            active_records: 2,
            max_active_records: 2,
        }
    );

    let old = MemoryRecordStorePort::retrieve_canonical(
        &store,
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
        &store,
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
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-quota-new".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-quota-created".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryOutboxStorePort::retrieve(
        &store,
        RetrieveMemoryOutboxQuery {
            scope: scope.clone(),
            outbox_id: "outbox-supersede-quota-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-quota-created".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    assert!(MemoryAuditStorePort::retrieve(
        &store,
        RetrieveMemoryAuditQuery {
            scope: scope.clone(),
            audit_id: "audit-supersede-quota-superseded".to_string(),
        },
    )
    .await
    .unwrap()
    .is_none());
    let rejected_fts_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("supersede-quota-new")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        rejected_fts_count, 0,
        "quota rejection must not create FTS state"
    );
}

#[tokio::test]
async fn sqlite_expiration_roundtrip_persists_declared_expiration() {
    let store = new_contract_store().await;

    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),

            memory_id: "expiring-record".to_string(),

            scope_label: "user".to_string(),

            memory_type: "semantic".to_string(),

            subject: Some("account".to_string()),

            predicate: Some("prefers".to_string()),

            object_text: "session ends 2027-01-01T00:00:00Z".to_string(),

            canonical_text: "Session ends 2027-01-01T00:00:00Z".to_string(),

            sensitivity_level: "internal".to_string(),

            expires_at: Some("2027-01-01T00:00:00Z".to_string()),

            journal: mutation_journal("expiring-record", "expiring-record-created"),
            metadata_json: None,
        },
    )
    .await
    .unwrap();

    MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),

            memory_id: "unexpiring-record".to_string(),

            scope_label: "user".to_string(),

            memory_type: "semantic".to_string(),

            subject: Some("account".to_string()),

            predicate: Some("prefers".to_string()),

            object_text: "no expiration declared".to_string(),

            canonical_text: "No expiration declared".to_string(),

            sensitivity_level: "internal".to_string(),

            expires_at: None,

            journal: mutation_journal("unexpiring-record", "unexpiring-record-created"),
            metadata_json: None,
        },
    )
    .await
    .unwrap();

    let expiring = MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "expiring-record".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("expiring record must exist");

    assert_eq!(
        expiring.expires_at.as_deref(),
        Some("2027-01-01T00:00:00Z"),
        "declared expiration must survive the write/read roundtrip"
    );

    let unexpiring = MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "unexpiring-record".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("unexpiring record must exist");

    assert_eq!(
        unexpiring.expires_at, None,
        "record created without expiration must read back as null, not a fabricated date"
    );
}

#[tokio::test]
async fn sqlite_expiration_roundtrip_through_supersede_keeps_old_record_unchanged() {
    let store = new_contract_store().await;

    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),

            memory_id: "supersede-exp-old".to_string(),

            scope_label: "user".to_string(),

            memory_type: "semantic".to_string(),

            subject: Some("account".to_string()),

            predicate: Some("prefers".to_string()),

            object_text: "old value".to_string(),

            canonical_text: "Old value".to_string(),

            sensitivity_level: "internal".to_string(),

            expires_at: Some("2026-01-01T00:00:00Z".to_string()),

            journal: mutation_journal("supersede-exp-old", "supersede-exp-old-created"),
            metadata_json: None,
        },
    )
    .await
    .unwrap();

    let admitted = match MemoryRecordStorePort::supersede_canonical_atomic_with_quota(
        &store,
        SupersedeCanonicalMemoryAtomicCommand {
            scope: scope.clone(),

            old_memory_id: "supersede-exp-old".to_string(),

            new_memory_id: "supersede-exp-new".to_string(),

            scope_label: "user".to_string(),

            memory_type: "semantic".to_string(),

            subject: Some("account".to_string()),

            predicate: Some("prefers".to_string()),

            object_text: "new value".to_string(),

            canonical_text: "New value".to_string(),

            sensitivity_level: "internal".to_string(),

            expires_at: Some("2027-06-01T00:00:00Z".to_string()),

            created_journal: mutation_journal("supersede-exp-new", "supersede-exp-new-created"),

            superseded_journal: mutation_journal(
                "supersede-exp-old",
                "supersede-exp-old-superseded",
            ),
            metadata_json: None,
        },
        0,
    )
    .await
    .unwrap()
    {
        MemoryRecordQuotaAdmission::Admitted(record) => record,

        MemoryRecordQuotaAdmission::QuotaExceeded { .. } => {
            panic!("supersede with unlimited quota must be admitted")
        }
    };

    assert_eq!(
        admitted.expires_at.as_deref(),
        Some("2027-06-01T00:00:00Z"),
        "replacement record must carry the declared expiration"
    );

    let old = MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),
            memory_id: "supersede-exp-old".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("superseded record must stay readable");

    assert_eq!(
        old.expires_at.as_deref(),
        Some("2026-01-01T00:00:00Z"),
        "superseding a record must not rewrite the old record's expiration"
    );
}

#[tokio::test]

async fn sqlite_retrieval_and_listing_hide_expired_records_unless_asked() {
    let store = new_contract_store().await;

    let scope = MemoryScopeContext::for_test(1, 1);

    for (memory_id, expires_at) in [
        ("expired-record", Some("2020-01-01T00:00:00Z".to_string())),
        ("living-record", Some("2099-01-01T00:00:00Z".to_string())),
        ("immortal-record", None),
    ] {
        MemoryRecordStorePort::create_canonical_atomic(
            &store,
            CreateCanonicalMemoryCommand {
                scope: scope.clone(),

                memory_id: memory_id.to_string(),

                scope_label: "user".to_string(),

                memory_type: "semantic".to_string(),

                subject: Some("account".to_string()),

                predicate: Some("prefers".to_string()),

                object_text: format!("kanata keymap {memory_id}"),

                canonical_text: format!("Kanata keymap {memory_id}"),

                sensitivity_level: "internal".to_string(),

                expires_at,

                journal: mutation_journal(memory_id, &format!("{memory_id}-created")),
                metadata_json: None,
            },
        )
        .await
        .unwrap();
    }

    let visible_ids = |include_expired: bool| {
        let store = &store;

        let scope = scope.clone();

        async move {
            let result = store
                .search_memory_candidates(&SearchMemoryCandidatesQuery {
                    scope,

                    query: "kanata keymap".to_string(),

                    limit: 10,

                    retriever_kinds: vec![MemoryRetrieverKind::Keyword],

                    memory_types: Vec::new(),

                    read_scope: MemorySensitivityReadScope::Owner,

                    metadata_filter: None,

                    include_expired,
                })
                .await
                .unwrap();

            let mut ids = result
                .records
                .iter()
                .map(|candidate| candidate.memory_id.clone())
                .collect::<Vec<_>>();

            ids.sort();

            ids
        }
    };

    assert_eq!(
        visible_ids(false).await,
        vec!["immortal-record".to_string(), "living-record".to_string()],
        "search must hide expired records by default and keep the rest"
    );

    assert_eq!(
        visible_ids(true).await.len(),
        3,
        "show_expired=true must bring the expired record back"
    );

    // Listing: the expired record is filtered out before LIMIT, so a page of

    // size 1 serves the first non-expired record instead of an empty page.

    let page = store
        .list_record_details(&scope, None, 1, None, SENSITIVITY_READ_OWNER, false)
        .await
        .unwrap();

    // The store serves page_size + 1 rows so the service layer can detect

    // `has_more`; with the expired record filtered before LIMIT, neither slot

    // is taken by it and the first served row is the next live record.

    assert_eq!(page.len(), 2);

    assert!(
        !page.iter().any(|row| row.memory_id == "expired-record"),
        "an expired record must not consume a page slot"
    );

    assert_eq!(page[0].memory_id, "immortal-record");

    let full = store
        .list_record_details(&scope, None, 10, None, SENSITIVITY_READ_OWNER, true)
        .await
        .unwrap();

    assert_eq!(
        full.len(),
        3,
        "show_expired=true must list all three records"
    );
}

#[tokio::test]
async fn sqlite_delete_all_sweeps_the_scope_with_journals_and_stays_idempotent() {
    let store = new_contract_store().await;

    let scope = MemoryScopeContext::for_test(1, 1);

    for memory_id in ["sweep-one", "sweep-two", "sweep-three"] {
        MemoryRecordStorePort::create_canonical_atomic(
            &store,
            CreateCanonicalMemoryCommand {
                scope: scope.clone(),

                memory_id: memory_id.to_string(),

                scope_label: "user".to_string(),

                memory_type: "semantic".to_string(),

                subject: Some("account".to_string()),

                predicate: Some("prefers".to_string()),

                object_text: format!("sweep record {memory_id}"),

                canonical_text: format!("Sweep record {memory_id}"),

                sensitivity_level: "internal".to_string(),

                expires_at: None,

                journal: mutation_journal(memory_id, &format!("{memory_id}-created")),
                metadata_json: None,
            },
        )
        .await
        .unwrap();
    }

    let receipt = MemoryRecordStorePort::delete_all_canonical_atomic(
        &store,
        DeleteAllCanonicalMemoryCommand {
            scope: scope.clone(),
            user_id: None,
        },
    )
    .await
    .unwrap();

    let deleted = receipt.deleted_count;

    assert_eq!(
        deleted, 3,
        "delete_all must sweep the active records of the scope"
    );

    let outbox_count: i64 = sqlx::query_scalar(

        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = 1 AND event_type = 'memory.record.deleted'",

    )

    .fetch_one(store.pool())

    .await

    .unwrap();

    assert_eq!(outbox_count, 3, "each deletion must journal its own event");

    // Repeat sweep: everything is already deleted, so nothing is deleted again

    // and no duplicate journal rows appear (deterministic journal ids).

    let second = MemoryRecordStorePort::delete_all_canonical_atomic(
        &store,
        DeleteAllCanonicalMemoryCommand {
            scope,
            user_id: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(second.deleted_count, 0);

    let outbox_after: i64 = sqlx::query_scalar(

        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = 1 AND event_type = 'memory.record.deleted'",

    )

    .fetch_one(store.pool())

    .await

    .unwrap();

    assert_eq!(outbox_after, 3);
}

#[tokio::test]

async fn sqlite_metadata_roundtrip_survives_write_read_and_supersede() {
    let store = new_contract_store().await;

    let scope = MemoryScopeContext::for_test(1, 1);

    MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),

            memory_id: "meta-record".to_string(),

            scope_label: "user".to_string(),

            memory_type: "semantic".to_string(),

            subject: Some("account".to_string()),

            predicate: Some("prefers".to_string()),

            object_text: "metadata carrier".to_string(),

            canonical_text: "Metadata carrier".to_string(),

            sensitivity_level: "internal".to_string(),

            expires_at: None,

            metadata_json: Some(r##"{"topic":"keybindings","priority":2}"##.to_string()),

            journal: mutation_journal("meta-record", "meta-record-created"),
        },
    )
    .await
    .unwrap();

    let loaded = MemoryRecordStorePort::retrieve_canonical(
        &store,
        RetrieveCanonicalMemoryQuery {
            scope: scope.clone(),

            memory_id: "meta-record".to_string(),
        },
    )
    .await
    .unwrap()
    .expect("meta record must exist");

    assert_eq!(
        loaded.metadata_json.as_deref(),
        Some(r##"{"topic":"keybindings","priority":2}"##),
        "caller metadata must survive the write/read roundtrip"
    );
}

#[tokio::test]
async fn sqlite_forget_all_records_in_space_purges_soft_deleted_records_and_event_sources() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let now = "2026-07-12T00:00:00Z";

    store
        .create_record_open_api(
            &scope,
            "forget-live-record",
            "user",
            "semantic",
            None,
            None,
            "live forget value",
            "The live forget value",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .create_record_open_api(
            &scope,
            "forget-soft-record",
            "user",
            "semantic",
            None,
            None,
            "soft forget value",
            "The soft forget value",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .mark_record_deleted(&scope, "forget-soft-record")
        .await
        .unwrap();

    // A soft-deleted record keeps its `ai_record_source` row. Seed one evidence
    // event plus source rows for both the live and the soft-deleted record: the
    // forget workflow must purge every source referencing the space's events or
    // the `ai_record_source.event_id` foreign key aborts the event purge.
    sqlx::query(
        r#"
        INSERT INTO ai_event (
          id, uuid, tenant_id, space_id, actor_type, event_type, source_type,
          event_time, payload_json, payload_hash, sensitivity_level,
          ingestion_status, created_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(912_001_i64)
    .bind("forget-event")
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind("user")
    .bind("memory.created")
    .bind("api")
    .bind(now)
    .bind(r#"{"kind":"forget-evidence"}"#)
    .bind("hash-forget-event")
    .bind("internal")
    .bind("committed")
    .bind(now)
    .execute(store.pool())
    .await
    .unwrap();
    for (source_id, source_uuid, memory_uuid) in [
        (912_002_i64, "forget-source-live", "forget-live-record"),
        (912_003_i64, "forget-source-soft", "forget-soft-record"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO ai_record_source (
              id, uuid, tenant_id, memory_id, event_id, source_role, created_at
            )
            SELECT ?, ?, ?, record.id, event.id, ?, ?
            FROM ai_record record
            JOIN ai_event event ON event.tenant_id = record.tenant_id
            WHERE record.tenant_id = ? AND record.uuid = ?
              AND event.tenant_id = ? AND event.uuid = ?
            "#,
        )
        .bind(source_id)
        .bind(source_uuid)
        .bind(scope.tenant_id)
        .bind("origin")
        .bind(now)
        .bind(scope.tenant_id)
        .bind(memory_uuid)
        .bind(scope.tenant_id)
        .bind("forget-event")
        .execute(store.pool())
        .await
        .unwrap();
    }

    let stats = store.forget_all_records_in_space(&scope).await.unwrap();
    assert_eq!(stats.deleted_records, 2, "soft-deleted records must be purged too");
    assert_eq!(stats.purged_events, 1);

    for (table, condition) in [
        ("ai_record", "uuid = 'forget-soft-record'"),
        ("ai_record", "uuid = 'forget-live-record'"),
        ("ai_record_source", "uuid = 'forget-source-soft'"),
        ("ai_record_source", "uuid = 'forget-source-live'"),
        ("ai_event", "uuid = 'forget-event'"),
    ] {
        // `table` and `condition` are literal pairs from the list above; the
        // audited escape hatch satisfies sqlx 0.9's `SqlSafeStr` bound.
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {table} WHERE {condition}"
        )))
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(count, 0, "{table} must not survive the space forget");
    }
}

#[tokio::test]
async fn sqlite_forget_all_records_in_space_sweeps_past_the_page_boundary() {
    // Regression: the sweep must keep walking when a fetch comes back exactly
    // page-sized. A `LIMIT page` query without the +1 sentinel reported
    // `has_more = false` on a full page and silently left every record past the
    // first page alive while the workflow still reported success.
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    let page_size = sdkwork_utils_rust::MAX_LIST_PAGE_SIZE;
    let total_records = i64::from(page_size) + 5;

    for index in 0..total_records {
        store
            .create_record_open_api(
                &scope,
                &format!("forget-page-record-{index:04}"),
                "user",
                "semantic",
                None,
                None,
                &format!("forget page value {index}"),
                &format!("The forget page value {index}"),
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }

    let stats = store.forget_all_records_in_space(&scope).await.unwrap();
    assert_eq!(
        stats.deleted_records,
        u32::try_from(total_records).expect("record count fits u32"),
        "every record past the first page must be purged too"
    );

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record WHERE tenant_id = ? AND space_id = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 0, "the space must be fully purged");
}

#[tokio::test]
async fn sqlite_record_update_preserves_object_text_and_refreshes_fts() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);

    store
        .create_record_open_api(
            &scope,
            "update-preserve-object",
            "user",
            "semantic",
            Some("preference"),
            None,
            "original object payload",
            "The canonical statement",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();

    let updated = store
        .update_record_open_api(
            &scope,
            "update-preserve-object",
            Some("Rewritten canonical statement"),
            Some("topic"),
        )
        .await
        .unwrap()
        .expect("updated record must exist");
    assert_eq!(updated.canonical_text, "Rewritten canonical statement");
    assert_eq!(updated.object_text, "original object payload");
    assert_eq!(updated.subject.as_deref(), Some("topic"));

    // The full-text mirror must reflect the rewritten canonical text while
    // keeping the untouched object text.
    let fts_row: (String, String) = sqlx::query_as(
        "SELECT canonical_text, object_text FROM ai_record_fts WHERE memory_uuid = ?",
    )
    .bind("update-preserve-object")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(fts_row.0, "Rewritten canonical statement");
    assert_eq!(fts_row.1, "original object payload");
}

#[tokio::test]
async fn sqlite_store_retention_purges_terminal_and_derived_rows_only() {
    let store = new_contract_store().await;
    let now = crate_now_text();
    let ancient = "2020-01-01T00:00:00.000Z";

    // Two outbox events: one terminal and ancient (must purge), one pending
    // and ancient (must survive — it is still awaiting delivery).
    sqlx::query(
        r#"
        INSERT INTO ai_outbox_event
            (id, uuid, tenant_id, aggregate_type, aggregate_id, event_type,
             event_version, payload_json, publish_state, created_at, updated_at)
        VALUES
            (901, 'ret-out-1', 1, 'memory_record', 'rec-1', 'memory.record.deleted',
             '1.0', '{}', 'published', ?, ?),
            (902, 'ret-out-2', 1, 'memory_record', 'rec-2', 'memory.record.deleted',
             '1.0', '{}', 'pending', ?, ?)
        "#,
    )
    .bind(ancient)
    .bind(ancient)
    .bind(ancient)
    .bind(&now)
    .execute(store.pool())
    .await
    .unwrap();

    let purged_outbox = store.purge_terminal_outbox_events(0).await.unwrap();
    assert_eq!(purged_outbox, 1, "only the terminal ancient event is purged");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_outbox_event")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(remaining, 1, "the pending event must survive the sweep");

    // A learning job still queued must survive; a dead one from 2020 purges.
    sqlx::query(
        r#"
        INSERT INTO ai_learning_job
            (id, uuid, tenant_id, space_id, job_type, state, priority, created_at, updated_at)
        VALUES
            (903, 'ret-job-1', 1, 1, 'extraction', 'dead', 0, ?, ?),
            (904, 'ret-job-2', 1, 1, 'extraction', 'queued', 0, ?, ?)
        "#,
    )
    .bind(ancient)
    .bind(ancient)
    .bind(&now)
    .bind(&now)
    .execute(store.pool())
    .await
    .unwrap();
    let purged_jobs = store.purge_terminal_learning_jobs(0).await.unwrap();
    assert_eq!(purged_jobs, 1);

    // Trace family: hits and packs go before the trace row.
    sqlx::query(
        r#"
        INSERT INTO ai_retrieval_trace
            (id, uuid, tenant_id, space_id, query_hash, result_count, degraded, created_at)
        VALUES (905, 'ret-trace-1', 1, 1, 'hash', 1, 0, ?)
        "#,
    )
    .bind(ancient)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_retrieval_hit
            (id, uuid, tenant_id, retrieval_trace_id, retriever_name, result_rank, status, created_at)
        VALUES (906, 'ret-hit-1', 1, 905, 'sql', 1, 'included', ?)
        "#,
    )
    .bind(ancient)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_context_pack
            (id, uuid, tenant_id, retrieval_trace_id, pack_json, estimated_tokens, truncated, created_at)
        VALUES (907, 'ret-pack-1', 1, 905, '{}', 1, 0, ?)
        "#,
    )
    .bind(ancient)
    .execute(store.pool())
    .await
    .unwrap();

    let stats = store.purge_retrieval_traces(0).await.unwrap();
    assert_eq!(
        (stats.hits, stats.context_packs, stats.traces),
        (1, 1, 1),
        "the whole trace family must be removed child-first"
    );
    let traces_left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_retrieval_trace")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(traces_left, 0);
}

fn crate_now_text() -> String {
    sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None)
}

#[tokio::test]
async fn sqlite_user_forget_rejects_pending_candidates_across_the_batch_boundary() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext {
        tenant_id: 1,
        space_id: 1,
        organization_id: None,
        user_id: Some(101),
    };
    // One candidate sweep batch is 500 rows, so 501 pending candidates force the
    // keyset enumeration to cross a batch boundary; a candidate decided before the
    // forget must never be flipped by the sweep.
    for index in 0..501 {
        MemoryCandidateStorePort::create(
            &store,
            candidate_command(scope.clone(), &format!("forget-candidate-{index}")),
        )
        .await
        .unwrap();
    }
    MemoryCandidateStorePort::create(
        &store,
        candidate_command(scope.clone(), "forget-candidate-approved"),
    )
    .await
    .unwrap();
    MemoryCandidateStorePort::approve(
        &store,
        ApproveMemoryCandidateCommand {
            scope: scope.clone(),
            candidate_id: "forget-candidate-approved".to_string(),
            decision_reason: Some("decided before forget".to_string()),
            decided_by: Some(7),
        },
    )
    .await
    .unwrap();

    let stats = store
        .forget_records_for_user(1, 101, Some(1))
        .await
        .unwrap();
    assert_eq!(
        stats.rejected_candidates, 501,
        "every pending candidate across both batches must be counted"
    );

    let rejected: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_candidate WHERE tenant_id = 1 AND space_id = 1 \
         AND user_id = 101 AND decision_state = 'rejected' \
         AND decision_reason = 'privacy_forget'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(rejected, 501);

    let decided_before_forget: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_candidate WHERE uuid = 'forget-candidate-approved' \
         AND decision_state = 'approved'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        decided_before_forget, 1,
        "the batched UPDATE must keep the original decision_state predicate"
    );
}

#[tokio::test]
async fn sqlite_forget_matching_query_sweeps_past_one_keyset_page() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext {
        tenant_id: 1,
        space_id: 1,
        organization_id: None,
        user_id: Some(101),
    };
    // One LIKE sweep page is MAX_LIST_PAGE_SIZE (200) rows, so 205 matches force the
    // keyset cursor to advance past the first page. A sweep that restarts from the top
    // or loses its ORDER BY would strand the tail rows instead of deleting all of them.
    for index in 0..205 {
        store
            .create_record_open_api(
                &scope,
                &format!("sweep-match-{index}"),
                "user",
                "semantic",
                None,
                None,
                "sweep needle target",
                "sweep needle target",
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }
    for memory_id in ["sweep-keep-a", "sweep-keep-b"] {
        store
            .create_record_open_api(
                &scope,
                memory_id,
                "user",
                "semantic",
                None,
                None,
                "unrelated content",
                "unrelated content",
                "internal",
                None,
                None,
            )
            .await
            .unwrap();
    }

    let stats = store
        .forget_records_matching_query(&scope, "sweep needle")
        .await
        .unwrap();
    assert_eq!(
        stats.deleted_records, 205,
        "matches beyond the first page must be swept by the advancing cursor"
    );

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_record WHERE tenant_id = 1 AND space_id = 1",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        remaining, 2,
        "records outside the LIKE match must survive the sweep"
    );
}

#[tokio::test]
async fn sqlite_entity_memory_links_are_truncated_at_the_ranking_cap() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(1, 1);
    store
        .create_record_open_api(
            &scope,
            "610000000000000001",
            "user",
            "semantic",
            None,
            None,
            "linked memory",
            "linked memory",
            "internal",
            None,
            None,
        )
        .await
        .unwrap();
    store
        .insert_entity(InsertEntityCommand {
            id: 5001,
            uuid: "cap-entity-src",
            tenant_id: 1,
            space_id: 1,
            entity_type: "concept",
            canonical_name: "Cap Source Entity",
            aliases_json: None,
            attributes_json: None,
            sensitivity_level: "internal",
        })
        .await
        .unwrap();
    store
        .insert_entity(InsertEntityCommand {
            id: 5002,
            uuid: "cap-entity-tgt",
            tenant_id: 1,
            space_id: 1,
            entity_type: "concept",
            canonical_name: "Cap Target Entity",
            aliases_json: None,
            attributes_json: None,
            sensitivity_level: "internal",
        })
        .await
        .unwrap();
    let record_id: i64 =
        sqlx::query_scalar("SELECT id FROM ai_record WHERE uuid = '610000000000000001'")
            .fetch_one(store.pool())
            .await
            .unwrap();

    // 1001 edges produce 1001 joined rows, one past MAX_ENTITY_LINK_RESULTS (1000), so
    // the lookup must truncate instead of returning the whole join.
    for index in 0..1001 {
        store
            .insert_edge(InsertEdgeCommand {
                id: 6000 + index,
                uuid: &format!("cap-edge-{index}"),
                tenant_id: 1,
                space_id: 1,
                source_entity_id: 5001,
                target_entity_id: 5002,
                relation_type: "mentions",
                source_record_id: Some(record_id),
                weight: Some(0.5),
                valid_from: None,
                valid_to: None,
                metadata_json: None,
            })
            .await
            .unwrap();
    }

    let links = store
        .list_entity_memory_links_for_memories(1, 1, &["610000000000000001".to_string()])
        .await
        .unwrap();
    assert_eq!(
        links.len(),
        2 * MAX_ENTITY_LINK_RESULTS as usize,
        "each capped row contributes one endpoint pair, so the join must truncate"
    );
    assert!(links
        .iter()
        .all(|link| link.memory_id == "610000000000000001" && !link.entity_name.is_empty()));
}

#[tokio::test]
async fn sqlite_commercial_list_page_sizes_are_clamped() {
    let store = new_contract_store().await;
    for index in 0..3 {
        store
            .insert_subject(InsertSubjectCommand {
                id: 7000 + index,
                uuid: &format!("clamp-subject-{index}"),
                tenant_id: 1,
                organization_id: None,
                subject_type: "user",
                subject_ref: &format!("clamp-actor-{index}"),
                display_name: "clamp fixture",
                default_space_id: None,
                metadata_json: None,
            })
            .await
            .unwrap();
    }

    // page_size 0 used to produce LIMIT 1, which broke the continuation lookahead; the
    // clamp lifts it to a one-row page plus the sentinel row.
    let zero_page = store
        .list_subjects(1, None, None, None, 0)
        .await
        .unwrap();
    assert_eq!(zero_page.len(), 2);
    // i32::MAX used to overflow `page_size + 1`; it must clamp to the crate-wide bound.
    let max_page = store
        .list_subjects(1, None, None, None, i32::MAX)
        .await
        .unwrap();
    assert_eq!(max_page.len(), 3);

    store
        .insert_binding(
            8000,
            "clamp-binding",
            1,
            None,
            "access",
            "viewer",
            Some(7000),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let binding_page = store
        .list_bindings(1, None, None, None, None, None, None, i32::MAX)
        .await
        .unwrap();
    assert_eq!(binding_page.len(), 1);

    store
        .insert_capability_binding(
            8100,
            "clamp-capability",
            1,
            "memory.read",
            "space",
            1,
            "enforce",
            1,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let capability_page = store
        .list_capability_bindings(1, None, None, None, None, None, i32::MAX)
        .await
        .unwrap();
    assert_eq!(capability_page.len(), 1);
}

// ---------------------------------------------------------------------------
// Usage metering facts (ai_usage_daily)
// ---------------------------------------------------------------------------

fn metered_journal(memory_id: &str, audit_action: &str) -> MemoryMutationJournal {
    // The create and the delete of one memory each journal their own outbox
    // event, so the outbox id must differ per action, not per memory.
    let suffix = audit_action.rsplit('.').next().unwrap_or("event");
    MemoryMutationJournal {
        outbox_id: format!("outbox-usage-{memory_id}-{suffix}"),
        aggregate_type: "memory_record".to_string(),
        aggregate_id: memory_id.to_string(),
        event_type: audit_action.to_string(),
        event_version: "1.0".to_string(),
        payload_json: format!(r#"{{"memoryId":"{memory_id}"}}"#),
        audit_id: format!("audit-usage-{memory_id}-{suffix}"),
        audit_action: audit_action.to_string(),
        audit_resource_type: "memory_record".to_string(),
        audit_resource_id: memory_id.to_string(),
        audit_result: "accepted".to_string(),
    }
}

async fn usage_charges_by_metric(
    store: &NativeSqlMemoryStore,
    tenant_id: i64,
) -> Vec<(String, String, i64)> {
    let mut rows = store
        .list_usage_daily_for_tenant(tenant_id, None, 200)
        .await
        .expect("list usage daily rows");
    rows.sort_by(|left, right| (&left.day, &left.metric).cmp(&(&right.day, &right.metric)));
    rows.into_iter()
        .map(|row| (row.day, row.metric, row.delta))
        .collect()
}

async fn charge_usage(
    store: &NativeSqlMemoryStore,
    tenant_id: i64,
    day: &str,
    metric: &str,
    delta: i64,
) {
    let mut tx = store.begin_tx().await.expect("begin usage tx");
    store
        .bump_usage_daily_on_tx(&mut tx, tenant_id, metric, delta, day)
        .await
        .expect("charge usage fact");
    tx.commit().await.expect("commit usage fact");
}

#[tokio::test]
async fn sqlite_usage_daily_bumps_accumulate_and_stay_day_isolated() {
    let store = new_contract_store().await;
    const TENANT: i64 = 4242;
    const DAY_ONE: &str = "2026-09-30";
    const DAY_TWO: &str = "2026-10-01";

    for _ in 0..3 {
        charge_usage(&store, TENANT, DAY_ONE, "record.create", 1).await;
    }
    charge_usage(&store, TENANT, DAY_ONE, "record.delete", -1).await;
    // The same (day, metric) charge repeats idempotently as accumulation, and
    // the same metric on the next day stays an independent counter.
    charge_usage(&store, TENANT, DAY_TWO, "record.create", 1).await;

    assert_eq!(
        usage_charges_by_metric(&store, TENANT).await,
        vec![
            (DAY_ONE.to_string(), "record.create".to_string(), 3),
            (DAY_ONE.to_string(), "record.delete".to_string(), -1),
            (DAY_TWO.to_string(), "record.create".to_string(), 1),
        ]
    );
    // A second tenant never sees another tenant's usage facts.
    assert!(usage_charges_by_metric(&store, 4243).await.is_empty());
}

#[tokio::test]
async fn sqlite_journal_mutations_charge_usage_in_the_same_transaction() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(51, 51);
    store
        .create_space_record(
            51,
            51,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "510".to_string(),
                space_type: "workspace".to_string(),
                display_name: "Usage Journal Space".to_string(),
                default_scope: "tenant".to_string(),
            },
        )
        .await
        .unwrap();

    MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "usage-record-1".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: None,
            predicate: None,
            object_text: "usage fact one".to_string(),
            canonical_text: "usage fact one".to_string(),
            sensitivity_level: "internal".to_string(),
            journal: metered_journal("usage-record-1", "memory.record.create"),
            expires_at: None,
            metadata_json: None,
        },
    )
    .await
    .expect("metered canonical create must commit");

    // An unknown journal action must stay unmetered and never fail.
    MemoryRecordStorePort::create_canonical_atomic(
        &store,
        CreateCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "usage-record-2".to_string(),
            scope_label: "user".to_string(),
            memory_type: "semantic".to_string(),
            subject: None,
            predicate: None,
            object_text: "unmetered promotion".to_string(),
            canonical_text: "unmetered promotion".to_string(),
            sensitivity_level: "internal".to_string(),
            journal: metered_journal("usage-record-2", "memory.candidate.promoted"),
            expires_at: None,
            metadata_json: None,
        },
    )
    .await
    .expect("unmetered canonical create must commit");

    let receipt = MemoryRecordStorePort::delete_canonical_atomic(
        &store,
        DeleteCanonicalMemoryCommand {
            scope: scope.clone(),
            memory_id: "usage-record-1".to_string(),
            journal: metered_journal("usage-record-1", "memory.record.delete"),
        },
    )
    .await
    .expect("metered canonical delete must commit");
    assert!(receipt.deleted);

    let charges = usage_charges_by_metric(&store, 51).await;
    let total = |metric: &str| {
        charges
            .iter()
            .filter(|(_, charge_metric, _)| charge_metric == metric)
            .map(|(_, _, delta)| *delta)
            .sum::<i64>()
    };
    assert_eq!(total("record.create"), 1, "only the create action charges");
    assert_eq!(total("record.delete"), -1);
    assert_eq!(
        total("event.create"),
        0,
        "no event action was journaled, so no event charge exists"
    );
}

#[tokio::test]
async fn sqlite_usage_daily_listing_is_keyset_paged_over_day_metric() {
    let store = new_contract_store().await;
    const TENANT: i64 = 4244;
    for (day, metric) in [
        ("2026-06-01", "event.create"),
        ("2026-06-01", "record.create"),
        ("2026-06-01", "record.delete"),
        ("2026-06-02", "retrieval.count"),
    ] {
        charge_usage(&store, TENANT, day, metric, 1).await;
    }

    let page_keys = |rows: &[NativeSqlUsageDailyRow]| {
        rows.iter()
            .map(|row| (row.day.clone(), row.metric.clone()))
            .collect::<Vec<_>>()
    };

    // page_size + 1 lookahead: the first page carries the sentinel row.
    let first_page = store
        .list_usage_daily_for_tenant(TENANT, None, 2)
        .await
        .expect("first usage page");
    assert_eq!(
        page_keys(&first_page),
        vec![
            ("2026-06-01".to_string(), "event.create".to_string()),
            ("2026-06-01".to_string(), "record.create".to_string()),
            ("2026-06-01".to_string(), "record.delete".to_string()),
        ]
    );

    let second_page = store
        .list_usage_daily_for_tenant(TENANT, Some(("2026-06-01", "record.create")), 2)
        .await
        .expect("second usage page");
    assert_eq!(
        page_keys(&second_page),
        vec![
            ("2026-06-01".to_string(), "record.delete".to_string()),
            ("2026-06-02".to_string(), "retrieval.count".to_string()),
        ]
    );

    let last_page = store
        .list_usage_daily_for_tenant(TENANT, Some(("2026-06-02", "retrieval.count")), 2)
        .await
        .expect("usage page past the end");
    assert!(
        last_page.is_empty(),
        "an exclusive cursor at the last row ends the window"
    );
}

#[tokio::test]
async fn sqlite_retrieval_trace_charges_usage_best_effort() {
    let store = new_contract_store().await;
    let scope = MemoryScopeContext::for_test(53, 53);
    store
        .create_space_record(
            53,
            53,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "530".to_string(),
                space_type: "workspace".to_string(),
                display_name: "Usage Trace Space".to_string(),
                default_scope: "tenant".to_string(),
            },
        )
        .await
        .unwrap();

    store
        .append_retrieval_trace(&AppendMemoryRetrievalTraceCommand {
            scope: scope.clone(),
            trace_id: "trace-usage-1".to_string(),
            actor_id: None,
            query_text: Some("usage probe".to_string()),
            query_hash: "sha256:usage-probe".to_string(),
            retrievers_json: None,
            latency_ms: Some(1),
            degraded: false,
            metadata_json: None,
            hits: Vec::new(),
            context_pack: None,
        })
        .await
        .expect("append retrieval trace");

    let charges = usage_charges_by_metric(&store, 53).await;
    assert_eq!(
        charges,
        vec![(charges[0].0.clone(), "retrieval.count".to_string(), 1)],
        "each stored retrieval trace charges exactly one retrieval.count"
    );
}

#[tokio::test]
async fn sqlite_find_active_tenant_quota_policy_parses_and_fails_closed() {
    let store = new_contract_store().await;
    async fn insert_quota_policy(
        store: &NativeSqlMemoryStore,
        id: i64,
        tenant_id: i64,
        policy_json: &str,
    ) {
        let uuid = id.to_string();
        let scope_ref = tenant_id.to_string();
        store
            .insert_policy(InsertPolicyCommand {
                id,
                uuid: &uuid,
                tenant_id,
                policy_type: "memory.quota",
                scope: "tenant",
                scope_ref: Some(&scope_ref),
                policy_json,
            })
            .await
            .expect("insert tenant quota policy");
    }

    insert_quota_policy(&store, 9001, 61, r#"{"maxRecordsPerSpace": 5, "maxSpacesPerUser": 7}"#)
        .await;
    insert_quota_policy(&store, 9002, 62, r#"{"maxRecordsPerSpace": 12345}"#).await;
    insert_quota_policy(&store, 9003, 63, "not-json").await;
    insert_quota_policy(&store, 9004, 64, r#"{"maxSpacesPerUser": 0}"#).await;

    let full = store
        .find_active_tenant_quota_policy(61)
        .await
        .expect("full policy resolves");
    assert_eq!(
        full.map(|policy| policy.max_records_per_space),
        Some(Some(5))
    );
    assert_eq!(
        full.map(|policy| policy.max_spaces_per_user),
        Some(Some(7))
    );

    let partial = store
        .find_active_tenant_quota_policy(62)
        .await
        .expect("partial policy resolves");
    assert_eq!(
        partial.map(|policy| policy.max_records_per_space),
        Some(Some(12_345))
    );
    assert_eq!(
        partial.map(|policy| policy.max_spaces_per_user),
        Some(None),
        "an absent key stays absent so the service can fall back to the environment default"
    );

    let missing = store
        .find_active_tenant_quota_policy(65)
        .await
        .expect("missing policy resolves to none");
    assert_eq!(missing, None);

    let malformed = store
        .find_active_tenant_quota_policy(63)
        .await
        .expect_err("malformed policy JSON must fail closed");
    assert!(matches!(
        malformed,
        NativeSqlStoreError::InvariantViolation { .. }
    ));

    let non_positive = store
        .find_active_tenant_quota_policy(64)
        .await
        .expect_err("a zero limit would disable the quota and must fail closed");
    assert!(matches!(
        non_positive,
        NativeSqlStoreError::InvariantViolation { .. }
    ));
}
