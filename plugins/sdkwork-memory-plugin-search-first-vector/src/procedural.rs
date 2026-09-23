//! Procedural-memory creation and the agent-context suffix.
//!
//! Two reference behaviours live here, both from the mem0 V3 pipeline:
//!
//! 1. **The agent-context suffix.** When a write is scoped to an agent and not
//!    to a user, the reference appends a fixed block to the extraction system
//!    prompt so memories are framed from the agent's point of view
//!    (`memory/main.py:941-944`, `:2603-2606`). The condition is exact: an agent
//!    id is present **and** no user id is.
//! 2. **The procedural-memory path.** `add(memory_type="procedural_memory")` does
//!    not run additive extraction at all. It asks the model to summarise the
//!    agent's execution history into a step-by-step record (`memory/main.py:1993-2037`),
//!    strips one enclosing fenced block, refuses an empty answer, forces
//!    `memory_type` onto the record, and stores the summary as a plain `ADD`.
//!    Any *other* `memory_type` is refused outright (`memory/main.py:831-837`),
//!    including the two names the reference enumerates but never produces.
//!
//! # Deviation: the prompts are ported by meaning, not copied
//!
//! The reference prompts are Apache-2.0 third-party text. Copying them verbatim
//! into an AGPL-3.0 source tree would create a second copy that needs separate
//! attribution and that silently drifts, because the reference checkout sits
//! outside version control (`.gitignore:102`). The normative content is therefore
//! re-expressed here, under the same policy already applied to the additive
//! prompt's examples (parity matrix §7).
//!
//! What this deviation does **not** cost: unlike the additive path, the
//! procedural path parses no structure out of the answer — its shape is free
//! text. The prompt therefore steers quality only, so re-expressing it by meaning
//! loses no interface contract. By contrast [`crate::extraction`] keeps its
//! output-format section, because that section *is* the contract.
//!
//! # Deviation: the summary is returned, this plugin stores nothing
//!
//! This plugin owns no canonical rows, so [`plan_procedural_memory`] returns the
//! validated summary together with the memory type the caller must write beside
//! it. The reference stores inline, which it can do only because it owns the
//! vector store. The rule that an empty summary and a missing metadata carrier
//! are both refusals is preserved either way.

use crate::error::{Result, SearchFirstVectorError};
use crate::extraction::ConversationTurn;

/// The one memory type this pipeline can create out of band.
///
/// The reference enumerates three types but accepts only this one in `add()`.
pub const MEMORY_TYPE_PROCEDURAL: &str = "procedural_memory";

/// Enumerated by the reference but never produced by any code path.
///
/// Named here so the rejection is testable by name rather than by string
/// literal, and so a future implementation of these types has one place to
/// change.
pub const MEMORY_TYPE_SEMANTIC: &str = "semantic_memory";

/// See [`MEMORY_TYPE_SEMANTIC`].
pub const MEMORY_TYPE_EPISODIC: &str = "episodic_memory";

/// The closing user turn of every procedural request.
///
/// Kept verbatim: it is the instruction the model is answering, not quality
/// guidance, so its exact wording is part of the behaviour being aligned.
pub const PROCEDURAL_MEMORY_REQUEST: &str = "Create procedural memory of the above conversation.";

/// Appended to the extraction system prompt for agent-scoped writes.
///
/// Applies only when [`is_agent_scoped`] holds. The three framing rules and the
/// reminder that `attributed_to` still records the original source are the whole
/// of the reference block's normative content.
pub const AGENT_CONTEXT_SUFFIX: &str = r##"

## Entity Context

The primary entity is an AI agent. Frame memories from the agent's perspective:

- A fact the user stated becomes agent knowledge: the agent was informed of it, or learned it.
- A thing the agent did is stated directly: the agent recommended something, or specialises in a domain.
- The agent's own configuration or instructions are captured as such.

`attributed_to` still records the original source: `user` for a fact the user stated, `assistant` for something the agent said or did.
"##;

/// System prompt for the procedural-memory path.
///
/// The summary this steers is free text — the reference parses no structure out
/// of it — so this is quality guidance rather than an interface contract. The
/// structural obligations the reference insists on are all preserved: verbatim
/// action results, no omitted steps, no summarised-away failures.
pub const PROCEDURAL_MEMORY_SYSTEM_PROMPT: &str = r##"# ROLE

You record and preserve the complete interaction history between a human and an AI agent. You are given the agent's execution history over the past steps. Produce a summary of that output history carrying every detail the agent would need in order to continue the task without ambiguity. Every output the agent produced is recorded verbatim as part of the summary.

# STRUCTURE

## Overview

- **Task Objective**: the overall goal the agent is working toward.
- **Progress Status**: how far the task has come, naming the milestones completed.

## Sequential Agent Actions

Each action is a self-contained numbered step that carries all of the following:

1. **Agent Action** — precisely what the agent did, including the parameters, target elements, and methods involved.
2. **Action Result** — the exact, unaltered output, placed immediately after the action. Record returned data, responses, markup fragments, JSON content, and error messages exactly as received; the final output is reconstructed from these later, so this is the part that must not be paraphrased.
3. **Embedded Metadata** — the identifiers, versions, and timestamps that the result depends on.
4. **Key Findings** — what this step established.
5. **Current Context** — where the task stands once the step is done.

# RULES

- Never paraphrase an action result. Verbatim capture is the point of this record.
- Never omit a step, however small. A skipped step is a hole the agent cannot recover from.
- Never summarise away a failure or an error message. Both are part of the history.
"##;

/// Which write path a request selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryWritePath {
    /// ADD-only extraction from conversation turns.
    Additive,
    /// One summary of the agent's execution history.
    Procedural,
}

/// Resolve a requested memory type to the write path that serves it.
///
/// The reference accepts exactly two states: no type at all, or the procedural
/// type. Every other value is refused, **including** [`MEMORY_TYPE_SEMANTIC`]
/// and [`MEMORY_TYPE_EPISODIC`], which the reference enumerates but never
/// produces (`memory/main.py:831-837`). Refusing them here keeps that asymmetry
/// visible instead of quietly routing them somewhere that looks plausible.
///
/// The comparison is on the exact string, with no trimming and no case folding,
/// because the reference compares exactly; a caller that passes `" Procedural "`
/// gets a refusal rather than a silent reinterpretation of their input.
///
/// # Errors
///
/// Returns [`SearchFirstVectorError::UnsupportedMemoryType`] for any value other
/// than an absent type or [`MEMORY_TYPE_PROCEDURAL`].
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{
///     resolve_write_path, MemoryWritePath, MEMORY_TYPE_PROCEDURAL, MEMORY_TYPE_SEMANTIC,
/// };
///
/// assert_eq!(resolve_write_path(None)?, MemoryWritePath::Additive);
/// assert_eq!(
///     resolve_write_path(Some(MEMORY_TYPE_PROCEDURAL))?,
///     MemoryWritePath::Procedural
/// );
///
/// // Enumerated by the reference, still not creatable.
/// assert!(resolve_write_path(Some(MEMORY_TYPE_SEMANTIC)).is_err());
/// # Ok::<(), sdkwork_memory_plugin_search_first_vector::SearchFirstVectorError>(())
/// ```
pub fn resolve_write_path(requested: Option<&str>) -> Result<MemoryWritePath> {
    match requested {
        None => Ok(MemoryWritePath::Additive),
        Some(name) if name == MEMORY_TYPE_PROCEDURAL => Ok(MemoryWritePath::Procedural),
        Some(name) => Err(SearchFirstVectorError::UnsupportedMemoryType {
            requested: name.to_string(),
        }),
    }
}

/// Whether a write is scoped to an agent and not to a user.
///
/// Ports the reference predicate `bool(agent_id) and not bool(user_id)`
/// (`memory/main.py:941`). Python's `bool()` treats an empty string as absent, so
/// a blank id is absent here too: treating `""` as present would frame a
/// user-scoped write as agent knowledge, which is the one outcome this rule
/// exists to prevent.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::is_agent_scoped;
///
/// assert!(is_agent_scoped(Some("agent-1"), None));
/// assert!(is_agent_scoped(Some("agent-1"), Some("  ")));
/// assert!(!is_agent_scoped(Some("agent-1"), Some("user-1")));
/// assert!(!is_agent_scoped(None, None));
/// ```
pub fn is_agent_scoped(agent_id: Option<&str>, user_id: Option<&str>) -> bool {
    is_present(agent_id) && !is_present(user_id)
}

/// Whether an optional identifier carries scope.
fn is_present(value: Option<&str>) -> bool {
    value.is_some_and(|text| !text.trim().is_empty())
}

/// Strip one enclosing fenced block, then any reasoning block.
///
/// Ports the reference `remove_code_blocks` (`memory/utils.py:115-132`),
/// including its exact ordering and anchoring:
///
/// * the fence is matched **first**, anchored to the whole trimmed string, so a
///   response whose fence does not enclose everything is returned untouched;
/// * the tag between the backticks and the newline may hold only ASCII
///   alphanumerics, so ````` ```json `` ``` reads as a tag and ````` ``` `hint` `` ``` does not;
/// * the `<think>…</think>` removal happens **after**, and an unterminated
///   `<think>` is left alone rather than swallowing the rest of the answer.
///
/// Doing the reasoning pass first would let a fence escape the anchored match
/// and be stored with its markers still attached.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::remove_code_blocks;
///
/// assert_eq!(remove_code_blocks("```json\n{\"a\":1}\n```"), "{\"a\":1}");
/// assert_eq!(remove_code_blocks("  plain text  "), "plain text");
/// // An unterminated reasoning block is preserved, not truncated.
/// assert_eq!(remove_code_blocks("<think>unclosed"), "<think>unclosed");
/// ```
pub fn remove_code_blocks(content: &str) -> String {
    let trimmed = content.trim();
    let fenced = strip_enclosing_fence(trimmed).unwrap_or(trimmed);

    strip_reasoning_blocks(fenced).trim().to_string()
}

/// The body of `text` when one fence encloses the whole of it.
///
/// Returns `None` when there is no opening fence, when the tag holds anything
/// outside ASCII alphanumerics, or when the closing fence is absent or not
/// preceded by a newline — each of which makes the reference regex fail to match.
fn strip_enclosing_fence(text: &str) -> Option<&str> {
    let after_fence = text.strip_prefix("```")?;
    let newline = after_fence.find('\n')?;
    let (tag, rest) = after_fence.split_at(newline);

    if !tag.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return None;
    }

    rest.strip_prefix('\n')?.strip_suffix("\n```")
}

/// Remove every terminated `<think>…</think>` span.
///
/// Non-greedy and repeatable, matching the reference `re.sub`. An opening tag
/// with no partner is left in place, which is what the reference regex does too.
fn strip_reasoning_blocks(text: &str) -> String {
    const OPEN: &str = "<think>";
    const CLOSE: &str = "</think>";

    let mut kept = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(open_at) = rest.find(OPEN) {
        kept.push_str(&rest[..open_at]);
        let after_open = &rest[open_at + OPEN.len()..];

        match after_open.find(CLOSE) {
            Some(close_at) => rest = &after_open[close_at + CLOSE.len()..],
            None => {
                kept.push_str(&rest[open_at..]);

                return kept;
            }
        }
    }

    kept.push_str(rest);
    kept
}

/// A procedural summary that has been cleaned and accepted, ready to commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProceduralMemoryPlan {
    /// Cleaned, non-empty summary; becomes the record's canonical text.
    pub summary: String,
    /// Memory type the caller must write beside the summary.
    pub memory_type: &'static str,
}

/// Turn a model answer into a committable procedural summary.
///
/// Applies the reference's two refusals in the reference's order
/// (`memory/main.py:2021-2028`):
///
/// 1. an answer that is empty after cleaning is refused, because committing it
///    would store a memory with no content;
/// 2. an absent metadata carrier is refused, because the record has nowhere to
///    carry its memory type.
///
/// `has_metadata` is supplied by the caller rather than an `Option` of a concrete
/// metadata type, so this plugin stays free of the canonical payload's shape.
///
/// # Errors
///
/// Returns [`SearchFirstVectorError::ProceduralSummaryEmpty`] when nothing
/// survives cleaning, and [`SearchFirstVectorError::ProceduralMetadataRequired`]
/// when `has_metadata` is false.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{
///     plan_procedural_memory, MEMORY_TYPE_PROCEDURAL,
/// };
///
/// let plan = plan_procedural_memory("```\nStep 1 …\n```", true)?;
/// assert_eq!(plan.summary, "Step 1 …");
/// assert_eq!(plan.memory_type, MEMORY_TYPE_PROCEDURAL);
///
/// // An answer that cleans away to nothing is refused.
/// assert!(plan_procedural_memory("<think>only reasoning</think>", true).is_err());
/// // So is a record with nowhere to carry its type.
/// assert!(plan_procedural_memory("Step 1 …", false).is_err());
/// # Ok::<(), sdkwork_memory_plugin_search_first_vector::SearchFirstVectorError>(())
/// ```
pub fn plan_procedural_memory(response: &str, has_metadata: bool) -> Result<ProceduralMemoryPlan> {
    let summary = remove_code_blocks(response);
    if summary.is_empty() {
        return Err(SearchFirstVectorError::ProceduralSummaryEmpty);
    }
    if !has_metadata {
        return Err(SearchFirstVectorError::ProceduralMetadataRequired);
    }

    Ok(ProceduralMemoryPlan {
        summary,
        memory_type: MEMORY_TYPE_PROCEDURAL,
    })
}

/// Build the single prompt string for the procedural-memory path.
///
/// The reference sends a system turn, the transcript, then a closing user turn
/// (`memory/main.py:2002-2007`). This plugin's language-model port takes one
/// prompt, so the three parts are rendered into one string in that order; the
/// closing request stays last, where the model reads it as the instruction.
///
/// `system_prompt_override` reproduces the reference's `prompt` argument: when it
/// carries content it replaces the system prompt entirely, which is how a
/// deployment supplies its own summarisation instruction.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{
///     build_procedural_prompt, ConversationTurn, PROCEDURAL_MEMORY_REQUEST,
/// };
///
/// let prompt = build_procedural_prompt(
///     &[ConversationTurn::new("assistant", "Clicked the export button.")],
///     None,
/// );
/// assert!(prompt.contains("Clicked the export button."));
/// assert!(prompt.ends_with(PROCEDURAL_MEMORY_REQUEST));
/// ```
pub fn build_procedural_prompt(
    turns: &[ConversationTurn],
    system_prompt_override: Option<&str>,
) -> String {
    let system = system_prompt_override
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(PROCEDURAL_MEMORY_SYSTEM_PROMPT);

    let mut prompt = String::with_capacity(system.len() + 512);
    prompt.push_str(system);
    prompt.push_str("\n\n## Execution History\n");

    let mut rendered_any = false;
    for turn in turns {
        if !turn.is_renderable() {
            continue;
        }
        prompt.push_str(&turn.render());
        prompt.push('\n');
        rendered_any = true;
    }

    if !rendered_any {
        // An explicit placeholder rather than an empty section: a model shown a
        // blank history would otherwise summarise nothing and look successful.
        prompt.push_str("(no renderable turns were supplied)\n");
    }

    prompt.push('\n');
    prompt.push_str(PROCEDURAL_MEMORY_REQUEST);

    prompt
}
