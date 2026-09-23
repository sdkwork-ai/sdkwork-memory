use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value;

use sdkwork_memory_contract::MemoryRecord;
use sdkwork_utils_rust::is_blank;

const RRF_RANK_CONSTANT: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRetrievalStrategy {
    Balanced,
    SearchFirst,
    EventAware,
}

impl MemoryRetrievalStrategy {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::SearchFirst => "search_first",
            Self::EventAware => "event_aware",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "balanced" => Ok(Self::Balanced),
            "search_first" | "search-first" => Ok(Self::SearchFirst),
            "event_aware" | "event-aware" => Ok(Self::EventAware),
            other => Err(format!(
                "memory retrieval strategy must be balanced, search_first, or event_aware; got {other}"
            )),
        }
    }

    pub fn retriever_profile(self) -> Value {
        match self {
            Self::Balanced => serde_json::json!({
                "keyword": { "weight": 1.0 },
                "dictionary": { "weight": 0.85 },
                "time": { "weight": 0.5 },
                "event": { "weight": 0.6 },
                "sql": { "weight": 0.75 }
            }),
            Self::SearchFirst => serde_json::json!({
                "keyword": { "weight": 1.0 },
                "dictionary": { "weight": 0.55 },
                "sql": { "weight": 0.35 }
            }),
            Self::EventAware => serde_json::json!({
                "event": { "weight": 1.0 },
                "keyword": { "weight": 0.8 },
                "time": { "weight": 0.65 },
                "dictionary": { "weight": 0.4 },
                "sql": { "weight": 0.2 }
            }),
        }
    }

    pub const fn all() -> [Self; 3] {
        [Self::Balanced, Self::SearchFirst, Self::EventAware]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RetrievalFusionPolicy {
    pub rank_constant: f64,
}

impl Default for RetrievalFusionPolicy {
    fn default() -> Self {
        Self {
            rank_constant: RRF_RANK_CONSTANT,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalCandidate {
    pub memory: MemoryRecord,
    pub retriever_name: String,
    pub raw_score: f64,
    pub rank: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FusedRetrievalHit {
    pub memory: MemoryRecord,
    pub retriever_name: String,
    pub retrievers: Vec<String>,
    pub raw_score: f64,
    pub fused_score: f64,
    pub rank: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalRecordInput {
    pub memory_id: String,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_text: String,
    pub canonical_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalEventInput {
    pub memory_id: Option<String>,
    pub event_id: String,
    pub payload_text: String,
    pub created_at: String,
}

/// A vector-store similarity for one memory, supplied by the caller.
///
/// This crate never embeds. The embedding port lives one layer above, so the caller
/// embeds the query, scores the candidates it already rehydrated, and passes the
/// similarities in. Keeping it that way preserves this module as a pure function: a
/// ranking call must not start issuing provider requests as a side effect.
///
/// `similarity` must use the same scale as every other signal -- cosine similarity of
/// unit-normalised embeddings in `[0.0, 1.0]` -- because the fused `raw_score` is
/// compared across retrievers whenever two RRF contributions tie. Out-of-scale input is
/// clamped rather than rescaled, so an unnormalised provider cannot dominate fusion.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorSimilarityInput {
    pub memory_id: String,
    pub similarity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrchestratedCandidate {
    pub record: RetrievalRecordInput,
    pub retriever_name: String,
    pub raw_score: f64,
    pub rank: i32,
}

#[derive(Debug)]
struct FusionAggregate {
    memory: MemoryRecord,
    raw_score: f64,
    rrf_score: f64,
    dominant_contribution: f64,
    dominant_retriever: String,
    retrievers: BTreeSet<String>,
}

fn score_order(left: f64, right: f64) -> Ordering {
    right.partial_cmp(&left).unwrap_or(Ordering::Equal)
}

/// Fuse independently ranked retriever results with weighted reciprocal rank fusion.
///
/// Each candidate's `raw_score` is its non-negative relevance multiplied by the
/// configured retriever weight. RRF makes the result robust to retrievers whose
/// native score distributions are not directly comparable. Duplicate hits from
/// one retriever are ignored so repeated provider output cannot amplify a memory.
pub fn fuse_retrieval_candidates(
    candidates: Vec<RetrievalCandidate>,
    top_k: usize,
) -> Vec<FusedRetrievalHit> {
    fuse_retrieval_candidates_with_policy(candidates, top_k, RetrievalFusionPolicy::default())
}

pub fn fuse_retrieval_candidates_with_policy(
    candidates: Vec<RetrievalCandidate>,
    top_k: usize,
    policy: RetrievalFusionPolicy,
) -> Vec<FusedRetrievalHit> {
    if top_k == 0 {
        return Vec::new();
    }
    let rank_constant = if policy.rank_constant.is_finite() && policy.rank_constant >= 1.0 {
        policy.rank_constant
    } else {
        RRF_RANK_CONSTANT
    };

    let mut by_retriever: HashMap<String, Vec<RetrievalCandidate>> = HashMap::new();
    for candidate in candidates {
        if candidate.raw_score.is_finite() && candidate.raw_score > 0.0 {
            by_retriever
                .entry(candidate.retriever_name.clone())
                .or_default()
                .push(candidate);
        }
    }

    let mut retriever_names = by_retriever.keys().cloned().collect::<Vec<_>>();
    retriever_names.sort();
    let mut aggregates: HashMap<u64, FusionAggregate> = HashMap::new();

    for retriever_name in retriever_names {
        let Some(mut retriever_candidates) = by_retriever.remove(&retriever_name) else {
            continue;
        };
        retriever_candidates.sort_by(|left, right| {
            score_order(left.raw_score, right.raw_score)
                .then_with(|| left.memory.memory_id.cmp(&right.memory.memory_id))
        });

        let mut seen_memory_ids = HashSet::new();
        let mut computed_rank = 0_i32;
        for candidate in retriever_candidates {
            if !seen_memory_ids.insert(candidate.memory.memory_id) {
                continue;
            }
            computed_rank += 1;
            // Searches run once per authorized space. Re-rank the merged list
            // here so several space-local rank-1 candidates do not all receive
            // the same global RRF contribution.
            let rank = computed_rank;
            let contribution = candidate.raw_score / (rank_constant + f64::from(rank.max(1)));
            let memory_id = candidate.memory.memory_id;
            aggregates
                .entry(memory_id)
                .and_modify(|aggregate| {
                    aggregate.raw_score = aggregate.raw_score.max(candidate.raw_score);
                    aggregate.rrf_score += contribution;
                    aggregate.retrievers.insert(retriever_name.clone());
                    if contribution > aggregate.dominant_contribution
                        || (contribution == aggregate.dominant_contribution
                            && retriever_name < aggregate.dominant_retriever)
                    {
                        aggregate.dominant_contribution = contribution;
                        aggregate.dominant_retriever = retriever_name.clone();
                    }
                })
                .or_insert_with(|| FusionAggregate {
                    memory: candidate.memory,
                    raw_score: candidate.raw_score,
                    rrf_score: contribution,
                    dominant_contribution: contribution,
                    dominant_retriever: retriever_name.clone(),
                    retrievers: BTreeSet::from([retriever_name.clone()]),
                });
        }
    }

    let mut fused = aggregates.into_values().collect::<Vec<_>>();
    fused.sort_by(|left, right| {
        score_order(left.rrf_score, right.rrf_score)
            .then_with(|| score_order(left.raw_score, right.raw_score))
            .then_with(|| left.memory.memory_id.cmp(&right.memory.memory_id))
    });

    fused
        .into_iter()
        .take(top_k)
        .enumerate()
        .map(|(index, aggregate)| FusedRetrievalHit {
            memory: aggregate.memory,
            retriever_name: aggregate.dominant_retriever,
            retrievers: aggregate.retrievers.into_iter().collect(),
            raw_score: aggregate.raw_score,
            // Map the unbounded weighted RRF sum to a stable [0, 1) confidence.
            fused_score: 1.0 - (-aggregate.rrf_score * (rank_constant + 1.0)).exp(),
            rank: (index as i32) + 1,
        })
        .collect()
}

pub fn keyword_match_score(query: &str, canonical_text: &str) -> f64 {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return 0.0;
    }

    let haystack = canonical_text.to_lowercase();
    if haystack == query {
        return 1.0;
    }
    if haystack.contains(&query) {
        return 0.85;
    }

    token_overlap_score(&haystack, &tokenise_query(&query))
}

pub fn dictionary_match_score(
    query: &str,
    subject: Option<&str>,
    predicate: Option<&str>,
    object_text: &str,
) -> f64 {
    let mut corpus = String::new();
    if let Some(subject) = subject {
        corpus.push_str(subject);
        corpus.push(' ');
    }
    if let Some(predicate) = predicate {
        corpus.push_str(predicate);
        corpus.push(' ');
    }
    corpus.push_str(object_text);
    token_overlap_score(&corpus.to_lowercase(), &tokenise_query(query))
}

pub fn sql_structured_match_score(
    query: &str,
    subject: Option<&str>,
    predicate: Option<&str>,
) -> f64 {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return 0.0;
    }

    let mut score: f64 = 0.0;
    let tokens = tokenise_query(&query);
    if subject
        .map(|value| value.to_lowercase() == query)
        .unwrap_or(false)
    {
        score = score.max(1.0);
    }
    if predicate
        .map(|value| value.to_lowercase() == query)
        .unwrap_or(false)
    {
        score = score.max(0.95);
    }

    for token in &tokens {
        if [subject, predicate]
            .into_iter()
            .flatten()
            .any(|value| value.to_lowercase().contains(token.as_str()))
        {
            score = score.max(0.8);
            break;
        }
    }
    score
}

pub fn time_recency_score(created_at: &str) -> f64 {
    let parsed = sdkwork_utils_rust::parse_datetime(created_at, None);
    let Some(timestamp) = parsed else {
        return 0.0;
    };
    let age_hours = (sdkwork_utils_rust::now() - timestamp).num_hours().max(0) as f64;
    recency_score_for_age_hours(age_hours)
}

fn recency_score_for_age_hours(age_hours: f64) -> f64 {
    // A seven-day half-life is useful as a tie-breaking retrieval signal without
    // erasing stable semantic memory after a single day.
    2_f64.powf(-age_hours / (24.0 * 7.0)).clamp(0.05, 1.0)
}

pub fn event_match_score(query: &str, payload_text: &str) -> f64 {
    keyword_match_score(query, payload_text)
}

fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{2F800}'..='\u{2FA1F}'
    )
}

fn tokenise_query(text: &str) -> Vec<String> {
    let text = text.trim().to_lowercase();
    if text.is_empty() {
        return Vec::new();
    }

    if text.chars().any(is_cjk) {
        let mut tokens = Vec::new();
        let mut word = String::new();
        for character in text.chars() {
            if is_cjk(character) {
                if !word.is_empty() {
                    tokens.push(std::mem::take(&mut word));
                }
                tokens.push(character.to_string());
            } else if character.is_alphanumeric() {
                word.push(character);
            } else if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
        }
        if !word.is_empty() {
            tokens.push(word);
        }
        let mut seen = HashSet::new();
        tokens.retain(|token| seen.insert(token.clone()));
        return tokens;
    }

    text.split(|ch: char| !ch.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn token_overlap_score(haystack: &str, tokens: &[String]) -> f64 {
    if tokens.is_empty() {
        return 0.0;
    }
    let matched = tokens
        .iter()
        .filter(|token| haystack.contains(token.as_str()))
        .count();
    matched as f64 / tokens.len() as f64
}

fn retriever_weight(profile: Option<&Value>, retriever: &str, default_weight: f64) -> f64 {
    match profile {
        Some(profile) => profile
            .get(retriever)
            .and_then(|entry| entry.get("weight"))
            .and_then(Value::as_f64)
            .filter(|weight| weight.is_finite() && *weight > 0.0)
            .unwrap_or(0.0),
        None => default_weight,
    }
}

/// Clamp a caller-supplied similarity onto the shared `[0.0, 1.0]` signal scale.
///
/// Non-finite input collapses to `0.0` instead of propagating a `NaN` ordering into
/// the sort, and non-positive input collapses to `0.0` so the shared signal filter
/// discards it: an anti-correlated embedding is not evidence for a memory. The upper
/// clamp mirrors upstream `min(combined / max_possible, 1.0)` -- no single provider may
/// outrank every lexical signal by reporting an unbounded dot product.
fn vector_similarity_score(similarity: f64) -> f64 {
    if !similarity.is_finite() {
        return 0.0;
    }
    similarity.clamp(0.0, 1.0)
}

fn append_ranked_signal(
    output: &mut Vec<OrchestratedCandidate>,
    retriever_name: &str,
    weight: f64,
    scored_records: impl IntoIterator<Item = (RetrievalRecordInput, f64)>,
    top_k: usize,
) {
    if weight <= 0.0 || top_k == 0 {
        return;
    }

    let mut best_by_memory: HashMap<String, (RetrievalRecordInput, f64)> = HashMap::new();
    for (record, score) in scored_records {
        if !score.is_finite() || score <= 0.0 {
            continue;
        }
        best_by_memory
            .entry(record.memory_id.clone())
            .and_modify(|existing| {
                if score > existing.1 {
                    *existing = (record.clone(), score);
                }
            })
            .or_insert((record, score));
    }

    let mut ranked = best_by_memory.into_values().collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        score_order(left.1, right.1).then_with(|| left.0.memory_id.cmp(&right.0.memory_id))
    });
    output.extend(
        ranked
            .into_iter()
            .take(top_k)
            .enumerate()
            .map(|(index, (record, score))| OrchestratedCandidate {
                record,
                retriever_name: retriever_name.to_string(),
                raw_score: score * weight,
                rank: (index as i32) + 1,
            }),
    );
}

/// Orchestrate the lexical and temporal recall signals into independently ranked lists.
///
/// This is the composition the memory service has always used. It is exactly
/// [`orchestrate_retrieval_candidates_with_vector`] with an empty similarity slice, so a
/// deployment with no embedding provider bound keeps byte-identical rankings.
pub fn orchestrate_retrieval_candidates(
    query: &str,
    records: &[RetrievalRecordInput],
    events: &[RetrievalEventInput],
    profile: Option<&Value>,
    top_k: usize,
) -> Vec<OrchestratedCandidate> {
    orchestrate_retrieval_candidates_with_vector(query, records, events, &[], profile, top_k)
}

/// Orchestrate every retrieval signal, optionally including a vector-similarity signal.
///
/// `vector_similarities` carries similarities the caller already computed; see
/// [`VectorSimilarityInput`]. The signal is opt-in through the profile: it contributes
/// only when the profile grants `vector` a positive weight, so a profile written before
/// vector recall existed cannot silently start scoring on it. That opt-in is stricter
/// than the other signals because vector recall needs a bound embedding provider, which
/// this crate cannot observe.
///
/// Similarities whose `memory_id` is absent from `records` are ignored. Vector recall
/// re-ranks the candidate universe the caller already rehydrated and authorized; it
/// never introduces a memory the caller did not read.
pub fn orchestrate_retrieval_candidates_with_vector(
    query: &str,
    records: &[RetrievalRecordInput],
    events: &[RetrievalEventInput],
    vector_similarities: &[VectorSimilarityInput],
    profile: Option<&Value>,
    top_k: usize,
) -> Vec<OrchestratedCandidate> {
    let mut output = Vec::new();

    let keyword_weight = retriever_weight(profile, "keyword", 1.0);
    append_ranked_signal(
        &mut output,
        "keyword",
        keyword_weight,
        records.iter().map(|record| {
            (
                record.clone(),
                keyword_match_score(query, &record.canonical_text),
            )
        }),
        top_k,
    );

    let dictionary_weight = retriever_weight(profile, "dictionary", 0.85);
    append_ranked_signal(
        &mut output,
        "dictionary",
        dictionary_weight,
        records.iter().cloned().map(|record| {
            let score = dictionary_match_score(
                query,
                record.subject.as_deref(),
                record.predicate.as_deref(),
                &record.object_text,
            );
            (record, score)
        }),
        top_k,
    );

    let sql_weight = retriever_weight(profile, "sql", 0.75);
    append_ranked_signal(
        &mut output,
        "sql",
        sql_weight,
        records.iter().cloned().map(|record| {
            let score = sql_structured_match_score(
                query,
                record.subject.as_deref(),
                record.predicate.as_deref(),
            );
            (record, score)
        }),
        top_k,
    );

    let time_weight = retriever_weight(profile, "time", 0.5);
    append_ranked_signal(
        &mut output,
        "time",
        time_weight,
        records.iter().filter_map(|record| {
            let lexical_score =
                keyword_match_score(query, &record.canonical_text).max(dictionary_match_score(
                    query,
                    record.subject.as_deref(),
                    record.predicate.as_deref(),
                    &record.object_text,
                ));
            (lexical_score > 0.0 || is_blank(Some(query)))
                .then(|| (record.clone(), time_recency_score(&record.created_at)))
        }),
        top_k,
    );

    let event_weight = retriever_weight(profile, "event", 0.6);
    append_ranked_signal(
        &mut output,
        "event",
        event_weight,
        events.iter().map(|event| {
            let record = RetrievalRecordInput {
                memory_id: event
                    .memory_id
                    .clone()
                    .unwrap_or_else(|| format!("event:{}", event.event_id)),
                subject: Some("event".to_string()),
                predicate: Some("mentions".to_string()),
                object_text: event.payload_text.clone(),
                canonical_text: event.payload_text.clone(),
                created_at: event.created_at.clone(),
            };
            (record, event_match_score(query, &event.payload_text))
        }),
        top_k,
    );

    let vector_weight = retriever_weight(profile, "vector", 0.0);
    if vector_weight > 0.0 {
        let records_by_id = records
            .iter()
            .map(|record| (record.memory_id.as_str(), record))
            .collect::<HashMap<&str, &RetrievalRecordInput>>();
        append_ranked_signal(
            &mut output,
            "vector",
            vector_weight,
            vector_similarities.iter().filter_map(|input| {
                let record = records_by_id.get(input.memory_id.as_str())?;
                Some(((*record).clone(), vector_similarity_score(input.similarity)))
            }),
            top_k,
        );
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(memory_id: &str, text: &str) -> RetrievalRecordInput {
        RetrievalRecordInput {
            memory_id: memory_id.to_string(),
            subject: None,
            predicate: None,
            object_text: text.to_string(),
            canonical_text: text.to_string(),
            created_at: sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None),
        }
    }

    #[test]
    fn orchestrator_preserves_independent_ranked_signals() {
        let records = vec![RetrievalRecordInput {
            subject: Some("preference".to_string()),
            predicate: Some("is".to_string()),
            ..record("1", "concise answers")
        }];
        let hits = orchestrate_retrieval_candidates(
            "concise answers",
            &records,
            &[],
            Some(&serde_json::json!({
                "keyword": { "weight": 1.0 },
                "dictionary": { "weight": 0.85 }
            })),
            5,
        );
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].retriever_name, "keyword");
        assert_eq!(hits[1].retriever_name, "dictionary");
        assert_eq!(hits[0].rank, 1);
    }

    #[test]
    fn orchestrator_applies_profile_weights_to_each_signal() {
        let records = vec![record("weighted", "alpha")];
        let hits = orchestrate_retrieval_candidates(
            "alpha",
            &records,
            &[],
            Some(&serde_json::json!({
                "keyword": { "weight": 0.1 },
                "dictionary": { "weight": 0.2 }
            })),
            1,
        );
        assert_eq!(hits.len(), 2);
        assert!((hits[0].raw_score - 0.1).abs() < 1e-9);
        assert!((hits[1].raw_score - 0.2).abs() < 1e-9);
    }

    #[test]
    fn orchestrator_ranks_all_bounded_inputs_before_top_k() {
        let records = vec![
            record("low-a", "needle one"),
            record("low-b", "needle two"),
            record("exact", "needle"),
        ];
        let hits = orchestrate_retrieval_candidates(
            "needle",
            &records,
            &[],
            Some(&serde_json::json!({ "keyword": { "weight": 1.0 } })),
            1,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.memory_id, "exact");
    }

    #[test]
    fn orchestrator_deduplicates_events_for_the_same_memory() {
        let events = vec![
            RetrievalEventInput {
                memory_id: Some("memory-from-event".to_string()),
                event_id: "event-1".to_string(),
                payload_text: "event needle".to_string(),
                created_at: sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None),
            },
            RetrievalEventInput {
                memory_id: Some("memory-from-event".to_string()),
                event_id: "event-2".to_string(),
                payload_text: "needle".to_string(),
                created_at: sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None),
            },
        ];
        let hits = orchestrate_retrieval_candidates(
            "needle",
            &[],
            &events,
            Some(&serde_json::json!({ "event": { "weight": 1.0 } })),
            5,
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].record.memory_id, "memory-from-event");
        assert_eq!(hits[0].retriever_name, "event");
        assert_eq!(hits[0].raw_score, 1.0);
    }

    #[test]
    fn seven_day_recency_half_life_does_not_erase_long_term_memory() {
        assert!((recency_score_for_age_hours(24.0 * 7.0) - 0.5).abs() < 1e-9);
        assert!(recency_score_for_age_hours(24.0) > 0.9);
        assert_eq!(time_recency_score("invalid"), 0.0);
    }

    #[test]
    fn commercial_retrieval_strategies_are_typed_and_materialize_real_retrievers() {
        for strategy in MemoryRetrievalStrategy::all() {
            let profile = strategy.retriever_profile();
            assert!(profile.as_object().is_some_and(|value| !value.is_empty()));
            assert_eq!(
                MemoryRetrievalStrategy::parse(strategy.code()),
                Ok(strategy)
            );
        }
        assert!(MemoryRetrievalStrategy::parse("vector_magic").is_err());
    }

    #[test]
    fn vector_similarity_is_clamped_onto_the_shared_signal_scale() {
        assert!((vector_similarity_score(0.5) - 0.5).abs() < 1e-9);
        assert_eq!(vector_similarity_score(1.0), 1.0);
        assert_eq!(vector_similarity_score(1.5), 1.0);
        assert_eq!(vector_similarity_score(0.0), 0.0);
        assert_eq!(vector_similarity_score(-1.0), 0.0);
        assert_eq!(vector_similarity_score(f64::NAN), 0.0);
        assert_eq!(vector_similarity_score(f64::INFINITY), 0.0);
        assert_eq!(vector_similarity_score(f64::NEG_INFINITY), 0.0);
    }
}
