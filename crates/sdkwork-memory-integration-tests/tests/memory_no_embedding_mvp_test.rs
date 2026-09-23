use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, ListCandidatesQuery, ListMemoriesQuery, MemoryContextPackRequest,
    MemoryEventRequest, MemoryExtractionRequest, MemoryImplementationKind, MemoryOpenApi,
    MemoryOpenApiRequestContext, MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalRequest,
    MemoryType,
};

/// Test double: vectors keyed by topic marker. Texts containing "gantt" land
/// on one direction, everything else on an orthogonal one, so cosine
/// similarity is fully determined by the fixture texts.
struct ScriptedEmbedder;

#[async_trait::async_trait]
impl EmbeddingModelPort for ScriptedEmbedder {
    fn provider_code(&self) -> &str {
        "scripted"
    }

    fn dimensions(&self) -> usize {
        2
    }

    async fn embed(&self, command: EmbeddingCommand) -> Result<Vec<f32>, MemorySpiError> {
        if command.input.to_lowercase().contains("review") {
            Ok(vec![0.0, 1.0])
        } else {
            Ok(vec![1.0, 0.0])
        }
    }
}

use sdkwork_memory_retrieval::MemoryRetrievalStrategy;
use sdkwork_memory_spi::{
    EmbeddingCommand, EmbeddingModelPort, LanguageModelCommand, LanguageModelPort, MemorySpiError,
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
                threshold: None,
                explain: None,
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
        threshold: None,
        explain: None,

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
                threshold: None,
                explain: None,
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
        threshold: None,
        explain: None,

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

#[tokio::test]
async fn vector_signal_promotes_the_semantically_near_memory_when_bound() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let pool = store.pool().clone();
    let service =
        OpenMemoryService::new(store).with_embedder(std::sync::Arc::new(ScriptedEmbedder));
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
    let near = service
        .create_memory(context.clone(), request("Gantt chart review cadence"))
        .await
        .expect("create near memory");
    let rival = service
        .create_memory(context.clone(), request("Gantt chart checklist"))
        .await
        .expect("create lexical rival");

    sqlx::query(
        r#"
        INSERT INTO ai_retrieval_profile (
          id, uuid, tenant_id, space_id, name, strategy, retrievers_json,
          fusion_policy_json, rerank_policy_json,
          top_k, context_budget_tokens, status, created_at, updated_at, version
        )
        VALUES (5002, '5002', 100001, NULL, 'vector-aware', 'custom_weighted_rrf',
                '{ "keyword": { "weight": 1.0 }, "vector": { "weight": 1.2 } }',
                NULL, NULL, 5, 512, 'active',
                '2026-09-24T00:00:00.000Z', '2026-09-24T00:00:00.000Z', 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed vector-aware profile");

    let request = |retrieval_profile_id: Option<u64>| MemoryRetrievalRequest {
        query: "gantt chart review".to_string(),
        space_ids: vec![2],
        actor_id: None,
        retrieval_profile_id,
        memory_types: None,
        filters: None,
        top_k: 5,
        context_budget_tokens: 512,
        show_expired: None,
        threshold: None,
        explain: None,
        include_trace: None,
    };

    let rank_of =
        |hits: &[sdkwork_memory_contract::MemoryRetrievalHit], target: u64| -> Option<usize> {
            hits.iter()
                .position(|hit| hit.memory.as_ref().is_some_and(|m| m.memory_id == target))
        };

    let with_vector = service
        .create_retrieval(context.clone(), request(Some(5002)))
        .await
        .expect("vector-aware retrieval");
    let lexical_only = service
        .create_retrieval(context, request(None))
        .await
        .expect("lexical retrieval");

    // The scripted embedder puts the "near" memory on the query vector and
    // the rival on an orthogonal one, so with the vector profile granted the
    // "near" hit must carry a vector contribution; the orthogonal rival must
    // not. Under the lexical-only profile no hit carries one.
    let has_vector_contribution = |hits: &[sdkwork_memory_contract::MemoryRetrievalHit],
                                   target: u64| {
        hits.iter()
            .filter(|hit| hit.memory.as_ref().is_some_and(|m| m.memory_id == target))
            .any(|hit| {
                hit.explanation
                    .as_ref()
                    .and_then(|explanation| explanation["contributingRetrievers"].as_array())
                    .is_some_and(|retrievers| retrievers.iter().any(|name| name == "vector"))
            })
    };
    println!(
        "HITS={:?}",
        with_vector
            .hits
            .iter()
            .map(|hit| (&hit.retriever_name, hit.explanation.clone()))
            .collect::<Vec<_>>()
    );
    assert!(
        has_vector_contribution(&with_vector.hits, near.memory_id),
        "the near memory must be scored by the vector signal when a provider is bound"
    );
    assert!(
        !has_vector_contribution(&with_vector.hits, rival.memory_id),
        "an orthogonal embedding must not earn a vector contribution"
    );
    assert!(!lexical_only.hits.iter().any(|hit| hit
        .explanation
        .as_ref()
        .and_then(|explanation| explanation["contributingRetrievers"].as_array())
        .is_some_and(|retrievers| retrievers.iter().any(|name| name == "vector"))));
}

#[tokio::test]
async fn additive_hybrid_strategy_gates_on_semantic_threshold_and_explains() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let service = OpenMemoryService::new(store)
        .with_embedder(std::sync::Arc::new(ScriptedEmbedder))
        .with_retrieval_strategy(MemoryRetrievalStrategy::AdditiveHybrid);
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
    // "review" puts the near memory on the query vector; the rival is
    // orthogonal, so a positive threshold must gate it out entirely.
    let near = service
        .create_memory(context.clone(), request("Gantt chart review cadence"))
        .await
        .expect("create near memory");
    let rival = service
        .create_memory(context.clone(), request("Gantt chart checklist"))
        .await
        .expect("create orthogonal memory");

    let run =
        async |explain: bool, threshold: Option<f64>, context: &MemoryOpenApiRequestContext| {
            service
                .create_retrieval(
                    context.clone(),
                    MemoryRetrievalRequest {
                        query: "gantt chart review".to_string(),
                        space_ids: vec![2],
                        actor_id: None,
                        retrieval_profile_id: None,
                        memory_types: None,
                        filters: None,
                        top_k: 5,
                        context_budget_tokens: 512,
                        show_expired: None,
                        threshold,
                        explain: Some(explain),
                        include_trace: None,
                    },
                )
                .await
                .expect("additive retrieval")
        };

    let explained = run(true, Some(0.0), &context).await;
    let near_hit = explained
        .hits
        .iter()
        .find(|hit| {
            hit.memory
                .as_ref()
                .is_some_and(|m| m.memory_id == near.memory_id)
        })
        .expect("near memory must survive a zero threshold");
    let details = near_hit.explanation.as_ref().expect("explanation")["scoreDetails"].clone();
    assert_eq!(
        details["semanticScore"],
        serde_json::json!(1.0),
        "the scripted near vector must score a full semantic match"
    );
    // maxPossible grows to 2.0 once BM25 participates, and the final score is
    // the raw sum clamped by it - mem0's exact arithmetic.
    let max_possible = details["maxPossibleScore"].as_f64().unwrap();
    assert!((max_possible - 2.0).abs() < 1e-9);
    let final_score = details["finalScore"].as_f64().unwrap();
    assert!(final_score > 0.5 && final_score <= 1.0);
    assert_eq!(details["threshold"], serde_json::json!(0.0));

    // A positive threshold gates on the semantic score alone: the orthogonal
    // memory has zero semantic score, so no keyword signal can rescue it.
    let gated = run(false, Some(0.5), &context).await;
    assert!(
        gated.hits.iter().all(|hit| hit
            .memory
            .as_ref()
            .is_some_and(|m| m.memory_id != rival.memory_id)),
        "the orthogonal memory must be thresholded out by its semantic score"
    );
    assert!(
        gated.hits.iter().any(|hit| hit
            .memory
            .as_ref()
            .is_some_and(|m| m.memory_id == near.memory_id)),
        "the on-vector memory must clear the threshold"
    );

    let _ = rival;
}

/// Test double LLM: returns a fixed additive envelope with two facts, one
/// attributed to the user and one to the assistant.
struct ScriptedLlm {
    seen_prompts: std::sync::Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl LanguageModelPort for ScriptedLlm {
    fn provider_code(&self) -> &str {
        "scripted"
    }

    async fn generate(&self, command: LanguageModelCommand) -> Result<String, MemorySpiError> {
        self.seen_prompts
            .lock()
            .unwrap()
            .push(command.prompt.clone());
        Ok(String::from(
            r#"{"memory": [
                {"id": "0", "text": "User prefers Portland roasters", "attributed_to": "user", "linked_memory_ids": []},
                {"id": "1", "text": "Assistant recommended Stumptown", "attributed_to": "assistant", "linked_memory_ids": []}
            ]}"#,
        ))
    }
}

#[tokio::test]
async fn extraction_runs_llm_additive_pipeline_when_a_provider_is_bound() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let llm = std::sync::Arc::new(ScriptedLlm {
        seen_prompts: std::sync::Mutex::new(Vec::new()),
    });
    let service = OpenMemoryService::new(store).with_llm(llm.clone());
    let context = open_context();

    let event = service
        .create_event(
            context.clone(),
            MemoryEventRequest {
                space_id: 2,
                user_id: None,
                actor_type: Some("user".to_string()),
                actor_id: Some("9001".to_string()),
                session_id: None,
                trace_id: None,
                event_type: "conversation.turn".to_string(),
                source_type: "conversation".to_string(),
                source_ref: None,
                event_time: "2026-09-24T00:00:00.000Z".to_string(),
                payload: serde_json::json!({
                    "content": "I tried Portland roasters and loved it"
                }),
                sensitivity_level: None,
            },
        )
        .await
        .expect("create conversation event");
    let event_id = event.event_id;

    // LLM mode (the default when a provider is bound): every accepted fact
    // becomes its own candidate.
    let result = service
        .run_extraction_now(
            context.clone(),
            MemoryExtractionRequest {
                space_id: 2,
                input_events: vec![event_id],
                extraction_mode: None,
                custom_instructions: Some("Focus on coffee preferences".to_string()),
                observation_date: Some("2026-09-20".to_string()),
            },
        )
        .await
        .expect("llm extraction");
    assert_eq!(result["extractionMode"], "additive_llm");
    assert_eq!(result["candidateCount"], 2);
    assert_eq!(result["refusedCount"], 0);

    // The prompt the model saw must carry the caller's temporal anchor and
    // custom instructions - the mem0 Observation Date / add(prompt=...) inputs.
    let prompts = llm.seen_prompts.lock().unwrap();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].contains("2026-09-20"), "observation date anchor");
    assert!(
        prompts[0].contains("Focus on coffee preferences"),
        "custom instructions"
    );

    let listed = service
        .list_candidates(
            context.clone(),
            ListCandidatesQuery {
                space_id: Some(2),
                cursor: None,
                page_size: None,
            },
        )
        .await
        .expect("list candidates");
    assert!(listed
        .items
        .iter()
        .any(|candidate| { candidate.proposed_text.contains("Portland roasters") }));

    // Deterministic mode stays available as the explicit fallback.
    let deterministic = service
        .run_extraction_now(
            context,
            MemoryExtractionRequest {
                space_id: 2,
                input_events: vec![event_id],
                extraction_mode: Some("deterministic".to_string()),
                custom_instructions: None,
                observation_date: None,
            },
        )
        .await
        .expect("deterministic extraction");
    assert_eq!(deterministic["extractionMode"], "deterministic");
    assert_eq!(deterministic["candidateCount"], 1);
}

#[tokio::test]
async fn extraction_without_a_provider_keeps_the_deterministic_path() {
    let store = sdkwork_memory_test_support::space_fixtures::new_seeded_in_memory_store().await;
    let service = OpenMemoryService::new(store);
    let context = open_context();

    let event = service
        .create_event(
            context.clone(),
            MemoryEventRequest {
                space_id: 2,
                user_id: None,
                actor_type: None,
                actor_id: None,
                session_id: None,
                trace_id: None,
                event_type: "conversation.turn".to_string(),
                source_type: "conversation".to_string(),
                source_ref: None,
                event_time: "2026-09-24T00:00:00.000Z".to_string(),
                payload: serde_json::json!({ "content": "plain deterministic content" }),
                sensitivity_level: None,
            },
        )
        .await
        .expect("create event");

    let result = service
        .run_extraction_now(
            context,
            MemoryExtractionRequest {
                space_id: 2,
                input_events: vec![event.event_id],
                extraction_mode: None,
                custom_instructions: None,
                observation_date: None,
            },
        )
        .await
        .expect("deterministic extraction without provider");
    assert_eq!(result["extractionMode"], "deterministic");
    assert_eq!(result["candidateCount"], 1);
}
