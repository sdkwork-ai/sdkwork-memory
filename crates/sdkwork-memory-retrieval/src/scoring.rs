//! Additive hybrid scoring with an explainable per-signal breakdown.
//!
//! This ports `score_and_rank` from `mem0/utils/scoring.py:60-139`. Upstream's
//! contract is:
//!
//! ```text
//! combined = (semantic + bm25 + entity_boost) / max_possible     (clamped to 1.0)
//! ```
//!
//! where `max_possible` grows with the number of active signals so that a candidate
//! cannot exceed `1.0` just because more retrievers ran:
//!
//! | Active signals | `max_possible` |
//! | --- | --- |
//! | semantic only | `1.0` |
//! | semantic + BM25 | `2.0` |
//! | semantic + entity boost | `1.5` |
//! | semantic + BM25 + entity boost | `2.5` |
//!
//! Two upstream semantics are preserved deliberately and are easy to get wrong:
//!
//! - **The threshold gates the semantic score only, before combining.** A candidate
//!   below the threshold is dropped even when BM25 or an entity boost would lift its
//!   combined score well above it. The gate answers "is this vector hit relevant at
//!   all", not "is the fused score high enough".
//! - **`max_possible` is derived from whether the signal maps are non-empty**, not
//!   from whether the specific candidate has a value in them. One candidate with a
//!   BM25 score reduces every candidate's combined score.
//!
//! This module is intentionally independent of [`crate::retrieval`]'s weighted
//! reciprocal-rank fusion. RRF ranks candidates by ordinal position and is robust to
//! incomparable score distributions; this additive model keeps the raw score
//! magnitudes and is what upstream ships. Both are available, and callers pick per
//! retrieval profile.

use std::collections::BTreeMap;

/// Weight applied to the entity-link boost before summing.
///
/// Ported from `ENTITY_BOOST_WEIGHT` in `mem0/utils/scoring.py:57`.
pub const ENTITY_BOOST_WEIGHT: f64 = 0.5;

/// Default minimum semantic score, matching upstream's `search(threshold=0.1)`.
pub const DEFAULT_SCORE_THRESHOLD: f64 = 0.1;

/// Minimum number of candidates to over-fetch relative to the caller's `top_k`.
///
/// Ported from upstream's `internal_limit = max(limit * 4, 60)`. Over-fetching gives
/// the threshold gate and the fusion step enough material to work with: filtering
/// after a truncation that already discarded the relevant rows cannot recover them.
pub const MIN_INTERNAL_FETCH_LIMIT: u32 = 60;

/// Over-fetch multiplier applied to the caller's `top_k`.
pub const INTERNAL_FETCH_MULTIPLIER: u32 = 4;

/// One candidate produced by semantic (vector) retrieval.
///
/// `semantic_score` is expected in `[0, 1]` with higher meaning more similar. A
/// non-finite value is treated as `0.0` rather than propagated: upstream keeps a
/// `NaN` here, where every comparison is false, so the candidate slips past the
/// threshold gate and poisons the fused score. Sanitizing is a deliberate hardening.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticCandidate {
    /// Stable memory identifier.
    pub memory_id: String,
    /// Vector similarity, higher is more similar.
    pub semantic_score: f64,
}

/// Secondary signals fused with the semantic score.
///
/// Keys are memory ids. An empty map means "this signal did not run", which is
/// different from "this signal ran and found nothing for anyone" — only the former
/// changes [`max_possible_score`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HybridSignals {
    /// Normalized keyword scores, e.g. from [`crate::bm25`].
    pub bm25_scores: BTreeMap<String, f64>,
    /// Entity-link boosts, e.g. from [`crate::entities::entity_boosts`].
    pub entity_boosts: BTreeMap<String, f64>,
}

/// Per-signal breakdown attached to a hit when `explain` is requested.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreDetails {
    /// Raw vector similarity, before fusion.
    pub semantic_score: f64,
    /// Normalized keyword signal, `0.0` when the candidate had no keyword score.
    pub bm25_score: f64,
    /// Entity-link boost, `0.0` when the candidate had no linked entity match.
    pub entity_boost: f64,
    /// `semantic_score + bm25_score + entity_boost`, before normalization.
    pub raw_score: f64,
    /// Divisor applied to `raw_score`.
    pub max_possible_score: f64,
    /// `min(raw_score / max_possible_score, 1.0)`.
    pub final_score: f64,
    /// The semantic threshold this candidate had to clear.
    pub threshold: f64,
}

/// A fused, ranked hit.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredMemoryHit {
    /// Stable memory identifier.
    pub memory_id: String,
    /// Fused score in `[0, 1]`.
    pub score: f64,
    /// Per-signal breakdown; present only when `explain` was requested.
    pub details: Option<ScoreDetails>,
}

/// The `max_possible` divisor for a given set of active signals.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::scoring::{ENTITY_BOOST_WEIGHT, max_possible_score};
///
/// assert_eq!(max_possible_score(false, false), 1.0);
/// assert_eq!(max_possible_score(false, true), 1.0 + ENTITY_BOOST_WEIGHT);
/// assert_eq!(max_possible_score(true, false), 2.0);
/// assert_eq!(max_possible_score(true, true), 2.0 + ENTITY_BOOST_WEIGHT);
/// ```
#[must_use]
pub fn max_possible_score(has_bm25: bool, has_entity_boost: bool) -> f64 {
    let mut total = 1.0;
    if has_bm25 {
        total += 1.0;
    }
    if has_entity_boost {
        total += ENTITY_BOOST_WEIGHT;
    }
    total
}

/// The internal fetch limit to use for a caller-visible `top_k`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::scoring::internal_fetch_limit;
///
/// assert_eq!(internal_fetch_limit(1), 60);
/// assert_eq!(internal_fetch_limit(20), 80);
/// ```
#[must_use]
pub fn internal_fetch_limit(top_k: u32) -> u32 {
    top_k
        .saturating_mul(INTERNAL_FETCH_MULTIPLIER)
        .max(MIN_INTERNAL_FETCH_LIMIT)
}

/// Validates a caller-supplied semantic threshold.
///
/// Upstream rejects anything outside `[0, 1]` with a `ValueError`
/// (`mem0/memory/main.py:223-229`). Rejecting is correct here too: a threshold above
/// `1.0` silently returns nothing for every query, which reads as "the memory store
/// is empty" rather than "the argument is wrong".
///
/// # Errors
///
/// Returns a human-readable reason when `threshold` is not a finite value in `[0, 1]`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::scoring::validate_threshold;
///
/// assert!(validate_threshold(0.1).is_ok());
/// assert!(validate_threshold(1.0).is_ok());
/// assert!(validate_threshold(1.5).is_err());
/// assert!(validate_threshold(f64::NAN).is_err());
/// ```
pub fn validate_threshold(threshold: f64) -> Result<(), String> {
    if !threshold.is_finite() {
        return Err(format!(
            "memory search threshold must be a finite number, got {threshold}"
        ));
    }
    if !(0.0..=1.0).contains(&threshold) {
        return Err(format!(
            "memory search threshold must be within [0, 1], got {threshold}"
        ));
    }
    Ok(())
}

/// Fuses semantic candidates with BM25 and entity-boost signals, then returns the
/// top `top_k` hits by descending fused score.
///
/// Ties are broken by ascending `memory_id`. Upstream relies on Python's stable sort
/// and therefore inherits the vector store's arrival order for ties, which is not
/// reproducible across providers; a total order is used here instead.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use sdkwork_memory_retrieval::scoring::{
///     HybridSignals, SemanticCandidate, score_and_rank,
/// };
///
/// let candidates = [
///     SemanticCandidate { memory_id: "a".into(), semantic_score: 0.50 },
///     SemanticCandidate { memory_id: "b".into(), semantic_score: 0.45 },
/// ];
/// let mut signals = HybridSignals::default();
/// signals.bm25_scores.insert("b".into(), 0.9);
///
/// // "b" is behind on the vector signal but the keyword signal carries it.
/// let hits = score_and_rank(&candidates, &signals, 0.1, 10, false);
/// assert_eq!(hits[0].memory_id, "b");
/// assert_eq!(hits[0].details, None);
/// ```
#[must_use]
pub fn score_and_rank(
    candidates: &[SemanticCandidate],
    signals: &HybridSignals,
    threshold: f64,
    top_k: usize,
    explain: bool,
) -> Vec<ScoredMemoryHit> {
    if top_k == 0 {
        return Vec::new();
    }

    let max_possible = max_possible_score(
        !signals.bm25_scores.is_empty(),
        !signals.entity_boosts.is_empty(),
    );
    let mut scored = Vec::new();

    for candidate in candidates {
        let semantic_score = sanitize(candidate.semantic_score);
        if semantic_score < threshold {
            // Upstream drops the candidate here, before either secondary signal is
            // consulted. A strong keyword match cannot rescue a weak vector hit.
            continue;
        }

        let bm25_score = signals
            .bm25_scores
            .get(&candidate.memory_id)
            .copied()
            .map_or(0.0, sanitize);
        let entity_boost = signals
            .entity_boosts
            .get(&candidate.memory_id)
            .copied()
            .map_or(0.0, sanitize);

        let raw_score = semantic_score + bm25_score + entity_boost;
        let final_score = (raw_score / max_possible).min(1.0);

        scored.push(ScoredMemoryHit {
            memory_id: candidate.memory_id.clone(),
            score: final_score,
            details: explain.then_some(ScoreDetails {
                semantic_score,
                bm25_score,
                entity_boost,
                raw_score,
                max_possible_score: max_possible,
                final_score,
                threshold,
            }),
        });
    }

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    scored.truncate(top_k);
    scored
}

/// Replaces a non-finite score with `0.0` so it cannot poison a fusion sum.
fn sanitize(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(memory_id: &str, semantic_score: f64) -> SemanticCandidate {
        SemanticCandidate {
            memory_id: memory_id.to_string(),
            semantic_score,
        }
    }

    fn signals(bm25: &[(&str, f64)], entities: &[(&str, f64)]) -> HybridSignals {
        HybridSignals {
            bm25_scores: bm25
                .iter()
                .map(|(id, score)| ((*id).to_string(), *score))
                .collect(),
            entity_boosts: entities
                .iter()
                .map(|(id, score)| ((*id).to_string(), *score))
                .collect(),
        }
    }

    #[test]
    fn max_possible_matches_upstream_four_case_table() {
        assert_eq!(max_possible_score(false, false), 1.0);
        assert_eq!(max_possible_score(false, true), 1.5);
        assert_eq!(max_possible_score(true, false), 2.0);
        assert_eq!(max_possible_score(true, true), 2.5);
    }

    #[test]
    fn internal_fetch_limit_uses_the_max_of_scaled_and_floor() {
        assert_eq!(internal_fetch_limit(0), 60);
        assert_eq!(internal_fetch_limit(1), 60);
        assert_eq!(internal_fetch_limit(14), 60);
        assert_eq!(internal_fetch_limit(15), 60);
        assert_eq!(internal_fetch_limit(16), 64);
        assert_eq!(internal_fetch_limit(20), 80);
        assert_eq!(internal_fetch_limit(u32::MAX), u32::MAX);
    }

    #[test]
    fn threshold_gates_the_semantic_score_before_any_secondary_signal() {
        let candidates = [candidate("weak", 0.05)];
        // A maximal keyword and entity signal must still not rescue it.
        let signals = signals(&[("weak", 1.0)], &[("weak", ENTITY_BOOST_WEIGHT)]);
        let hits = score_and_rank(&candidates, &signals, 0.1, 10, false);
        assert!(hits.is_empty(), "the gate runs before fusion");
    }

    #[test]
    fn a_secondary_signal_can_reorder_candidates_above_the_threshold() {
        let candidates = [candidate("a", 0.50), candidate("b", 0.45)];
        let signals = signals(&[("b", 0.9)], &[]);
        let hits = score_and_rank(&candidates, &signals, 0.1, 10, false);
        assert_eq!(hits[0].memory_id, "b");
        // (0.45 + 0.9) / 2.0 = 0.675 versus 0.50 / 2.0 = 0.25.
        assert!((hits[0].score - 0.675).abs() < 1e-12);
        assert!((hits[1].score - 0.25).abs() < 1e-12);
    }

    #[test]
    fn an_active_signal_rescales_every_candidate_including_unmatched_ones() {
        let candidates = [candidate("matched", 1.0), candidate("unmatched", 1.0)];
        let without_bm25 = score_and_rank(&candidates, &HybridSignals::default(), 0.1, 10, false);
        assert!(without_bm25.iter().all(|hit| hit.score == 1.0));

        let with_bm25 = score_and_rank(
            &candidates,
            &signals(&[("matched", 1.0)], &[]),
            0.1,
            10,
            false,
        );
        let unmatched = with_bm25
            .iter()
            .find(|hit| hit.memory_id == "unmatched")
            .expect("candidate survives the gate");
        assert!(
            (unmatched.score - 0.5).abs() < 1e-12,
            "one candidate's keyword score halves everyone else's fused score"
        );
    }

    #[test]
    fn empty_signal_maps_are_treated_as_inactive() {
        let candidates = [candidate("a", 1.0)];
        let hits = score_and_rank(&candidates, &HybridSignals::default(), 0.1, 10, false);
        assert_eq!(
            hits[0].score, 1.0,
            "no divisor beyond the semantic baseline"
        );
    }

    #[test]
    fn combined_score_is_clamped_to_one() {
        // semantic is 1.0 by contract, but a provider may return more.
        let candidates = [candidate("a", 1.4)];
        let signals = signals(&[("a", 1.0)], &[("a", ENTITY_BOOST_WEIGHT)]);
        let hits = score_and_rank(&candidates, &signals, 0.1, 10, false);
        assert_eq!(hits[0].score, 1.0);
    }

    #[test]
    fn explain_produces_the_full_breakdown() {
        let candidates = [candidate("a", 0.4)];
        let signals = signals(&[("a", 0.8)], &[("a", 0.25)]);
        let hits = score_and_rank(&candidates, &signals, 0.1, 10, true);
        let details = hits[0].details.clone().expect("explain requested");

        assert!((details.semantic_score - 0.4).abs() < 1e-12);
        assert!((details.bm25_score - 0.8).abs() < 1e-12);
        assert!((details.entity_boost - 0.25).abs() < 1e-12);
        assert!((details.raw_score - 1.45).abs() < 1e-12);
        assert!((details.max_possible_score - 2.5).abs() < 1e-12);
        assert!((details.threshold - 0.1).abs() < 1e-12);
        assert!((details.final_score - 0.58).abs() < 1e-12);
        assert_eq!(hits[0].score, details.final_score);
    }

    #[test]
    fn explain_reports_zero_for_signals_the_candidate_has_no_value_for() {
        let candidates = [candidate("a", 0.5)];
        let hits = score_and_rank(&candidates, &signals(&[("other", 1.0)], &[]), 0.1, 10, true);
        let details = hits[0].details.clone().expect("explain requested");
        assert_eq!(details.bm25_score, 0.0);
        assert_eq!(details.entity_boost, 0.0);
        assert!((details.max_possible_score - 2.0).abs() < 1e-12);
    }

    #[test]
    fn hits_are_truncated_to_top_k_after_ranking() {
        let candidates = [
            candidate("a", 0.1),
            candidate("b", 0.9),
            candidate("c", 0.5),
        ];
        let hits = score_and_rank(&candidates, &HybridSignals::default(), 0.1, 2, false);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].memory_id, "b");
        assert_eq!(hits[1].memory_id, "c");
    }

    #[test]
    fn top_k_zero_short_circuits() {
        let candidates = [candidate("a", 1.0)];
        assert!(score_and_rank(&candidates, &HybridSignals::default(), 0.0, 0, false).is_empty());
    }

    #[test]
    fn ties_break_by_ascending_memory_id() {
        let candidates = [
            candidate("zzz", 0.5),
            candidate("aaa", 0.5),
            candidate("mmm", 0.5),
        ];
        let hits = score_and_rank(&candidates, &HybridSignals::default(), 0.1, 10, false);
        let ids = hits
            .iter()
            .map(|hit| hit.memory_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["aaa", "mmm", "zzz"]);
    }

    #[test]
    fn non_finite_scores_are_sanitized_instead_of_poisoning_the_result() {
        let candidates = [candidate("nan", f64::NAN), candidate("inf", f64::INFINITY)];
        let signals = signals(&[("nan", f64::NAN)], &[("inf", f64::INFINITY)]);
        let hits = score_and_rank(&candidates, &signals, 0.0, 10, true);

        for hit in &hits {
            assert!(hit.score.is_finite(), "fused score must stay finite");
            let details = hit.details.as_ref().expect("explain requested");
            assert!(details.raw_score.is_finite());
            assert_eq!(details.bm25_score, 0.0);
            assert_eq!(details.entity_boost, 0.0);
        }
        // NaN sanitizes to 0.0, which is not below a zero threshold, so it survives.
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn zero_threshold_admits_weak_candidates() {
        let candidates = [candidate("a", 0.0)];
        let hits = score_and_rank(&candidates, &HybridSignals::default(), 0.0, 10, false);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn validate_threshold_accepts_the_closed_unit_interval_only() {
        assert!(validate_threshold(0.0).is_ok());
        assert!(validate_threshold(0.1).is_ok());
        assert!(validate_threshold(1.0).is_ok());
        assert!(validate_threshold(-0.001).is_err());
        assert!(validate_threshold(1.001).is_err());
        assert!(validate_threshold(f64::NAN).is_err());
        assert!(validate_threshold(f64::INFINITY).is_err());
    }

    #[test]
    fn empty_candidate_set_yields_no_hits() {
        assert!(score_and_rank(&[], &HybridSignals::default(), 0.1, 10, true).is_empty());
    }
}
