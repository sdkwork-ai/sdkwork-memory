//! mem0 V3 additive memory extraction through the injected language model port.
//!
//! mem0's V3 write path is **additive**: one language-model call turns a batch of
//! conversation turns into a batch of self-contained memories, each optionally
//! linked to memories that already exist. There is no arbitration step, no
//! `UPDATE`, no `DELETE` — the only event the store ever records for this path is
//! `ADD`. This module owns that contract.
//!
//! The model is reached **only** through
//! [`LanguageModelPort`](sdkwork_memory_spi::LanguageModelPort), so no provider
//! client, endpoint, or credential lives here.
//!
//! # Provenance
//!
//! The prompt and wire format are ported from mem0 `f8082a7`
//! (`mem0/configs/prompts.py`, `ADDITIVE_EXTRACTION_PROMPT` and
//! `generate_additive_extraction_prompt`, Apache-2.0). The upstream few-shot
//! examples are deliberately **not** embedded: they are prompt-tuning artefacts
//! rather than interface contract, and inlining roughly 33 kB of them would
//! create a second copy that silently drifts from the reference clone. The
//! contract-bearing sections — roles, input semantics, linking, and the output
//! format — are ported verbatim.

use sdkwork_memory_spi::{LanguageModelCommand, LanguageModelPort};

use crate::error::{Result, SearchFirstVectorError};
use crate::procedural::{is_agent_scoped, AGENT_CONTEXT_SUFFIX};

/// Maximum number of memories accepted from a single extraction response.
///
/// A bound is required because the response is model-generated: without one, a
/// runaway answer would be projected into the index unbounded.
pub const MAX_EXTRACTED_MEMORIES: usize = 64;

/// Maximum number of existing memories rendered into a prompt as link evidence.
///
/// Mirrors the reference implementation's `top_k=10` evidence retrieval: the
/// model may only link to memories the caller actually offered, and a bounded
/// evidence list keeps the prompt from growing with the store.
pub const EXISTING_EVIDENCE_LIMIT: usize = 10;

/// Maximum number of recently extracted memories rendered into a prompt.
pub const RECENT_REFERENCE_LIMIT: usize = 20;

/// Maximum number of preceding context messages rendered into a prompt.
pub const CONTEXT_MESSAGE_LIMIT: usize = 20;

/// `YYYY-MM-DD`, the only date format the extraction contract emits.
pub const DATE_PATTERN: &str = "%Y-%m-%d";

/// Prompt asset instructing the model to answer with an additive memory envelope.
///
/// The envelope is the reference implementation's: one JSON object carrying a
/// `memory` array — singular, not `facts` — and nothing else, which is why the
/// parser below accepts no bare array and no prose wrapper.
pub const ADDITIVE_EXTRACTION_PROMPT: &str = r##"# ROLE

You are a Memory Extractor — a precise, evidence-bound processor responsible for
extracting rich, contextual memories from conversations. Your sole operation is
ADD: identify every piece of memorable information and produce self-contained,
contextually rich factual statements.

You extract from BOTH user and assistant messages. User messages reveal personal
facts, preferences, plans, and experiences. Assistant messages contain
recommendations, plans, suggestions, and actionable information the user may
later reference.

Accuracy and completeness are critical. Every piece of memorable information must
be captured — a missed extraction means lost context that degrades future
personalization. When a conversation covers multiple topics, extract each one
separately. Do not let a dominant topic cause you to miss secondary information.

# INPUTS

## New Messages

The current conversation turn(s) with "role" (user/assistant) and "content".

Both roles contain extractable information:
- User messages: personal facts, preferences, plans, experiences, things done or
  never done before, opinions, requests, implicit preferences revealed through
  questions.
- Assistant messages: specific recommendations given, plans or schedules
  created, information researched, solutions provided, agreements reached.

Attribute correctly: use "user" for user-stated facts. For assistant-generated
content, frame in terms of the user's context (for example "User was recommended
X" or "User's plan includes X as discussed in conversation").

Do NOT extract:
- Vague assistant characterizations ("you seem passionate", "that sounds
  stressful") unless the user explicitly confirms them.
- Generic assistant acknowledgments ("Sure!", "Great question!").
- Assistant meta-commentary about its own capabilities.

## Summary

A narrative summary of the user's profile from prior conversations. May be empty
for new users. Use it to enrich extractions — it holds established context such
as names, locations, and relationships.

## Recently Extracted Memories

Memories already captured from recent messages in this session. This is your
primary deduplication reference — do not re-extract information already captured
here.

## Existing Memories

Memories currently in the system relevant to this conversation.

Use these ONLY for deduplication and linking — do NOT extract new memories from
Existing Memories. Your extractions must come exclusively from New Messages. If
new information in New Messages is semantically equivalent to an Existing Memory
with no meaningful new context, skip it.

When a new memory is related to an Existing Memory — same topic, overlapping
entities, updated or shifted preference, follow-up event, or continuation of a
narrative — include that Existing Memory's ID in the new memory's
"linked_memory_ids" array. Your ADD output IDs remain sequential ("0", "1", ...)
but linked_memory_ids uses the IDs from this list.

IMPORTANT: an Existing Memory about an entity (for example "User has a dog named
Max") does NOT mean all information about that entity has been captured. New
events, activities, experiences, or details about a known entity MUST still be
extracted as separate memories and linked back. Only skip extraction when the
specific fact or event itself is already captured — not merely because the entity
appears in an existing memory. "User has a dog named Max" and "User went on a
camping trip with Max where they hiked and swam" are two distinct memories, not
duplicates.

## Last k Messages

Recent messages preceding New Messages. Use to resolve references and pronouns in
New Messages.

## Observation Date

When the conversation actually took place (for example "2023-05-24"). This is
your ONLY temporal anchor for resolving time references.

Resolve ALL relative references against Observation Date:
- "yesterday" means the day before Observation Date
- "last week" means the week preceding Observation Date
- "next month" means the month following Observation Date
- "recently" means shortly before Observation Date
- "just finished", "today" mean on or near Observation Date

CRITICAL: "User went to Paris last week" is useless six months later. "User went
to Paris the week of May 15, 2023" is meaningful forever. Always ground relative
references to specific dates.

## Current Date

Today's system date. May be years after Observation Date. Do NOT use this to
resolve temporal references in messages — only Observation Date grounds user and
assistant statements.

# GUIDELINES

## What to Extract

Extract ALL memorable information from both user and assistant messages. Think
broadly:

From user messages:
- Personal details, preferences, plans, relationships, professional context.
- Health, wellness, opinions, hobbies, emotional states.
- Entity attributes such as breed, model, colour, make, or size.
- Implicit preferences revealed through requests.
- Shared content and reference material — when a user shares a document, case
  study, article, data, specification, or any structured information, extract the
  key factual data FROM that content. The user shared it because they want it
  remembered.
- Firsts and milestones.
- Specific foods, meals, and who was present.
- Inspiration and motivation.

From assistant messages, ONLY when genuinely new:
- Specific recommendations given (books, restaurants, products, services).
- Plans or schedules created for the user.
- Information researched or provided.
- Agreements reached during conversation.
- Personal facts shared by named speakers — in multi-speaker conversations the
  "assistant" role may represent a real person sharing their own life. Extract
  their personal information with the same rigour as user-stated facts.

Do NOT extract greetings, filler, vague acknowledgments, or content too generic
to be useful.

### Extract Incidental Facts, Not Just Requests

When a user asks a question or makes a request, the message often contains
incidental personal facts stated as context. These are just as extractable as the
request itself. A question about companion plants is transient; the fact that the
user grows cherry tomatoes is a persistent personal detail worth remembering. Do
not let the request overshadow the facts.

### Casual Topics Are Still Extractable

Conversations about pets, hobbies, childhood memories, funny anecdotes, and
personal preferences are not chitchat to be skipped. Someone's pet's name, a
childhood activity, a funny incident, a new hobby — these are often the most
valuable. Only skip messages that are purely phatic with zero informational
content.

When in doubt, extract. A slightly redundant memory is far less costly than a
missing one; the deduplication step downstream handles true duplicates.

## Memory Quality Standards

### Contextually Rich, Not Atomic

Capture the full picture — the fact AND its surrounding context — in a single
unified memory, not scattered fragments.

Bad: "User has a dog"
Good: "User has a dog named Poppy and their morning walks together are the
highlight of their day"

This applies especially to transitions and changes. When the user describes
changing, switching, replacing, stopping, or trying something new in place of
something else, the memory MUST capture the transition — what the new state is
AND what it replaces, because the relationship between old and new is critical
context.

Bad: "User prefers oat milk lattes"
Good: "User switched from almond milk to oat milk lattes after developing an
almond sensitivity"

When a change is explicitly temporary or a trial, capture that too — "for a
month", "trying out", "testing" — because these signal the old arrangement may
resume.

### Ground Time References

See Observation Date above. A memory that says "last week" loses its meaning; a
memory that says "the week of 2023-05-15" keeps it.

# OUTPUT FORMAT

Return ONLY valid JSON parsable by json.loads(). No text, reasoning,
explanations, or wrappers.

## Structure

{"memory": [
  {"id": "0", "text": "First extracted memory", "attributed_to": "user", "linked_memory_ids": ["id-of-related-existing-memory"]},
  {"id": "1", "text": "Second extracted memory", "attributed_to": "assistant"}
]}

## Fields

- id (string, required): sequential integers as strings starting at "0".
- text (string, required): a contextually rich, self-contained factual statement,
  normally 15 to 80 words.
- attributed_to (string, required): who this memory is about. Use "user" for
  facts stated by or about the user (preferences, plans, personal facts). Use
  "assistant" for information provided by the assistant (recommendations,
  confirmations, plans created, information researched).
- linked_memory_ids (array of strings, optional): IDs of Existing Memories that
  this new memory relates to. Use the exact IDs from the Existing Memories list.
  Omit or pass [] when no existing memories are related.

## Rules

- Extract every piece of memorable information as a separate memory object.
- If nothing is worth extracting, return: {"memory": []}
- No duplicate IDs. Use double quotes. No trailing commas."##;

/// Who a memory is about, as declared by the extraction contract.
///
/// This is a closed vocabulary: the reference prompt tells the model to answer
/// with exactly `"user"` or `"assistant"`, so any other value is a contract
/// violation rather than a new category to invent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributedTo {
    /// Stated by, or about, the user.
    User,
    /// Provided by the assistant: a recommendation, confirmation, or researched
    /// information.
    Assistant,
}

impl AttributedTo {
    /// Wire value this attribution is persisted and rendered with.
    pub fn wire_value(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    /// Parse a wire value, accepting surrounding whitespace and any casing.
    ///
    /// Returns `None` for a value outside the closed vocabulary, so a caller
    /// decides whether to refuse the emission rather than silently inventing a
    /// third category.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

/// One conversation turn offered to the extractor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationTurn {
    /// Speaker role, rendered verbatim (for example `user` or `assistant`).
    pub role: String,
    /// Turn text.
    pub content: String,
}

impl ConversationTurn {
    /// Build a turn from any pair of string-like values.
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
        }
    }

    /// Whether this turn carries something to render.
    ///
    /// A turn with a blank role or blank content is dropped rather than rendered
    /// as `": text"`, which would present the model with a turn it cannot
    /// attribute. Shared with [`crate::procedural`], which renders a transcript
    /// the same way.
    pub(crate) fn is_renderable(&self) -> bool {
        !self.role.trim().is_empty() && !self.content.trim().is_empty()
    }

    pub(crate) fn render(&self) -> String {
        format!("{}: {}", self.role.trim(), self.content.trim())
    }
}

/// One already-known memory offered to the model as link evidence.
///
/// Only the identifier and text travel: the identifier is what the model may
/// name in `linked_memory_ids`, and the text is what it deduplicates against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingMemoryView {
    /// Identifier taken verbatim from the canonical store.
    pub memory_id: String,
    /// Current canonical text.
    pub text: String,
}

/// Everything the additive extraction prompt needs.
///
/// Every field is optional or defaulted except the new messages, because in the
/// reference pipeline every other section degrades to an empty placeholder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdditiveExtractionRequest {
    /// Narrative profile summary, when the deployment maintains one.
    pub summary: Option<String>,
    /// Memories already captured from recent messages in this session.
    pub recent_memories: Vec<String>,
    /// Memories already in the store, offered for deduplication and linking.
    pub existing_memories: Vec<ExistingMemoryView>,
    /// Messages preceding the new ones, used to resolve references.
    pub last_k_messages: Vec<ConversationTurn>,
    /// The messages to extract from.
    pub new_messages: Vec<ConversationTurn>,
    /// When the conversation took place, as `YYYY-MM-DD`.
    ///
    /// This is the model's only temporal anchor. Defaults to the current date
    /// when absent.
    pub observation_date: Option<String>,
    /// Today's date, as `YYYY-MM-DD`. Defaults to the current date when absent.
    pub current_date: Option<String>,
    /// Deployment-supplied extraction rules. Highest priority for the model.
    pub custom_instructions: Option<String>,
    /// Agent id in force for this write, when the caller scoped it to an agent.
    ///
    /// Consulted only through [`is_agent_scoped`], together with
    /// [`Self::user_id`]: an agent-scoped write is framed from the agent's point
    /// of view, a user-scoped one is not. Absent means "no agent scope", which is
    /// a different statement from an agent id that happens to be blank.
    pub agent_id: Option<String>,
    /// User id in force for this write, when the caller scoped it to a user.
    ///
    /// See [`Self::agent_id`]. Its presence cancels the agent framing even when an
    /// agent id is also present, because a write addressed to both a user and an
    /// agent is a user's memory.
    pub user_id: Option<String>,
}

/// One memory parsed out of an extraction response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedMemory {
    /// Sequential identifier the model assigned, kept so a refusal or a dropped
    /// link can be traced back to the emission it came from.
    pub index: String,
    /// The memory text. Never blank for an accepted emission.
    pub text: String,
    /// Who the memory is about, when the model declared it recognisably.
    pub attributed_to: Option<AttributedTo>,
    /// Identifiers of existing memories this one relates to, as the model
    /// declared them.
    ///
    /// Unvalidated on purpose: [`crate::additive`] is what checks them against
    /// the evidence the caller actually offered.
    pub linked_memory_ids: Vec<String>,
}

impl ExtractedMemory {
    /// Text used for content-hash deduplication and link evidence.
    pub fn comparable_text(&self) -> &str {
        self.text.as_str()
    }
}

/// Why one emission was dropped instead of being turned into a memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionRejection {
    /// The emission carried no usable `text`.
    BlankText,
    /// `attributed_to` was present but outside the closed vocabulary.
    ///
    /// Refused rather than defaulted: provenance that cannot be read is not the
    /// same as provenance that was never stated, and relabelling one as the
    /// other would silently rewrite what the model said.
    UnrecognisedAttribution {
        /// The value the model supplied.
        value: String,
    },
}

impl EmissionRejection {
    /// Short, stable label suitable for a metric or trace field.
    pub fn label(&self) -> &'static str {
        match self {
            Self::BlankText => "blank_text",
            Self::UnrecognisedAttribution { .. } => "unrecognised_attribution",
        }
    }
}

/// One emission the parser refused, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefusedEmission {
    /// Sequential identifier the model assigned to the refused emission, or
    /// `<unnumbered>` when it supplied none.
    pub index: String,
    /// Why it was refused.
    pub rejection: EmissionRejection,
}

/// What one extraction produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionReport {
    /// Memories the parser accepted.
    pub memories: Vec<ExtractedMemory>,
    /// Emissions the parser refused, each with its reason.
    pub refused: Vec<RefusedEmission>,
    /// How many accepted emissions were dropped by
    /// [`MAX_EXTRACTED_MEMORIES`].
    ///
    /// Reported instead of silently clamped: a bound that fires invisibly would
    /// hide a runaway model from whoever is watching extraction quality.
    pub truncated: usize,
}

impl ExtractionReport {
    /// Whether the response yielded nothing usable at all.
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }
}

#[derive(Debug, serde::Deserialize)]
struct MemoryEnvelope {
    /// Required, not defaulted: `serde` ignores unknown keys, so a defaulted
    /// field would turn a response keyed `facts` — or keyed anything else — into
    /// a silent empty success. Absence of `memory` is a contract violation.
    memory: Vec<MemoryWire>,
}

#[derive(Debug, serde::Deserialize)]
struct MemoryWire {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    attributed_to: Option<String>,
    #[serde(default)]
    linked_memory_ids: Option<Vec<String>>,
}

/// Extract memories from conversation turns using `language_model`.
///
/// A request with no renderable new messages short-circuits without a provider
/// call: there is nothing to extract, and calling the model anyway would spend a
/// request to learn that.
///
/// # Errors
///
/// Returns [`SearchFirstVectorError::ProviderCallFailed`] when the language model
/// port refuses the request, and
/// [`SearchFirstVectorError::MemoryExtractionUnparseable`] when the answer cannot
/// be interpreted as an additive memory envelope.
///
/// # Examples
///
/// ```
/// # use sdkwork_memory_plugin_search_first_vector::{
/// #     extract_memories, AdditiveExtractionRequest, AttributedTo, ConversationTurn,
/// # };
/// # use sdkwork_memory_spi::{LanguageModelCommand, LanguageModelPort, MemorySpiResult};
/// # use async_trait::async_trait;
/// #
/// # struct FixedProvider;
/// #
/// # #[async_trait]
/// # impl LanguageModelPort for FixedProvider {
/// #     fn provider_code(&self) -> &str {
/// #         "doctest-provider"
/// #     }
/// #
/// #     async fn generate(&self, _command: LanguageModelCommand) -> MemorySpiResult<String> {
/// #         Ok(r#"{"memory":[{"id":"0","text":"User prefers dark mode.","attributed_to":"user"}]}"#
/// #             .to_string())
/// #     }
/// # }
/// #
/// # #[tokio::main]
/// # async fn main() {
/// let request = AdditiveExtractionRequest {
///     new_messages: vec![ConversationTurn::new("user", "I only use dark mode.")],
///     ..AdditiveExtractionRequest::default()
/// };
///
/// let report = extract_memories(&FixedProvider, &request).await.unwrap();
///
/// assert_eq!(report.memories.len(), 1);
/// assert_eq!(report.memories[0].text, "User prefers dark mode.");
/// assert_eq!(report.memories[0].attributed_to, Some(AttributedTo::User));
/// # }
/// ```
#[must_use = "an extraction report must be handled, not discarded"]
pub async fn extract_memories(
    language_model: &dyn LanguageModelPort,
    request: &AdditiveExtractionRequest,
) -> Result<ExtractionReport> {
    if prompt_is_empty(request) {
        return Ok(ExtractionReport {
            memories: Vec::new(),
            refused: Vec::new(),
            truncated: 0,
        });
    }

    let prompt = build_additive_extraction_prompt(request);
    let response = language_model
        .generate(LanguageModelCommand { prompt })
        .await
        .map_err(|source| SearchFirstVectorError::ProviderCallFailed {
            port: crate::error::LANGUAGE_MODEL_PORT,
            provider: language_model.provider_code().to_string(),
            source,
        })?;

    parse_additive_response(&response)
}

/// Whether the request carries no new message worth a provider call.
fn prompt_is_empty(request: &AdditiveExtractionRequest) -> bool {
    !request
        .new_messages
        .iter()
        .any(ConversationTurn::is_renderable)
}

/// Compose the additive extraction prompt for a request.
///
/// The sections appear in the reference order, and a section with no content is
/// rendered as an empty body rather than omitted, so the model always sees the
/// same shape. The evidence, context, and recent lists are each truncated to
/// their declared bound before rendering.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{
///     build_additive_extraction_prompt, AdditiveExtractionRequest, ConversationTurn,
/// };
///
/// let request = AdditiveExtractionRequest {
///     new_messages: vec![ConversationTurn::new("user", "I adopted a beagle.")],
///     ..AdditiveExtractionRequest::default()
/// };
/// let prompt = build_additive_extraction_prompt(&request);
///
/// assert!(prompt.contains("## New Messages"));
/// assert!(prompt.contains("user: I adopted a beagle."));
/// ```
pub fn build_additive_extraction_prompt(request: &AdditiveExtractionRequest) -> String {
    let current_date = resolve_date(request.current_date.as_deref(), &today_utc_date());
    let observation_date = resolve_date(request.observation_date.as_deref(), &current_date);

    let mut prompt = String::with_capacity(ADDITIVE_EXTRACTION_PROMPT.len() + 2_048);
    prompt.push_str(ADDITIVE_EXTRACTION_PROMPT);

    // Appended to the contract prompt, before the input sections are rendered,
    // because that is where the reference appends it (`memory/main.py:941-944`).
    // An agent-scoped write is framed from the agent's point of view; a write that
    // also names a user is not, however many agent ids are present.
    if is_agent_scoped(request.agent_id.as_deref(), request.user_id.as_deref()) {
        prompt.push_str(AGENT_CONTEXT_SUFFIX);
    }

    prompt.push_str("\n\n## Summary\n");
    if let Some(summary) = request.summary.as_deref().map(str::trim) {
        if !summary.is_empty() {
            prompt.push_str(summary);
        }
    }

    prompt.push_str("\n\n## Last k Messages\n");
    render_turns(&mut prompt, &request.last_k_messages, CONTEXT_MESSAGE_LIMIT);

    prompt.push_str("\n\n## Recently Extracted Memories\n");
    render_text_list(
        &mut prompt,
        &request.recent_memories,
        RECENT_REFERENCE_LIMIT,
    );

    prompt.push_str("\n\n## Existing Memories\n");
    render_existing_memories(&mut prompt, &request.existing_memories);

    prompt.push_str("\n\n## New Messages\n");
    render_turns(&mut prompt, &request.new_messages, usize::MAX);

    prompt.push_str("\n\n## Observation Date\n");
    prompt.push_str(&observation_date);
    prompt.push_str("\n\n## Current Date\n");
    prompt.push_str(&current_date);

    if let Some(instructions) = request.custom_instructions.as_deref().map(str::trim) {
        if !instructions.is_empty() {
            prompt.push_str("\n\n## Custom Instructions\n");
            prompt.push_str(instructions);
        }
    }

    prompt.push_str("\n\n# Output:");
    prompt
}

/// Today's UTC date as `YYYY-MM-DD`.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::today_utc_date;
///
/// let today = today_utc_date();
///
/// assert_eq!(today.len(), 10);
/// assert_eq!(today.as_bytes()[4], b'-');
/// ```
pub fn today_utc_date() -> String {
    sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), Some(DATE_PATTERN))
}

/// Use the supplied date when it carries content, otherwise the fallback.
fn resolve_date(supplied: Option<&str>, fallback: &str) -> String {
    match supplied.map(str::trim) {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => fallback.to_string(),
    }
}

fn render_turns(prompt: &mut String, turns: &[ConversationTurn], limit: usize) {
    for turn in turns.iter().filter(|turn| turn.is_renderable()).take(limit) {
        prompt.push_str(&turn.render());
        prompt.push('\n');
    }
}

fn render_text_list(prompt: &mut String, values: &[String], limit: usize) {
    for value in values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .take(limit)
    {
        prompt.push_str("- ");
        prompt.push_str(value);
        prompt.push('\n');
    }
}

fn render_existing_memories(prompt: &mut String, existing: &[ExistingMemoryView]) {
    for view in existing
        .iter()
        .filter(|view| !view.memory_id.trim().is_empty() && !view.text.trim().is_empty())
        .take(EXISTING_EVIDENCE_LIMIT)
    {
        prompt.push_str(&format!(
            "- id: {} | text: {}\n",
            view.memory_id.trim(),
            view.text.trim()
        ));
    }
}

/// Parse an extraction response into memories.
///
/// Accepts the documented `memory` envelope, optionally wrapped in a markdown
/// code fence. Anything else is refused: guessing at malformed output would let
/// an unparseable answer be projected as though it were a memory. In particular a
/// bare array is **not** accepted, because the contract the prompt states is an
/// object keyed `memory`, and silently accepting another shape would hide a model
/// that has stopped following it.
///
/// # Errors
///
/// Returns [`SearchFirstVectorError::MemoryExtractionUnparseable`] when the
/// response is empty or carries no `memory` array.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{parse_additive_response, AttributedTo};
///
/// let report = parse_additive_response(
///     r#"{"memory":[{"id":"0","text":"User drinks tea.","attributed_to":"user"}]}"#,
/// )
/// .unwrap();
///
/// assert_eq!(report.memories.len(), 1);
/// assert_eq!(report.memories[0].attributed_to, Some(AttributedTo::User));
/// assert!(report.refused.is_empty());
/// ```
#[must_use = "a parsed extraction report must be handled, not discarded"]
pub fn parse_additive_response(response: &str) -> Result<ExtractionReport> {
    let payload = strip_code_fence(response).trim();
    if payload.is_empty() {
        return Err(SearchFirstVectorError::MemoryExtractionUnparseable {
            reason: "the language model returned an empty response".to_string(),
        });
    }

    let envelope: MemoryEnvelope = serde_json::from_str(payload).map_err(|error| {
        SearchFirstVectorError::MemoryExtractionUnparseable {
            reason: format!("expected an additive memory envelope: {error}"),
        }
    })?;

    let mut memories = Vec::new();
    let mut refused = Vec::new();
    let mut truncated = 0usize;

    for wire in envelope.memory {
        let index = wire.index_or_placeholder();

        match wire.into_memory() {
            Ok(memory) => {
                if memories.len() >= MAX_EXTRACTED_MEMORIES {
                    truncated += 1;
                    continue;
                }
                memories.push(memory);
            }
            Err(rejection) => refused.push(RefusedEmission { index, rejection }),
        }
    }

    Ok(ExtractionReport {
        memories,
        refused,
        truncated,
    })
}

impl MemoryWire {
    fn index_or_placeholder(&self) -> String {
        match self.id.as_deref().map(str::trim) {
            Some(value) if !value.is_empty() => value.to_string(),
            _ => "<unnumbered>".to_string(),
        }
    }

    fn into_memory(self) -> std::result::Result<ExtractedMemory, EmissionRejection> {
        let index = self.index_or_placeholder();

        let Some(text) = normalize_optional(self.text) else {
            return Err(EmissionRejection::BlankText);
        };

        let attributed_to = match normalize_optional(self.attributed_to) {
            Some(value) => match AttributedTo::parse(&value) {
                Some(attributed_to) => Some(attributed_to),
                None => {
                    return Err(EmissionRejection::UnrecognisedAttribution { value });
                }
            },
            None => None,
        };

        let linked_memory_ids = self
            .linked_memory_ids
            .unwrap_or_default()
            .into_iter()
            .filter_map(normalize_entry)
            .collect();

        Ok(ExtractedMemory {
            index,
            text,
            attributed_to,
            linked_memory_ids,
        })
    }
}

/// Remove a single surrounding markdown code fence, if present.
fn strip_code_fence(response: &str) -> &str {
    let trimmed = response.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };

    // Drop the optional language tag on the opening fence.
    let body_start = match rest.find('\n') {
        Some(index) => index + 1,
        None => return trimmed,
    };
    let body = &rest[body_start..];

    match body.rfind("```") {
        Some(index) => &body[..index],
        None => trimmed,
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Trim a present value, mapping a blank result to absence.
fn normalize_entry(value: String) -> Option<String> {
    normalize_optional(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_attribution_wire_values_are_pinned() {
        assert_eq!(AttributedTo::User.wire_value(), "user");
        assert_eq!(AttributedTo::Assistant.wire_value(), "assistant");
        assert_eq!(AttributedTo::parse(" USER "), Some(AttributedTo::User));
        assert_eq!(AttributedTo::parse("system"), None);
    }

    #[test]
    fn envelope_payload_is_parsed() {
        let report = parse_additive_response(
            r#"{"memory":[{"id":"0","text":"User prefers dark mode.","attributed_to":"user"}]}"#,
        )
        .unwrap();

        assert_eq!(report.memories.len(), 1);
        assert_eq!(report.memories[0].index, "0");
        assert_eq!(report.memories[0].text, "User prefers dark mode.");
        assert_eq!(report.memories[0].attributed_to, Some(AttributedTo::User));
        assert!(report.memories[0].linked_memory_ids.is_empty());
    }

    #[test]
    fn linked_memory_ids_are_preserved_verbatim() {
        let report = parse_additive_response(
            r#"{"memory":[{"id":"0","text":"User walks Poppy daily.","attributed_to":"user","linked_memory_ids":["m-1","m-2"]}]}"#,
        )
        .unwrap();

        assert_eq!(report.memories[0].linked_memory_ids, vec!["m-1", "m-2"]);
    }

    #[test]
    fn a_duplicate_id_is_parsed_and_labelled() {
        let report = parse_additive_response(
            r#"{"memory":[
                {"id":"0","text":"User drinks tea.","attributed_to":"user"},
                {"id":"0","text":"User keeps a garden.","attributed_to":"user"}
            ]}"#,
        )
        .unwrap();

        assert_eq!(report.memories.len(), 2);
        assert_eq!(report.memories[0].index, report.memories[1].index);
    }

    #[test]
    fn fenced_payload_is_parsed() {
        let report = parse_additive_response(
            "```json\n{\"memory\":[{\"text\":\"User lives in Berlin.\",\"attributed_to\":\"user\"}]}\n```",
        )
        .unwrap();

        assert_eq!(report.memories.len(), 1);
        assert_eq!(report.memories[0].text, "User lives in Berlin.");
    }

    #[test]
    fn missing_attribution_is_tolerated() {
        let report =
            parse_additive_response(r#"{"memory":[{"text":"User drinks tea."}]}"#).unwrap();

        assert_eq!(report.memories.len(), 1);
        assert_eq!(report.memories[0].attributed_to, None);
        assert!(report.refused.is_empty());
    }

    #[test]
    fn attribution_is_case_insensitive() {
        let report = parse_additive_response(
            r#"{"memory":[{"text":"Agent advised X.","attributed_to":"Assistant"}]}"#,
        )
        .unwrap();

        assert_eq!(
            report.memories[0].attributed_to,
            Some(AttributedTo::Assistant)
        );
    }

    #[test]
    fn an_unrecognised_attribution_refuses_only_that_emission() {
        let report = parse_additive_response(
            r#"{"memory":[
                {"id":"0","text":"User drinks tea.","attributed_to":"robot"},
                {"id":"1","text":"User keeps a garden.","attributed_to":"user"}
            ]}"#,
        )
        .unwrap();

        assert_eq!(report.memories.len(), 1);
        assert_eq!(report.memories[0].index, "1");
        assert_eq!(
            report.refused,
            vec![RefusedEmission {
                index: "0".to_string(),
                rejection: EmissionRejection::UnrecognisedAttribution {
                    value: "robot".to_string()
                },
            }]
        );
    }

    #[test]
    fn empty_memory_list_is_accepted() {
        let report = parse_additive_response(r#"{"memory":[]}"#).unwrap();

        assert!(report.is_empty());
        assert!(report.refused.is_empty());
        assert_eq!(report.truncated, 0);
    }

    #[test]
    fn a_bare_array_is_refused() {
        let error = parse_additive_response(r#"[{"text":"User drinks tea."}]"#).unwrap_err();

        assert!(matches!(
            error,
            SearchFirstVectorError::MemoryExtractionUnparseable { .. }
        ));
    }

    #[test]
    fn the_facts_envelope_is_refused() {
        // The envelope key is `memory`, singular. A response still shaped like the
        // retired fact envelope must not be accepted as if it were aligned.
        assert!(parse_additive_response(r#"{"facts":[{"text":"User drinks tea."}]}"#).is_err());
    }

    #[test]
    fn prose_wrapper_is_refused() {
        let error =
            parse_additive_response("Sure! Here are the memories you asked for.").unwrap_err();

        assert!(matches!(
            error,
            SearchFirstVectorError::MemoryExtractionUnparseable { .. }
        ));
    }

    #[test]
    fn empty_response_is_refused() {
        assert!(parse_additive_response("   ").is_err());
    }

    #[test]
    fn blank_texts_are_refused_rather_than_projected() {
        let report = parse_additive_response(
            r#"{"memory":[{"id":"0","text":"   "},{"id":"1","text":"User keeps a garden."}]}"#,
        )
        .unwrap();

        assert_eq!(report.memories.len(), 1);
        assert_eq!(
            report.refused,
            vec![RefusedEmission {
                index: "0".to_string(),
                rejection: EmissionRejection::BlankText,
            }]
        );
    }

    #[test]
    fn extraction_response_is_bounded_and_the_overflow_is_reported() {
        let wires: Vec<String> = (0..(MAX_EXTRACTED_MEMORIES + 10))
            .map(|index| format!(r#"{{"id":"{index}","text":"fact {index}"}}"#))
            .collect();
        let payload = format!(r#"{{"memory":[{}]}}"#, wires.join(","));

        let report = parse_additive_response(&payload).unwrap();

        assert_eq!(report.memories.len(), MAX_EXTRACTED_MEMORIES);
        assert_eq!(report.truncated, 10);
    }

    #[test]
    fn the_prompt_renders_every_section_in_order() {
        let request = AdditiveExtractionRequest {
            summary: Some("User is a beekeeper.".to_string()),
            recent_memories: vec!["User lives in Berlin.".to_string()],
            existing_memories: vec![ExistingMemoryView {
                memory_id: "m-1".to_string(),
                text: "User has a dog named Max.".to_string(),
            }],
            last_k_messages: vec![ConversationTurn::new("assistant", "How is Max?")],
            new_messages: vec![ConversationTurn::new("user", "We hiked last week.")],
            observation_date: Some("2023-05-24".to_string()),
            current_date: Some("2025-01-01".to_string()),
            custom_instructions: Some("Prefer metric units.".to_string()),
            // No agent scope, so the prompt is the contract asset plus sections
            // only — which is what lets the ordering assertions below anchor on
            // the asset's own length.
            agent_id: None,
            user_id: None,
        };

        let prompt = build_additive_extraction_prompt(&request);

        // Anchor the ordering assertions on the rendered suffix: the prompt asset
        // itself also contains these headings, in a different order.
        let rendered = &prompt[ADDITIVE_EXTRACTION_PROMPT.len()..];

        let summary_at = rendered.find("## Summary").unwrap();
        let last_at = rendered.find("## Last k Messages").unwrap();
        let recent_at = rendered.find("## Recently Extracted Memories").unwrap();
        let existing_at = rendered.find("## Existing Memories").unwrap();
        let new_at = rendered.find("## New Messages").unwrap();
        let observation_at = rendered.find("## Observation Date").unwrap();
        let current_at = rendered.find("## Current Date").unwrap();

        assert!(summary_at < last_at);
        assert!(last_at < recent_at);
        assert!(recent_at < existing_at);
        assert!(existing_at < new_at);
        assert!(new_at < observation_at);
        assert!(observation_at < current_at);

        assert!(prompt.contains("User is a beekeeper."));
        assert!(prompt.contains("assistant: How is Max?"));
        assert!(prompt.contains("user: We hiked last week."));
        assert!(prompt.contains("- id: m-1 | text: User has a dog named Max."));
        assert!(prompt.contains("Prefer metric units."));
        assert!(prompt.contains("2023-05-24"));
        assert!(prompt.contains("2025-01-01"));
    }

    #[test]
    fn the_observation_date_defaults_to_the_current_date() {
        let request = AdditiveExtractionRequest {
            current_date: Some("2025-06-01".to_string()),
            new_messages: vec![ConversationTurn::new("user", "hello")],
            ..AdditiveExtractionRequest::default()
        };

        let prompt = build_additive_extraction_prompt(&request);

        assert!(prompt.contains("## Observation Date\n2025-06-01"));
        assert!(prompt.contains("## Current Date\n2025-06-01"));
    }

    #[test]
    fn the_evidence_list_is_bounded_at_the_declared_limit() {
        let existing: Vec<ExistingMemoryView> = (0..(EXISTING_EVIDENCE_LIMIT + 5))
            .map(|index| ExistingMemoryView {
                memory_id: format!("m-{index}"),
                text: format!("memory {index}"),
            })
            .collect();
        let request = AdditiveExtractionRequest {
            existing_memories: existing,
            new_messages: vec![ConversationTurn::new("user", "hello")],
            ..AdditiveExtractionRequest::default()
        };

        let prompt = build_additive_extraction_prompt(&request);

        assert!(prompt.contains(&format!("- id: m-{} |", EXISTING_EVIDENCE_LIMIT - 1)));
        assert!(!prompt.contains(&format!("- id: m-{} |", EXISTING_EVIDENCE_LIMIT)));
    }

    #[test]
    fn blank_turns_are_not_rendered() {
        let request = AdditiveExtractionRequest {
            new_messages: vec![
                ConversationTurn::new("user", "   "),
                ConversationTurn::new("   ", "content with no speaker"),
                ConversationTurn::new("user", "User keeps a garden."),
            ],
            ..AdditiveExtractionRequest::default()
        };

        let prompt = build_additive_extraction_prompt(&request);

        assert!(prompt.contains("user: User keeps a garden."));
        assert!(!prompt.contains("content with no speaker"));
    }
}
