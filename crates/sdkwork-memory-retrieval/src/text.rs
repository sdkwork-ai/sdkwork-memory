//! Shared CJK classification and bigram segmentation.
//!
//! The RRF lexical tokenizer ([`crate::retrieval`]) and the additive BM25
//! pipeline ([`crate::lemmatization`], [`crate::bm25`]) must agree on what
//! counts as a CJK character and how a CJK run becomes matchable terms, so
//! both strategies produce symmetric lexical signals. There is no equivalent
//! helper in `sdkwork-utils-rust`, so the ranges are owned here.

use std::collections::HashSet;

/// Whether `character` belongs to a script without whitespace boundaries
/// (CJK ideographs, kana, or Hangul syllables) that needs bigram
/// segmentation for lexical matching.
pub(crate) fn is_cjk(character: char) -> bool {
    matches!(character,
        '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{20000}'..='\u{2FA1F}'
        | '\u{3040}'..='\u{30FF}'
        | '\u{AC00}'..='\u{D7A3}'
    )
}

/// Emits a CJK run as adjacent character pairs (bigrams). Unigram scoring let
/// any document sharing one common character earn a full token's worth of
/// overlap, so short Chinese queries pulled in floods of near-zero-evidence
/// matches; a bigram only matches where the two characters are adjacent,
/// which is real lexical evidence. A lone CJK character has no pair and is
/// emitted as itself so single-character queries still score.
pub(crate) fn flush_cjk_run(run: &mut Vec<char>, tokens: &mut Vec<String>) {
    if run.len() == 1 {
        tokens.push(
            run.pop()
                .expect("non-empty by the length check")
                .to_string(),
        );
        return;
    }
    for pair in run.windows(2) {
        tokens.push(pair.iter().collect());
    }
    run.clear();
}

/// Segments every maximal CJK run in `text` into bigrams, deduplicated and in
/// first-occurrence order. A run of one character degrades to that unigram;
/// non-CJK characters break runs without contributing tokens.
pub(crate) fn cjk_bigrams(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut run: Vec<char> = Vec::new();
    for character in text.chars() {
        if is_cjk(character) {
            run.push(character);
        } else {
            flush_cjk_run(&mut run, &mut tokens);
        }
    }
    flush_cjk_run(&mut run, &mut tokens);
    let mut seen = HashSet::new();
    tokens.retain(|token| seen.insert(token.clone()));
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_cjk_covers_the_shared_ranges_and_nothing_else() {
        for character in [
            '\u{3400}',
            '\u{4DBF}',
            '\u{4E00}',
            '\u{9FFF}',
            '\u{F900}',
            '\u{FAFF}',
            '\u{20000}',
            '\u{2FA1F}',
            '\u{3040}',
            '\u{30FF}',
            '\u{AC00}',
            '\u{D7A3}',
        ] {
            assert!(is_cjk(character), "{character:?} must be CJK");
        }
        for character in [
            'a',
            '1',
            '-',
            ' ',
            '\u{FF21}',
            '\u{3000}',
            '\u{D7A4}',
            '\u{2FA20}',
        ] {
            assert!(!is_cjk(character), "{character:?} must not be CJK");
        }
    }

    #[test]
    fn cjk_bigrams_segment_adjacent_pairs_in_order() {
        assert_eq!(
            cjk_bigrams("用户偏好"),
            vec!["用户".to_string(), "户偏".to_string(), "偏好".to_string()]
        );
    }

    #[test]
    fn a_single_character_run_degrades_to_a_unigram() {
        assert_eq!(cjk_bigrams("知"), vec!["知".to_string()]);
        assert_eq!(cjk_bigrams("a知b"), vec!["知".to_string()]);
    }

    #[test]
    fn cjk_bigrams_are_deduplicated_keeping_first_order() {
        assert_eq!(cjk_bigrams("好好"), vec!["好好".to_string()]);
        assert_eq!(
            cjk_bigrams("用户用户"),
            vec!["用户".to_string(), "户用".to_string()]
        );
    }

    #[test]
    fn non_cjk_text_produces_no_tokens() {
        assert!(cjk_bigrams("").is_empty());
        assert!(cjk_bigrams("plain latin 123").is_empty());
    }

    #[test]
    fn kana_and_hangul_count_as_cjk_runs() {
        assert_eq!(cjk_bigrams("かな"), vec!["かな".to_string()]);
        assert_eq!(cjk_bigrams("한글"), vec!["한글".to_string()]);
    }
}
