//! BM25 keyword scoring with query-length-adaptive sigmoid normalization.
//!
//! Keywords retrieval in mem0 is two decoupled stages, and this module ports both:
//!
//! 1. The vector store returns an **unbounded** native BM25 score
//!    (upstream: a Qdrant sparse vector, or PostgreSQL `ts_rank_cd`).
//! 2. `mem0/utils/scoring.py` normalizes that raw score into `[0, 1]` with a
//!    logistic sigmoid whose midpoint and steepness are chosen from the query's
//!    lemmatized term count.
//!
//! Stage 1 needs a real corpus, so this module owns a bounded in-memory Okapi BM25
//! index. Stage 2 is a pure function and is ported term-for-term from upstream so
//! that the same raw score yields the same normalized score.
//!
//! This module deliberately does **not** decide how the normalized score is fused
//! with other signals; see [`crate::scoring`] for the additive fusion contract.

use std::collections::{BTreeMap, HashMap};

/// Okapi BM25 term-frequency saturation constant (`k1`).
pub const BM25_K1: f64 = 1.2;

/// Okapi BM25 document-length normalization constant (`b`).
pub const BM25_B: f64 = 0.75;

/// Default (midpoint, steepness) used when a query has no usable terms.
///
/// Mirrors upstream's `num_terms = len(lemmatized.split()) if lemmatized else 1`
/// fallback, which lands in the `num_terms <= 3` band.
pub const DEFAULT_BM25_PARAMS: (f64, f64) = (5.0, 0.7);

/// One document offered to a [`Bm25Index`].
///
/// `lemmatized_text` must already be the output of
/// [`crate::lemmatization::lemmatize_for_bm25`], so that indexing and querying
/// apply the same token normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bm25Document<'a> {
    /// Stable identifier echoed back in [`Bm25Index::score`] results.
    pub memory_id: &'a str,
    /// Space-joined lemmas for this document.
    pub lemmatized_text: &'a str,
}

/// Split lemmatized text into index terms.
///
/// Terms are lowercased and stripped of empty fragments. Non-Latin scripts are
/// preserved verbatim: CJK text has no whitespace boundaries, so a lemmatized CJK
/// string contributes whole-word terms rather than per-character ones (per-character
/// segmentation already happens in [`crate::retrieval`]'s tokenizer).
#[must_use]
pub fn bm25_terms(lemmatized_text: &str) -> Vec<String> {
    lemmatized_text
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|term| !term.is_empty())
        .collect()
}

/// A fitted BM25 index over a bounded document set.
///
/// The index is immutable after [`Bm25Index::fit`]. `value.index([])` on an empty
/// corpus is valid and returns no scores, so a caller never has to special-case an
/// empty candidate set.
#[derive(Debug, Clone)]
pub struct Bm25Index {
    memory_ids: Vec<String>,
    term_frequencies: Vec<HashMap<String, u32>>,
    document_lengths: Vec<usize>,
    document_frequency: BTreeMap<String, u32>,
    average_document_length: f64,
}

impl Bm25Index {
    /// Fits an index over `documents`.
    ///
    /// Documents whose `memory_id` repeats are kept as separate postings; callers
    /// that need one row per memory must deduplicate before fitting. Insertion
    /// order is preserved so scoring stays deterministic.
    #[must_use]
    pub fn fit(documents: &[Bm25Document<'_>]) -> Self {
        let mut memory_ids = Vec::with_capacity(documents.len());
        let mut term_frequencies = Vec::with_capacity(documents.len());
        let mut document_lengths = Vec::with_capacity(documents.len());
        let mut document_frequency: BTreeMap<String, u32> = BTreeMap::new();
        let mut total_length: usize = 0;

        for document in documents {
            let terms = bm25_terms(document.lemmatized_text);
            let mut frequencies: HashMap<String, u32> = HashMap::new();
            for term in terms {
                *frequencies.entry(term).or_insert(0) += 1;
            }
            for term in frequencies.keys() {
                *document_frequency.entry(term.clone()).or_insert(0) += 1;
            }
            total_length += frequencies
                .values()
                .map(|count| *count as usize)
                .sum::<usize>();
            document_lengths.push(
                frequencies
                    .values()
                    .map(|count| *count as usize)
                    .sum::<usize>(),
            );
            term_frequencies.push(frequencies);
            memory_ids.push(document.memory_id.to_string());
        }

        let average_document_length = if document_lengths.is_empty() {
            0.0
        } else {
            total_length as f64 / document_lengths.len() as f64
        };

        Self {
            memory_ids,
            term_frequencies,
            document_lengths,
            document_frequency,
            average_document_length,
        }
    }

    /// Number of documents the index was fitted over.
    #[must_use]
    pub fn len(&self) -> usize {
        self.memory_ids.len()
    }

    /// Whether the index holds no documents.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.memory_ids.is_empty()
    }

    /// Raw (unbounded) BM25 score for every document whose score is strictly positive.
    ///
    /// Results are sorted by descending score, then by ascending `memory_id`, so the
    /// ranking is total and reproducible. Scores are raw: normalize them with
    /// [`normalize_bm25`] before fusing with bounded signals.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_retrieval::bm25::{Bm25Document, Bm25Index, bm25_terms};
    ///
    /// let index = Bm25Index::fit(&[
    ///     Bm25Document { memory_id: "a", lemmatized_text: "prefer concise answer" },
    ///     Bm25Document { memory_id: "b", lemmatized_text: "enjoy hiking mountain" },
    /// ]);
    /// let scored = index.score(&bm25_terms("concise answer"));
    /// assert_eq!(scored.len(), 1);
    /// assert_eq!(scored[0].0, "a");
    /// ```
    #[must_use]
    pub fn score(&self, query_terms: &[String]) -> Vec<(String, f64)> {
        if query_terms.is_empty() || self.memory_ids.is_empty() {
            return Vec::new();
        }

        let total_documents = self.memory_ids.len() as f64;
        let mut scored = Vec::new();

        for (position, memory_id) in self.memory_ids.iter().enumerate() {
            let frequencies = &self.term_frequencies[position];
            let document_length = self.document_lengths[position] as f64;
            let mut score = 0.0_f64;

            for term in query_terms {
                let Some(term_frequency) = frequencies.get(term.as_str()) else {
                    continue;
                };
                let document_frequency = self
                    .document_frequency
                    .get(term.as_str())
                    .copied()
                    .unwrap_or(0) as f64;
                if document_frequency == 0.0 {
                    continue;
                }
                score += idf(document_frequency, total_documents)
                    * term_frequency_saturation(
                        f64::from(*term_frequency),
                        document_length,
                        self.average_document_length,
                    );
            }

            if score.is_finite() && score > 0.0 {
                scored.push((memory_id.clone(), score));
            }
        }

        scored.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.0.cmp(&right.0))
        });
        scored
    }
}

/// Probabilistic IDF in the always-positive Lucene form.
///
/// `ln(1 + (N - df + 0.5) / (df + 0.5))` never returns zero and never goes negative,
/// so a term that appears in every document cannot *penalize* the documents that
/// contain it. The older Robertson form without the leading `1 +` does reach zero —
/// and goes negative for `df > N / 2` — which would let a common word push a
/// genuinely relevant memory below the relevance threshold.
///
/// The trade-off is that a universal term contributes a small equal score to every
/// document instead of exactly nothing. Equal contributions do not change relative
/// order, and [`crate::scoring`] guards the absolute scale with its `max_possible`
/// divisor.
fn idf(document_frequency: f64, total_documents: f64) -> f64 {
    let numerator = total_documents - document_frequency + 0.5;
    let denominator = document_frequency + 0.5;
    if total_documents <= 0.0 || document_frequency <= 0.0 || numerator <= 0.0 || denominator <= 0.0
    {
        return 0.0;
    }
    (1.0 + numerator / denominator).ln()
}

/// BM25 term-frequency saturation, including the document-length penalty.
fn term_frequency_saturation(
    term_frequency: f64,
    document_length: f64,
    average_document_length: f64,
) -> f64 {
    let length_ratio = if average_document_length > 0.0 {
        document_length / average_document_length
    } else {
        1.0
    };
    let denominator = term_frequency + BM25_K1 * (1.0 - BM25_B + BM25_B * length_ratio);
    if denominator <= 0.0 {
        return 0.0;
    }
    term_frequency * (BM25_K1 + 1.0) / denominator
}

/// Query-length-adaptive sigmoid parameters `(midpoint, steepness)`.
///
/// Longer queries produce higher raw BM25 scores, so the midpoint rises with the
/// term count. Bands are ported from upstream `get_bm25_params`
/// (`mem0/utils/scoring.py:16-40`) and must stay in sync with `normalize_bm25`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::bm25::{DEFAULT_BM25_PARAMS, get_bm25_params};
///
/// assert_eq!(get_bm25_params(&["wine".to_string()]), (5.0, 0.7));
/// assert_eq!(get_bm25_params(&[]), DEFAULT_BM25_PARAMS);
/// let long = get_bm25_params(&vec!["term".to_string(); 20]);
/// assert_eq!(long, (12.0, 0.5));
/// ```
#[must_use]
pub fn get_bm25_params(query_terms: &[String]) -> (f64, f64) {
    let term_count = query_terms.len().max(1);
    match term_count {
        0..=3 => (5.0, 0.7),
        4..=6 => (7.0, 0.6),
        7..=9 => (9.0, 0.5),
        10..=15 => (10.0, 0.5),
        _ => (12.0, 0.5),
    }
}

/// Normalizes a raw BM25 score into `[0, 1]` with a logistic sigmoid.
///
/// Ported term-for-term from upstream `normalize_bm25`
/// (`mem0/utils/scoring.py:43-54`): `1 / (1 + exp(-steepness * (raw - midpoint)))`.
/// A non-finite input, or a non-positive steepness, yields `0.0` rather than a
/// `NaN`, so a malformed provider score can never poison a fusion sum.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::bm25::normalize_bm25;
///
/// assert!((normalize_bm25(5.0, 5.0, 0.7) - 0.5).abs() < 1e-12);
/// assert!(normalize_bm25(f64::NAN, 5.0, 0.7) == 0.0);
/// ```
#[must_use]
pub fn normalize_bm25(raw_score: f64, midpoint: f64, steepness: f64) -> f64 {
    if !raw_score.is_finite() || !midpoint.is_finite() || !steepness.is_finite() || steepness <= 0.0
    {
        return 0.0;
    }
    (1.0 / (1.0 + (-steepness * (raw_score - midpoint)).exp())).clamp(0.0, 1.0)
}

/// Scores `documents` against a lemmatized query and returns **normalized** scores
/// keyed by `memory_id` in descending order.
///
/// This is the composed entry point for callers that want upstream's two-stage
/// keyword signal without wiring the two stages themselves. Only documents that
/// share at least one query term are emitted; a document with no match has no
/// keyword signal at all and is absent from the result rather than present as `0.0`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::bm25::{Bm25Document, normalized_keyword_scores};
///
/// let documents = [
///     Bm25Document { memory_id: "a", lemmatized_text: "prefer concise answer" },
///     Bm25Document { memory_id: "b", lemmatized_text: "prefer hiking" },
/// ];
/// let scores = normalized_keyword_scores("concise answer", &documents);
/// assert_eq!(scores.len(), 1);
/// assert_eq!(scores[0].0, "a");
/// assert!(scores[0].1 > 0.0 && scores[0].1 <= 1.0);
/// ```
#[must_use]
pub fn normalized_keyword_scores(
    lemmatized_query: &str,
    documents: &[Bm25Document<'_>],
) -> Vec<(String, f64)> {
    let query_terms = bm25_terms(lemmatized_query);
    if query_terms.is_empty() || documents.is_empty() {
        return Vec::new();
    }
    let (midpoint, steepness) = get_bm25_params(&query_terms);
    let index = Bm25Index::fit(documents);
    index
        .score(&query_terms)
        .into_iter()
        .map(|(memory_id, raw)| (memory_id, normalize_bm25(raw, midpoint, steepness)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document<'a>(memory_id: &'a str, text: &'a str) -> Bm25Document<'a> {
        Bm25Document {
            memory_id,
            lemmatized_text: text,
        }
    }

    #[test]
    fn idf_decreases_as_a_term_becomes_more_common_and_never_goes_negative() {
        let total = 10.0;
        let rare = idf(1.0, total);
        let middling = idf(5.0, total);
        let universal = idf(total, total);
        assert!(rare > middling, "rarer terms must weigh more");
        assert!(middling > universal);
        assert!(universal > 0.0, "Lucene-style idf stays strictly positive");
        assert_eq!(idf(0.0, 0.0), 0.0, "degenerate input must not produce NaN");
    }

    #[test]
    fn score_is_positive_only_for_documents_sharing_a_query_term() {
        let index = Bm25Index::fit(&[
            document("a", "prefer concise answer"),
            document("b", "enjoy hiking"),
        ]);
        let scored = index.score(&bm25_terms("concise answer"));
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].0, "a");
    }

    #[test]
    fn score_is_empty_for_a_blank_query_or_an_empty_corpus() {
        let index = Bm25Index::fit(&[document("a", "prefer concise answer")]);
        assert!(index.score(&[]).is_empty());
        assert!(Bm25Index::fit(&[])
            .score(&bm25_terms("anything"))
            .is_empty());
        assert!(Bm25Index::fit(&[]).is_empty());
        assert_eq!(Bm25Index::fit(&[]).len(), 0);
    }

    #[test]
    fn rarer_terms_outrank_common_terms() {
        let index = Bm25Index::fit(&[
            document("common-a", "wine pairing"),
            document("common-b", "wine tasting"),
            document("rare", "wine sommelier certificate"),
        ]);
        let scored = index.score(&bm25_terms("certificate"));
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].0, "rare");
    }

    #[test]
    fn universal_terms_do_not_discriminate_between_documents() {
        let index = Bm25Index::fit(&[
            document("a", "wine pairing"),
            document("b", "wine tasting"),
            document("c", "wine cellar"),
        ]);
        let scored = index.score(&bm25_terms("wine"));
        assert_eq!(scored.len(), 3, "every document contains the term");
        // Equal contributions must not reorder anything, so the ranking falls back
        // to the deterministic tie-break.
        assert!(scored.iter().all(|(_, score)| *score == scored[0].1));
        assert_eq!(scored[0].0, "a");
        assert_eq!(scored[1].0, "b");
        assert_eq!(scored[2].0, "c");
    }

    #[test]
    fn a_discriminating_term_dominates_a_universal_one() {
        let index = Bm25Index::fit(&[
            document("noise", "wine cellar"),
            document("match", "wine sommelier"),
        ]);
        let combined = index.score(&bm25_terms("wine sommelier"));
        assert_eq!(combined.len(), 2);
        assert_eq!(
            combined[0].0, "match",
            "the rare term must outweigh the universal one"
        );
        let universal_only = index.score(&bm25_terms("wine"));
        assert!(
            combined[0].1 > universal_only[0].1,
            "adding a discriminating term must raise the top score"
        );
    }

    #[test]
    fn longer_documents_are_penalized_for_the_same_term_frequency() {
        let index = Bm25Index::fit(&[
            document("short", "needle"),
            document(
                "long",
                "needle filler filler filler filler filler filler filler filler filler",
            ),
        ]);
        let scored = index.score(&bm25_terms("needle"));
        assert_eq!(scored.len(), 2);
        assert_eq!(scored[0].0, "short", "shorter document must score higher");
    }

    #[test]
    fn repeated_query_terms_amplify_a_single_document() {
        let index = Bm25Index::fit(&[document("a", "needle"), document("b", "needle thread")]);
        let once = index.score(&bm25_terms("needle"));
        let twice = index.score(&bm25_terms("needle needle"));
        assert!((twice[0].1 / once[0].1 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn scores_are_totally_ordered_for_tied_values() {
        let index = Bm25Index::fit(&[document("zzz", "needle"), document("aaa", "needle")]);
        let scored = index.score(&bm25_terms("needle"));
        assert_eq!(scored.len(), 2);
        assert_eq!(scored[0].1, scored[1].1);
        assert_eq!(scored[0].0, "aaa", "ties break by ascending memory id");
    }

    #[test]
    fn adaptive_params_follow_upstream_bands() {
        assert_eq!(get_bm25_params(&[]), (5.0, 0.7));
        assert_eq!(get_bm25_params(&terms(3)), (5.0, 0.7));
        assert_eq!(get_bm25_params(&terms(4)), (7.0, 0.6));
        assert_eq!(get_bm25_params(&terms(6)), (7.0, 0.6));
        assert_eq!(get_bm25_params(&terms(7)), (9.0, 0.5));
        assert_eq!(get_bm25_params(&terms(9)), (9.0, 0.5));
        assert_eq!(get_bm25_params(&terms(10)), (10.0, 0.5));
        assert_eq!(get_bm25_params(&terms(15)), (10.0, 0.5));
        assert_eq!(get_bm25_params(&terms(16)), (12.0, 0.5));
    }

    fn terms(count: usize) -> Vec<String> {
        (0..count).map(|index| format!("t{index}")).collect()
    }

    #[test]
    fn sigmoid_midpoint_is_one_half_and_saturates_at_the_bounds() {
        assert!((normalize_bm25(5.0, 5.0, 0.7) - 0.5).abs() < 1e-12);
        assert!(normalize_bm25(1_000.0, 5.0, 0.7) > 0.999);
        assert!(normalize_bm25(0.0, 5.0, 0.7) < 0.03);
        assert_eq!(normalize_bm25(f64::NAN, 5.0, 0.7), 0.0);
        assert_eq!(normalize_bm25(f64::INFINITY, 5.0, 0.7), 0.0);
        assert_eq!(normalize_bm25(5.0, 5.0, 0.0), 0.0);
        assert_eq!(normalize_bm25(5.0, f64::NAN, 0.7), 0.0);
    }

    #[test]
    fn composed_scoring_normalizes_into_the_unit_interval() {
        let scores = normalized_keyword_scores(
            "sommelier certificate",
            &[
                document("a", "wine sommelier certificate"),
                document("b", "wine pairing"),
            ],
        );
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].0, "a");
        assert!(scores[0].1 > 0.0 && scores[0].1 <= 1.0);
    }

    #[test]
    fn composed_scoring_is_empty_for_blank_input() {
        let documents = [document("a", "wine sommelier")];
        assert!(normalized_keyword_scores("", &documents).is_empty());
        assert!(normalized_keyword_scores("   ", &documents).is_empty());
        assert!(normalized_keyword_scores("wine", &[]).is_empty());
    }

    #[test]
    fn terms_are_lowercased_and_empties_are_dropped() {
        assert_eq!(
            bm25_terms("  Wine   SOMMELIER  "),
            vec!["wine".to_string(), "sommelier".to_string()]
        );
        assert!(bm25_terms("   ").is_empty());
    }
}
