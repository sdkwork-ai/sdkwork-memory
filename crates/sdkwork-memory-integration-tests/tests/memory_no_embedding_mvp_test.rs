use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, ListMemoriesQuery, MemoryContextPackRequest,
    MemoryImplementationKind, MemoryOpenApi, MemoryOpenApiRequestContext, MemoryRecordRequest,
    MemoryRetrievalRequest, MemoryType,
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
