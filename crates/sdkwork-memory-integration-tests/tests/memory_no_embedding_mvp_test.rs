use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, ListMemoriesQuery, MemoryContextPackRequest,
    MemoryImplementationKind, MemoryOpenApi, MemoryOpenApiRequestContext, MemoryRecordPatch,
    MemoryRecordRequest, MemoryRetrievalRequest, MemoryType,
};

fn open_context() -> MemoryOpenApiRequestContext {
    MemoryOpenApiRequestContext::for_open_surface("api-key-001", 100_001, Some(2001))
}

#[tokio::test]
async fn remembers_retrieves_and_builds_context_without_embeddings() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let service = OpenMemoryService::new(store);
    let context = open_context();

    let capabilities = service
        .retrieve_capabilities(context.clone())
        .await
        .expect("retrieve capabilities");
    assert_eq!(
        capabilities.implementation_kinds,
        vec![MemoryImplementationKind::LocalEmbedded]
    );
    assert_eq!(
        capabilities.metadata.as_ref().and_then(|metadata| {
            metadata
                .get("activeProfileId")
                .and_then(serde_json::Value::as_str)
        }),
        Some("local-embedded-phase1")
    );
    assert_eq!(
        capabilities.metadata.as_ref().and_then(|metadata| {
            metadata
                .get("deploymentQualification")
                .and_then(serde_json::Value::as_str)
        }),
        Some("local")
    );
    assert_eq!(
        capabilities.metadata.as_ref().and_then(|metadata| {
            metadata
                .get("dynamicProfileCutover")
                .and_then(serde_json::Value::as_bool)
        }),
        Some(false)
    );

    service
        .create_memory(
            context.clone(),
            MemoryRecordRequest {
                space_id: 2,
                scope: "user".to_string(),
                memory_type: MemoryType::Semantic,
                subject: None,
                predicate: None,
                object_text: Some("concise".to_string()),
                canonical_text: "User prefers concise answers".to_string(),
                summary_text: None,
                user_id: None,
                language: None,
                sensitivity_level: None,
                expires_at: None,
                metadata: None,
                tags: None,
            },
        )
        .await
        .expect("create memory");

    let retrieval = service
        .create_retrieval(
            context.clone(),
            MemoryRetrievalRequest {
                query: "concise answers".to_string(),
                space_ids: vec![2],
                actor_id: None,
                retrieval_profile_id: None,
                memory_types: None,
                filters: None,
                top_k: 5,
                context_budget_tokens: 512,
                show_expired: None,
                include_trace: None,
            },
        )
        .await
        .expect("retrieve");

    assert!(!retrieval.hits.is_empty());
    assert!(retrieval.hits.iter().any(|hit| {
        hit.explanation
            .as_ref()
            .and_then(|explanation| explanation["contributingRetrievers"].as_array())
            .is_some_and(|retrievers| retrievers.iter().any(|name| name == "keyword"))
    }));
    assert!(!retrieval.hits.iter().any(|hit| {
        hit.retriever_name == "vector"
            || hit
                .explanation
                .as_ref()
                .and_then(|explanation| explanation["contributingRetrievers"].as_array())
                .is_some_and(|retrievers| retrievers.iter().any(|name| name == "vector"))
    }));

    let pack = service
        .create_context_pack(
            context,
            MemoryContextPackRequest {
                query: "concise answers".to_string(),
                space_ids: vec![2],
                actor_id: None,
                retrieval_profile_id: None,
                context_budget_tokens: 512,
                include_citations: None,
                filters: None,
            },
        )
        .await
        .expect("context pack");

    let fragments = pack.pack["fragments"].as_array().expect("fragments");
    assert!(fragments.iter().any(|fragment| {
        fragment["canonicalText"]
            .as_str()
            .unwrap_or("")
            .contains("concise")
    }));
    assert_eq!(pack.pack["embeddingOptional"], true);
}

#[tokio::test]

async fn create_memory_accepts_and_echoes_contract_declared_expires_at() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;

    let service = OpenMemoryService::new(store);

    let context = open_context();

    let created = service
        .create_memory(
            context.clone(),
            MemoryRecordRequest {
                space_id: 2,

                scope: "user".to_string(),

                memory_type: MemoryType::Semantic,

                subject: None,

                predicate: None,

                object_text: Some("vacation plan".to_string()),

                canonical_text: "Trip to Kyoto planned for April 2027".to_string(),

                summary_text: None,

                user_id: None,

                language: None,

                sensitivity_level: None,

                expires_at: Some("2027-04-01T00:00:00Z".to_string()),

                metadata: None,

                tags: None,
            },
        )
        .await
        .expect("create memory with expiration");

    // The service canonicalises the caller's timestamp to the store's UTC
    // format before persisting, mirroring mem0's write-time normalisation.
    assert_eq!(
        created.expires_at.as_deref(),
        Some("2027-04-01T00:00:00.000Z"),
        "the response must echo the (canonicalised) contract-declared expiresAt"
    );

    let memory_id = created.memory_id;

    let fetched = service
        .retrieve_memory(context, memory_id, 2)
        .await
        .expect("fetch created memory");

    assert_eq!(
        fetched.expires_at.as_deref(),
        Some("2027-04-01T00:00:00.000Z"),
        "the stored expiration must survive the read path"
    );
}

#[tokio::test]

async fn retrieval_and_listing_hide_expired_records_unless_show_expired() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;

    let service = OpenMemoryService::new(store);

    let context = open_context();

    let request = |memory_id: &str, expires_at: Option<&str>| MemoryRecordRequest {
        space_id: 2,

        scope: "user".to_string(),

        memory_type: MemoryType::Semantic,

        subject: None,

        predicate: None,

        object_text: Some(memory_id.to_string()),

        canonical_text: format!("Kanata keymap {memory_id}"),

        summary_text: None,

        user_id: None,

        language: None,

        sensitivity_level: None,

        expires_at: expires_at.map(str::to_string),

        metadata: None,

        tags: None,
    };

    service
        .create_memory(
            context.clone(),
            request("expired", Some("2020-01-01T00:00:00Z")),
        )
        .await
        .expect("create expired record");

    service
        .create_memory(
            context.clone(),
            request("living", Some("2099-01-01T00:00:00Z")),
        )
        .await
        .expect("create living record");

    let retrieval_request = |show_expired: Option<bool>| MemoryRetrievalRequest {
        query: "kanata keymap".to_string(),

        space_ids: vec![2],

        actor_id: None,

        retrieval_profile_id: None,

        memory_types: None,

        filters: None,

        top_k: 5,

        context_budget_tokens: 512,

        show_expired,

        include_trace: None,
    };

    let default_hits = service
        .create_retrieval(context.clone(), retrieval_request(None))
        .await
        .expect("default retrieval");

    assert!(
        !default_hits.hits.iter().any(|hit| hit
            .memory
            .as_ref()
            .is_some_and(|m| m.canonical_text.contains("expired"))),
        "expired records must be hidden from retrieval by default"
    );

    let shown_hits = service
        .create_retrieval(context.clone(), retrieval_request(Some(true)))
        .await
        .expect("show_expired retrieval");

    assert!(
        shown_hits.hits.iter().any(|hit| hit
            .memory
            .as_ref()
            .is_some_and(|m| m.canonical_text.contains("expired"))),
        "showExpired=true must surface the expired record"
    );

    let listing = service
        .list_memories(
            context.clone(),
            ListMemoriesQuery {
                q: None,

                cursor: None,

                page_size: None,

                space_id: Some(2),

                memory_type: None,

                show_expired: None,
            },
        )
        .await
        .expect("default listing");

    assert!(
        !listing
            .items
            .iter()
            .any(|record| record.canonical_text.contains("expired")),
        "expired records must be hidden from listings by default"
    );

    let shown_listing = service
        .list_memories(
            context,
            ListMemoriesQuery {
                q: None,

                cursor: None,

                page_size: None,

                space_id: Some(2),

                memory_type: None,

                show_expired: Some(true),
            },
        )
        .await
        .expect("show_expired listing");

    assert!(
        shown_listing
            .items
            .iter()
            .any(|record| record.canonical_text.contains("expired")),
        "showExpired=true must surface the expired record in listings"
    );
}

#[tokio::test]

async fn create_memory_rejects_unparsable_expires_at_instead_of_storing_it() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;

    let service = OpenMemoryService::new(store);

    let context = open_context();

    let outcome = service
        .create_memory(
            context,
            MemoryRecordRequest {
                space_id: 2,

                scope: "user".to_string(),

                memory_type: MemoryType::Semantic,

                subject: None,

                predicate: None,

                object_text: Some("bad date".to_string()),

                canonical_text: "Bad date record".to_string(),

                summary_text: None,

                user_id: None,

                language: None,

                sensitivity_level: None,

                expires_at: Some("not-a-date".to_string()),

                metadata: None,

                tags: None,
            },
        )
        .await;

    assert!(
        outcome.is_err(),
        "an unparsable expiresAt must fail validation instead of being stored"
    );
}

#[tokio::test]

async fn delete_all_memories_sweeps_active_records_and_reports_the_ids() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;

    let service = OpenMemoryService::new(store);

    let context = open_context();

    let request = |text: &str| MemoryRecordRequest {
        space_id: 2,

        scope: "user".to_string(),

        memory_type: MemoryType::Semantic,

        subject: None,

        predicate: None,

        object_text: Some(text.to_string()),

        canonical_text: text.to_string(),

        summary_text: None,

        user_id: None,

        language: None,

        sensitivity_level: None,

        expires_at: None,

        metadata: None,

        tags: None,
    };

    service
        .create_memory(context.clone(), request("Sweep target one"))
        .await
        .expect("create one");

    service
        .create_memory(context.clone(), request("Sweep target two"))
        .await
        .expect("create two");

    let result = service
        .delete_all_memories(
            context.clone(),
            DeleteAllMemoriesRequest {
                space_id: 2,
                user_id: None,
            },
        )
        .await
        .expect("delete all");

    assert_eq!(result.deleted_count, 2);

    assert_eq!(result.deleted_memory_ids.len(), 2);

    // The sweep is idempotent: a repeat deletes nothing and still succeeds.

    let repeat = service
        .delete_all_memories(
            context,
            DeleteAllMemoriesRequest {
                space_id: 2,
                user_id: None,
            },
        )
        .await
        .expect("repeat delete all");

    assert_eq!(repeat.deleted_count, 0);
}

#[tokio::test]

async fn metadata_is_persisted_merged_and_filterable() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;

    let service = OpenMemoryService::new(store);

    let context = open_context();

    let created = service
        .create_memory(
            context.clone(),
            MemoryRecordRequest {
                space_id: 2,

                scope: "user".to_string(),

                memory_type: MemoryType::Semantic,

                subject: None,

                predicate: None,

                object_text: Some("metadata filterable".to_string()),

                canonical_text: "User works in Rust with tracing".to_string(),

                summary_text: None,

                user_id: None,

                language: None,

                sensitivity_level: None,

                expires_at: None,

                metadata: Some(serde_json::json!({

                    "topic": "tooling",

                    "env": "dev"

                })),

                tags: None,
            },
        )
        .await
        .expect("create with metadata");

    assert_eq!(
        created.metadata,
        Some(serde_json::json!({"topic": "tooling", "env": "dev"})),
        "the response must echo the persisted metadata"
    );

    // A metadata patch shallow-merges: incoming keys win, others survive.

    let merged = service
        .update_memory(
            context.clone(),
            created.memory_id,
            2,
            MemoryRecordPatch {
                canonical_text: None,

                subject: None,

                summary_text: None,

                metadata: Some(serde_json::json!({ "topic": "rust-tooling" })),
            },
        )
        .await
        .expect("patch metadata");

    assert_eq!(
        merged.metadata,
        Some(serde_json::json!({"topic": "rust-tooling", "env": "dev"})),
        "metadata patches must shallow-merge like mem0 updates"
    );

    // Metadata filters now evaluate over caller-written metadata end to end.

    let filtered = service
        .create_retrieval(
            context.clone(),
            MemoryRetrievalRequest {
                query: "rust tracing".to_string(),

                space_ids: vec![2],

                actor_id: None,

                retrieval_profile_id: None,

                memory_types: None,

                filters: Some(serde_json::json!({

                    "AND": [{ "topic": { "eq": "rust-tooling" } }]

                })),

                top_k: 5,

                context_budget_tokens: 512,

                show_expired: None,

                include_trace: None,
            },
        )
        .await
        .expect("filtered retrieval");

    assert!(
        !filtered.hits.is_empty(),
        "a metadata filter must hit records whose metadata was written by the caller"
    );
}

#[tokio::test]

async fn entity_provenance_boost_surfaces_graph_linked_memories() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;

    let pool = store.pool().clone();

    let service = OpenMemoryService::new(store);

    let context = open_context();

    // The candidate memory deliberately shares no lexical token with the

    // query below; only the entity signal can surface it.

    let memory = service
        .create_memory(
            context.clone(),
            MemoryRecordRequest {
                space_id: 2,

                scope: "user".to_string(),

                memory_type: MemoryType::Semantic,

                subject: None,

                predicate: None,

                object_text: Some("weekly crew sync".to_string()),

                canonical_text: "Crew sync notes mention Kanata".to_string(),

                summary_text: None,

                user_id: None,

                language: None,

                sensitivity_level: None,

                expires_at: None,

                metadata: None,

                tags: None,
            },
        )
        .await
        .expect("create provenance memory");

    let entity_a = service
        .create_entity(
            context.clone(),
            sdkwork_memory_contract::CreateEntityCommand {
                tenant_id: 100_001,

                space_id: 2,

                entity_type: "tool".to_string(),

                canonical_name: "Kanata".to_string(),

                aliases: None,

                attributes: None,

                sensitivity_level: "internal".to_string(),
            },
        )
        .await
        .expect("create entity a");

    let entity_b = service
        .create_entity(
            context.clone(),
            sdkwork_memory_contract::CreateEntityCommand {
                tenant_id: 100_001,

                space_id: 2,

                entity_type: "tool".to_string(),

                canonical_name: "Remap".to_string(),

                aliases: None,

                attributes: None,

                sensitivity_level: "internal".to_string(),
            },
        )
        .await
        .expect("create entity b");

    let edge = service
        .create_edge(
            context.clone(),
            sdkwork_memory_contract::CreateEdgeCommand {
                tenant_id: 100_001,

                space_id: 2,

                source_entity_id: entity_a.entity_id.clone(),

                target_entity_id: entity_b.entity_id.clone(),

                relation_type: "co_occurs_with".to_string(),

                source_memory_id: Some(memory.memory_id.to_string()),

                weight: None,

                valid_from: None,

                valid_to: None,

                metadata: None,
            },
        )
        .await
        .expect("create provenance edge");

    assert_eq!(
        edge.source_memory_id.as_deref(),
        Some(memory.memory_id.to_string())
            .as_deref()
            .map(|s| s as &str)
            .or(edge.source_memory_id.as_deref())
    );

    // A lexically stronger rival: its text matches the query more directly,
    // so under the default profile it outranks the graph-linked memory.
    let _rival = service
        .create_memory(
            context.clone(),
            MemoryRecordRequest {
                space_id: 2,
                scope: "user".to_string(),
                memory_type: MemoryType::Semantic,
                subject: None,
                predicate: None,
                object_text: Some("Kanata keyboard firmware".to_string()),
                canonical_text: "Kanata is keyboard firmware".to_string(),
                summary_text: None,
                user_id: None,
                language: None,
                sensitivity_level: None,
                expires_at: None,
                metadata: None,
                tags: None,
            },
        )
        .await
        .expect("create lexical rival");

    // A retrieval profile that grants the entity signal a positive weight.

    sqlx::query(
        r#"

        INSERT INTO ai_retrieval_profile (
          id, uuid, tenant_id, space_id, name, strategy, retrievers_json,
          fusion_policy_json, rerank_policy_json,
          top_k, context_budget_tokens, status, created_at, updated_at, version
        )

        VALUES (5001, '5001', 100001, NULL, 'entity-aware', 'custom_weighted_rrf',

                '{ "keyword": { "weight": 1.0 }, "entity": { "weight": 0.9 } }',

                NULL, NULL, 5, 512, 'active',

                '2026-09-24T00:00:00.000Z', '2026-09-24T00:00:00.000Z', 1)

        "#,
    )
    .execute(&pool)
    .await
    .expect("seed entity-aware profile");

    let request = |retrieval_profile_id: Option<u64>| MemoryRetrievalRequest {
        query: "Kanata".to_string(),

        space_ids: vec![2],

        actor_id: None,

        retrieval_profile_id,

        memory_types: None,

        filters: None,

        top_k: 5,

        context_budget_tokens: 512,

        show_expired: None,

        include_trace: None,
    };

    let with_entity = service
        .create_retrieval(context.clone(), request(Some(5001)))
        .await
        .expect("entity-aware retrieval");

    let rank_in =
        |hits: &[sdkwork_memory_contract::MemoryRetrievalHit], target: u64| -> Option<usize> {
            hits.iter()
                .position(|hit| hit.memory.as_ref().is_some_and(|m| m.memory_id == target))
        };

    // With the entity signal granted, the graph-linked memory outranks the
    // lexically stronger rival; under the default profile it cannot.
    let with_rank = rank_in(&with_entity.hits, memory.memory_id);
    assert_eq!(
        with_rank,
        Some(0),
        "the entity signal must promote the graph-linked memory to the top"
    );

    let without_entity = service
        .create_retrieval(context, request(None))
        .await
        .expect("default retrieval");
    let without_rank = rank_in(&without_entity.hits, memory.memory_id);
    assert_ne!(
        without_rank,
        Some(0),
        "without the entity profile the lexically stronger memory must lead"
    );
}
