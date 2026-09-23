pub mod bm25;
pub mod context_pack;
pub mod entities;
pub mod lemmatization;
pub mod retrieval;
pub mod scoring;

pub use bm25::{
    get_bm25_params, normalize_bm25, normalized_keyword_scores, Bm25Document, Bm25Index, BM25_B,
    BM25_K1, DEFAULT_BM25_PARAMS,
};
pub use context_pack::{build_context_pack_from_hits, estimate_tokens};
pub use entities::{
    clean_text, entity_boosts, extract_entities, extract_entity_texts, has_artifacts,
    looks_like_technical_identifier, memory_count_weight, normalize_entity_text,
    select_query_entities, EntityKind, EntitySpan, ExtractedEntity, LinkedEntityMatch,
    ENTITY_MERGE_SIMILARITY, ENTITY_SIMILARITY_THRESHOLD, MAX_ENTITY_TEXT_CHARS,
    MAX_QUERY_ENTITIES, SOURCE_IDENTIFIER_HEURISTIC, SOURCE_PROPER_NAME_SPAN, SOURCE_QUOTED,
    SOURCE_TECHNICAL_IDENTIFIER, SOURCE_TOPIC_PHRASE,
};
pub use lemmatization::{is_stopword, lemmatize_for_bm25, lemmatize_token};
pub use retrieval::{
    dictionary_match_score, event_match_score, fuse_retrieval_candidates,
    fuse_retrieval_candidates_with_policy, keyword_match_score, orchestrate_retrieval_candidates,
    orchestrate_retrieval_candidates_with_vector, sql_structured_match_score, time_recency_score,
    FusedRetrievalHit, MemoryRetrievalStrategy, OrchestratedCandidate, RetrievalCandidate,
    RetrievalEventInput, RetrievalFusionPolicy, RetrievalRecordInput, VectorSimilarityInput,
};
pub use scoring::{
    internal_fetch_limit, max_possible_score, score_and_rank, validate_threshold, HybridSignals,
    ScoreDetails, ScoredMemoryHit, SemanticCandidate, DEFAULT_SCORE_THRESHOLD,
    INTERNAL_FETCH_MULTIPLIER, MIN_INTERNAL_FETCH_LIMIT,
};
