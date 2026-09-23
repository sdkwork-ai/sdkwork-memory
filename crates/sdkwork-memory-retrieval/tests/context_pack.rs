use sdkwork_memory_contract::{MemoryRecord, MemoryRetrievalHit, MemoryType};
use sdkwork_memory_retrieval::{build_context_pack_from_hits, estimate_tokens};

fn sample_hit(text: &str) -> MemoryRetrievalHit {
    MemoryRetrievalHit {
        hit_id: 1,
        memory: Some(MemoryRecord {
            memory_id: 1,
            uuid: Some("1".to_string()),
            space_id: 1,
            user_id: None,
            scope: "user".to_string(),
            memory_type: MemoryType::Semantic,
            subject: None,
            predicate: None,
            object_text: Some(text.to_string()),
            canonical_text: text.to_string(),
            summary_text: None,
            confidence: 1.0,
            evidence_count: Some(1),
            contradiction_count: Some(0),
            status: "active".to_string(),
            sensitivity_level: "internal".to_string(),
            supersedes_memory_id: None,
            superseded_by_memory_id: None,
            created_at: "2026-06-10T00:00:00Z".to_string(),
            updated_at: "2026-06-10T00:00:00Z".to_string(),
            version: 1,
            expires_at: None,
            metadata: None,
        }),
        memory_id: Some(1),
        retriever_name: "keyword".to_string(),
        result_rank: 1,
        raw_score: Some(0.9),
        fused_score: Some(0.9),
        explanation: None,
        status: "accepted".to_string(),
    }
}

#[test]
fn context_pack_builder_respects_token_budget_without_embeddings() {
    let hits = vec![
        sample_hit("short"),
        sample_hit("another concise memory fragment"),
    ];
    let (pack, tokens, _truncated) = build_context_pack_from_hits(&hits, 8);
    assert!(tokens <= 8);
    assert_eq!(pack["embeddingOptional"], true);
    assert_eq!(pack["selection"]["algorithm"], "ranked_budgeted_dedup");
    assert!(!pack["fragments"].as_array().unwrap().is_empty());
    assert!(estimate_tokens("hello world") >= 2);
}

#[test]
fn context_pack_truncates_the_first_long_fragment_inside_budget() {
    let hits = vec![sample_hit(
        "This memory is intentionally much longer than the available context budget",
    )];
    let (pack, tokens, truncated) = build_context_pack_from_hits(&hits, 3);
    let fragment = &pack["fragments"][0];

    assert!(truncated);
    assert!(tokens <= 3);
    assert_eq!(fragment["truncated"], true);
    assert!(fragment["canonicalText"].as_str().unwrap().len() < 72);
}

#[test]
fn context_pack_suppresses_duplicate_memories() {
    let hits = vec![
        sample_hit("user prefers concise technical answers"),
        sample_hit("user prefers concise technical answers"),
    ];
    let (pack, _tokens, truncated) = build_context_pack_from_hits(&hits, 100);

    assert!(truncated);
    assert_eq!(pack["fragments"].as_array().unwrap().len(), 1);
    assert_eq!(pack["selection"]["deduplicatedCount"], 1);
}
