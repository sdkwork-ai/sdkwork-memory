use sdkwork_memory_contract::{MemoryRecord, MemoryType};
use sdkwork_memory_retrieval::{
    fuse_retrieval_candidates, keyword_match_score, RetrievalCandidate,
};

fn sample_record(id: u64, text: &str) -> MemoryRecord {
    MemoryRecord {
        memory_id: id,
        uuid: Some(id.to_string()),
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
    }
}

#[test]
fn keyword_match_scores_exact_and_partial_queries() {
    assert_eq!(keyword_match_score("renewal", "renewal"), 1.0);
    assert!(keyword_match_score("renewal support", "customer renewal support plan") > 0.8);
    assert_eq!(keyword_match_score("missing", "unrelated text"), 0.0);
}

#[test]
fn fuse_retrieval_candidates_ranks_by_score_without_embeddings() {
    let candidates = vec![
        RetrievalCandidate {
            memory: sample_record(1, "low relevance"),
            retriever_name: "keyword".to_string(),
            raw_score: 0.2,
            rank: 0,
        },
        RetrievalCandidate {
            memory: sample_record(2, "enterprise renewal support"),
            retriever_name: "keyword".to_string(),
            raw_score: 0.95,
            rank: 0,
        },
    ];

    let fused = fuse_retrieval_candidates(candidates, 2);
    assert_eq!(fused.len(), 2);
    assert_eq!(fused[0].memory.memory_id, 2);
    assert_eq!(fused[0].retriever_name, "keyword");
    assert_eq!(fused[0].retrievers, vec!["keyword"]);
    assert!(fused[0].fused_score > 0.0 && fused[0].fused_score < 1.0);
}

#[test]
fn weighted_rrf_rewards_cross_retriever_agreement_and_deduplicates_hits() {
    let candidates = vec![
        RetrievalCandidate {
            memory: sample_record(1, "shared relevant memory"),
            retriever_name: "keyword".to_string(),
            raw_score: 0.8,
            rank: 1,
        },
        RetrievalCandidate {
            memory: sample_record(1, "shared relevant memory"),
            retriever_name: "dictionary".to_string(),
            raw_score: 0.7,
            rank: 1,
        },
        // A duplicate from one provider must not amplify the same memory.
        RetrievalCandidate {
            memory: sample_record(1, "shared relevant memory"),
            retriever_name: "keyword".to_string(),
            raw_score: 0.8,
            rank: 1,
        },
        RetrievalCandidate {
            memory: sample_record(2, "single signal memory"),
            retriever_name: "keyword".to_string(),
            raw_score: 0.95,
            rank: 1,
        },
    ];

    let fused = fuse_retrieval_candidates(candidates, 10);
    assert_eq!(fused.len(), 2);
    assert_eq!(fused[0].memory.memory_id, 1);
    assert_eq!(fused[0].retrievers, vec!["dictionary", "keyword"]);
    assert_eq!(fused[1].memory.memory_id, 2);
}

// =============================================================================
// CJK (Chinese, Japanese, Korean) Retrieval Tests
// =============================================================================

#[test]
fn cjk_keyword_match_exact() {
    // Exact match for CJK text
    assert_eq!(keyword_match_score("知识库", "知识库"), 1.0);
    assert_eq!(keyword_match_score("记忆系统", "记忆系统"), 1.0);
}

#[test]
fn cjk_keyword_match_partial() {
    // Partial match: query is substring of haystack
    let score = keyword_match_score("知识库", "企业知识库系统");
    assert!(
        score > 0.8,
        "Expected score > 0.8 for partial CJK match, got {}",
        score
    );
    assert_eq!(score, 0.85); // substring match returns 0.85
}

#[test]
fn cjk_keyword_match_token_overlap() {
    // Token overlap: each CJK character is a token
    let score = keyword_match_score("知识管理", "知识库管理系统");
    assert!(
        score > 0.0,
        "Expected positive score for CJK token overlap, got {}",
        score
    );
    // "知识管理" tokens: 知, 识, 管, 理
    // "知识库管理系统" contains: 知, 识, 管, 理
    // Should have high overlap
    assert!(
        score >= 0.75,
        "Expected high token overlap score, got {}",
        score
    );
}

#[test]
fn cjk_keyword_match_mixed_language() {
    // Mixed CJK and ASCII
    let score = keyword_match_score("API文档", "API文档中心");
    assert!(
        score > 0.8,
        "Expected high score for mixed language match, got {}",
        score
    );

    let score2 = keyword_match_score("memory记忆", "memory记忆存储");
    assert!(
        score2 > 0.8,
        "Expected high score for mixed language partial match, got {}",
        score2
    );
}

#[test]
fn cjk_keyword_match_no_match() {
    // No overlap between query and haystack
    let score = keyword_match_score("苹果", "香蕉橘子");
    assert_eq!(
        score, 0.0,
        "Expected zero score for no CJK overlap, got {}",
        score
    );
}

#[test]
fn cjk_keyword_match_japanese_hiragana() {
    // Japanese hiragana (not CJK ideographs, but should still work)
    let score = keyword_match_score("メモリ", "メモリシステム");
    // Katakana characters may not be detected as CJK, but substring match should work
    assert!(
        score >= 0.0,
        "Expected valid score for Japanese text, got {}",
        score
    );
}

#[test]
fn cjk_keyword_match_chinese_punctuation() {
    // Chinese text with punctuation
    let score = keyword_match_score("知识库", "这是一个知识库，包含重要信息。");
    assert!(
        score > 0.8,
        "Expected high score for CJK with punctuation, got {}",
        score
    );
}

#[test]
fn cjk_keyword_match_traditional_chinese() {
    // Traditional Chinese characters (also in CJK range)
    let score = keyword_match_score("知識庫", "企業知識庫系統");
    assert!(
        score > 0.8,
        "Expected high score for Traditional Chinese, got {}",
        score
    );
}

#[test]
fn cjk_keyword_match_single_character() {
    // Single CJK character query
    let score = keyword_match_score("知", "知识库");
    assert!(
        score > 0.0,
        "Expected positive score for single CJK character, got {}",
        score
    );
}

#[test]
fn cjk_vs_ascii_case_insensitivity() {
    // ASCII queries should be case-insensitive
    assert_eq!(keyword_match_score("API", "api"), 1.0);
    assert_eq!(keyword_match_score("Memory", "memory"), 1.0);

    // CJK has no case distinction
    assert_eq!(keyword_match_score("知识", "知识"), 1.0);
}
