//! Contract tests for the opt-in vector-similarity retrieval signal.
//!
//! The signal exists so a deployment that binds an embedding provider can add vector
//! recall without perturbing the ranking of a deployment that does not. These tests pin
//! both halves of that promise: off unless the profile grants it a weight, and bounded
//! onto the shared signal scale once it is on.

use std::collections::HashMap;

use serde_json::{json, Value};

use sdkwork_memory_retrieval::{
    orchestrate_retrieval_candidates, orchestrate_retrieval_candidates_with_vector,
    MemoryRetrievalStrategy, OrchestratedCandidate, RetrievalRecordInput, VectorSimilarityInput,
};

fn record(memory_id: &str, text: &str) -> RetrievalRecordInput {
    RetrievalRecordInput {
        memory_id: memory_id.to_string(),
        subject: None,
        predicate: None,
        object_text: text.to_string(),
        canonical_text: text.to_string(),
        created_at: "2026-09-23T00:00:00Z".to_string(),
    }
}

fn similarity(memory_id: &str, similarity: f64) -> VectorSimilarityInput {
    VectorSimilarityInput {
        memory_id: memory_id.to_string(),
        similarity,
    }
}

fn retriever_names(candidates: &[OrchestratedCandidate]) -> Vec<String> {
    candidates
        .iter()
        .map(|candidate| candidate.retriever_name.clone())
        .collect()
}

fn vector_candidates(candidates: &[OrchestratedCandidate]) -> Vec<OrchestratedCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.retriever_name == "vector")
        .cloned()
        .collect()
}

#[test]
fn vector_signal_stays_off_when_no_profile_is_supplied() {
    let records = [record("memory-1", "renewal plan")];
    let similarities = [similarity("memory-1", 1.0)];

    let with_vector = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &similarities,
        None,
        5,
    );
    let legacy = orchestrate_retrieval_candidates("renewal plan", &records, &[], None, 5);

    assert!(
        vector_candidates(&with_vector).is_empty(),
        "no profile granted the vector weight, so the signal must not contribute"
    );
    assert_eq!(with_vector, legacy);
}

#[test]
fn vector_signal_stays_off_when_the_profile_omits_it() {
    let records = [record("memory-1", "renewal plan")];
    let profile = json!({ "keyword": { "weight": 1.0 } });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[similarity("memory-1", 1.0)],
        Some(&profile),
        5,
    );

    assert_eq!(retriever_names(&hits), vec!["keyword".to_string()]);
}

#[test]
fn vector_signal_contributes_only_when_the_profile_grants_it_a_weight() {
    let records = [record("memory-1", "renewal plan")];
    let profile = json!({
        "keyword": { "weight": 0.5 },
        "vector": { "weight": 0.9 }
    });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[similarity("memory-1", 0.8)],
        Some(&profile),
        5,
    );

    assert_eq!(
        retriever_names(&hits),
        vec!["keyword".to_string(), "vector".to_string()]
    );
    let vector_hits = vector_candidates(&hits);
    assert_eq!(vector_hits.len(), 1);
    assert_eq!(vector_hits[0].record.memory_id, "memory-1");
    assert_eq!(vector_hits[0].rank, 1);
    assert!(
        (vector_hits[0].raw_score - 0.8 * 0.9).abs() < 1e-9,
        "got {}",
        vector_hits[0].raw_score
    );
}

#[test]
fn an_out_of_scale_similarity_cannot_outrank_a_perfect_lexical_match() {
    let records = [
        record("memory-1", "renewal plan"),
        record("memory-2", "renewal plan"),
    ];
    let profile = json!({ "vector": { "weight": 1.0 } });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[similarity("memory-1", 1.0), similarity("memory-2", 9.0)],
        Some(&profile),
        5,
    );

    let vector_hits = vector_candidates(&hits);
    assert_eq!(vector_hits.len(), 2);
    for hit in &vector_hits {
        assert!(
            hit.raw_score <= 1.0,
            "an unbounded provider score escaped the clamp: {}",
            hit.raw_score
        );
    }
    let by_id = vector_hits
        .iter()
        .map(|hit| (hit.record.memory_id.as_str(), hit.raw_score))
        .collect::<HashMap<_, _>>();
    assert_eq!(by_id.get("memory-1"), by_id.get("memory-2"));
}

#[test]
fn similarities_for_memories_outside_the_candidate_universe_are_dropped() {
    let records = [record("memory-1", "renewal plan")];
    let profile = json!({ "vector": { "weight": 1.0 } });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[similarity("ghost-memory", 1.0)],
        Some(&profile),
        5,
    );

    assert!(
        hits.is_empty(),
        "vector recall must not introduce an unrehydrated memory: {hits:?}"
    );
}

#[test]
fn non_positive_and_non_finite_similarities_are_discarded() {
    let records = [
        record("zero", "renewal plan"),
        record("negative", "renewal plan"),
        record("not-a-number", "renewal plan"),
        record("infinite", "renewal plan"),
    ];
    let profile = json!({ "vector": { "weight": 1.0 } });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[
            similarity("zero", 0.0),
            similarity("negative", -1.0),
            similarity("not-a-number", f64::NAN),
            similarity("infinite", f64::INFINITY),
        ],
        Some(&profile),
        5,
    );

    assert!(hits.is_empty(), "got: {hits:?}");
}

#[test]
fn duplicate_similarities_keep_the_best_score_per_memory() {
    let records = [record("memory-1", "renewal plan")];
    let profile = json!({ "vector": { "weight": 1.0 } });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[similarity("memory-1", 0.2), similarity("memory-1", 0.7)],
        Some(&profile),
        5,
    );

    let vector_hits = vector_candidates(&hits);
    assert_eq!(
        vector_hits.len(),
        1,
        "one memory yields one vector candidate"
    );
    assert!(
        (vector_hits[0].raw_score - 0.7).abs() < 1e-9,
        "got {}",
        vector_hits[0].raw_score
    );
}

#[test]
fn vector_recall_joins_the_lexical_signals_instead_of_replacing_them() {
    let records = [
        record("memory-1", "renewal plan"),
        record("memory-2", "unrelated topic"),
    ];
    let profile = json!({
        "keyword": { "weight": 1.0 },
        "vector": { "weight": 0.8 }
    });

    let hits = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[similarity("memory-2", 0.95)],
        Some(&profile),
        5,
    );

    assert_eq!(
        retriever_names(&hits),
        vec!["keyword".to_string(), "vector".to_string()]
    );
    assert_eq!(hits[0].record.memory_id, "memory-1");
    assert_eq!(hits[1].record.memory_id, "memory-2");
    assert!((vector_candidates(&hits)[0].raw_score - 0.95 * 0.8).abs() < 1e-9);
}

#[test]
fn the_vector_aware_entry_point_with_an_empty_slice_is_the_legacy_entry_point() {
    let records = [record("memory-1", "renewal plan")];
    let profile = json!({
        "keyword": { "weight": 1.0 },
        "vector": { "weight": 0.9 }
    });

    let legacy = orchestrate_retrieval_candidates("renewal plan", &records, &[], Some(&profile), 5);
    let vector_aware = orchestrate_retrieval_candidates_with_vector(
        "renewal plan",
        &records,
        &[],
        &[],
        Some(&profile),
        5,
    );

    assert_eq!(legacy, vector_aware);
    assert_eq!(retriever_names(&legacy), vec!["keyword".to_string()]);
}

#[test]
fn commercial_retrieval_strategies_do_not_silently_enable_vector_recall() {
    for strategy in MemoryRetrievalStrategy::all() {
        let profile: Value = strategy.retriever_profile();
        let weight = profile
            .get("vector")
            .and_then(|entry| entry.get("weight"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if strategy == MemoryRetrievalStrategy::AdditiveHybrid {
            // The additive strategy is the explicit opt-in: choosing it *is*
            // the declaration that an embedding provider is part of the
            // deployment, so its profile legitimately grants vector weight.
            assert!(
                weight > 0.0,
                "the additive strategy must grant vector recall"
            );
            continue;
        }
        assert_eq!(
            weight,
            0.0,
            "strategy {} enabled vector recall while no embedding provider is bound",
            strategy.code()
        );
    }
}
