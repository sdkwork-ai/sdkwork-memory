//! Contract tests for the C1/C4/C5/C7/C8 service repairs: additive retrieval
//! degradation without an embedder, provider health aggregation that degrades
//! instead of failing, extraction runs that answer zero candidates as success,
//! mem0 history truncation reporting, and the batch memory update analogue.

use sdkwork_intelligence_memory_service::{platform, OpenMemoryService};
use sdkwork_memory_contract::{
    MemoryEventRequest, MemoryOpenApi, MemoryOpenApiRequestContext, MemoryProviderHealthStatus,
    MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalRequest, MemoryServiceErrorKind,
    MemoryType,
};
use sdkwork_memory_plugin_native_sql::{NativeSqlCreateSpaceCommand, NativeSqlMemoryStore};
use sdkwork_memory_retrieval::MemoryRetrievalStrategy;
use sdkwork_memory_spi::{LanguageModelCommand, LanguageModelPort, MemorySpiError};
use serde_json::json;

const TENANT_ID: u64 = 91_201;
const ACTOR_ID: u64 = 42;

fn context() -> MemoryOpenApiRequestContext {
    MemoryOpenApiRequestContext::for_open_surface(
        "service-repair-contract-key",
        TENANT_ID,
        Some(ACTOR_ID),
    )
}

async fn store_with_owned_space() -> NativeSqlMemoryStore {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("repair contract sqlite store must open");
    store
        .create_space_record(
            TENANT_ID as i64,
            1,
            &NativeSqlCreateSpaceCommand {
                organization_id: None,
                owner_subject_type: "user".to_string(),
                owner_subject_id: ACTOR_ID.to_string(),
                space_type: "workspace".to_string(),
                display_name: "Service Repair Space".to_string(),
                default_scope: "user".to_string(),
            },
        )
        .await
        .expect("repair contract space must be created");
    store
}

async fn owned_space_service() -> OpenMemoryService {
    let store = store_with_owned_space().await;
    OpenMemoryService::new(store)
}

fn memory_request(space_id: u64, text: &str) -> MemoryRecordRequest {
    memory_request_with_metadata(space_id, text, None)
}

fn memory_request_with_metadata(
    space_id: u64,
    text: &str,
    metadata: Option<serde_json::Value>,
) -> MemoryRecordRequest {
    MemoryRecordRequest {
        space_id,
        scope: "user".to_string(),
        memory_type: MemoryType::Semantic,
        subject: Some("repair".to_string()),
        predicate: Some("matches".to_string()),
        object_text: Some(text.to_string()),
        canonical_text: text.to_string(),
        summary_text: None,
        user_id: Some(ACTOR_ID),
        language: Some("en".to_string()),
        sensitivity_level: Some("internal".to_string()),
        metadata,
        tags: None,
        expires_at: None,
    }
}

fn retrieval_request(query: &str, threshold: Option<f64>) -> MemoryRetrievalRequest {
    MemoryRetrievalRequest {
        query: query.to_string(),
        space_ids: vec![1],
        actor_id: Some(ACTOR_ID.to_string()),
        retrieval_profile_id: None,
        memory_types: None,
        filters: None,
        top_k: 5,
        context_budget_tokens: 512,
        show_expired: None,
        threshold,
        explain: None,
        include_trace: Some(false),
    }
}

fn conversation_event(space_id: u64, content: &str) -> MemoryEventRequest {
    MemoryEventRequest {
        space_id,
        user_id: None,
        actor_type: Some("user".to_string()),
        actor_id: Some(ACTOR_ID.to_string()),
        session_id: None,
        trace_id: None,
        event_type: "conversation.turn".to_string(),
        source_type: "conversation".to_string(),
        source_ref: None,
        event_time: "2026-09-24T00:00:00.000Z".to_string(),
        payload: json!({ "content": content }),
        sensitivity_level: None,
    }
}

fn memory_patch(
    canonical_text: Option<String>,
    metadata: Option<serde_json::Value>,
) -> MemoryRecordPatch {
    MemoryRecordPatch {
        canonical_text,
        subject: None,
        summary_text: None,
        metadata,
    }
}

/// C1 regression: an embedding-optional deployment running the additive hybrid
/// strategy with a caller threshold must answer with the lexical ranking and a
/// degraded marker. The pre-fix build gated at the threshold with every
/// semantic score at 0 and returned an empty result.
#[tokio::test]
async fn additive_retrieval_without_an_embedder_degrades_to_lexical_under_a_threshold() {
    let store = store_with_owned_space().await;
    let prepared = OpenMemoryService::new(store.clone());
    let service = OpenMemoryService::try_from_core_runtime_with_retrieval_strategy(
        sdkwork_memory_plugin_native_sql::NativeSqlPhase1Runtime::from_store(store),
        prepared.core_runtime().clone(),
        MemoryRetrievalStrategy::AdditiveHybrid,
    )
    .expect("additive hybrid must compose with the native SQL runtime");
    let context = context();
    service
        .create_memory(
            context.clone(),
            memory_request(1, "lexdegrade needle memory"),
        )
        .await
        .unwrap();
    service
        .create_memory(
            context.clone(),
            memory_request(1, "unrelated filler record"),
        )
        .await
        .unwrap();

    let result = service
        .create_retrieval(context, retrieval_request("lexdegrade needle", Some(0.1)))
        .await
        .expect("embedding-optional additive retrieval must not fail");
    assert!(
        !result.hits.is_empty(),
        "the lexical ranking must survive the unhonored threshold; the pre-fix \
         build gated at the threshold with all-semantic-zero scores and returned none"
    );
    assert_eq!(
        result.hits[0]
            .memory
            .as_ref()
            .unwrap()
            .canonical_text
            .as_str(),
        "lexdegrade needle memory"
    );
    assert!(
        result.degraded,
        "a relaxed caller threshold must mark the retrieval degraded"
    );
}

/// A provider binding set larger than the aggregation cap is a size fact: the
/// summary carries the aggregated prefix and `bindings_truncated`, and the
/// aggregate stays healthy when every covered binding is healthy. The pre-fix
/// build failed the whole read with a validation error.
#[tokio::test]
async fn provider_health_summary_truncates_at_the_binding_cap_instead_of_failing() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("health contract sqlite store must open");
    let cap = platform::max_provider_health_bindings();
    for index in 0..(cap + 1) {
        store
            .insert_mem_provider_binding(
                TENANT_ID as i64,
                &(9100 + index).to_string(),
                "llm",
                &format!("scripted-{index}"),
                &format!("Scripted LLM {index}"),
                "{}",
                None,
                None,
                None,
                None,
                "healthy",
                None,
            )
            .await
            .unwrap();
    }
    let service = OpenMemoryService::new(store);
    let summary = service
        .retrieve_provider_health_summary(context())
        .await
        .expect("an over-cap tenant must degrade, not fail");
    assert_eq!(summary.health.providers.len(), cap);
    assert!(summary.bindings_truncated);
    assert_eq!(summary.state_counts.total, cap);
    assert_eq!(summary.state_counts.healthy, cap);
    assert_eq!(summary.state_counts.unknown, 0);
    assert_eq!(summary.health.status, MemoryProviderHealthStatus::Healthy);

    // The typed trait entry projects exactly the aggregated summary.
    let typed = service
        .retrieve_provider_health(context())
        .await
        .expect("the typed health entry must follow the summary");
    assert_eq!(typed.providers.len(), cap);
    assert_eq!(typed.status, MemoryProviderHealthStatus::Healthy);
}

/// `unknown` health (a binding whose health was never observed) is its own
/// signal: it never counts toward `healthy`, and a set of only-unobserved
/// bindings reports `unknown`, not healthy or degraded.
#[tokio::test]
async fn provider_health_keeps_unknown_out_of_the_healthy_aggregate() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("health contract sqlite store must open");
    for (index, state) in ["unknown", "unknown", "healthy"].iter().enumerate() {
        store
            .insert_mem_provider_binding(
                TENANT_ID as i64,
                &(9200 + index).to_string(),
                "llm",
                &format!("scripted-{index}"),
                &format!("Scripted LLM {index}"),
                "{}",
                None,
                None,
                None,
                None,
                state,
                None,
            )
            .await
            .unwrap();
    }
    let service = OpenMemoryService::new(store);
    let mixed = service
        .retrieve_provider_health_summary(context())
        .await
        .unwrap();
    assert_eq!(mixed.state_counts.healthy, 1);
    assert_eq!(mixed.state_counts.unknown, 2);
    assert_eq!(mixed.health.status, MemoryProviderHealthStatus::Degraded);

    // A tenant with only unobserved bindings reports the unobserved state
    // instead of inventing health.
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("health contract sqlite store must open");
    for index in 0..2 {
        store
            .insert_mem_provider_binding(
                TENANT_ID as i64,
                &(9300 + index).to_string(),
                "llm",
                &format!("unobserved-{index}"),
                &format!("Scripted LLM {index}"),
                "{}",
                None,
                None,
                None,
                None,
                "unknown",
                None,
            )
            .await
            .unwrap();
    }
    let service = OpenMemoryService::new(store);
    let unobserved = service
        .retrieve_provider_health_summary(context())
        .await
        .unwrap();
    assert_eq!(unobserved.state_counts.unknown, 2);
    assert_eq!(unobserved.health.status, MemoryProviderHealthStatus::Unknown);
}

/// A model that refuses (or reports nothing usable for) every input event
/// produced a legal outcome. The run must answer success with a note, not the
/// retriable validation error the pre-fix build raised.
#[tokio::test]
async fn llm_extraction_that_yields_no_candidates_answers_ok_with_a_note() {
    struct RefusingLlm;

    #[async_trait::async_trait]
    impl LanguageModelPort for RefusingLlm {
        fn provider_code(&self) -> &str {
            "refusing"
        }

        async fn generate(
            &self,
            _command: LanguageModelCommand,
        ) -> Result<String, MemorySpiError> {
            Ok(String::from(r#"{"memory": []}"#))
        }
    }

    let store = store_with_owned_space().await;
    let service = OpenMemoryService::new(store).with_llm(std::sync::Arc::new(RefusingLlm));
    let context = context();
    let event = service
        .create_event(context.clone(), conversation_event(1, "refusal bait content"))
        .await
        .unwrap();

    let summary = service
        .run_extraction_now(
            context.clone(),
            sdkwork_memory_contract::MemoryExtractionRequest {
                space_id: 1,
                input_events: vec![event.event_id],
                extraction_mode: None,
                custom_instructions: None,
                observation_date: None,
            },
        )
        .await
        .expect("a model refusing every input is a completed run, not a fault");
    assert_eq!(summary["candidateCount"], 0);
    assert_eq!(summary["refusedCount"], 0);
    assert_eq!(summary["extractionMode"], "additive_llm");
    assert_eq!(summary["note"], "extraction produced no candidates");
}

/// The deterministic pass-through with no extractable input (every referenced
/// event missing) is likewise a completed, observable run.
#[tokio::test]
async fn deterministic_extraction_without_extractable_content_answers_ok_with_a_note() {
    let service = owned_space_service().await;
    let summary = service
        .run_extraction_now(
            context(),
            sdkwork_memory_contract::MemoryExtractionRequest {
                space_id: 1,
                input_events: vec![987_654],
                extraction_mode: Some("deterministic".to_string()),
                custom_instructions: None,
                observation_date: None,
            },
        )
        .await
        .expect("an input without extractable content is not a fault");
    assert_eq!(summary["candidateCount"], 0);
    assert_eq!(summary["missingEventCount"], 1);
    assert_eq!(summary["extractionMode"], "deterministic");
    assert_eq!(summary["note"], "extraction produced no candidates");
}

/// C7: the mem0 history read reports whether the journal holds more entries
/// than the caller's page bound, so the wire can refuse a partial history
/// instead of serving one that is indistinguishable from complete.
#[tokio::test]
async fn mem0_history_reports_truncation_against_the_page_bound() {
    let service = owned_space_service().await;
    let context = context();
    let memory = service
        .create_memory(context.clone(), memory_request(1, "history probe memory"))
        .await
        .unwrap();
    service
        .update_memory(
            context.clone(),
            memory.memory_id,
            1,
            memory_patch(Some("history probe memory (revised)".to_string()), None),
        )
        .await
        .unwrap();

    let complete = service
        .mem0_memory_history(&context, 1, memory.memory_id, 2)
        .await
        .unwrap();
    assert_eq!(complete.events.len(), 2);
    assert!(!complete.truncated);
    assert_eq!(complete.events[0].action, "memory.record.update");
    assert_eq!(complete.events[1].action, "memory.record.create");

    let truncated = service
        .mem0_memory_history(&context, 1, memory.memory_id, 1)
        .await
        .unwrap();
    assert_eq!(truncated.events.len(), 1);
    assert!(
        truncated.truncated,
        "one page of a two-entry journal must be reported as truncated"
    );

    // An unknown memory still serves its (empty) trail.
    let missing = service
        .mem0_memory_history(&context, 1, 987_654, 5)
        .await
        .unwrap();
    assert!(missing.events.is_empty());
    assert!(!missing.truncated);
}

/// C8: the batch update keeps single-record semantics — per-item results in
/// input order, shallow metadata merge, per-record NotFound for unknown ids —
/// while authorizing and prechecking once per call, and a whole-call
/// authorization failure fails every slot.
#[tokio::test]
async fn batch_memory_updates_keep_input_order_and_per_item_semantics() {
    let service = owned_space_service().await;
    let context = context();
    let first = service
        .create_memory(
            context.clone(),
            memory_request_with_metadata(1, "batch update target one", Some(json!({"origin": "a"}))),
        )
        .await
        .unwrap();
    let second = service
        .create_memory(
            context.clone(),
            memory_request_with_metadata(1, "batch update target two", Some(json!({"origin": "b"}))),
        )
        .await
        .unwrap();

    let results = service
        .update_memories_batch(
            context.clone(),
            1,
            vec![
                (
                    first.memory_id,
                    memory_patch(
                        Some("batch updated text one".to_string()),
                        Some(json!({"origin": "a2", "extra": true})),
                    ),
                ),
                (
                    second.memory_id,
                    memory_patch(None, Some(json!({"origin": "b2"}))),
                ),
                (
                    987_654,
                    memory_patch(Some("never applied".to_string()), None),
                ),
            ],
        )
        .await;
    assert_eq!(results.len(), 3);
    let first_updated = results[0]
        .as_ref()
        .expect("the first known record must update");
    assert_eq!(first_updated.canonical_text, "batch updated text one");
    let first_metadata = first_updated.metadata.as_ref().unwrap();
    assert_eq!(first_metadata["origin"], "a2");
    assert_eq!(first_metadata["extra"], true);

    let second_updated = results[1]
        .as_ref()
        .expect("the second known record must update");
    assert_eq!(
        second_updated.canonical_text, "batch update target two",
        "an absent canonicalText patch must leave the text in place"
    );
    assert_eq!(second_updated.metadata.as_ref().unwrap()["origin"], "b2");

    let missing = results[2].as_ref().unwrap_err();
    assert_eq!(missing.kind, MemoryServiceErrorKind::NotFound);

    // The updates persisted.
    let reloaded = service
        .retrieve_memory(context.clone(), first.memory_id, 1)
        .await
        .unwrap();
    assert_eq!(reloaded.canonical_text, "batch updated text one");

    // An empty batch is an empty result, not an error.
    assert!(
        service
            .update_memories_batch(context.clone(), 1, Vec::new())
            .await
            .is_empty()
    );

    // A caller without access to the space fails every slot with the same
    // whole-call denial instead of a partial write.
    let outsider = MemoryOpenApiRequestContext::for_open_surface(
        "service-repair-outsider-key",
        TENANT_ID,
        Some(9_999),
    );
    let denied = service
        .update_memories_batch(
            outsider,
            1,
            vec![
                (first.memory_id, memory_patch(None, None)),
                (second.memory_id, memory_patch(None, None)),
            ],
        )
        .await;
    assert_eq!(denied.len(), 2);
    assert!(denied
        .iter()
        .all(|result| result.as_ref().unwrap_err().kind == MemoryServiceErrorKind::Forbidden));
}
