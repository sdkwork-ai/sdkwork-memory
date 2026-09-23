//! Optional Postgres contract tests — set `SDKWORK_DATABASE_TEST_POSTGRES_URL` to run.

use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};
use sdkwork_memory_plugin_native_sql::{
    ConsolidateDuplicateRecordsCommand, InsertEntityCommand, NativeSqlAppendOutboxEventCommand,
    NativeSqlCreateSpaceCommand, NativeSqlMemoryStore, UpdateEvalRunStateCommand,
};
use sdkwork_memory_spi::{
    AppendMemoryEventCommand, AppendMemoryRetrievalTraceCommand, CreateMemoryRecordCommand,
    MemoryContextPackSnapshot, MemoryEventStorePort, MemoryMutationJournal, MemoryRecordStorePort,
    MemoryRetrievalHitDraft, MemoryRetrievalTraceStorePort, MemoryScopeContext,
    RetrieveMemoryRetrievalTraceQuery,
};
async fn postgres_store(space_ids: &[i64]) -> Option<NativeSqlMemoryStore> {
    let url = match std::env::var("SDKWORK_DATABASE_TEST_POSTGRES_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => return None,
    };
    let config = DatabaseConfig {
        engine: DatabaseEngine::Postgres,
        url,
        max_connections: 4,
        ..DatabaseConfig::default()
    };
    let store = NativeSqlMemoryStore::connect(&config)
        .await
        .expect("postgres connect and migration must succeed");
    for &space_id in space_ids {
        store
            .create_space_record(
                100_001,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: format!("postgres-contract-{space_id}"),
                    space_type: "personal".to_string(),
                    display_name: format!("PostgreSQL contract {space_id}"),
                    default_scope: "user".to_string(),
                },
            )
            .await
            .expect("create PostgreSQL contract space");
    }
    Some(store)
}

#[tokio::test]
async fn postgres_store_applies_phase1_migration_when_url_configured() {
    let Some(store) = postgres_store(&[42]).await else {
        return;
    };
    store.ping().await.expect("postgres ping must succeed");
    store
        .verify_canonical_schema()
        .await
        .expect("postgres canonical schema must be ready");

    let scope = MemoryScopeContext::for_test(100_001, 42);
    MemoryEventStorePort::append(
        &store,
        AppendMemoryEventCommand {
            scope: scope.clone(),
            event_id: "pg-event-1".to_string(),
            content: "postgres contract probe".to_string(),
        },
    )
    .await
    .expect("append event on postgres");

    MemoryRecordStorePort::create(
        &store,
        CreateMemoryRecordCommand {
            scope,
            memory_id: "pg-rec-1".to_string(),
            content: "postgres contract probe".to_string(),
        },
    )
    .await
    .expect("create record on postgres");
}

#[tokio::test]
async fn postgres_provider_health_advisory_lease_is_exclusive_and_released() {
    let Some(store) = postgres_store(&[]).await else {
        return;
    };

    let first = store
        .try_acquire_provider_health_lease()
        .await
        .expect("acquire first provider-health lease")
        .expect("first provider-health lease must be available");
    assert!(store
        .try_acquire_provider_health_lease()
        .await
        .expect("contending provider-health lease query")
        .is_none());

    drop(first);

    assert!(store
        .try_acquire_provider_health_lease()
        .await
        .expect("provider-health lease query after release")
        .is_some());
}

#[tokio::test]
async fn postgres_graph_mutation_commits_business_outbox_and_audit_atomically() {
    let Some(store) = postgres_store(&[89]).await else {
        return;
    };
    let scope = MemoryScopeContext {
        tenant_id: 100_001,
        space_id: 89,
        organization_id: None,
        user_id: Some(9001),
    };
    let journal = MemoryMutationJournal {
        outbox_id: "pg-graph-outbox-1".to_string(),
        aggregate_type: "ai_entity".to_string(),
        aggregate_id: "pg-graph-entity-1".to_string(),
        event_type: "memory.entity.created".to_string(),
        event_version: "1".to_string(),
        payload_json: r#"{"resourceId":"pg-graph-entity-1"}"#.to_string(),
        audit_id: "pg-graph-audit-1".to_string(),
        audit_action: "memory.entity.created".to_string(),
        audit_resource_type: "entity".to_string(),
        audit_resource_id: "pg-graph-entity-1".to_string(),
        audit_result: "accepted".to_string(),
    };
    store
        .insert_entity_with_journal(
            InsertEntityCommand {
                id: 8_901,
                uuid: "pg-graph-entity-1",
                tenant_id: scope.tenant_id,
                space_id: scope.space_id,
                entity_type: "person",
                canonical_name: "PostgreSQL graph entity",
                aliases_json: None,
                attributes_json: None,
                sensitivity_level: "internal",
            },
            &scope,
            &journal,
        )
        .await
        .expect("journaled PostgreSQL graph insert");

    let business_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_entity WHERE tenant_id = $1 AND uuid = $2")
            .bind(scope.tenant_id)
            .bind("pg-graph-entity-1")
            .fetch_one(store.pool())
            .await
            .expect("count PostgreSQL graph entity");
    let outbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_outbox_event WHERE tenant_id = $1 AND uuid = $2",
    )
    .bind(scope.tenant_id)
    .bind("pg-graph-outbox-1")
    .fetch_one(store.pool())
    .await
    .expect("count PostgreSQL graph outbox");
    let audit_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = $1 AND uuid = $2")
            .bind(scope.tenant_id)
            .bind("pg-graph-audit-1")
            .fetch_one(store.pool())
            .await
            .expect("count PostgreSQL graph audit");
    assert_eq!((business_count, outbox_count, audit_count), (1, 1, 1));
}

#[tokio::test]
async fn postgres_store_rebuilds_search_index_for_space_scope() {
    let Some(store) = postgres_store(&[77]).await else {
        return;
    };

    let scope = MemoryScopeContext::for_test(100_001, 77);
    store
        .create_record_open_api(
            &scope,
            "pg-pred-mem",
            "user",
            "semantic",
            Some("Berlin"),
            Some("capital_of"),
            "Germany",
            "Germany capital",
            "internal",
            None,
            None,
        )
        .await
        .expect("create open api record on postgres");

    let rebuilt = store
        .rebuild_record_search_indexes_for_space(100_001, 77)
        .await
        .expect("scoped rebuild on postgres");
    assert!(rebuilt >= 1, "scoped rebuild must touch at least one row");

    let hits = store
        .search_record_details_fulltext(&scope, "capital_of", 5)
        .await
        .expect("fulltext search on postgres");
    assert!(
        hits.iter().any(|row| row.memory_id == "pg-pred-mem"),
        "postgres tsvector must index predicate"
    );
}

#[tokio::test]
async fn postgres_store_claims_eval_runs_atomically() {
    let Some(store) = postgres_store(&[]).await else {
        return;
    };

    let tenant_id = 100_001_i64;
    let eval_uuid = "pg-eval-claim-1";
    store
        .insert_mem_eval_run(tenant_id, eval_uuid, "retrieval_quality", "queued", None)
        .await
        .expect("seed eval run");

    let first = store
        .claim_queued_eval_runs(4, "eval-worker-a", "eval-lease-a", 30)
        .await
        .expect("first claim");
    assert!(
        first.iter().any(|run| run.eval_run_uuid == eval_uuid),
        "first claim must include seeded eval run"
    );
    let second = store
        .claim_queued_eval_runs(4, "eval-worker-b", "eval-lease-b", 30)
        .await
        .expect("second claim");
    assert!(
        !second.iter().any(|run| run.eval_run_uuid == eval_uuid),
        "second claim must not re-select the same eval run"
    );
    assert!(!store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id,
            eval_run_uuid: eval_uuid,
            lease_owner: "eval-worker-a",
            lease_token: "wrong-token",
            state: "succeeded",
            metrics_json: None,
            result_json: Some(r#"{"status":"wrong-token"}"#),
        })
        .await
        .expect("fence wrong eval token"));

    sqlx::query(
        "UPDATE ai_eval_run SET lease_expires_at = '1970-01-01T00:00:00.000Z' WHERE tenant_id = $1 AND uuid = $2",
    )
    .bind(tenant_id)
    .bind(eval_uuid)
    .execute(store.pool())
    .await
    .expect("expire eval lease");
    assert_eq!(
        store
            .requeue_stale_running_eval_runs(30)
            .await
            .expect("requeue expired eval lease"),
        1
    );
    let replacement = store
        .claim_queued_eval_runs(4, "eval-worker-b", "eval-lease-b", 30)
        .await
        .expect("replacement eval claim");
    assert!(replacement.iter().any(|run| run.eval_run_uuid == eval_uuid));
    assert!(!store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id,
            eval_run_uuid: eval_uuid,
            lease_owner: "eval-worker-a",
            lease_token: "eval-lease-a",
            state: "succeeded",
            metrics_json: None,
            result_json: Some(r#"{"status":"stale"}"#),
        })
        .await
        .expect("fence stale eval completion"));
    assert!(store
        .update_eval_run_state(UpdateEvalRunStateCommand {
            tenant_id,
            eval_run_uuid: eval_uuid,
            lease_owner: "eval-worker-b",
            lease_token: "eval-lease-b",
            state: "succeeded",
            metrics_json: None,
            result_json: Some(r#"{"status":"current"}"#),
        })
        .await
        .expect("complete eval with current lease"));
}

#[tokio::test]
async fn postgres_store_persists_retrieval_trace_boolean_fields() {
    let Some(store) = postgres_store(&[88]).await else {
        return;
    };

    let scope = MemoryScopeContext::for_test(100_001, 88);
    store
        .create_record(&scope, "pg-trace-rec", "answer_style", "concise")
        .await
        .expect("create record for retrieval trace");

    let appended = MemoryRetrievalTraceStorePort::append(
        &store,
        AppendMemoryRetrievalTraceCommand {
            scope: scope.clone(),
            trace_id: "pg-trace-bool".to_string(),
            actor_id: Some("user-42".to_string()),
            query_text: Some("postgres bool roundtrip".to_string()),
            query_hash: "hash:pg-trace-bool".to_string(),
            retrievers_json: Some(r#"["native_sql"]"#.to_string()),
            latency_ms: Some(21),
            degraded: true,
            metadata_json: Some(r#"{"profile":"native_sql"}"#.to_string()),
            hits: vec![MemoryRetrievalHitDraft {
                hit_id: "pg-trace-bool-hit-1".to_string(),
                memory_id: Some("pg-trace-rec".to_string()),
                space_id: Some(scope.space_id),
                retriever_name: "native_sql".to_string(),
                result_rank: 1,
                raw_score: Some(0.75),
                fused_score: Some(0.9),
                explanation_json: None,
                status: "selected".to_string(),
            }],
            context_pack: Some(MemoryContextPackSnapshot {
                context_pack_id: "pg-trace-bool-pack".to_string(),
                pack_json: r#"{"memoryIds":["pg-trace-rec"]}"#.to_string(),
                estimated_tokens: 8,
                truncated: true,
            }),
        },
    )
    .await
    .expect("append retrieval trace on postgres");

    let retrieved = MemoryRetrievalTraceStorePort::retrieve(
        &store,
        RetrieveMemoryRetrievalTraceQuery {
            scope,
            trace_id: appended.trace_id,
        },
    )
    .await
    .expect("retrieve retrieval trace on postgres")
    .expect("retrieval trace must exist");

    assert!(
        retrieved.degraded,
        "postgres degraded boolean must roundtrip as true"
    );
    assert!(
        retrieved
            .context_pack
            .as_ref()
            .map(|pack| pack.truncated)
            .unwrap_or(false),
        "postgres truncated boolean must roundtrip as true"
    );
}

#[tokio::test]
async fn postgres_store_fences_expired_outbox_delivery_leases() {
    let Some(store) = postgres_store(&[99]).await else {
        return;
    };
    let scope = MemoryScopeContext::for_test(100_001, 99);
    store
        .append_outbox_event(NativeSqlAppendOutboxEventCommand {
            scope: &scope,
            outbox_id: "pg-outbox-lease-1",
            aggregate_type: "ai_record",
            aggregate_id: "pg-record-lease-1",
            event_type: "memory.record.created",
            event_version: "1",
            payload_json: r#"{"memoryId":"pg-record-lease-1"}"#,
        })
        .await
        .expect("append PostgreSQL outbox event");

    let first = store
        .claim_global_pending_outbox_events(100, "publisher-a", "lease-a", 30)
        .await
        .expect("first PostgreSQL outbox claim");
    assert!(
        first
            .iter()
            .any(|row| row.outbox.outbox_id == "pg-outbox-lease-1"),
        "the global claim must include the lease-fencing fixture",
    );
    assert!(store
        .ack_outbox_delivery_success(
            scope.tenant_id,
            "pg-outbox-lease-1",
            "publisher-a",
            "wrong-token",
        )
        .await
        .expect("fenced PostgreSQL acknowledgement")
        .is_none());
    assert!(store
        .renew_outbox_delivery_lease(
            scope.tenant_id,
            "pg-outbox-lease-1",
            "publisher-a",
            "lease-a",
            30,
        )
        .await
        .expect("renew PostgreSQL outbox lease"));

    sqlx::query(
        "UPDATE ai_outbox_event SET lease_expires_at = '1970-01-01T00:00:00.000Z' WHERE tenant_id = $1 AND uuid = $2",
    )
    .bind(scope.tenant_id)
    .bind("pg-outbox-lease-1")
    .execute(store.pool())
    .await
    .expect("expire PostgreSQL outbox lease");
    assert_eq!(
        store
            .requeue_stale_processing_outbox_events(30)
            .await
            .expect("requeue expired PostgreSQL outbox lease"),
        1
    );

    let second = store
        .claim_global_pending_outbox_events(1, "publisher-b", "lease-b", 30)
        .await
        .expect("replacement PostgreSQL outbox claim");
    assert_eq!(second.len(), 1);
    assert!(store
        .ack_outbox_delivery_success(
            scope.tenant_id,
            "pg-outbox-lease-1",
            "publisher-a",
            "lease-a",
        )
        .await
        .expect("stale PostgreSQL acknowledgement")
        .is_none());
    let published = store
        .ack_outbox_delivery_success(
            scope.tenant_id,
            "pg-outbox-lease-1",
            "publisher-b",
            "lease-b",
        )
        .await
        .expect("current PostgreSQL acknowledgement")
        .expect("current lease must publish the event");
    assert_eq!(published.publish_state, "published");
}

#[tokio::test]
async fn postgres_consolidation_recovery_scans_more_than_one_bounded_batch() {
    let Some(store) = postgres_store(&[991]).await else {
        return;
    };
    const RECOVERY_ROWS: u32 = 501;
    let scope = MemoryScopeContext::for_test(100_001, 991);
    let operation_id = "postgres-bounded-consolidation-recovery";
    let prefix = sdkwork_utils_rust::sha256_hash(
        format!(
            "memory-consolidation:outbox:{}:{}:{operation_id}",
            scope.tenant_id, scope.space_id
        )
        .as_bytes(),
    );
    let prefix = &prefix[..32];
    let timestamp = "2026-07-22T00:00:00.000Z";

    sqlx::query(
        "DELETE FROM ai_outbox_event WHERE tenant_id = $1 AND event_type = $2 AND uuid LIKE $3",
    )
    .bind(scope.tenant_id)
    .bind("memory.record.superseded")
    .bind(format!("{prefix}%"))
    .execute(store.pool())
    .await
    .expect("clear prior PostgreSQL consolidation recovery journals");

    for index in 0..RECOVERY_ROWS {
        let memory_uuid = format!("postgres-bounded-recovery-{index}");
        let suffix = sdkwork_utils_rust::sha256_hash(memory_uuid.as_bytes());
        let journal_uuid = format!("{prefix}{}", &suffix[..32]);
        let payload = serde_json::json!({
            "operationId": operation_id,
            "tenantId": scope.tenant_id.to_string(),
            "spaceId": scope.space_id.to_string(),
            "memoryId": memory_uuid,
            "supersededByMemoryId": "postgres-bounded-recovery-winner",
            "consolidationMode": "identity_bounded_supersession",
            "transferredSources": 2,
            "deduplicatedSources": 3,
        });
        sqlx::query(
            r#"
            INSERT INTO ai_outbox_event (
              id, uuid, tenant_id, aggregate_type, aggregate_id,
              event_type, event_version, payload_json, publish_state,
              created_at, updated_at
            ) VALUES ($1, $2, $3, 'memory_record', $4, $5, '1.0', $6, 'published', $7, $7)
            "#,
        )
        .bind(-9_000_000_i64 - i64::from(index))
        .bind(journal_uuid)
        .bind(scope.tenant_id)
        .bind(&memory_uuid)
        .bind("memory.record.superseded")
        .bind(payload.to_string())
        .bind(timestamp)
        .execute(store.pool())
        .await
        .expect("insert PostgreSQL consolidation recovery journal");
    }

    let recovered = store
        .consolidate_duplicate_records_in_scope_detailed(ConsolidateDuplicateRecordsCommand {
            scope: &scope,
            operation_id,
        })
        .await
        .expect("recover bounded PostgreSQL consolidation result");

    assert_eq!(recovered.superseded_records, RECOVERY_ROWS);
    assert_eq!(recovered.transferred_sources, RECOVERY_ROWS * 2);
    assert_eq!(recovered.deduplicated_sources, RECOVERY_ROWS * 3);
}
