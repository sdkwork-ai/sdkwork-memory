use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{
    MemoryFeedbackRequest, MemoryOpenApi, MemoryOpenApiRequestContext, MemoryRecordRequest,
    MemoryRetrievalRequest, MemoryServiceErrorKind, MemoryType,
};
use sdkwork_memory_plugin_native_sql::{
    InsertSubjectCommand, NativeSqlCreateSpaceCommand, NativeSqlMemoryStore, NativeSqlPhase1Runtime,
};
use sdkwork_memory_retrieval::MemoryRetrievalStrategy;
use serde_json::json;

const TENANT_ID: u64 = 91_001;
const ACTOR_ID: u64 = 42;

fn context() -> MemoryOpenApiRequestContext {
    MemoryOpenApiRequestContext::for_open_surface(
        "retrieval-contract-key",
        TENANT_ID,
        Some(ACTOR_ID),
    )
}

async fn service_with_spaces() -> OpenMemoryService {
    service_with_strategy(MemoryRetrievalStrategy::Balanced).await
}

async fn service_with_strategy(strategy: MemoryRetrievalStrategy) -> OpenMemoryService {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("retrieval contract sqlite store must open");
    for (space_id, space_type) in [(1_i64, "workspace"), (2_i64, "shared")] {
        store
            .create_space_record(
                TENANT_ID as i64,
                space_id,
                &NativeSqlCreateSpaceCommand {
                    organization_id: None,
                    owner_subject_type: "user".to_string(),
                    owner_subject_id: ACTOR_ID.to_string(),
                    space_type: space_type.to_string(),
                    display_name: format!("Retrieval Contract Space {space_id}"),
                    default_scope: "user".to_string(),
                },
            )
            .await
            .expect("retrieval contract space must be created");
    }
    let prepared = OpenMemoryService::new(store.clone());
    OpenMemoryService::try_from_core_runtime_with_retrieval_strategy(
        NativeSqlPhase1Runtime::from_store(store),
        prepared.core_runtime().clone(),
        strategy,
    )
    .expect("qualified strategy must compose with native SQL runtime")
}

fn memory_request(space_id: u64, memory_type: MemoryType, text: &str) -> MemoryRecordRequest {
    MemoryRecordRequest {
        space_id,
        scope: "user".to_string(),
        memory_type,
        subject: Some("retrieval".to_string()),
        predicate: Some("matches".to_string()),
        object_text: Some(text.to_string()),
        canonical_text: text.to_string(),
        summary_text: None,
        user_id: Some(ACTOR_ID),
        language: Some("en".to_string()),
        sensitivity_level: Some("internal".to_string()),
        metadata: None,
        tags: None,
        expires_at: None,
    }
}

fn retrieval_request(
    query: &str,
    space_ids: Vec<u64>,
    top_k: i32,
    memory_types: Option<Vec<MemoryType>>,
) -> MemoryRetrievalRequest {
    MemoryRetrievalRequest {
        query: query.to_string(),
        space_ids,
        actor_id: Some(ACTOR_ID.to_string()),
        retrieval_profile_id: None,
        memory_types,
        filters: None,
        top_k,
        context_budget_tokens: 512,
        show_expired: None,
        threshold: None,
        explain: None,
        include_trace: Some(true),
    }
}

#[tokio::test]
async fn multi_space_retrieval_trace_round_trips_each_hit_scope() {
    let service = service_with_spaces().await;
    let context = context();
    for (space_id, text) in [
        (1, "shared multispace needle from space one"),
        (2, "shared multispace needle from space two"),
    ] {
        service
            .create_memory(
                context.clone(),
                memory_request(space_id, MemoryType::Semantic, text),
            )
            .await
            .unwrap();
    }

    let created = service
        .create_retrieval(
            context.clone(),
            retrieval_request("multispace needle", vec![1, 2], 5, None),
        )
        .await
        .unwrap();
    let mut created_spaces = created
        .hits
        .iter()
        .filter_map(|hit| hit.memory.as_ref().map(|memory| memory.space_id))
        .collect::<Vec<_>>();
    created_spaces.sort_unstable();
    assert_eq!(created_spaces, vec![1, 2]);

    let retrieved = service
        .retrieve_retrieval(context, created.retrieval_id)
        .await
        .unwrap();
    let mut retrieved_spaces = retrieved
        .hits
        .iter()
        .filter_map(|hit| hit.memory.as_ref().map(|memory| memory.space_id))
        .collect::<Vec<_>>();
    retrieved_spaces.sort_unstable();
    assert_eq!(retrieved_spaces, vec![1, 2]);
    assert!(retrieved.hits.iter().all(|hit| hit.memory.is_some()));
}

#[tokio::test]
async fn retrieval_filters_before_top_k_and_rejects_invalid_query_bounds() {
    let service = service_with_spaces().await;
    let context = context();
    service
        .create_memory(
            context.clone(),
            memory_request(
                1,
                MemoryType::Episodic,
                "typefilterneedle episodic should be excluded",
            ),
        )
        .await
        .unwrap();
    service
        .create_memory(
            context.clone(),
            memory_request(
                1,
                MemoryType::Semantic,
                "typefilterneedle semantic should be returned",
            ),
        )
        .await
        .unwrap();

    let result = service
        .create_retrieval(
            context.clone(),
            retrieval_request(
                "typefilterneedle",
                vec![1],
                1,
                Some(vec![MemoryType::Semantic]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(result.hits.len(), 1);
    assert_eq!(
        result.hits[0].memory.as_ref().unwrap().memory_type,
        MemoryType::Semantic
    );

    let mut invalid_budget = retrieval_request("typefilterneedle", vec![1], 1, None);
    invalid_budget.context_budget_tokens = 0;
    for request in [
        retrieval_request("   ", vec![1], 1, None),
        retrieval_request("typefilterneedle", vec![1], 0, None),
        retrieval_request("typefilterneedle", vec![1], 101, None),
        invalid_budget,
    ] {
        let error = service
            .create_retrieval(context.clone(), request)
            .await
            .expect_err("invalid retrieval input must fail before adapter search");
        assert_eq!(error.kind, MemoryServiceErrorKind::Validation);
    }
}

#[tokio::test]
async fn search_first_scheme_executes_only_its_materialized_retrievers() {
    let service = service_with_strategy(MemoryRetrievalStrategy::SearchFirst).await;
    let context = context();
    service
        .create_memory(
            context.clone(),
            memory_request(1, MemoryType::Semantic, "strategyneedle searchable memory"),
        )
        .await
        .unwrap();

    let result = service
        .create_retrieval(
            context,
            retrieval_request("strategyneedle", vec![1], 5, None),
        )
        .await
        .unwrap();
    let retrievers = result.hits[0].explanation.as_ref().unwrap()["contributingRetrievers"]
        .as_array()
        .unwrap();
    assert!(retrievers.iter().any(|value| value == "keyword"));
    assert!(retrievers.iter().any(|value| value == "dictionary"));
    assert!(!retrievers.iter().any(|value| value == "time"));
    assert!(!retrievers.iter().any(|value| value == "event"));
}

#[tokio::test]
async fn retrieval_trace_drops_hits_after_cross_space_access_is_revoked() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("retrieval contract sqlite store must open");
    store
        .create_space_record(
            TENANT_ID as i64,
            1,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: ACTOR_ID.to_string(),
                space_type: "workspace".to_string(),
                display_name: "Owned Space".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .unwrap();
    store
        .create_space_record(
            TENANT_ID as i64,
            2,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: "other-user".to_string(),
                space_type: "shared".to_string(),
                display_name: "Shared Space".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .unwrap();
    let actor_ref = ACTOR_ID.to_string();
    store
        .insert_subject(InsertSubjectCommand {
            id: 701,
            uuid: "retrieval-contract-actor",
            tenant_id: TENANT_ID as i64,
            organization_id: None,
            subject_type: "user",
            subject_ref: &actor_ref,
            display_name: "Retrieval Contract Actor",
            default_space_id: Some(1),
            metadata_json: None,
        })
        .await
        .unwrap();
    store
        .insert_binding(
            801,
            "retrieval-contract-space-binding",
            TENANT_ID as i64,
            None,
            "access",
            "learner",
            Some(701),
            None,
            Some(2),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

    let service = OpenMemoryService::new(store.clone());
    let context = context();
    for (space_id, text) in [
        (1, "revocation needle from owned space"),
        (2, "revocation needle from shared space"),
    ] {
        service
            .create_memory(
                context.clone(),
                memory_request(space_id, MemoryType::Semantic, text),
            )
            .await
            .unwrap();
    }

    let created = service
        .create_retrieval(
            context.clone(),
            retrieval_request("revocation needle", vec![1, 2], 5, None),
        )
        .await
        .unwrap();
    assert_eq!(created.hits.len(), 2);
    service
        .create_feedback(
            context.clone(),
            MemoryFeedbackRequest {
                target_type: "retrieval".to_string(),
                target_id: created.retrieval_id,
                feedback_type: "useful".to_string(),
                rating: Some(1),
                comment: None,
                metadata: None,
            },
        )
        .await
        .expect("retrieval feedback must resolve the trace through the typed trace port");

    assert!(store
        .delete_binding(TENANT_ID as i64, "retrieval-contract-space-binding")
        .await
        .unwrap());
    let retrieved = service
        .retrieve_retrieval(context, created.retrieval_id)
        .await
        .expect("revoked secondary-space hits must be filtered without failing the trace");
    assert_eq!(retrieved.hits.len(), 1);
    assert_eq!(retrieved.hits[0].memory.as_ref().unwrap().space_id, 1);
    assert_eq!(retrieved.hits[0].result_rank, 1);
    assert_eq!(retrieved.trace.as_ref().unwrap().result_count, 1);
}

#[tokio::test]
async fn retrieval_applies_the_metadata_filter_instead_of_ignoring_it() {
    let service = service_with_spaces().await;
    let context = context();
    service
        .create_memory(
            context.clone(),
            memory_request(1, MemoryType::Semantic, "filterneedle metadata record"),
        )
        .await
        .unwrap();

    // Control: with no filter the record is returned, so the filter below is what
    // changes the outcome rather than the record being absent for another reason.
    let unfiltered = service
        .create_retrieval(
            context.clone(),
            retrieval_request("filterneedle", vec![1], 5, None),
        )
        .await
        .unwrap();
    assert_eq!(unfiltered.hits.len(), 1);

    // An empty object is not a filter, so it must behave exactly like the control.
    let mut empty_filter = retrieval_request("filterneedle", vec![1], 5, None);
    empty_filter.filters = Some(json!({}));
    let empty_filter = service
        .create_retrieval(context.clone(), empty_filter)
        .await
        .expect("an empty filter object means no filter");
    assert_eq!(empty_filter.hits.len(), 1);

    // A real filter is parsed and pushed into the store search. No canonical record
    // carries a metadata document yet, so an equality filter matches nothing: the hit
    // count must drop, which can only happen if the filter was actually applied. A
    // build that ignored `filters` would return the control's single hit here.
    let mut filtered = retrieval_request("filterneedle", vec![1], 5, None);
    filtered.filters = Some(json!({"owner": "alice"}));
    let filtered = service
        .create_retrieval(context.clone(), filtered)
        .await
        .expect("a well-formed filter must be accepted");
    assert!(
        filtered.hits.is_empty(),
        "an equality filter that no record satisfies must narrow the result, got {} hit(s)",
        filtered.hits.len()
    );
}

#[tokio::test]
async fn retrieval_refuses_a_metadata_filter_it_cannot_parse() {
    let service = service_with_spaces().await;
    let context = context();
    service
        .create_memory(
            context.clone(),
            memory_request(1, MemoryType::Semantic, "filterneedle metadata record"),
        )
        .await
        .unwrap();

    // A previously shipped version accepted `filters`, dropped it, and answered with
    // unfiltered hits and no signal. Each entry below is unrepresentable in the filter
    // language, so the request must fail rather than quietly degrade to "no filter".
    for filters in [
        json!("tenant"),                      // a root that is not an object
        json!({"a": {"matches": "x"}}),       // an operator the language does not define
        json!({"AND": []}),                   // an empty logical list widens the result set
        json!({"a": {"eq": "1", "ne": "2"}}), // two operators on one field
    ] {
        let mut request = retrieval_request("filterneedle", vec![1], 5, None);
        request.filters = Some(filters.clone());
        let error = service
            .create_retrieval(context.clone(), request)
            .await
            .expect_err("an unparseable filter must fail closed");
        assert_eq!(error.kind, MemoryServiceErrorKind::Validation);
        assert!(
            error.detail.contains("filters"),
            "the failure must name the offending field for {filters}: {}",
            error.detail
        );
    }
}
