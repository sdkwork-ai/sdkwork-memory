use std::collections::HashSet;

use sdkwork_memory_contract::MemoryRetrievalHit;
use serde_json::{json, Value};

use crate::text::is_cjk;

const NEAR_DUPLICATE_THRESHOLD: f64 = 0.85;

pub fn estimate_tokens(text: &str) -> i32 {
    if text.trim().is_empty() {
        return 0;
    }

    let mut cjk_characters = 0_i32;
    let mut other_characters = 0_i32;
    for character in text.chars() {
        if is_cjk(character) {
            cjk_characters += 1;
        } else if !character.is_whitespace() {
            other_characters += 1;
        }
    }
    cjk_characters + (other_characters + 3) / 4
}

/// Incremental form of [`estimate_tokens`] for a running character count.
///
/// `estimate_tokens` is `cjk + ceil(other / 4)` for any string with a
/// non-whitespace character, and the all-whitespace case collapses to the same
/// `0 + 0`, so a prefix's token count can be maintained in O(1) per character
/// instead of re-scanning the whole output.
fn estimate_tokens_from_counts(cjk_characters: i32, other_characters: i32) -> i32 {
    cjk_characters + (other_characters + 3) / 4
}

pub fn build_context_pack_from_hits(
    hits: &[MemoryRetrievalHit],
    budget_tokens: i32,
) -> (Value, i32, bool) {
    let budget_tokens = budget_tokens.max(0);
    let mut fragments = Vec::new();
    // Token sets are computed once per selected text instead of once per
    // (candidate, selected) pair, keeping dedup O(selected) per candidate.
    let mut selected_token_sets: Vec<HashSet<String>> = Vec::new();
    let mut used_tokens = 0_i32;
    let mut truncated = false;
    let mut deduplicated_count = 0_i32;

    for hit in hits {
        let Some(memory) = hit.memory.as_ref() else {
            continue;
        };
        // Dedup and over-budget skips are recorded separately; only budget
        // pressure sets the pack-level `truncated` flag.
        if is_redundant(&memory.canonical_text, &selected_token_sets) {
            deduplicated_count += 1;
            continue;
        }

        let remaining_tokens = budget_tokens - used_tokens;
        if remaining_tokens <= 0 {
            truncated = true;
            continue;
        }

        let original_tokens = estimate_tokens(&memory.canonical_text);
        let (canonical_text, fragment_tokens, fragment_truncated) =
            if original_tokens <= remaining_tokens {
                (memory.canonical_text.clone(), original_tokens, false)
            } else if remaining_tokens < 2 {
                // Reserving one token for the ellipsis marker would leave zero
                // tokens for content, producing a bare "…" fragment that
                // carries no memory. Report the budget pressure instead.
                truncated = true;
                continue;
            } else if fragments.is_empty() {
                // Reserve one token for the ellipsis marker so the cut is
                // visible inside the text itself, not only in the flags.
                let text = truncate_to_token_budget(&memory.canonical_text, remaining_tokens - 1);
                let text = format!("{text}…");
                let tokens = estimate_tokens(&text);
                (text, tokens, true)
            } else {
                truncated = true;
                continue;
            };

        if canonical_text.is_empty() || fragment_tokens <= 0 {
            continue;
        }

        selected_token_sets.push(similarity_tokens(&canonical_text));
        fragments.push(json!({
            "memoryId": memory.memory_id.to_string(),
            "canonicalText": canonical_text,
            "memoryType": memory.memory_type,
            "retrieverName": hit.retriever_name,
            "rank": hit.result_rank,
            "fusedScore": hit.fused_score,
            "truncated": fragment_truncated,
        }));
        used_tokens += fragment_tokens;
        truncated |= fragment_truncated;
    }

    let pack = json!({
        "fragments": fragments,
        "embeddingOptional": true,
        "selection": {
            "algorithm": "ranked_budgeted_dedup",
            "deduplicatedCount": deduplicated_count,
        }
    });
    (pack, used_tokens, truncated)
}

fn truncate_to_token_budget(text: &str, budget_tokens: i32) -> String {
    if budget_tokens <= 0 {
        return String::new();
    }

    let mut output = String::new();
    let mut cjk_characters = 0_i32;
    let mut other_characters = 0_i32;
    for character in text.chars() {
        output.push(character);
        if is_cjk(character) {
            cjk_characters += 1;
        } else if !character.is_whitespace() {
            other_characters += 1;
        }
        if estimate_tokens_from_counts(cjk_characters, other_characters) > budget_tokens {
            output.pop();
            break;
        }
    }
    output.trim_end().to_string()
}

fn is_redundant(candidate: &str, selected_token_sets: &[HashSet<String>]) -> bool {
    let candidate_tokens = similarity_tokens(candidate);
    selected_token_sets.iter().any(|selected_tokens| {
        jaccard_similarity(&candidate_tokens, selected_tokens) >= NEAR_DUPLICATE_THRESHOLD
    })
}

fn similarity_tokens(text: &str) -> HashSet<String> {
    let normalized = text.to_lowercase();
    if normalized.chars().any(is_cjk) {
        let characters = normalized
            .chars()
            .filter(|character| character.is_alphanumeric())
            .collect::<Vec<_>>();
        if characters.len() < 2 {
            return characters
                .into_iter()
                .map(|character| character.to_string())
                .collect();
        }
        return characters
            .windows(2)
            .map(|pair| pair.iter().collect::<String>())
            .collect();
    }

    normalized
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn jaccard_similarity(left: &HashSet<String>, right: &HashSet<String>) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(right).count();
    let union = left.union(right).count();
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_cjk_and_latin_without_using_utf8_byte_length() {
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        assert_eq!(estimate_tokens("\u{77e5}\u{8bc6}\u{5e93}"), 3);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn incremental_token_counts_match_estimate_tokens_for_every_prefix() {
        let text = "  ab\u{77e5}\u{8bc6}cd \u{30e6}\u{30fc}\u{30b6}  x  \u{4e00}\u{4e8c}\u{4e09} ";
        let mut cjk = 0_i32;
        let mut other = 0_i32;
        assert_eq!(estimate_tokens_from_counts(cjk, other), estimate_tokens(""));
        for (index, character) in text.chars().enumerate() {
            if is_cjk(character) {
                cjk += 1;
            } else if !character.is_whitespace() {
                other += 1;
            }
            let prefix: String = text.chars().take(index + 1).collect();
            assert_eq!(
                estimate_tokens_from_counts(cjk, other),
                estimate_tokens(&prefix),
                "prefix {prefix:?}"
            );
        }
    }

    #[test]
    fn truncation_never_exceeds_the_budget() {
        let truncated = truncate_to_token_budget("a long memory fragment", 2);
        assert!(!truncated.is_empty());
        assert!(estimate_tokens(&truncated) <= 2);
    }

    #[test]
    fn truncation_matches_the_full_rescan_reference_for_mixed_scripts() {
        // Pins the incremental loop to the naive per-character full-rescan
        // result it replaced, so the O(L) rewrite cannot drift per script.
        let text = "\u{7528}\u{6237}abc \u{30c6}\u{30b9}\u{30c8} token budget \u{4e00}\u{4e8c}tail";
        let reference = |budget: i32| -> String {
            let mut output = String::new();
            for character in text.chars() {
                output.push(character);
                if estimate_tokens(&output) > budget {
                    output.pop();
                    break;
                }
            }
            output.trim_end().to_string()
        };
        for budget in 0..=12 {
            assert_eq!(truncate_to_token_budget(text, budget), reference(budget));
        }
    }

    #[test]
    fn detects_near_duplicate_fragments() {
        let selected = vec![similarity_tokens("user prefers concise technical answers")];
        assert!(is_redundant(
            "user prefers concise technical answers",
            &selected
        ));
        assert!(!is_redundant("project deadline is Friday", &selected));
    }
}
