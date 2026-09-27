use std::collections::HashSet;

use sdkwork_memory_contract::MemoryRetrievalHit;
use serde_json::{json, Value};

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

pub fn build_context_pack_from_hits(
    hits: &[MemoryRetrievalHit],
    budget_tokens: i32,
) -> (Value, i32, bool) {
    let budget_tokens = budget_tokens.max(0);
    let mut fragments = Vec::new();
    let mut selected_texts = Vec::new();
    let mut used_tokens = 0_i32;
    let mut truncated = false;
    let mut deduplicated_count = 0_i32;

    for hit in hits {
        let Some(memory) = hit.memory.as_ref() else {
            continue;
        };
        // Dedup and over-budget skips are recorded separately; only budget
        // pressure sets the pack-level `truncated` flag.
        if is_redundant(&memory.canonical_text, &selected_texts) {
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

        selected_texts.push(canonical_text.clone());
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
    for character in text.chars() {
        output.push(character);
        if estimate_tokens(&output) > budget_tokens {
            output.pop();
            break;
        }
    }
    output.trim_end().to_string()
}

fn is_redundant(candidate: &str, selected: &[String]) -> bool {
    let candidate_tokens = similarity_tokens(candidate);
    selected.iter().any(|text| {
        let selected_tokens = similarity_tokens(text);
        jaccard_similarity(&candidate_tokens, &selected_tokens) >= NEAR_DUPLICATE_THRESHOLD
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

fn is_cjk(character: char) -> bool {
    matches!(character,
        '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{2F800}'..='\u{2FA1F}'
    )
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
    fn truncation_never_exceeds_the_budget() {
        let truncated = truncate_to_token_budget("a long memory fragment", 2);
        assert!(!truncated.is_empty());
        assert!(estimate_tokens(&truncated) <= 2);
    }

    #[test]
    fn detects_near_duplicate_fragments() {
        let selected = vec!["user prefers concise technical answers".to_string()];
        assert!(is_redundant(
            "user prefers concise technical answers",
            &selected
        ));
        assert!(!is_redundant("project deadline is Friday", &selected));
    }
}
