//! Entity extraction and entity-link boosting for hybrid retrieval.
//!
//! Ports two pieces of upstream mem0's entity pipeline:
//!
//! - [`extract_entities`] — the candidate generator, from `mem0/utils/entity_extraction.py`.
//! - [`entity_boosts`] — the boost arithmetic, from `Memory._compute_entity_boosts`
//!   (`mem0/memory/main.py:1733-1813`).
//!
//! The entity **lookup** between them (embed the entity text, search the entity store)
//! is not here: it needs a vector store, so it belongs behind a port. Callers hand
//! [`entity_boosts`] the already-resolved matches.
//!
//! # Fidelity boundary
//!
//! Upstream's [`extract_entities`] is **spaCy-only**: it calls `get_nlp_full()` and
//! returns `[]` when the model is unavailable
//! (`mem0/utils/entity_extraction.py:751-758`). Every candidate class except one is
//! driven by spaCy part-of-speech tags, dependency labels, or the statistical NER
//! model:
//!
//! | Class | Upstream driver | Here |
//! | --- | --- | --- |
//! | `PROPER` from NER | `doc.ents` (`spacy_ner`) | **Absent** — needs a model |
//! | `PROPER` from capitalization | `pos_`/`tag_`/`dep_`/`is_stop` | Deterministic approximation |
//! | `TOPIC` | `doc.noun_chunks` + POS | Deterministic approximation |
//! | `IDENTIFIER` | regex over spaCy tokens | **Exact** |
//! | `QUOTED` | pure regex over the raw text | **Exact** |
//!
//! So this module is not a faithful port of upstream *with* spaCy; it is a
//! deterministic, dependency-free implementation of upstream's *intent*. Two
//! consequences are deliberate and must not be papered over:
//!
//! - Where upstream returns `[]` without spaCy, this module still extracts. That is a
//!   strict capability gain for a store that cannot ship a 500 MB model.
//! - The POS-dependent classes are reconstructed from capitalization, closed word
//!   tables, and position. Every such substitute is marked in the code and named
//!   [`SOURCE_IDENTIFIER_HEURISTIC`] / documented on the function that applies it, so
//!   a reviewer can tell exact parity from approximation without reading upstream.
//!
//! Everything that is *not* POS-dependent is ported exactly: [`clean_text`],
//! [`has_artifacts`], [`normalize_entity_text`], the `len(text) > 2` acceptance rule,
//! the deduplication ranking, the overlap rule (including its deliberate asymmetry),
//! the final positional ordering, and the boost arithmetic.
//!
//! Extraction is recall-oriented: a missed entity only loses a boost, while a wrong
//! entity injects noise. The heuristics therefore err towards fewer, higher-precision
//! candidates.

use std::collections::BTreeMap;

use crate::lemmatization::{is_stopword, lemmatize_token};
use crate::scoring::ENTITY_BOOST_WEIGHT;

/// Maximum number of query entities considered when computing boosts.
///
/// Ported from upstream's `query_entities[:8]` slice
/// (`mem0/memory/main.py:1747`). The cap applies to **query entities**, before the
/// entity store is searched — not to the matches that come back. Apply it with
/// [`select_query_entities`].
pub const MAX_QUERY_ENTITIES: usize = 8;

/// Minimum similarity for a resolved entity to contribute a boost.
///
/// Ported from upstream's `if similarity < 0.5: continue` gate
/// (`mem0/memory/main.py:1793`). The gate is inclusive.
pub const ENTITY_SIMILARITY_THRESHOLD: f64 = 0.5;

/// Similarity at or above which two entity texts are considered the same entity.
///
/// Ported from the entity merge rule (`mem0/memory/main.py:1155`).
pub const ENTITY_MERGE_SIMILARITY: f64 = 0.95;

/// Longest candidate text accepted.
///
/// Upstream rejects anything longer as a formatting artifact
/// (`mem0/utils/entity_extraction.py:387`). Measured in characters, matching Python's
/// `len()`, not bytes.
pub const MAX_ENTITY_TEXT_CHARS: usize = 100;

/// Provenance tag for the exact technical-identifier rule.
pub const SOURCE_TECHNICAL_IDENTIFIER: &str = "technical_identifier";

/// Provenance tag for the documented identifier approximation.
///
/// Emitted when a token is identifier-shaped but would not be caught by upstream's
/// dotted-path rule, which is the only identifier rule upstream can evaluate without
/// spaCy. Kept distinct from [`SOURCE_TECHNICAL_IDENTIFIER`] so callers and tests can
/// separate exact parity from approximation.
pub const SOURCE_IDENTIFIER_HEURISTIC: &str = "identifier_heuristic";

/// Provenance tag for the capitalization-based proper-name rule.
pub const SOURCE_PROPER_NAME_SPAN: &str = "proper_name_span";

/// Provenance tag for the quoted-text rule.
pub const SOURCE_QUOTED: &str = "quoted";

/// Provenance tag for the topic-phrase approximation.
pub const SOURCE_TOPIC_PHRASE: &str = "topic_phrase";

/// Words too generic to serve as the head of a topic phrase.
///
/// Ported verbatim from `_GENERIC_HEADS` (`mem0/utils/entity_extraction.py:36-96`).
const GENERIC_HEADS: &[&str] = &[
    "thing",
    "stuff",
    "way",
    "time",
    "experience",
    "situation",
    "case",
    "fact",
    "matter",
    "issue",
    "idea",
    "thought",
    "feeling",
    "place",
    "area",
    "part",
    "kind",
    "type",
    "sort",
    "lot",
    "bit",
    "day",
    "year",
    "week",
    "month",
    "moment",
    "instance",
    "example",
    "technique",
    "method",
    "approach",
    "process",
    "step",
    "tool",
    "result",
    "outcome",
    "goal",
    "task",
    "item",
    "topic",
    "scale",
    "size",
    "level",
    "degree",
    "amount",
    "number",
    "style",
    "look",
    "color",
    "colour",
    "shape",
    "form",
    "piece",
    "section",
    "side",
    "end",
    "edge",
    "surface",
    "point",
];

/// Generic role words that must not become single-token entities.
///
/// Ported verbatim from `_GENERIC_SINGLE_ENTITY_TERMS`
/// (`mem0/utils/entity_extraction.py:126-142`).
const GENERIC_SINGLE_ENTITY_TERMS: &[&str] = &[
    "user",
    "assistant",
    "agent",
    "customer",
    "client",
    "person",
    "people",
    "human",
    "memory",
    "message",
    "conversation",
    "chat",
    "session",
    "system",
    "top",
];

/// Adjectives too vague to make a topic phrase specific.
///
/// Ported verbatim from `_NON_SPECIFIC_ADJ`
/// (`mem0/utils/entity_extraction.py:164-281`).
const NON_SPECIFIC_ADJ: &[&str] = &[
    "many",
    "few",
    "several",
    "some",
    "any",
    "all",
    "most",
    "more",
    "less",
    "much",
    "little",
    "enough",
    "various",
    "numerous",
    "multiple",
    "countless",
    "great",
    "good",
    "bad",
    "nice",
    "terrible",
    "awful",
    "awesome",
    "amazing",
    "wonderful",
    "horrible",
    "excellent",
    "poor",
    "best",
    "worst",
    "fine",
    "okay",
    "new",
    "old",
    "recent",
    "past",
    "future",
    "current",
    "previous",
    "next",
    "last",
    "first",
    "latest",
    "early",
    "late",
    "former",
    "modern",
    "ancient",
    "big",
    "small",
    "large",
    "tiny",
    "huge",
    "enormous",
    "long",
    "short",
    "tall",
    "high",
    "low",
    "wide",
    "narrow",
    "thick",
    "thin",
    "deep",
    "shallow",
    "similar",
    "different",
    "same",
    "other",
    "another",
    "such",
    "certain",
    "important",
    "main",
    "major",
    "minor",
    "key",
    "primary",
    "real",
    "actual",
    "true",
    "whole",
    "entire",
    "full",
    "complete",
    "total",
    "basic",
    "simple",
    "interesting",
    "boring",
    "exciting",
    "special",
    "particular",
    "general",
    "common",
    "unique",
    "rare",
    "typical",
    "usual",
    "normal",
    "regular",
    "possible",
    "likely",
    "potential",
    "available",
    "necessary",
    "only",
    "solo",
    "individual",
    "team",
    "group",
    "joint",
    "collaborative",
    "final",
    "initial",
    "side",
];

/// Generic tail words stripped from a topic phrase.
///
/// Ported verbatim from `_GENERIC_ENDINGS`
/// (`mem0/utils/entity_extraction.py:284-317`).
const GENERIC_ENDINGS: &[&str] = &[
    "work",
    "works",
    "job",
    "jobs",
    "task",
    "tasks",
    "stuff",
    "things",
    "thing",
    "info",
    "information",
    "details",
    "data",
    "content",
    "material",
    "materials",
    "activities",
    "activity",
    "efforts",
    "effort",
    "options",
    "option",
    "choices",
    "choice",
    "results",
    "result",
    "output",
    "outputs",
    "products",
    "product",
    "items",
    "item",
];

/// Capitalized words too generic to be proper nouns.
///
/// Ported verbatim from `_GENERIC_CAPS` (`mem0/utils/entity_extraction.py:320-350`).
const GENERIC_CAPS: &[&str] = &[
    "works",
    "items",
    "things",
    "stuff",
    "resources",
    "options",
    "tips",
    "ideas",
    "steps",
    "ways",
    "methods",
    "tools",
    "features",
    "benefits",
    "examples",
    "details",
    "notes",
    "instructions",
    "guidelines",
    "recommendations",
    "suggestions",
    "overview",
    "summary",
    "conclusion",
    "introduction",
    "pros",
    "cons",
    "advantages",
    "disadvantages",
];

/// Markdown markers that begin a list item or an emphasis span.
///
/// Ported verbatim from `_FORMATTING_MARKERS`
/// (`mem0/utils/entity_extraction.py:353`).
const FORMATTING_MARKERS: &[&str] = &[
    "*", "-", "+", "\u{2022}", "\u{2013}", "\u{2014}", "#", "##", "###", "**", "__",
];

/// Words that may join a capitalized span without breaking it.
///
/// Ported verbatim from `allowed_inner_connectors`
/// (`mem0/utils/entity_extraction.py:544`). Note this list is narrower than the
/// author's intuition suggests: it has no `"de"`, `"van"`, or `"von"`.
const ALLOWED_INNER_CONNECTORS: &[&str] = &["of", "the", "for", "at", "in"];

/// Sentence-opening verb forms that cannot begin a proper-name span.
///
/// **Approximation.** Upstream decides this with spaCy's part-of-speech tagger: the
/// span builder starts only at a token spaCy tagged `PROPN`/`NNP`/`NNPS`, so a verb
/// like `Prefers` never opens a span. No model is available here, so a closed table
/// stands in. The table is deliberately a *closed list* rather than a suffix rule
/// (`…s` / `…ed` / `…ing`): a suffix rule would misfire on names such as
/// `Charles` or `Miles` and shred a real entity, whereas a missing verb only leaves
/// the verb attached to the following name.
///
/// Known limitation, accepted because it requires a co-occurrence of two rare events
/// (a sentence opening with a name that is also a verb form): a person named
/// `Will` or `Mark` at the very start of a sentence loses their first name.
const SENTENCE_OPENER_VERBS: &[&str] = &[
    "adds",
    "admires",
    "adores",
    "agrees",
    "answers",
    "asked",
    "asks",
    "avoids",
    "believes",
    "bought",
    "brings",
    "builds",
    "buys",
    "called",
    "calls",
    "came",
    "can",
    "carries",
    "chose",
    "chooses",
    "cooks",
    "costs",
    "could",
    "creates",
    "decided",
    "designed",
    "developed",
    "dislikes",
    "does",
    "drank",
    "drinks",
    "drives",
    "drove",
    "eats",
    "enjoys",
    "expects",
    "explains",
    "feels",
    "felt",
    "finished",
    "finds",
    "found",
    "gave",
    "gets",
    "gives",
    "goes",
    "got",
    "graduated",
    "had",
    "handles",
    "has",
    "hates",
    "helps",
    "hopes",
    "improves",
    "includes",
    "is",
    "joined",
    "joins",
    "keeps",
    "kept",
    "knows",
    "learned",
    "learns",
    "leaves",
    "led",
    "left",
    "likes",
    "lives",
    "loves",
    "made",
    "makes",
    "manages",
    "may",
    "mentioned",
    "mentions",
    "might",
    "moved",
    "moves",
    "must",
    "needs",
    "noted",
    "notes",
    "offers",
    "owns",
    "plans",
    "played",
    "plays",
    "prefers",
    "produces",
    "reads",
    "recommended",
    "recommends",
    "reduced",
    "refused",
    "remembers",
    "replied",
    "requires",
    "returned",
    "returns",
    "reads",
    "runs",
    "said",
    "says",
    "seems",
    "sells",
    "should",
    "sings",
    "sold",
    "speaks",
    "spoke",
    "started",
    "starts",
    "stopped",
    "studied",
    "studies",
    "suggested",
    "suggests",
    "supports",
    "takes",
    "talked",
    "talks",
    "tells",
    "thinks",
    "told",
    "took",
    "travels",
    "tried",
    "tries",
    "understands",
    "uses",
    "visited",
    "visits",
    "walks",
    "wanted",
    "wants",
    "was",
    "watches",
    "were",
    "will",
    "wishes",
    "worked",
    "works",
    "would",
    "writes",
    "wrote",
];

/// A candidate entity class.
///
/// Variant order is upstream priority order, so `Ord` and [`EntityKind::priority`]
/// agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EntityKind {
    /// A dotted technical path such as `config.load_default`.
    Identifier,
    /// A capitalized sequence, e.g. a person, place, or product.
    Proper,
    /// Text delimited by single or double quotes.
    Quoted,
    /// A multi-word lowercase phrase.
    Topic,
}

impl EntityKind {
    /// The wire value, matching upstream's `entity_type` strings exactly.
    ///
    /// Upstream reports these uppercase (`"PROPER"`, `"QUOTED"`, `"TOPIC"`,
    /// `"IDENTIFIER"`). The value is carried through to the entity store, whose
    /// `entity_type` is an open string, so no mapping is needed at the boundary.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_retrieval::entities::EntityKind;
    ///
    /// assert_eq!(EntityKind::Proper.code(), "PROPER");
    /// assert_eq!(EntityKind::Identifier.code(), "IDENTIFIER");
    /// ```
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Identifier => "IDENTIFIER",
            Self::Proper => "PROPER",
            Self::Quoted => "QUOTED",
            Self::Topic => "TOPIC",
        }
    }

    /// Upstream precedence; lower wins when two candidates are identical.
    ///
    /// Ported from the `priority` field passed to `_add_candidate`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_retrieval::entities::EntityKind;
    ///
    /// assert!(EntityKind::Identifier.priority() < EntityKind::Proper.priority());
    /// ```
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::Identifier => 1,
            Self::Proper => 2,
            Self::Quoted => 3,
            Self::Topic => 4,
        }
    }

    /// Upstream confidence for this class.
    ///
    /// Ported from the `confidence` field passed to `_add_candidate`. The
    /// spaCy-NER class (`0.95`, priority `0`) has no counterpart here.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_retrieval::entities::EntityKind;
    ///
    /// assert!((EntityKind::Identifier.confidence() - 0.90).abs() < 1e-12);
    /// assert!((EntityKind::Topic.confidence() - 0.45).abs() < 1e-12);
    /// ```
    #[must_use]
    pub const fn confidence(self) -> f64 {
        match self {
            Self::Identifier => 0.90,
            Self::Proper => 0.80,
            Self::Quoted => 0.75,
            Self::Topic => 0.45,
        }
    }
}

/// A character span in the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntitySpan {
    /// Inclusive character offset where the candidate starts.
    pub start: usize,
    /// Exclusive character offset where the candidate ends.
    pub end: usize,
}

impl EntitySpan {
    /// Length in characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_retrieval::entities::EntitySpan;
    ///
    /// assert_eq!(EntitySpan { start: 2, end: 9 }.len(), 7);
    /// ```
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether the span covers no characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_retrieval::entities::EntitySpan;
    ///
    /// assert!(EntitySpan { start: 4, end: 4 }.is_empty());
    /// assert!(!EntitySpan { start: 4, end: 5 }.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// One extracted entity candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedEntity {
    /// Candidate class.
    pub kind: EntityKind,
    /// Cleaned entity text.
    pub text: String,
    /// Character span, or `None` for a candidate upstream records without one.
    ///
    /// Upstream stores `-1, -1` for quoted candidates
    /// (`mem0/utils/entity_extraction.py:584`), which makes them exempt from overlap
    /// resolution and puts them last in the output. `None` carries the same semantics.
    pub span: Option<EntitySpan>,
    /// Class confidence, from [`EntityKind::confidence`].
    pub confidence: f64,
    /// Class priority, from [`EntityKind::priority`].
    pub priority: u8,
    /// Which rule produced this candidate; one of the `SOURCE_*` constants.
    ///
    /// Upstream's `_EntityCandidate.source`, used here to separate exact parity from
    /// described approximation.
    pub source: &'static str,
}

/// An entity the caller resolved against the entity store, with its linked memories.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkedEntityMatch {
    /// Similarity between the query entity and the stored entity, in `[0, 1]`.
    pub similarity: f64,
    /// Memories the stored entity links to, from its `linked_memory_ids` payload.
    pub memory_ids: Vec<String>,
}

/// Extracts entity candidates from `text`, in upstream's output order.
///
/// Candidates are generated, deduplicated by [`normalize_entity_text`] (keeping the
/// strictly better `(priority, -confidence)` pair, first-wins on ties), reduced by
/// overlap, then ordered by position. Quoted candidates have no span, so they never
/// collide and always sort last.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::{EntityKind, extract_entities};
///
/// let entities = extract_entities("Ada Lovelace preferred \"Analytical Engine\" \
/// over config.load_default.");
/// let texts = entities.iter().map(|entity| entity.text.as_str()).collect::<Vec<_>>();
/// assert!(texts.contains(&"Ada Lovelace"));
/// assert!(texts.contains(&"Analytical Engine"));
/// assert!(texts.contains(&"config.load_default"));
/// assert!(entities.iter().any(|entity| entity.kind == EntityKind::Identifier));
/// ```
#[must_use]
pub fn extract_entities(text: &str) -> Vec<ExtractedEntity> {
    let characters: Vec<char> = text.chars().collect();
    let tokens = tokenize(&characters);
    let mut candidates: Vec<ExtractedEntity> = Vec::new();

    // Upstream generation order (`_extract_entities_from_doc`): NER, identifier,
    // proper name, quoted, topic. NER is absent, so identifier leads. Order matters
    // because deduplication keeps the first candidate on a tie.
    collect_technical_identifiers(&tokens, &mut candidates);
    collect_proper_names(&tokens, &characters, &mut candidates);
    collect_quoted(text, &mut candidates);
    collect_topics(&tokens, &mut candidates);

    resolve_candidates(candidates)
}

/// Extracts entity texts only, in upstream's output order.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::extract_entity_texts;
///
/// let texts = extract_entity_texts("Ada Lovelace liked \"Notes\".");
/// assert!(texts.contains(&"Ada Lovelace".to_string()));
/// assert!(texts.contains(&"Notes".to_string()));
/// ```
#[must_use]
pub fn extract_entity_texts(text: &str) -> Vec<String> {
    extract_entities(text)
        .into_iter()
        .map(|entity| entity.text)
        .collect()
}

/// Reduces extracted entities to the query set used for entity boosting.
///
/// Ports upstream's pre-search reduction (`mem0/memory/main.py:1744-1754`): take the
/// first [`MAX_QUERY_ENTITIES`] entities, drop blanks, and deduplicate by
/// [`normalize_entity_text`] preserving first-seen order.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::{extract_entities, select_query_entities};
///
/// let entities = extract_entities("Ada Lovelace met Ada Lovelace in London.");
/// let selected = select_query_entities(&entities);
/// assert!(selected.len() <= 8);
/// let texts = selected.iter().map(|entity| entity.text.as_str()).collect::<Vec<_>>();
/// // Normalized duplicates collapse.
/// assert_eq!(texts.iter().filter(|text| **text == "Ada Lovelace").count(), 1);
/// ```
#[must_use]
pub fn select_query_entities(entities: &[ExtractedEntity]) -> Vec<ExtractedEntity> {
    let mut seen: Vec<String> = Vec::new();
    let mut selected = Vec::new();

    for entity in entities.iter().take(MAX_QUERY_ENTITIES) {
        let key = normalize_entity_text(&entity.text);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        selected.push(entity.clone());
    }

    selected
}

/// Lowercases and collapses whitespace, matching upstream `_normalize_entity_text`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::normalize_entity_text;
///
/// assert_eq!(normalize_entity_text("  Ada   Lovelace "), "ada lovelace");
/// ```
#[must_use]
pub fn normalize_entity_text(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes the per-memory entity boost.
///
/// Ports `Memory._compute_entity_boosts` (`mem0/memory/main.py:1791-1808`). For every
/// match whose `similarity` clears [`ENTITY_SIMILARITY_THRESHOLD`] and whose linked
/// memory list is non-empty, each linked memory receives
///
/// ```text
/// similarity * ENTITY_BOOST_WEIGHT * 1 / (1 + 0.001 * (n - 1)^2)
/// ```
///
/// where `n` is how many memories the entity links to. The denominator makes a hub
/// entity that links to hundreds of memories contribute far less per memory than a
/// specific one — without it, one common entity would dominate the ranking.
///
/// A memory linked by several matching entities takes the **maximum** contribution,
/// not the sum, exactly as upstream's `max(...)` does.
///
/// No cap is applied to `matches`: upstream caps *query entities* at
/// [`MAX_QUERY_ENTITIES`] before searching, so the cap belongs on the query side. Use
/// [`select_query_entities`] for that.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::{LinkedEntityMatch, entity_boosts};
///
/// let matches = [
///     LinkedEntityMatch { similarity: 0.9, memory_ids: vec!["m1".into()] },
///     LinkedEntityMatch { similarity: 0.4, memory_ids: vec!["m2".into()] },
/// ];
/// let boosts = entity_boosts(&matches);
/// assert!(boosts.contains_key("m1"));
/// assert!(!boosts.contains_key("m2"), "below the similarity gate");
/// ```
#[must_use]
pub fn entity_boosts(matches: &[LinkedEntityMatch]) -> BTreeMap<String, f64> {
    let mut boosts: BTreeMap<String, f64> = BTreeMap::new();

    for matched in matches {
        if !matched.similarity.is_finite()
            || matched.similarity < ENTITY_SIMILARITY_THRESHOLD
            || matched.memory_ids.is_empty()
        {
            continue;
        }
        let weight = memory_count_weight(matched.memory_ids.len());
        let contribution = matched.similarity * ENTITY_BOOST_WEIGHT * weight;
        if !contribution.is_finite() || contribution <= 0.0 {
            continue;
        }

        for memory_id in &matched.memory_ids {
            // Upstream skips falsy ids (`if memory_id:`).
            if memory_id.is_empty() {
                continue;
            }
            boosts
                .entry(memory_id.clone())
                .and_modify(|existing| {
                    if contribution > *existing {
                        *existing = contribution;
                    }
                })
                .or_insert(contribution);
        }
    }

    boosts
}

/// Down-weighting factor for entities that link to many memories.
///
/// Upstream formula: `1 / (1 + 0.001 * (n - 1)^2)` (`mem0/memory/main.py:1801-1802`),
/// with `n = max(len(linked_memory_ids), 1)`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::memory_count_weight;
///
/// assert_eq!(memory_count_weight(0), 1.0);
/// assert_eq!(memory_count_weight(1), 1.0);
/// assert!(memory_count_weight(100) < memory_count_weight(2));
/// ```
#[must_use]
pub fn memory_count_weight(linked_memory_count: usize) -> f64 {
    let linked = linked_memory_count.max(1);
    let excess = (linked - 1) as f64;
    1.0 / (1.0 + 0.001 * excess * excess)
}

/// Whether `text` matches upstream's `_looks_like_technical_identifier`.
///
/// Upstream regex: `[A-Za-z_][\w-]*(?:\.[A-Za-z_][\w-]*)+`
/// (`mem0/utils/entity_extraction.py:404-405`). At least one dot is **required**, so
/// `gpt-4o` and `snake_case_id` are *not* identifiers by this rule — upstream reaches
/// them only through spaCy. See [`SOURCE_IDENTIFIER_HEURISTIC`] for the labelled
/// extension that recovers them here.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::looks_like_technical_identifier;
///
/// assert!(looks_like_technical_identifier("config.load_default"));
/// assert!(looks_like_technical_identifier("a.b.c"));
/// assert!(!looks_like_technical_identifier("gpt-4o"), "no dot");
/// assert!(!looks_like_technical_identifier("1.2.3"), "starts with a digit");
/// ```
#[must_use]
pub fn looks_like_technical_identifier(text: &str) -> bool {
    let segments = text.split('.').collect::<Vec<_>>();
    if segments.len() < 2 {
        return false;
    }
    segments.iter().all(|segment| {
        let mut characters = segment.chars();
        match characters.next() {
            Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
            _ => return false,
        }
        characters
            .all(|character| character.is_alphanumeric() || character == '_' || character == '-')
    })
}

/// Cleans a raw candidate span the way upstream `_clean_text` does.
///
/// Applied in upstream's exact order (`mem0/utils/entity_extraction.py:393-397`):
/// strip surrounding asterisk emphasis, then trailing colons, then a leading list
/// number, then collapse whitespace.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::clean_text;
///
/// assert_eq!(clean_text("**Ada Lovelace**"), "Ada Lovelace");
/// assert_eq!(clean_text("Topic:"), "Topic");
/// assert_eq!(clean_text("3. Ada Lovelace"), "Ada Lovelace");
/// assert_eq!(clean_text("  Ada   Lovelace  "), "Ada Lovelace");
/// ```
#[must_use]
pub fn clean_text(raw: &str) -> String {
    let mut text = raw.trim();

    // `re.sub(r"^\*+\s*|\s*\*+$", "", ...)`
    text = text.trim_start_matches('*');
    text = text.trim_start();
    text = text.trim_end_matches('*');
    text = text.trim_end();

    // `re.sub(r"\s*:+$", "", ...)`
    text = text.trim_end_matches(':');
    text = text.trim_end();

    // `re.sub(r"^\d+\s*\.\s*", "", ...)` — digits, optional space, one dot, optional space.
    let digits_end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    if digits_end > 0 {
        let after_digits = text[digits_end..].trim_start();
        if let Some(after_dot) = after_digits.strip_prefix('.') {
            text = after_dot.trim_start();
        }
    }

    collapse_whitespace(text)
}

/// Whether `text` looks like a formatting artifact rather than an entity.
///
/// Ports `_has_artifacts` (`mem0/utils/entity_extraction.py:380-390`) including its
/// asterisk-position regex and its character-count limit.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_retrieval::entities::has_artifacts;
///
/// assert!(has_artifacts("- bullet item"));
/// assert!(has_artifacts("bold **text**"));
/// assert!(!has_artifacts("Ada Lovelace"));
/// ```
#[must_use]
pub fn has_artifacts(text: &str) -> bool {
    if text.contains("**")
        || text.contains("__")
        || text.contains(":*")
        || text.contains("  ")
        || text.contains('\n')
        || text.contains('\t')
    {
        return true;
    }
    if text.chars().count() > MAX_ENTITY_TEXT_CHARS {
        return true;
    }
    if text.starts_with(['\u{2022}', '-', '+', '\u{2013}', '\u{2014}', '#', '*']) {
        return true;
    }
    has_star_artifact(text)
}

/// Upstream's `\s\*\s|\s\*$|^\*\s` asterisk-position check.
fn has_star_artifact(text: &str) -> bool {
    let characters = text.chars().collect::<Vec<_>>();
    let spaced_star = characters
        .windows(3)
        .any(|window| window[0].is_whitespace() && window[1] == '*' && window[2].is_whitespace());
    if spaced_star {
        return true;
    }
    let length = characters.len();
    let trailing =
        length >= 2 && characters[length - 2].is_whitespace() && characters[length - 1] == '*';
    let leading = length >= 2 && characters[0] == '*' && characters[1].is_whitespace();
    trailing || leading
}

/// Collapses runs of whitespace into single spaces.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A tokenized word with its character span.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    start: usize,
    end: usize,
}

/// Splits text into word tokens, keeping `.`, `_`, and `-` inside a token.
///
/// **Approximation.** spaCy's tokenizer is not available; punctuation-splitting is
/// emulated by trimming those three characters off the ends of each token, which is
/// what the identifier rule depends on (a sentence-final period must not become part
/// of the token, or `config.load_default.` would fail the dotted-path check).
fn tokenize(characters: &[char]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;

    for (index, character) in characters.iter().enumerate() {
        let is_word = character.is_alphanumeric() || matches!(character, '_' | '-' | '.');
        if is_word {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(begin) = start.take() {
            push_token(&mut tokens, characters, begin, index);
        }
    }
    if let Some(begin) = start {
        push_token(&mut tokens, characters, begin, characters.len());
    }

    tokens
}

/// Records a token, trimming connector punctuation from both ends.
fn push_token(tokens: &mut Vec<Token>, characters: &[char], begin: usize, end: usize) {
    let raw = &characters[begin..end];
    let is_trimmed = |character: char| matches!(character, '.' | '-' | '_');

    let leading = raw.iter().take_while(|c| is_trimmed(**c)).count();
    let trailing = raw.iter().rev().take_while(|c| is_trimmed(**c)).count();
    if leading + trailing >= raw.len() {
        return;
    }
    let text = raw[leading..raw.len() - trailing]
        .iter()
        .collect::<String>();
    tokens.push(Token {
        text,
        start: begin + leading,
        end: end - trailing,
    });
}

/// Whether `offset` sits at the start of the text or after sentence punctuation.
///
/// Mirrors `_is_sentence_start` (`mem0/utils/entity_extraction.py:356-364`). spaCy's
/// `is_sent_start` is unavailable, so the raw characters decide.
fn is_sentence_start(characters: &[char], offset: usize) -> bool {
    let mut cursor = offset;
    while cursor > 0 {
        cursor -= 1;
        let previous = characters[cursor];
        if previous == '\n' {
            return true;
        }
        if previous.is_whitespace() {
            continue;
        }
        return matches!(
            previous,
            '.' | '!' | '?' | ':' | '\u{2022}' | '-' | '+' | '#' | '*' | '\u{2013}' | '\u{2014}'
        );
    }
    true
}

/// Whether a token carries digits, an internal capital, or underscores.
///
/// Ports `_has_internal_cap_or_digit` (`mem0/utils/entity_extraction.py:408-409`) and
/// extends it with the underscore test, which is what makes `snake_case_id` visible
/// without a POS tagger.
fn has_internal_cap_or_digit(text: &str) -> bool {
    text.chars().any(|character| character.is_numeric())
        || text
            .chars()
            .skip(1)
            .any(|character| character.is_uppercase())
        || text.contains('_')
}

/// Words that must never become a single-token entity.
///
/// Ports `_is_bad_single_name_token` (`mem0/utils/entity_extraction.py:467-469`).
fn is_bad_single_name_token(text: &str) -> bool {
    let lower = text.to_lowercase();
    GENERIC_SINGLE_ENTITY_TERMS.contains(&lower.as_str())
        || GENERIC_CAPS.contains(&lower.as_str())
        || is_stopword(&lower)
}

/// Whether a token can open or extend a capitalized proper-name span.
///
/// **Approximation** of `_is_name_like_token`
/// (`mem0/utils/entity_extraction.py:443-464`). Upstream's decision is dominated by
/// `pos_`/`tag_`/`dep_`; without them this reduces to capitalization plus the closed
/// generic-word tables, and the "not at a sentence start" clause moves to the span
/// level in [`collect_proper_names`], because upstream applies it to single-token
/// spans only.
fn is_name_like_token(text: &str, at_sentence_start: bool) -> bool {
    if text.is_empty() || FORMATTING_MARKERS.contains(&text) {
        return false;
    }
    if !text.chars().next().is_some_and(char::is_uppercase) {
        return false;
    }
    if !text.chars().any(char::is_alphabetic) {
        return false;
    }
    if is_bad_single_name_token(text) {
        return false;
    }
    if at_sentence_start && SENTENCE_OPENER_VERBS.contains(&text.to_lowercase().as_str()) {
        return false;
    }
    true
}

/// Whether a token is "topic eligible": lowercase, non-stopword, at least 3 chars.
fn is_topic_token(text: &str) -> bool {
    text.chars().count() >= 3
        && text.chars().all(|character| !character.is_uppercase())
        && text.chars().any(char::is_alphabetic)
        && !is_stopword(&text.to_lowercase())
}

/// Lemma of a token, lowercased.
fn lemma(text: &str) -> String {
    lemmatize_token(&text.to_lowercase())
}

/// Collects dotted technical identifiers, plus the labelled heuristic extension.
fn collect_technical_identifiers(tokens: &[Token], candidates: &mut Vec<ExtractedEntity>) {
    for token in tokens {
        if looks_like_technical_identifier(&token.text) {
            push_candidate(
                candidates,
                EntityKind::Identifier,
                &token.text,
                Some(EntitySpan {
                    start: token.start,
                    end: token.end,
                }),
                SOURCE_TECHNICAL_IDENTIFIER,
            );
        } else if looks_like_heuristic_identifier(&token.text) {
            push_candidate(
                candidates,
                EntityKind::Identifier,
                &token.text,
                Some(EntitySpan {
                    start: token.start,
                    end: token.end,
                }),
                SOURCE_IDENTIFIER_HEURISTIC,
            );
        }
    }
}

/// Whether a token is identifier-shaped but not by upstream's dotted-path rule.
///
/// **Approximation.** Upstream finds `gpt-4o` and `snake_case_id` only via spaCy NER.
/// This rule recovers them from the shape of the text: a digit, an internal capital,
/// or an underscore. A bare hyphen is deliberately not enough, because English
/// hyphenated words (`well-known`, `state-of-the-art`) would then be read as
/// identifiers.
fn looks_like_heuristic_identifier(text: &str) -> bool {
    text.chars().count() >= 3
        && !text.contains('.')
        && (text.chars().any(|character| character.is_numeric())
            || text
                .chars()
                .skip(1)
                .any(|character| character.is_uppercase())
            || text.contains('_'))
}

/// Collects capitalized proper-name spans.
///
/// Follows upstream's span builder (`_add_proper_name_candidates`,
/// `mem0/utils/entity_extraction.py:543-578`): extend while the next token is
/// name-like, or while the next token is a permitted inner connector followed by a
/// name-like token. Emits when the span holds more than one name token, or when its
/// single token is not a generic word.
///
/// The POS clause upstream applies to single tokens is emulated here at span level: a
/// single-token span is dropped when it opens a sentence, because a capitalized word
/// in that position is more often sentence case than a name.
fn collect_proper_names(
    tokens: &[Token],
    characters: &[char],
    candidates: &mut Vec<ExtractedEntity>,
) {
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        let at_sentence_start = is_sentence_start(characters, token.start);
        if !is_name_like_token(&token.text, at_sentence_start) {
            index += 1;
            continue;
        }

        let mut span_tokens = vec![index];
        let mut cursor = index + 1;
        while cursor < tokens.len() {
            let current = &tokens[cursor];
            let current_at_start = is_sentence_start(characters, current.start);
            if is_name_like_token(&current.text, current_at_start) {
                span_tokens.push(cursor);
                cursor += 1;
                continue;
            }
            let is_connector =
                ALLOWED_INNER_CONNECTORS.contains(&current.text.to_lowercase().as_str());
            if is_connector && cursor + 1 < tokens.len() {
                let next = &tokens[cursor + 1];
                let next_at_start = is_sentence_start(characters, next.start);
                if is_name_like_token(&next.text, next_at_start) {
                    span_tokens.push(cursor);
                    span_tokens.push(cursor + 1);
                    cursor += 2;
                    continue;
                }
            }
            break;
        }

        let name_token_count = span_tokens
            .iter()
            .filter(|&&position| {
                let candidate = &tokens[position];
                is_name_like_token(
                    &candidate.text,
                    is_sentence_start(characters, candidate.start),
                )
            })
            .count();
        let first = &tokens[span_tokens[0]];
        let accepts_span = name_token_count > 1
            || !(is_bad_single_name_token(&first.text)
                || (is_sentence_start(characters, first.start)
                    && !has_internal_cap_or_digit(&first.text)));

        if accepts_span {
            let text = span_tokens
                .iter()
                .map(|&position| tokens[position].text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let start = tokens[span_tokens[0]].start;
            let end = tokens[*span_tokens.last().expect("span is non-empty")].end;
            push_candidate(
                candidates,
                EntityKind::Proper,
                &text,
                Some(EntitySpan { start, end }),
                SOURCE_PROPER_NAME_SPAN,
            );
        }

        index = cursor.max(index + 1);
    }
}

/// Collects single- and double-quoted spans, exactly as upstream does.
///
/// Ports `_add_quoted_candidates` (`mem0/utils/entity_extraction.py:581-587`),
/// including the fact that quoted candidates are recorded **without a span**, which
/// exempts them from overlap resolution. This is the one upstream class that is not
/// POS-dependent, so the port is exact.
fn collect_quoted(text: &str, candidates: &mut Vec<ExtractedEntity>) {
    let characters = text.chars().collect::<Vec<_>>();

    for (quote, allow_prefix) in [('"', false), ('\'', true)] {
        let mut index = 0;
        while index < characters.len() {
            if characters[index] != quote {
                index += 1;
                continue;
            }
            // A single quote must be preceded by a boundary and closed by one, so
            // contractions such as `don't` are not read as an quoted span.
            if allow_prefix && index > 0 && !is_quote_boundary(characters[index - 1]) {
                index += 1;
                continue;
            }
            let Some(close) = characters[index + 1..]
                .iter()
                .position(|character| *character == quote)
                .map(|offset| index + 1 + offset)
            else {
                break;
            };
            let inner = characters[index + 1..close].iter().collect::<String>();
            if allow_prefix
                && close + 1 < characters.len()
                && !is_quote_boundary(characters[close + 1])
            {
                index = close + 1;
                continue;
            }
            if inner.trim().chars().count() > 2 {
                push_candidate(candidates, EntityKind::Quoted, &inner, None, SOURCE_QUOTED);
            }
            index = close + 1;
        }
    }
}

/// Characters that may sit just outside a single-quoted span.
///
/// Ported from the lookaround in upstream's single-quote pattern:
/// `(?:^|[\s\(\[{,;])'([^']+)'(?=[\s\.,;:!?\)\]]|$)`.
fn is_quote_boundary(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '(' | '[' | '{' | ',' | ';' | '.' | ':' | '!' | '?' | ')' | ']'
        )
}

/// Collects lowercase multi-word topic phrases.
///
/// **Approximation** of `_add_topic_phrase_candidates`
/// (`mem0/utils/entity_extraction.py:590-696`), which walks `doc.noun_chunks` and
/// filters by POS and dependency label. The only boundary signals available here are
/// capitalization and stopwords: a topic run is a maximal sequence of lowercase
/// non-stopword tokens, so an uppercase word (which belongs to the proper-name
/// channel) or a stopword ends it.
///
/// The practical difference from a noun chunk is at the **tail**: an adverb or a
/// second clause element that a parser would leave outside the chunk still joins the
/// phrase here, because separating it needs POS. `"prefers concise answers always"`
/// therefore yields `"concise answers always"` where upstream yields
/// `"concise answers"`. A longer phrase only weakens the entity-store lookup, so the
/// cost of this divergence is recall on the boost, never a wrong boost.
///
/// Upstream's POS-independent filters are still applied exactly: the generic-head
/// exclusion (unless a specific modifier or a compound is present), the
/// non-specific-adjective removal, the generic-ending strip, and the
/// `len > 3 && " " in phrase` acceptance test.
///
/// **One branch is not ported.** Upstream first checks whether a phrase contains a
/// *circumstantial* modifier (`_CIRCUMSTANTIAL_MODS`: `solo`, `team`, `joint`, …) and,
/// when it does, emits the bare head instead of the phrase
/// (`mem0/utils/entity_extraction.py:644-658`). That test runs over tokens tagged
/// `dep_ == "compound"`, so it is not reproducible here. The consequence is
/// conservative in the wrong direction for that narrow case: `"team planning session"`
/// yields the phrase rather than the head `"session"`. The word table is not carried
/// as dead data; this note is the record.
fn collect_topics(tokens: &[Token], candidates: &mut Vec<ExtractedEntity>) {
    let mut run: Vec<&Token> = Vec::new();

    let flush = |run: &mut Vec<&Token>, candidates: &mut Vec<ExtractedEntity>| {
        if !run.is_empty() {
            emit_topic(run, candidates);
            run.clear();
        }
    };

    for token in tokens {
        if is_topic_token(&token.text) {
            run.push(token);
        } else {
            flush(&mut run, candidates);
        }
    }
    flush(&mut run, candidates);
}

/// Emits a topic phrase from a run of lowercase content tokens.
fn emit_topic(run: &[&Token], candidates: &mut Vec<ExtractedEntity>) {
    let mut words = run.to_vec();
    if words.len() < 2 {
        return;
    }

    // A leading sentence-opening verb is not part of the noun phrase.
    if SENTENCE_OPENER_VERBS.contains(&words[0].text.to_lowercase().as_str()) {
        words.remove(0);
    }
    if words.len() < 2 {
        return;
    }

    let head_generic = GENERIC_HEADS.contains(&lemma(&words[words.len() - 1].text).as_str());
    let has_specific_modifier = words[..words.len() - 1]
        .iter()
        .any(|word| !NON_SPECIFIC_ADJ.contains(&lemma(&word.text).as_str()));
    let has_compound = words.len() > 2;
    if head_generic && !has_specific_modifier && !has_compound {
        return;
    }

    let mut filtered = words
        .iter()
        .filter(|word| !NON_SPECIFIC_ADJ.contains(&lemma(&word.text).as_str()))
        .copied()
        .collect::<Vec<_>>();
    if filtered.len() > 2
        && GENERIC_ENDINGS.contains(&lemma(&filtered[filtered.len() - 1].text).as_str())
    {
        filtered.pop();
    }
    if filtered.is_empty() {
        return;
    }

    let text = filtered
        .iter()
        .map(|word| word.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if text.chars().count() > 3 && text.contains(' ') {
        push_candidate(
            candidates,
            EntityKind::Topic,
            &text,
            Some(EntitySpan {
                start: filtered[0].start,
                end: filtered[filtered.len() - 1].end,
            }),
            SOURCE_TOPIC_PHRASE,
        );
    }
}

/// Pushes a candidate when its cleaned text is usable.
///
/// Ports `_add_candidate` (`mem0/utils/entity_extraction.py:472-495`): clean, then
/// reject blanks, anything of 3 characters or fewer, and formatting artifacts.
fn push_candidate(
    candidates: &mut Vec<ExtractedEntity>,
    kind: EntityKind,
    raw: &str,
    span: Option<EntitySpan>,
    source: &'static str,
) {
    let text = clean_text(raw);
    if text.is_empty() || text.chars().count() <= 2 || has_artifacts(&text) {
        return;
    }
    candidates.push(ExtractedEntity {
        kind,
        text,
        span,
        confidence: kind.confidence(),
        priority: kind.priority(),
        source,
    });
}

/// Deduplicates by normalized text, keeping the strongest variant.
///
/// Ports the first half of `_resolve_candidates`
/// (`mem0/utils/entity_extraction.py:706-711`): replace only on a strictly better
/// `(priority, -confidence)` pair, so the first candidate wins a tie. Iteration order
/// is preserved, because the later stable sorts depend on it.
fn deduplicate(candidates: Vec<ExtractedEntity>) -> Vec<ExtractedEntity> {
    let mut positions: BTreeMap<String, usize> = BTreeMap::new();
    let mut deduped: Vec<ExtractedEntity> = Vec::new();

    for candidate in candidates {
        let key = normalize_entity_text(&candidate.text);
        match positions.get(&key) {
            Some(&index) => {
                let existing = &deduped[index];
                let existing_rank = (existing.priority, -existing.confidence);
                let candidate_rank = (candidate.priority, -candidate.confidence);
                if candidate_rank < existing_rank {
                    deduped[index] = candidate;
                }
            }
            None => {
                positions.insert(key, deduped.len());
                deduped.push(candidate);
            }
        }
    }

    deduped
}

/// Start key for ordering; span-less candidates sort last.
fn start_key(candidate: &ExtractedEntity) -> usize {
    candidate.span.map_or(usize::MAX, |span| span.start)
}

/// End key for ordering; span-less candidates use `0`, matching upstream's `-1`.
fn end_key(candidate: &ExtractedEntity) -> usize {
    candidate.span.map_or(0, |span| span.end)
}

/// Whether two candidates overlap; a span-less candidate never overlaps.
fn spans_overlap(left: &ExtractedEntity, right: &ExtractedEntity) -> bool {
    match (left.span, right.span) {
        (Some(left), Some(right)) => left.start < right.end && right.start < left.end,
        _ => false,
    }
}

/// Deduplicates, resolves overlaps, and orders candidates positionally.
///
/// Ports `_resolve_candidates` (`mem0/utils/entity_extraction.py:705-728`) in full,
/// including the two details a re-implementation usually gets wrong:
///
/// - The overlap exemption is **asymmetric**: only a multi-word `TOPIC` may coexist
///   with an already-accepted `PROPER`, not the other way round.
/// - The accepted set is re-sorted **by position**, not by precedence, before being
///   returned. Precedence only decides who wins an overlap.
///
/// # Note on the asymmetric exemption
///
/// The branch is preserved from upstream but is **unreachable with the current
/// generators**: a topic run admits only lowercase non-stopword tokens
/// ([`is_topic_token`]), while a proper-name span always begins with a capitalized
/// token, and both of upstream's permitted inner connectors (`of`, `the`, `for`,
/// `at`, `in`) are stopwords. So the two classes cannot overlap here. Upstream reaches
/// the branch because `doc.noun_chunks` may mix case inside one chunk
/// (`"Ada Lovelace project"`). It is kept rather than deleted so that adding a
/// POS-aware generator later does not silently resurrect a lost upstream rule; the
/// `topic_and_proper_spans_never_overlap` test pins the invariant that currently
/// makes it dead, so the moment that invariant breaks, the branch goes live and the
/// test says so.
fn resolve_candidates(candidates: Vec<ExtractedEntity>) -> Vec<ExtractedEntity> {
    let mut ordered = deduplicate(candidates);
    ordered.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| {
                (-left.confidence)
                    .partial_cmp(&(-right.confidence))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                start_key(right)
                    .saturating_sub(end_key(right))
                    .cmp(&start_key(left).saturating_sub(end_key(left)))
            })
            .then_with(|| start_key(left).cmp(&start_key(right)))
    });

    let mut accepted: Vec<ExtractedEntity> = Vec::new();
    for candidate in ordered {
        let conflicts = accepted.iter().any(|existing| {
            spans_overlap(&candidate, existing)
                && !(candidate.kind == EntityKind::Topic
                    && candidate.text.contains(' ')
                    && existing.kind == EntityKind::Proper)
        });
        if !conflicts {
            accepted.push(candidate);
        }
    }

    accepted.sort_by(|left, right| {
        start_key(left)
            .cmp(&start_key(right))
            .then_with(|| end_key(left).cmp(&end_key(right)))
            .then_with(|| left.priority.cmp(&right.priority))
    });
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<String> {
        extract_entity_texts(text)
    }

    fn by_source(text: &str, source: &str) -> Vec<String> {
        extract_entities(text)
            .into_iter()
            .filter(|entity| entity.source == source)
            .map(|entity| entity.text)
            .collect()
    }

    #[test]
    fn dotted_paths_are_identifiers_and_match_upstreams_regex() {
        assert!(looks_like_technical_identifier("config.load_default"));
        assert!(looks_like_technical_identifier("a.b.c"));
        assert!(looks_like_technical_identifier("_private.value"));
        assert!(!looks_like_technical_identifier("gpt-4o"));
        assert!(!looks_like_technical_identifier("snake_case_id"));
        assert!(!looks_like_technical_identifier("1.2.3"));
        assert!(!looks_like_technical_identifier("trailing."));
        assert!(!looks_like_technical_identifier(".leading"));

        let extracted = by_source(
            "Deployed config.load_default yesterday.",
            SOURCE_TECHNICAL_IDENTIFIER,
        );
        assert_eq!(extracted, vec!["config.load_default"]);
    }

    #[test]
    fn shape_shaped_identifiers_come_from_the_labelled_extension_only() {
        let extracted = by_source(
            "Uses gpt-4o and snake_case_id for tracking.",
            SOURCE_IDENTIFIER_HEURISTIC,
        );
        assert!(extracted.contains(&"gpt-4o".to_string()));
        assert!(extracted.contains(&"snake_case_id".to_string()));
        // Exact upstream rule must not have claimed them.
        assert!(by_source(
            "Uses gpt-4o and snake_case_id.",
            SOURCE_TECHNICAL_IDENTIFIER
        )
        .is_empty());
    }

    #[test]
    fn hyphenated_english_words_are_not_identifiers() {
        assert!(!looks_like_heuristic_identifier("well-known"));
        assert!(!looks_like_heuristic_identifier("state-of-the-art"));
        assert!(!looks_like_heuristic_identifier("e-mail"));
        assert!(looks_like_heuristic_identifier("gpt-4o"));
        assert!(looks_like_heuristic_identifier("GPT4"));
    }

    #[test]
    fn quoted_spans_are_extracted_with_both_quote_styles_and_without_a_span() {
        let entities = extract_entities("Preferred \"Analytical Engine\" and 'Notes' today.");
        let quoted = entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Quoted)
            .collect::<Vec<_>>();
        let texts = quoted
            .iter()
            .map(|entity| entity.text.as_str())
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Analytical Engine"));
        assert!(texts.contains(&"Notes"));
        assert!(quoted.iter().all(|entity| entity.span.is_none()));
    }

    #[test]
    fn contractions_are_not_read_as_single_quoted_spans() {
        let extracted = texts("She doesn't like that.");
        assert!(!extracted.iter().any(|entity| entity == "t like that"));
    }

    #[test]
    fn capitalized_runs_are_extracted_as_proper_nouns() {
        let extracted = texts("Ada Lovelace met Charles Babbage in London.");
        assert!(
            extracted.contains(&"Ada Lovelace".to_string()),
            "got {extracted:?}"
        );
        // `in` is in upstream's `allowed_inner_connectors`, so the span builder runs
        // straight through it and swallows the following name. This is upstream's
        // behaviour, not an artefact of the approximation — pinned so that a future
        // "cleanup" cannot silently change it.
        assert!(
            extracted.contains(&"Charles Babbage in London".to_string()),
            "got {extracted:?}"
        );
        // Because the connector extension consumed it, the place name is not separate.
        assert!(
            !extracted.contains(&"London".to_string()),
            "got {extracted:?}"
        );
    }

    #[test]
    fn a_connector_only_extends_when_a_name_token_follows() {
        // `at` is a permitted connector but `the`/`yesterday` are not names, so the
        // span stops at the name and the trailing noun is not swallowed.
        let extracted = texts("Worked at Bank of England.");
        assert!(
            extracted.contains(&"Bank of England".to_string()),
            "got {extracted:?}"
        );
        let extracted = texts("Ada Lovelace at the conference.");
        assert!(
            extracted.iter().any(|entity| entity == "Ada Lovelace"),
            "got {extracted:?}"
        );
        assert!(!extracted.iter().any(|entity| entity.contains(" at the")));
    }

    #[test]
    fn a_sentence_opening_verb_does_not_join_the_following_name() {
        let extracted = texts("Prefers Ada Lovelace style.");
        assert!(extracted.iter().any(|entity| entity == "Ada Lovelace"));
        assert!(!extracted
            .iter()
            .any(|entity| entity == "Prefers Ada Lovelace"));
    }

    #[test]
    fn a_lone_sentence_opener_is_not_an_entity() {
        let extracted = texts("Prefers concise answers.");
        assert!(!extracted.iter().any(|entity| entity == "Prefers"));
    }

    #[test]
    fn a_name_ending_in_s_is_not_mistaken_for_a_verb() {
        // "Charles" ends in `s`; a suffix rule would have destroyed the name, which
        // is why the opener table is a closed list.
        let extracted = texts("Charles Babbage designed it.");
        assert!(
            extracted.iter().any(|entity| entity == "Charles Babbage"),
            "got {extracted:?}"
        );
    }

    #[test]
    fn inner_connectors_join_a_capitalized_run() {
        let extracted = texts("Worked at Bank of England.");
        assert!(
            extracted.iter().any(|entity| entity == "Bank of England"),
            "got {extracted:?}"
        );
    }

    #[test]
    fn generic_capitalized_words_are_rejected_at_span_level() {
        assert!(!texts("Alice reviewed the Notes.")
            .iter()
            .any(|entity| entity == "Notes"));
        assert!(!texts("The User prefers tea.")
            .iter()
            .any(|entity| entity == "User"));
        assert!(!texts("Saw the Overview yesterday.")
            .iter()
            .any(|entity| entity == "Overview"));
    }

    #[test]
    fn topics_are_multi_word_lowercase_phrases() {
        // The run is a maximal lowercase non-stopword sequence, and the leading verb
        // is dropped. A trailing adverb still joins the phrase, because separating it
        // needs a POS tagger — documented on `emit_topic`.
        let extracted = texts("prefers concise answers always");
        assert!(
            extracted
                .iter()
                .any(|entity| entity == "concise answers always"),
            "got {extracted:?}"
        );
        assert!(!extracted
            .iter()
            .any(|entity| entity == "prefers concise answers always"));
    }

    #[test]
    fn stopwords_and_capitalization_bound_a_topic_run() {
        // A stopword breaks the run, so the topic stops before it.
        let extracted = texts("concise answers and rolling deployment");
        assert!(
            extracted.iter().any(|entity| entity == "concise answers"),
            "got {extracted:?}"
        );
        assert!(
            extracted
                .iter()
                .any(|entity| entity == "rolling deployment"),
            "got {extracted:?}"
        );

        // A capitalized token breaks the run too, because it belongs to the proper channel.
        let extracted = texts("concise answers Ada Lovelace");
        assert!(
            extracted.iter().any(|entity| entity == "concise answers"),
            "got {extracted:?}"
        );
    }

    #[test]
    fn stopword_runs_do_not_become_topics() {
        assert!(texts("and then the of").is_empty());
    }

    #[test]
    fn generic_topic_heads_need_a_specific_modifier() {
        // "method" is a generic head; "important" is a non-specific adjective.
        assert!(texts("that important method").is_empty());
        assert!(!texts("the Bayesian method helped").is_empty());
    }

    #[test]
    fn generic_endings_are_stripped_only_when_two_words_remain() {
        let extracted = texts("rolling deployment items");
        assert!(
            extracted
                .iter()
                .any(|entity| entity == "rolling deployment"),
            "got {extracted:?}"
        );
    }

    #[test]
    fn artifacts_and_short_spans_are_rejected() {
        assert!(texts("- bullet item")
            .iter()
            .all(|entity| !entity.starts_with('-')));
        assert!(texts(&"x".repeat(MAX_ENTITY_TEXT_CHARS + 5)).is_empty());
        assert!(texts("**bold**").is_empty());
        // Upstream requires strictly more than 2 characters.
        assert!(texts("'ok'").is_empty());
        assert!(texts("'three'").iter().any(|entity| entity == "three"));
        // `'ok'` is rejected by the quote rule's own pre-check, so it cannot prove the
        // shared acceptance rule. `A1` reaches the acceptance rule through a channel
        // with no pre-check of its own: it is name-like (internal digit) and would be
        // emitted if the length bound were relaxed to `<= 1`.
        assert!(
            texts("A1 shipped").is_empty(),
            "the 2-character bound lives in the shared acceptance rule"
        );
    }

    #[test]
    fn deduplication_prefers_the_more_specific_class() {
        // Same normalized text from the quoted and proper channels: the proper-name
        // candidate has the lower priority number, and it is generated first.
        let entities = extract_entities("\"Ada Lovelace\" wrote notes.");
        let matching = entities
            .iter()
            .filter(|entity| normalize_entity_text(&entity.text) == "ada lovelace")
            .collect::<Vec<_>>();
        assert_eq!(matching.len(), 1, "one entity survives");
        assert_eq!(matching[0].kind, EntityKind::Proper);
    }

    #[test]
    fn deduplication_keeps_the_first_candidate_on_a_rank_tie() {
        // Both runs produce the same normalized text at the same rank, so the tie is
        // broken by generation order: the earlier span must survive.
        let sample = "concise answers and concise answers";
        let entities = extract_entities(sample);
        let survivors = entities
            .iter()
            .filter(|entity| entity.text == "concise answers")
            .collect::<Vec<_>>();
        assert_eq!(survivors.len(), 1, "normalized duplicates collapse to one");
        assert_eq!(
            survivors[0].span.expect("topic has a span").start,
            sample.find("concise answers").expect("first occurrence"),
            "the first candidate wins a rank tie"
        );
    }

    #[test]
    fn topic_and_proper_spans_never_overlap() {
        // This is the invariant that makes `resolve_candidates`' asymmetric exemption
        // unreachable. It is asserted rather than assumed: if a future generator lets
        // a topic run contain a capitalized token, this fails and the exemption
        // becomes live — which is exactly the signal a maintainer needs.
        let corpus = [
            "Ada Lovelace joined \"Project X\" at config.load_default for rolling deployment practice.",
            "Charles Babbage designed the Analytical Engine and preferred concise answers always.",
            "Bank of England staff use snake_case_id and gpt-4o in joint planning sessions.",
        ];
        for sample in corpus {
            let entities = extract_entities(sample);
            let topics = entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::Topic)
                .collect::<Vec<_>>();
            let propers = entities
                .iter()
                .filter(|entity| entity.kind == EntityKind::Proper)
                .collect::<Vec<_>>();
            for topic in &topics {
                assert!(
                    topic
                        .text
                        .chars()
                        .all(|character| !character.is_uppercase()),
                    "topic runs must stay lowercase: {}",
                    topic.text
                );
                for proper in &propers {
                    assert!(
                        !spans_overlap(topic, proper),
                        "{:?} overlaps {:?} in {sample:?}",
                        topic.text,
                        proper.text
                    );
                }
            }
        }
    }

    #[test]
    fn output_is_reordered_by_position_not_left_in_precedence_order() {
        // Precedence order would put the IDENTIFIER (priority 1) first. The output
        // must instead follow position, so the sentence-opening name leads.
        let entities =
            extract_entities("Ada Lovelace joined \"Project X\" at config.load_default.");
        let ordered = entities
            .iter()
            .filter(|entity| entity.span.is_some())
            .collect::<Vec<_>>();
        assert!(
            ordered.len() >= 2,
            "need two positioned candidates, got {ordered:?}"
        );
        assert_eq!(
            ordered[0].text, "Ada Lovelace",
            "positional order, not precedence"
        );
        for window in ordered.windows(2) {
            assert!(
                window[0].span.expect("has span").start < window[1].span.expect("has span").start,
                "{:?} must precede {:?}",
                window[0].text,
                window[1].text
            );
        }
    }

    #[test]
    fn span_less_candidates_sort_after_positioned_ones() {
        // The quoted phrase is lowercase and contains stopwords, so no other channel
        // produces the same normalized text and the span-less candidate survives.
        let entities = extract_entities("Ada Lovelace joined \"state of the art\" yesterday.");
        let first_spanless = entities
            .iter()
            .position(|entity| entity.span.is_none())
            .expect("a quoted candidate exists");
        let last_spanless = entities
            .iter()
            .rposition(|entity| entity.span.is_none())
            .expect("a quoted candidate exists");
        assert_eq!(
            first_spanless, last_spanless,
            "quoted candidates are contiguous"
        );
        assert_eq!(
            entities[first_spanless].text,
            "state of the art",
            "quoted candidates come last: {:?}",
            entities
                .iter()
                .map(|entity| &entity.text)
                .collect::<Vec<_>>()
        );
        assert!(
            entities[..first_spanless]
                .iter()
                .all(|entity| entity.span.is_some()),
            "everything before the quoted block is positioned"
        );
    }

    #[test]
    fn extraction_is_deterministic() {
        let sample = "Ada Lovelace preferred \"Analytical Engine\" over config.load_default.";
        assert_eq!(extract_entity_texts(sample), extract_entity_texts(sample));
    }

    #[test]
    fn blank_and_punctuation_only_text_yields_nothing() {
        assert!(texts("").is_empty());
        assert!(texts("   ...   ").is_empty());
        assert!(texts("-- ++").is_empty());
    }

    #[test]
    fn clean_text_matches_upstream_rule_order() {
        assert_eq!(clean_text("**Ada**"), "Ada");
        assert_eq!(clean_text("Topic:"), "Topic");
        assert_eq!(clean_text("12. Thing"), "Thing");
        assert_eq!(clean_text("3.Ada"), "Ada");
        assert_eq!(clean_text("1.2.3 stays"), "2.3 stays");
        assert_eq!(clean_text("  a   b  "), "a b");
    }

    #[test]
    fn has_artifacts_matches_upstream_checks() {
        assert!(has_artifacts("- listed"));
        assert!(has_artifacts("+ listed"));
        assert!(has_artifacts("\u{2022} listed"));
        assert!(has_artifacts("a  b"));
        assert!(has_artifacts("a\nb"));
        assert!(has_artifacts("a\tb"));
        assert!(has_artifacts("a ** b"));
        assert!(has_artifacts("a :* b"));
        assert!(has_artifacts("a * b"));
        assert!(has_artifacts(&"x".repeat(MAX_ENTITY_TEXT_CHARS + 1)));
        assert!(!has_artifacts("Ada Lovelace"));
        assert!(!has_artifacts(&"x".repeat(MAX_ENTITY_TEXT_CHARS)));
    }

    #[test]
    fn query_entities_are_capped_and_deduplicated_in_order() {
        let entities = extract_entities(
            "Ada Lovelace met Ada Lovelace in London with Charles Babbage, Alan Turing, \
             Grace Hopper, John von Neumann, Ada Lovelace, and Katherine Johnson.",
        );
        let selected = select_query_entities(&entities);
        assert!(selected.len() <= MAX_QUERY_ENTITIES);
        let keys = selected
            .iter()
            .map(|entity| normalize_entity_text(&entity.text))
            .collect::<Vec<_>>();
        let mut unique = keys.clone();
        unique.dedup();
        assert_eq!(keys, unique, "no normalized duplicates survive");
    }

    #[test]
    fn select_query_entities_drops_blank_text() {
        let blank = ExtractedEntity {
            kind: EntityKind::Proper,
            text: "   ".to_string(),
            span: None,
            confidence: 0.8,
            priority: 2,
            source: SOURCE_PROPER_NAME_SPAN,
        };
        assert!(select_query_entities(&[blank]).is_empty());
    }

    #[test]
    fn memory_count_weight_matches_upstream_formula() {
        assert_eq!(memory_count_weight(0), 1.0, "max(len, 1)");
        assert_eq!(memory_count_weight(1), 1.0);
        assert!((memory_count_weight(10) - 1.0 / 1.081).abs() < 1e-12);
        assert!(memory_count_weight(100) < memory_count_weight(10));
        assert!(memory_count_weight(10) < memory_count_weight(2));
    }

    #[test]
    fn boosts_apply_the_similarity_gate_inclusively() {
        let matches = [
            LinkedEntityMatch {
                similarity: 0.49,
                memory_ids: vec!["below".into()],
            },
            LinkedEntityMatch {
                similarity: 0.5,
                memory_ids: vec!["at".into()],
            },
        ];
        let boosts = entity_boosts(&matches);
        assert!(!boosts.contains_key("below"));
        assert!(!boosts.is_empty(), "the gate is inclusive");
    }

    #[test]
    fn boosts_take_the_maximum_across_entities_not_the_sum() {
        let matches = [
            LinkedEntityMatch {
                similarity: 0.9,
                memory_ids: vec!["shared".into()],
            },
            LinkedEntityMatch {
                similarity: 0.6,
                memory_ids: vec!["shared".into()],
            },
        ];
        let boosts = entity_boosts(&matches);
        assert!((boosts["shared"] - 0.9 * ENTITY_BOOST_WEIGHT).abs() < 1e-12);
    }

    #[test]
    fn boosts_never_exceed_the_entity_weight() {
        let boosts = entity_boosts(&[LinkedEntityMatch {
            similarity: 1.0,
            memory_ids: vec!["m".into()],
        }]);
        assert!(boosts["m"] <= ENTITY_BOOST_WEIGHT);
    }

    #[test]
    fn hub_entities_contribute_less_per_memory() {
        let specific = entity_boosts(&[LinkedEntityMatch {
            similarity: 1.0,
            memory_ids: vec!["m".into()],
        }]);
        let hub = entity_boosts(&[LinkedEntityMatch {
            similarity: 1.0,
            memory_ids: (0..200).map(|index| format!("m{index}")).collect(),
        }]);
        assert!(hub["m0"] < specific["m"]);
    }

    #[test]
    fn boosts_are_not_capped_because_the_cap_belongs_on_the_query_side() {
        let matches = (0..12)
            .map(|index| LinkedEntityMatch {
                similarity: 1.0,
                memory_ids: vec![format!("m{index}")],
            })
            .collect::<Vec<_>>();
        let boosts = entity_boosts(&matches);
        assert_eq!(boosts.len(), 12, "entity_boosts applies no match cap");
    }

    #[test]
    fn unlinked_and_blank_memory_ids_produce_no_boost() {
        assert!(entity_boosts(&[]).is_empty());
        assert!(entity_boosts(&[LinkedEntityMatch {
            similarity: 1.0,
            memory_ids: vec![],
        }])
        .is_empty());
        assert!(entity_boosts(&[LinkedEntityMatch {
            similarity: 1.0,
            memory_ids: vec![String::new()],
        }])
        .is_empty());
    }

    #[test]
    fn non_finite_similarity_is_ignored() {
        let boosts = entity_boosts(&[
            LinkedEntityMatch {
                similarity: f64::NAN,
                memory_ids: vec!["nan".into()],
            },
            LinkedEntityMatch {
                similarity: f64::INFINITY,
                memory_ids: vec!["inf".into()],
            },
        ]);
        assert!(boosts.is_empty());
    }

    #[test]
    fn thresholds_match_upstream_constants() {
        assert!((ENTITY_MERGE_SIMILARITY - 0.95).abs() < 1e-12);
        assert!((ENTITY_SIMILARITY_THRESHOLD - 0.5).abs() < 1e-12);
        assert_eq!(MAX_QUERY_ENTITIES, 8);
        assert_eq!(MAX_ENTITY_TEXT_CHARS, 100);
    }

    #[test]
    fn kind_codes_match_upstream_wire_values() {
        assert_eq!(EntityKind::Identifier.code(), "IDENTIFIER");
        assert_eq!(EntityKind::Proper.code(), "PROPER");
        assert_eq!(EntityKind::Quoted.code(), "QUOTED");
        assert_eq!(EntityKind::Topic.code(), "TOPIC");
    }

    #[test]
    fn entity_span_length_is_saturating() {
        assert_eq!(EntitySpan { start: 2, end: 9 }.len(), 7);
        assert_eq!(EntitySpan { start: 9, end: 2 }.len(), 0);
    }

    #[test]
    fn every_emitted_candidate_carries_a_known_source_tag() {
        let known = [
            SOURCE_TECHNICAL_IDENTIFIER,
            SOURCE_IDENTIFIER_HEURISTIC,
            SOURCE_PROPER_NAME_SPAN,
            SOURCE_QUOTED,
            SOURCE_TOPIC_PHRASE,
        ];
        let entities = extract_entities(
            "Ada Lovelace used config.load_default and gpt-4o, called \"Project X\", \
             rolling deployment practice.",
        );
        assert!(!entities.is_empty());
        for entity in entities {
            assert!(
                known.contains(&entity.source),
                "unknown source {}",
                entity.source
            );
        }
    }

    #[test]
    fn candidate_texts_always_pass_the_acceptance_rules() {
        let entities = extract_entities(
            "Ada Lovelace used config.load_default and gpt-4o, called \"Project X\", \
             rolling deployment practice.",
        );
        for entity in entities {
            assert!(entity.text.chars().count() > 2, "{}", entity.text);
            assert!(!has_artifacts(&entity.text), "{}", entity.text);
            assert_eq!(
                entity.text,
                clean_text(&entity.text),
                "{} is not cleaned",
                entity.text
            );
        }
    }
}
