//! Procedural-memory and agent-context contracts.
//!
//! Every assertion here names a reference behaviour: the two refusals, the exact
//! condition that appends the agent block, and the fence/reasoning stripping
//! order all come from specific lines of the mem0 checkout under `external/`.

mod common;

use std::sync::Arc;

use common::ScriptedLanguageProvider;
use sdkwork_memory_plugin_search_first_vector::{
    build_additive_extraction_prompt, build_procedural_prompt, is_agent_scoped,
    plan_procedural_memory, remove_code_blocks, resolve_write_path, AdditiveExtractionRequest,
    ConversationTurn, MemoryWritePath, SearchFirstVectorConfig, SearchFirstVectorError,
    SearchFirstVectorRuntime, ADDITIVE_EXTRACTION_PROMPT, AGENT_CONTEXT_SUFFIX,
    LANGUAGE_MODEL_PORT, MEMORY_TYPE_EPISODIC, MEMORY_TYPE_PROCEDURAL, MEMORY_TYPE_SEMANTIC,
    PROCEDURAL_MEMORY_REQUEST, PROCEDURAL_MEMORY_SYSTEM_PROMPT,
};

fn config() -> SearchFirstVectorConfig {
    SearchFirstVectorConfig {
        use_rerank_when_bound: false,
        ..SearchFirstVectorConfig::default()
    }
}

fn runtime_with_language(language: Arc<ScriptedLanguageProvider>) -> SearchFirstVectorRuntime {
    SearchFirstVectorRuntime::new(config(), None, Some(language), None).expect("runtime")
}

fn history() -> Vec<ConversationTurn> {
    vec![
        ConversationTurn::new("user", "Export the report."),
        ConversationTurn::new("assistant", "Clicked the export button."),
    ]
}

// ---------------------------------------------------------------------------
// Write-path routing
// ---------------------------------------------------------------------------

#[test]
fn an_absent_memory_type_takes_the_additive_path() {
    assert_eq!(
        resolve_write_path(None).expect("no type is routable"),
        MemoryWritePath::Additive
    );
}

#[test]
fn the_procedural_type_takes_the_procedural_path() {
    assert_eq!(
        resolve_write_path(Some(MEMORY_TYPE_PROCEDURAL)).expect("procedural is routable"),
        MemoryWritePath::Procedural
    );
}

#[test]
fn every_other_memory_type_is_refused_by_name() {
    // Two of these are enumerated by the reference and still not creatable
    // (`main.py:831-837`); the rest are simply wrong. All must be refused rather
    // than routed somewhere that looks plausible.
    for requested in [
        MEMORY_TYPE_SEMANTIC,
        MEMORY_TYPE_EPISODIC,
        "",
        "fact",
        " Procedural_memory",
        "PROCEDURAL_MEMORY",
    ] {
        let error = resolve_write_path(Some(requested))
            .expect_err("an unsupported memory type must be refused");
        match error {
            SearchFirstVectorError::UnsupportedMemoryType { requested: quoted } => {
                assert_eq!(
                    quoted, requested,
                    "the refusal must quote what was compared"
                );
            }
            other => panic!("expected an unsupported-type refusal, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Agent scoping
// ---------------------------------------------------------------------------

#[test]
fn agent_scoping_needs_an_agent_and_no_user() {
    assert!(is_agent_scoped(Some("agent-1"), None));
    // A blank id cannot scope anything, and Python's `bool("")` is false, so a
    // blank user id must not cancel the agent framing.
    assert!(is_agent_scoped(Some("agent-1"), Some("   ")));
    assert!(!is_agent_scoped(None, None));
    assert!(!is_agent_scoped(Some("  "), None));
}

#[test]
fn a_named_user_cancels_the_agent_framing() {
    // A write addressed to both a user and an agent is a user's memory.
    assert!(!is_agent_scoped(Some("agent-1"), Some("user-1")));
    assert!(!is_agent_scoped(None, Some("user-1")));
}

#[test]
fn the_agent_block_is_appended_only_for_agent_scoped_writes() {
    let base = AdditiveExtractionRequest {
        new_messages: history(),
        ..AdditiveExtractionRequest::default()
    };

    let user_scoped = build_additive_extraction_prompt(&base);
    assert!(!user_scoped.contains(AGENT_CONTEXT_SUFFIX));

    let agent_scoped = build_additive_extraction_prompt(&AdditiveExtractionRequest {
        agent_id: Some("agent-1".to_string()),
        ..base.clone()
    });
    assert_eq!(
        agent_scoped.matches(AGENT_CONTEXT_SUFFIX).count(),
        1,
        "the block is appended exactly once"
    );

    // The contract prompt carries a legend that names every section heading, so
    // a bare `find("## Summary")` would land on the legend and prove nothing.
    // Ordering has to be read from the appended region onwards.
    let appended = &agent_scoped[ADDITIVE_EXTRACTION_PROMPT.len()..];
    assert_eq!(
        appended.matches(AGENT_CONTEXT_SUFFIX).count(),
        1,
        "the block is appended, not baked into the contract prompt"
    );
    let block_at = appended.find(AGENT_CONTEXT_SUFFIX).expect("block present");
    let summary_at = appended.find("\n\n## Summary\n").expect("section present");
    assert!(
        block_at < summary_at,
        "the block precedes the rendered input sections"
    );

    let both_scoped = build_additive_extraction_prompt(&AdditiveExtractionRequest {
        agent_id: Some("agent-1".to_string()),
        user_id: Some("user-1".to_string()),
        ..base
    });
    assert!(!both_scoped.contains(AGENT_CONTEXT_SUFFIX));
}

// ---------------------------------------------------------------------------
// Fence and reasoning stripping
// ---------------------------------------------------------------------------

#[test]
fn a_fence_that_encloses_everything_is_stripped() {
    assert_eq!(remove_code_blocks("```json\n{\"a\":1}\n```"), "{\"a\":1}");
    assert_eq!(remove_code_blocks("```\nplain\n```"), "plain");
    // Untagged and surrounded by whitespace, which the reference trims first.
    assert_eq!(remove_code_blocks("  \n```\nbody\n```\n  "), "body");
}

#[test]
fn a_fence_that_does_not_enclose_everything_is_left_alone() {
    // The reference anchors the pattern to the whole trimmed string, so a fence
    // with text outside it is not a wrapper and keeps its markers.
    assert_eq!(
        remove_code_blocks("prefix\n```\nbody\n```"),
        "prefix\n```\nbody\n```"
    );
    assert_eq!(
        remove_code_blocks("```\nbody\n```\nsuffix"),
        "```\nbody\n```\nsuffix"
    );
}

#[test]
fn a_non_alphanumeric_tag_is_not_a_tag() {
    // `[a-zA-Z0-9]*` cannot span a backtick or a space, so the pattern fails.
    assert_eq!(remove_code_blocks("``` j\nbody\n```"), "``` j\nbody\n```");
    assert_eq!(remove_code_blocks("```a`b\nbody\n```"), "```a`b\nbody\n```");
}

#[test]
fn reasoning_blocks_are_removed_after_the_fence_pass() {
    assert_eq!(remove_code_blocks("<think>why</think>body"), "body");
    assert_eq!(
        remove_code_blocks("<think>a</think>keep<think>b</think>"),
        "keep"
    );
    // An unterminated block is preserved rather than swallowing the answer.
    assert_eq!(
        remove_code_blocks("keep<think>unclosed"),
        "keep<think>unclosed"
    );
}

#[test]
fn the_fence_pass_runs_before_the_reasoning_pass() {
    // The ordering is observable when the reasoning block sits flush against the
    // fence. Stripping the fence first leaves the reasoning pass to run on what
    // has then become a bare body, so the markers are already gone. Running the
    // reasoning pass first hands the fence pass a string that now *starts* with
    // a fence, and the markers are stripped instead of surviving.
    assert_eq!(
        remove_code_blocks("<think>a</think>```\nbody\n```"),
        "```\nbody\n```"
    );

    // With a newline in between, the two orders agree: the reasoning pass leaves
    // a leading newline, which keeps the fence pass from matching at all. Kept
    // as the reference's own answer for this shape, not as a discriminator.
    assert_eq!(
        remove_code_blocks("<think>a</think>\n```\nbody\n```"),
        "```\nbody\n```"
    );

    // And a reasoning block *inside* a fence is removed by the second pass.
    assert_eq!(remove_code_blocks("```\n<think>a</think>body\n```"), "body");
}

// ---------------------------------------------------------------------------
// Planning the record
// ---------------------------------------------------------------------------

#[test]
fn a_clean_answer_becomes_a_procedural_record() {
    let plan = plan_procedural_memory("```\nStep 1: clicked export.\n```", true)
        .expect("a clean answer is committable");

    assert_eq!(plan.summary, "Step 1: clicked export.");
    assert_eq!(plan.memory_type, MEMORY_TYPE_PROCEDURAL);
}

#[test]
fn an_answer_that_cleans_away_to_nothing_is_refused() {
    for answer in [
        "",
        "   ",
        "<think>only reasoning</think>",
        "```\n<think>x</think>\n```",
    ] {
        assert_eq!(
            plan_procedural_memory(answer, true),
            Err(SearchFirstVectorError::ProceduralSummaryEmpty),
            "answer {answer:?} must be refused rather than stored empty"
        );
    }
}

#[test]
fn a_record_with_nowhere_to_carry_its_type_is_refused() {
    assert_eq!(
        plan_procedural_memory("Step 1", false),
        Err(SearchFirstVectorError::ProceduralMetadataRequired)
    );
}

#[test]
fn the_empty_answer_refusal_is_decided_before_the_metadata_one() {
    // The reference checks the answer first (`main.py:2021` before `:2027`), so
    // an answer that is both empty and metadata-less reports the empty answer.
    assert_eq!(
        plan_procedural_memory("<think>x</think>", false),
        Err(SearchFirstVectorError::ProceduralSummaryEmpty)
    );
}

// ---------------------------------------------------------------------------
// Prompt assembly
// ---------------------------------------------------------------------------

#[test]
fn the_procedural_prompt_ends_with_the_closing_request() {
    let prompt = build_procedural_prompt(&history(), None);

    assert!(prompt.starts_with(PROCEDURAL_MEMORY_SYSTEM_PROMPT));
    assert!(prompt.contains("Clicked the export button."));
    assert!(prompt.ends_with(PROCEDURAL_MEMORY_REQUEST));
}

#[test]
fn an_supplied_instruction_replaces_the_system_prompt() {
    let prompt = build_procedural_prompt(&history(), Some("  Summarise tersely.  "));

    assert!(prompt.starts_with("Summarise tersely."));
    assert!(!prompt.contains(PROCEDURAL_MEMORY_SYSTEM_PROMPT));
    // A blank override is not an override.
    assert!(build_procedural_prompt(&history(), Some("   "))
        .starts_with(PROCEDURAL_MEMORY_SYSTEM_PROMPT));
}

#[test]
fn unrenderable_turns_are_skipped_and_an_empty_history_says_so() {
    let prompt = build_procedural_prompt(
        &[
            ConversationTurn::new("user", "   "),
            ConversationTurn::new("assistant", "kept"),
        ],
        None,
    );
    assert!(prompt.contains("assistant: kept"));
    assert!(!prompt.contains("user:"));

    // An empty history is stated rather than rendered as a blank section the
    // model would summarise into nothing while still looking successful.
    let empty = build_procedural_prompt(&[], None);
    assert!(empty.contains("(no renderable turns were supplied)"));
}

// ---------------------------------------------------------------------------
// Runtime wiring
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_runtime_returns_the_planned_record() {
    let language =
        ScriptedLanguageProvider::new(vec!["```\nStep 1: clicked export.\n```".to_string()]);
    let runtime = runtime_with_language(Arc::clone(&language));

    let plan = runtime
        .plan_procedural_memory(&history(), None, true)
        .await
        .expect("a scripted answer is planned");

    assert_eq!(plan.summary, "Step 1: clicked export.");
    assert_eq!(plan.memory_type, MEMORY_TYPE_PROCEDURAL);

    // The transcript reached the provider, and the prompt closed with the request.
    let sent = language.recorded_prompts();
    assert_eq!(sent.len(), 1);
    assert!(sent[0].contains("Clicked the export button."));
    assert!(sent[0].ends_with(PROCEDURAL_MEMORY_REQUEST));
}

#[tokio::test]
async fn the_runtime_refuses_without_a_language_model() {
    let runtime = SearchFirstVectorRuntime::without_providers(config()).expect("runtime");

    let error = runtime
        .plan_procedural_memory(&history(), None, true)
        .await
        .expect_err("no language model is bound");

    assert_eq!(
        error,
        SearchFirstVectorError::RequiredPortMissing {
            port: LANGUAGE_MODEL_PORT
        }
    );
}

#[tokio::test]
async fn a_provider_refusal_is_reported_on_the_language_model_port() {
    let language = ScriptedLanguageProvider::new(Vec::new()).failing("model offline");
    let runtime = runtime_with_language(language);

    let error = runtime
        .plan_procedural_memory(&history(), None, true)
        .await
        .expect_err("the provider refused");

    match error {
        SearchFirstVectorError::ProviderCallFailed {
            port,
            provider,
            source,
        } => {
            assert_eq!(port, LANGUAGE_MODEL_PORT);
            assert_eq!(provider, "stub-language-model");
            assert!(matches!(
                source,
                sdkwork_memory_spi::MemorySpiError::PortOperationFailed { .. }
            ));
        }
        other => panic!("expected a provider call failure, got {other:?}"),
    }
}
