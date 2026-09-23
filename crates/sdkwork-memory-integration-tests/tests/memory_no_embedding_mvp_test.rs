use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    MemoryContextPackRequest, MemoryImplementationKind, MemoryOpenApi, MemoryOpenApiRequestContext,
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

    assert_eq!(
        created.expires_at.as_deref(),
        Some("2027-04-01T00:00:00Z"),
        "the response must echo the contract-declared expiresAt instead of dropping it"
    );

    let memory_id = created.memory_id;

    let fetched = service
        .retrieve_memory(context, memory_id, 2)
        .await
        .expect("fetch created memory");

    assert_eq!(
        fetched.expires_at.as_deref(),
        Some("2027-04-01T00:00:00Z"),
        "the stored expiration must survive the read path"
    );
}
