//! Deterministic lemmatization for BM25 keyword matching.
//!
//! Upstream mem0 lemmatizes with spaCy and then stores the result in the
//! `text_lemmatized` payload field, so that indexing and querying agree on token
//! surface forms (`mem0/utils/lemmatization.py`). spaCy is a large model
//! dependency this workspace does not take, so this module implements the same
//! *contract* with a closed, auditable rule set.
//!
//! The rules are deliberately conservative, mirroring upstream's stated intent:
//!
//! - **Inflectional** endings are folded: `attending` / `attends` / `attended` → `attend`,
//!   `memories` → `memory`, `older` / `oldest` → `old`.
//! - **Derivational** endings are left alone, so `organization` does **not** collapse
//!   into `organize`. Over-stemming destroys keyword precision, and upstream calls
//!   this out explicitly.
//! - Words whose stripped form would be too short to be meaningful are left intact.
//!
//! Upstream also appends the original token when it ends in `-ing` and differs from
//! the lemma, to survive noun/verb ambiguity (`meeting` is both a noun and a verb
//! form). [`lemmatize_for_bm25`] preserves that duplication so a keyword query for
//! `meeting` still matches a document that stored `meet`.

/// English stopwords dropped from lemmatized output.
///
/// Kept small on purpose: BM25's IDF already suppresses terms that appear in every
/// document, so an aggressive stopword list mostly removes signal. This list covers
/// only words that carry no topical content in a memory statement.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "if", "then", "than", "so", "as", "at", "by", "for",
    "from", "in", "into", "of", "on", "onto", "to", "with", "without", "is", "are", "was", "were",
    "be", "been", "being", "am", "do", "does", "did", "have", "has", "had", "it", "its", "this",
    "that", "these", "those", "i", "you", "he", "she", "we", "they", "them", "his", "her", "our",
    "their", "my", "your",
];

/// Derivational suffixes that mark a different lexeme rather than an inflection.
///
/// Stripping any of these would over-stem (`organization` → `organize`,
/// `happiness` → `happi`), so a token carrying one is returned unchanged.
const DERIVATIONAL_SUFFIXES: &[&str] = &[
    "tion", "sion", "ation", "ment", "ness", "ity", "ance", "ence", "ism", "ist", "ize", "ise",
    "ology", "ship", "hood", "ward",
];

/// Irregular forms that no suffix rule can recover.
///
/// Keys are the observed surface form, values the lemma. Only high-frequency
/// irregulars are listed; anything absent falls through to the suffix rules.
const IRREGULARS: &[(&str, &str)] = &[
    ("are", "be"),
    ("was", "be"),
    ("were", "be"),
    ("been", "be"),
    ("being", "be"),
    ("am", "be"),
    ("is", "be"),
    ("has", "have"),
    ("had", "have"),
    ("having", "have"),
    ("does", "do"),
    ("did", "do"),
    ("done", "do"),
    ("doing", "do"),
    ("went", "go"),
    ("gone", "go"),
    ("going", "go"),
    ("children", "child"),
    ("men", "man"),
    ("women", "woman"),
    ("people", "person"),
    ("feet", "foot"),
    ("teeth", "tooth"),
    ("mice", "mouse"),
    ("geese", "goose"),
    ("better", "good"),
    ("best", "good"),
    ("worse", "bad"),
    ("worst", "bad"),
    ("more", "much"),
    ("most", "much"),
    ("said", "say"),
    ("made", "make"),
    ("took", "take"),
    ("taken", "take"),
    ("gave", "give"),
    ("given", "give"),
    ("got", "get"),
    ("gotten", "get"),
    ("ran", "run"),
    ("saw", "see"),
    ("seen", "see"),
    ("came", "come"),
    ("became", "become"),
];

/// Minimum length a stripped stem must retain to be accepted.
///
/// Tokens whose stem would be shorter are returned unchanged. Without a full
/// vocabulary there is no safe way to tell `seed` from `see` + `d`, so the rule
/// trades a few recall misses for never corrupting a short word.
const MIN_STEM_LEN: usize = 3;

/// Stems that only become a word once a silent `e` is restored.
///
/// English elides a final `e` before `-ing` / `-ed` (`make` → `making`), but the
/// stripped stem is not always recoverable by rule: `mak` needs the `e` back while
/// `meet` (from `meeting`) must not get one. Upstream resolves this with spaCy's
/// full vocabulary lookup; this table is the curated equivalent for the stems that
/// actually occur in memory statements. Anything absent simply keeps its stripped
/// stem, which stays consistent between indexing and querying.
const SILENT_E_STEMS: &[&str] = &[
    "mak", "tak", "giv", "com", "hav", "us", "writ", "driv", "choos", "mov", "lov", "lik", "serv",
    "creat", "manag", "sourc", "typ", "nam", "stor", "updat", "remov", "delet", "merg", "split",
    "measur", "requir", "achiev", "includ", "larg", "chang", "shar", "valu", "siz", "tim", "dat",
    "not", "quot", "defin", "describ", "prefer", "configur", "regist", "valid", "cach", "handl",
    "compil", "rout", "resolv", "provid", "schedul", "notifi", "encod", "decod", "escap",
];

/// Whether `stem` is a known silent-`e` stem.
fn is_silent_e_stem(stem: &str) -> bool {
    SILENT_E_STEMS.contains(&stem)
}

/// Whether `stem` ends in a sibilant that takes `-es` rather than `-s` as its plural.
fn ends_with_sibilant(stem: &str) -> bool {
    stem.ends_with("ss")
        || stem.ends_with('x')
        || stem.ends_with('z')
        || stem.ends_with("sh")
        || stem.ends_with("ch")
}

/// Whether `token` is an English stopword.
#[must_use]
pub fn is_stopword(token: &str) -> bool {
    STOPWORDS.contains(&token)
}

/// Lemmatizes a single lowercase-or-mixed-case token.
///
/// The returned lemma is lowercase. Non-alphabetic input (digits, identifiers such
/// as `gpt-4o`, or CJK text) is returned lowercased but otherwise untouched, because
/// suffix stripping on a non-word is meaningless and would corrupt identifiers that
/// keyword search must match exactly.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::lemmatization::lemmatize_token;
///
/// assert_eq!(lemmatize_token("memories"), "memory");
/// assert_eq!(lemmatize_token("attended"), "attend");
/// assert_eq!(lemmatize_token("oldest"), "old");
/// // Derivational: must NOT over-stem.
/// assert_eq!(lemmatize_token("organization"), "organization");
/// // Identifiers survive intact.
/// assert_eq!(lemmatize_token("gpt-4o"), "gpt-4o");
/// ```
#[must_use]
pub fn lemmatize_token(token: &str) -> String {
    let lowered = token.to_lowercase();
    if lowered.is_empty() {
        return lowered;
    }

    if let Some((_, lemma)) = IRREGULARS.iter().find(|(form, _)| *form == lowered) {
        return (*lemma).to_string();
    }

    // Only pure ASCII alphabetic words take part in suffix stripping. Identifiers,
    // numbers, and CJK must match exactly for keyword search to stay useful.
    if !lowered
        .chars()
        .all(|character| character.is_ascii_alphabetic())
    {
        return lowered;
    }
    if lowered.len() < MIN_STEM_LEN {
        return lowered;
    }
    if DERIVATIONAL_SUFFIXES
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
    {
        return lowered;
    }

    strip_suffix_rules(&lowered).unwrap_or(lowered)
}

/// Applies the inflectional suffix rules in decreasing specificity.
///
/// Returns `None` when no rule yields a stem long enough to be trusted, which keeps
/// the caller's original token in place.
fn strip_suffix_rules(token: &str) -> Option<String> {
    // Plurals whose final consonant changes.
    if let Some(stem) = token.strip_suffix("ies") {
        if stem.len() >= MIN_STEM_LEN {
            return Some(format!("{stem}y"));
        }
    }
    if let Some(stem) = token.strip_suffix("ves") {
        if stem.len() >= MIN_STEM_LEN {
            return Some(format!("{stem}f"));
        }
    }

    // Sibilant plurals: `-es` is a plural marker only after a sibilant, so
    // `classes` -> `class` and `boxes` -> `box`, while `houses` falls through to
    // the bare `-s` rule and becomes `house`.
    if let Some(stem) = token.strip_suffix("es") {
        if stem.len() >= MIN_STEM_LEN && ends_with_sibilant(stem) {
            return Some(stem.to_string());
        }
    }

    if let Some(stem) = token.strip_suffix("ed") {
        if acceptable_stem(stem) {
            return Some(restore_after_suffix_strip(stem));
        }
    }
    if let Some(stem) = token.strip_suffix("ing") {
        if acceptable_stem(stem) {
            return Some(restore_after_suffix_strip(stem));
        }
    }
    if let Some(stem) = token.strip_suffix("est") {
        if stem.len() >= MIN_STEM_LEN {
            return Some(stem.to_string());
        }
    }
    if let Some(stem) = token.strip_suffix("er") {
        if stem.len() >= MIN_STEM_LEN && !stem.ends_with('h') {
            return Some(stem.to_string());
        }
    }

    // Bare plural -s, rejecting the singular-looking endings that are not plurals.
    if let Some(stem) = token.strip_suffix('s') {
        let looks_singular =
            token.ends_with("ss") || token.ends_with("us") || token.ends_with("is");
        if !looks_singular && stem.len() >= MIN_STEM_LEN {
            return Some(stem.to_string());
        }
    }

    None
}

/// Whether a stripped `-ed` / `-ing` stem is trustworthy.
///
/// Normally a stem must reach [`MIN_STEM_LEN`]. The exception is a known silent-`e`
/// stem as short as `us` (from `used` / `using`), which the allowlist vouches for
/// while `seed` and `red` stay untouched.
fn acceptable_stem(stem: &str) -> bool {
    stem.len() >= MIN_STEM_LEN || is_silent_e_stem(stem)
}

/// Undoes the two surface changes English applies when adding `-ed` / `-ing`:
/// consonant doubling (`running` → `runn` → `run`) and silent-`e` elision
/// (`making` → `mak` → `make`).
fn restore_after_suffix_strip(stem: &str) -> String {
    let bytes = stem.as_bytes();
    let doubled = bytes.len() >= 2 && bytes[bytes.len() - 1] == bytes[bytes.len() - 2];
    // `l`, `s`, `z`, `f` are the consonants English genuinely doubles word-finally
    // (`call`, `pass`, `buzz`, `stuff`), so those stems must not be undoubled.
    let genuine_double = doubled && matches!(bytes[bytes.len() - 1], b'l' | b's' | b'z' | b'f');
    if doubled && !genuine_double {
        return stem[..stem.len() - 1].to_string();
    }
    if doubled {
        return stem.to_string();
    }

    if is_silent_e_stem(stem) {
        return format!("{stem}e");
    }

    stem.to_string()
}

/// Lemmatizes text for BM25 storage and querying.
///
/// Returns space-joined lemmas. Punctuation and stopwords are dropped; a token
/// ending in `-ing` is emitted **twice** (lemma first, then the original surface
/// form) to preserve noun/verb ambiguity, matching upstream behaviour.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::lemmatization::lemmatize_for_bm25;
///
/// assert_eq!(lemmatize_for_bm25("The team is attending meetings"), "team attend attending meeting");
/// assert_eq!(lemmatize_for_bm25("organization"), "organization");
/// ```
#[must_use]
pub fn lemmatize_for_bm25(text: &str) -> String {
    let mut lemmas: Vec<String> = Vec::new();
    for raw_token in tokenize(text) {
        let lowered = raw_token.to_lowercase();
        if is_stopword(&lowered) {
            continue;
        }
        let lemma = lemmatize_token(&raw_token);
        if lemma.is_empty() {
            continue;
        }
        let keep_surface_form = lowered.ends_with("ing") && lemma != lowered;
        lemmas.push(lemma);
        if keep_surface_form {
            lemmas.push(lowered);
        }
    }
    lemmas.join(" ")
}

/// Splits text into word tokens, treating `_` as a word character.
///
/// Underscores are kept so identifiers survive as single tokens
/// (`claude_3` stays `claude_3` and can be matched exactly). Hyphens are treated as
/// separators, so `gpt-4o` contributes `gpt` and `4o`, which keeps each fragment
/// independently searchable. Word order is preserved because
/// [`lemmatize_for_bm25`] emits lemmas in token order.
///
/// This is intentionally **not** identical to [`crate::retrieval`]'s query tokenizer,
/// which splits on every non-alphanumeric character. The retrieval tokenizer feeds
/// the pre-existing lexical scorers, whose behaviour must not change.
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plural_forms_fold_to_the_singular() {
        assert_eq!(lemmatize_token("memories"), "memory");
        assert_eq!(lemmatize_token("wines"), "wine");
        assert_eq!(lemmatize_token("boxes"), "box");
        assert_eq!(lemmatize_token("dishes"), "dish");
        assert_eq!(lemmatize_token("classes"), "class");
        assert_eq!(lemmatize_token("leaves"), "leaf");
    }

    #[test]
    fn singular_words_ending_in_s_are_not_mangled() {
        assert_eq!(lemmatize_token("class"), "class");
        assert_eq!(lemmatize_token("status"), "status");
        assert_eq!(lemmatize_token("analysis"), "analysis");
        assert_eq!(lemmatize_token("gas"), "gas");
    }

    #[test]
    fn verb_forms_fold_and_undo_their_surface_changes() {
        assert_eq!(lemmatize_token("attending"), "attend");
        assert_eq!(lemmatize_token("attended"), "attend");
        assert_eq!(lemmatize_token("running"), "run");
        assert_eq!(lemmatize_token("stopped"), "stop");
        assert_eq!(lemmatize_token("making"), "make");
        assert_eq!(lemmatize_token("used"), "use");
        assert_eq!(lemmatize_token("using"), "use");
    }

    #[test]
    fn genuine_word_final_doubles_are_not_undoubled() {
        assert_eq!(lemmatize_token("calling"), "call");
        assert_eq!(lemmatize_token("passing"), "pass");
        assert_eq!(lemmatize_token("buzzing"), "buzz");
        assert_eq!(lemmatize_token("stuffing"), "stuff");
    }

    #[test]
    fn comparatives_fold_to_the_positive_form() {
        assert_eq!(lemmatize_token("older"), "old");
        assert_eq!(lemmatize_token("oldest"), "old");
        assert_eq!(lemmatize_token("faster"), "fast");
        assert_eq!(lemmatize_token("highest"), "high");
    }

    #[test]
    fn derivational_endings_are_not_over_stemmed() {
        // Upstream calls this out by name: organization must not become organize.
        assert_eq!(lemmatize_token("organization"), "organization");
        assert_eq!(lemmatize_token("happiness"), "happiness");
        assert_eq!(lemmatize_token("management"), "management");
        assert_eq!(lemmatize_token("ability"), "ability");
        assert_eq!(lemmatize_token("biology"), "biology");
        assert_eq!(lemmatize_token("leadership"), "leadership");
    }

    #[test]
    fn irregular_forms_resolve_through_the_table() {
        assert_eq!(lemmatize_token("children"), "child");
        assert_eq!(lemmatize_token("went"), "go");
        assert_eq!(lemmatize_token("better"), "good");
        assert_eq!(lemmatize_token("were"), "be");
    }

    #[test]
    fn identifiers_and_non_alphabetic_tokens_survive_intact() {
        assert_eq!(lemmatize_token("gpt-4o"), "gpt-4o");
        assert_eq!(lemmatize_token("snake_case_id"), "snake_case_id");
        assert_eq!(lemmatize_token("2024"), "2024");
        assert_eq!(lemmatize_token("GPT4"), "gpt4");
    }

    #[test]
    fn short_tokens_and_empty_input_are_returned_unchanged() {
        assert_eq!(lemmatize_token(""), "");
        assert_eq!(lemmatize_token("is"), "be");
        assert_eq!(lemmatize_token("as"), "as");
        assert_eq!(lemmatize_token("go"), "go");
    }

    #[test]
    fn stopwords_are_dropped_from_full_text() {
        let lemmatized = lemmatize_for_bm25("The wine is on the table");
        assert_eq!(lemmatized, "wine table");
    }

    #[test]
    fn ing_forms_are_kept_alongside_their_lemma() {
        // The ambiguous surface form is emitted right after its lemma, in token order.
        let lemmatized = lemmatize_for_bm25("The team is attending meetings");
        assert_eq!(lemmatized, "team attend attending meeting");
        assert!(lemmatized
            .split_whitespace()
            .any(|token| token == "attending"));
    }

    #[test]
    fn ing_surface_form_is_not_duplicated_when_it_equals_the_lemma() {
        // "thing" lemmatizes to itself, so it must appear exactly once.
        assert_eq!(lemmatize_for_bm25("thing"), "thing");
    }

    #[test]
    fn punctuation_is_removed_and_identifiers_are_preserved() {
        assert_eq!(
            lemmatize_for_bm25("Prefers gpt-4o, not claude_3."),
            "prefer gpt 4o not claude_3"
        );
    }

    #[test]
    fn blank_input_yields_an_empty_string() {
        assert_eq!(lemmatize_for_bm25(""), "");
        assert_eq!(lemmatize_for_bm25("   ...   "), "");
        assert_eq!(lemmatize_for_bm25("the and of"), "");
    }

    #[test]
    fn token_lemmatization_is_idempotent() {
        for input in [
            "memories",
            "wine",
            "attend",
            "old",
            "organization",
            "children",
            "gpt-4o",
        ] {
            let once = lemmatize_token(input);
            let twice = lemmatize_token(&once);
            assert_eq!(once, twice, "token lemma must be stable for {input:?}");
        }
    }

    #[test]
    fn stopword_detection_is_case_insensitive_after_lowering() {
        assert!(is_stopword("the"));
        assert!(!is_stopword("table"));
    }
}
