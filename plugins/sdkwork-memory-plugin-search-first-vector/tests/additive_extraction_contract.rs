//! Additive extraction and write-planning contracts.
//!
//! mem0 V3's load-bearing write behaviours are covered here: turning turns into
//! self-contained memories attributed to a speaker, linking a new memory to the
//! ones already known, and committing the batch without ever updating or
//! deleting what is already stored.

mod common;

use std::sync::Arc;

use common::ScriptedLanguageProvider;
use sdkwork_memory_plugin_search_first_vector::{
    AdditiveExtractionRequest, AttributedTo, ConversationTurn, DuplicateReason, ExistingMemoryView,
    SearchFirstVectorConfig, SearchFirstVectorError, SearchFirstVectorRuntime,
};

const ONE_MEMORY: &str =
    r#"{"memory":[{"id":"0","text":"User only uses dark mode.","attributed_to":"user"}]}"#;

fn runtime_with_language_model(
    language_model: Arc<ScriptedLanguageProvider>,
) -> SearchFirstVectorRuntime {
    SearchFirstVectorRuntime::new(
        SearchFirstVectorConfig::default(),
        None,
        Some(language_model),
        None,
    )
    .expect("runtime construction")
}

fn request(turns: &[&str]) -> AdditiveExtractionRequest {
    AdditiveExtractionRequest {
        new_messages: turns
            .iter()
            .map(|turn| ConversationTurn::new("user", *turn))
            .collect(),
        ..AdditiveExtractionRequest::default()
    }
}

fn known(memory_id: &str, text: &str) -> ExistingMemoryView {
    ExistingMemoryView {
        memory_id: memory_id.to_string(),
        text: text.to_string(),
    }
}

#[tokio::test]
async fn memories_are_extracted_through_the_language_model_port() {
    let provider = ScriptedLanguageProvider::new(vec![ONE_MEMORY.to_string()]);
    let runtime = runtime_with_language_model(Arc::clone(&provider));

    let report = runtime
        .extract_memories(&request(&["I only use dark mode."]))
        .await
        .expect("extraction");

    assert_eq!(report.memories.len(), 1);
    assert_eq!(report.memories[0].text, "User only uses dark mode.");
    assert_eq!(report.memories[0].attributed_to, Some(AttributedTo::User));
    assert!(report.refused.is_empty());
    assert_eq!(report.truncated, 0);
}

#[tokio::test]
async fn assistant_attributed_memories_are_extracted_too() {
    let provider = ScriptedLanguageProvider::new(vec![
        r#"{"memory":[{"id":"0","text":"User was advised to rest.","attributed_to":"assistant"}]}"#
            .to_string(),
    ]);
    let runtime = runtime_with_language_model(provider);

    let report = runtime
        .extract_memories(&request(&["Should I train today?"]))
        .await
        .expect("extraction");

    assert_eq!(
        report.memories[0].attributed_to,
        Some(AttributedTo::Assistant)
    );
}

#[tokio::test]
async fn the_extraction_prompt_carries_every_turn() {
    let provider = ScriptedLanguageProvider::new(vec![r#"{"memory":[]}"#.to_string()]);
    let runtime = runtime_with_language_model(Arc::clone(&provider));

    runtime
        .extract_memories(&request(&["My name is Ada.", "I keep a garden."]))
        .await
        .expect("extraction");

    let prompts = provider.recorded_prompts();
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].contains("My name is Ada."));
    assert!(prompts[0].contains("I keep a garden."));
}

#[tokio::test]
async fn the_extraction_prompt_offers_the_existing_memory_ids_it_expects_links_to() {
    let provider = ScriptedLanguageProvider::new(vec![r#"{"memory":[]}"#.to_string()]);
    let runtime = runtime_with_language_model(Arc::clone(&provider));

    let request = AdditiveExtractionRequest {
        existing_memories: vec![known("m-1", "User has a dog named Poppy.")],
        ..request(&["We walked Poppy today."])
    };

    runtime
        .extract_memories(&request)
        .await
        .expect("extraction");

    let prompts = provider.recorded_prompts();
    assert!(
        prompts[0].contains("m-1"),
        "the model can only link to ids the caller actually offered"
    );
    assert!(prompts[0].contains("User has a dog named Poppy."));
}

#[tokio::test]
async fn extraction_short_circuits_without_calling_the_provider_for_empty_turns() {
    let provider = ScriptedLanguageProvider::new(vec![]);
    let runtime = runtime_with_language_model(Arc::clone(&provider));

    let report = runtime
        .extract_memories(&request(&["   "]))
        .await
        .expect("no turns is not an error");

    assert!(report.is_empty());
    assert!(
        provider.recorded_prompts().is_empty(),
        "an empty turn list must not spend a provider request"
    );
}

#[tokio::test]
async fn extraction_refuses_when_no_language_model_is_bound() {
    let runtime = SearchFirstVectorRuntime::without_providers(SearchFirstVectorConfig::default())
        .expect("runtime");

    let error = runtime
        .extract_memories(&request(&["anything"]))
        .await
        .expect_err("extraction without a language model must fail closed");

    assert!(matches!(
        error,
        SearchFirstVectorError::RequiredPortMissing {
            port: "LanguageModelPort"
        }
    ));
}

#[tokio::test]
async fn extraction_reports_a_provider_refusal() {
    let provider = ScriptedLanguageProvider::new(vec![]).failing("model offline");
    let runtime = runtime_with_language_model(provider);

    let error = runtime
        .extract_memories(&request(&["anything"]))
        .await
        .expect_err("a provider refusal must surface");

    assert!(matches!(
        error,
        SearchFirstVectorError::ProviderCallFailed {
            port: "LanguageModelPort",
            ..
        }
    ));
}

#[tokio::test]
async fn extraction_refuses_an_unparseable_answer() {
    let provider =
        ScriptedLanguageProvider::new(vec!["Here are the memories I found.".to_string()]);
    let runtime = runtime_with_language_model(provider);

    let error = runtime
        .extract_memories(&request(&["anything"]))
        .await
        .expect_err("prose must be refused, not guessed at");

    assert!(matches!(
        error,
        SearchFirstVectorError::MemoryExtractionUnparseable { .. }
    ));
}

#[tokio::test]
async fn extraction_refuses_an_answer_still_shaped_like_the_retired_envelope() {
    let provider =
        ScriptedLanguageProvider::new(vec![r#"{"facts":[{"object":"dark mode"}]}"#.to_string()]);
    let runtime = runtime_with_language_model(provider);

    let error = runtime
        .extract_memories(&request(&["anything"]))
        .await
        .expect_err("the envelope key is `memory`, not `facts`");

    assert!(matches!(
        error,
        SearchFirstVectorError::MemoryExtractionUnparseable { .. }
    ));
}

#[tokio::test]
async fn the_write_plan_commits_and_links_in_one_step() {
    let provider = ScriptedLanguageProvider::new(vec![
        r#"{"memory":[
            {"id":"0","text":"User walked Poppy today.","attributed_to":"user","linked_memory_ids":["m-1"]},
            {"id":"1","text":"User has a dog named Poppy.","attributed_to":"user"}
        ]}"#
        .to_string(),
    ]);
    // The second emission duplicates a memory the caller already knows.
    let runtime = runtime_with_language_model(provider);

    let plan = runtime
        .plan_write(
            &request(&["We walked Poppy today."]),
            &[known("m-1", "User has a dog named Poppy.")],
        )
        .await
        .expect("write plan");

    assert_eq!(plan.report.memories.len(), 2);
    assert_eq!(plan.plan.additions.len(), 1);
    assert_eq!(plan.plan.additions[0].text, "User walked Poppy today.");
    assert_eq!(plan.plan.additions[0].linked_memory_ids, vec!["m-1"]);
    assert_eq!(
        plan.plan.additions[0].content_hash,
        sdkwork_memory_plugin_search_first_vector::content_hash("User walked Poppy today.")
    );
    assert_eq!(plan.plan.suppressed.len(), 1);
    assert_eq!(
        plan.plan.suppressed[0].reason,
        DuplicateReason::KnownMemory {
            memory_id: "m-1".to_string()
        }
    );
}

#[tokio::test]
async fn the_write_plan_reports_a_link_the_caller_never_offered() {
    let provider = ScriptedLanguageProvider::new(vec![
        r#"{"memory":[{"id":"0","text":"User walked Poppy today.","attributed_to":"user","linked_memory_ids":["m-ghost"]}]}"#
            .to_string(),
    ]);
    let runtime = runtime_with_language_model(provider);

    let plan = runtime
        .plan_write(
            &request(&["We walked Poppy today."]),
            &[known("m-1", "User has a dog named Poppy.")],
        )
        .await
        .expect("write plan");

    assert!(plan.plan.additions[0].linked_memory_ids.is_empty());
    assert_eq!(plan.plan.unverified_links.len(), 1);
    assert_eq!(plan.plan.unverified_links[0].linked_memory_id, "m-ghost");
}

#[tokio::test]
async fn the_write_plan_never_updates_or_deletes() {
    // A batch that would have been an UPDATE or a DELETE under the retired
    // four-operation contract is now simply a new addition: the additive path
    // has no other operation to choose.
    let provider = ScriptedLanguageProvider::new(vec![
        r#"{"memory":[{"id":"0","text":"User switched to light mode.","attributed_to":"user","linked_memory_ids":["m-1"]}]}"#
            .to_string(),
    ]);
    let runtime = runtime_with_language_model(provider);

    let plan = runtime
        .plan_write(
            &request(&["Actually I use light mode now."]),
            &[known("m-1", "User only uses dark mode.")],
        )
        .await
        .expect("write plan");

    assert_eq!(plan.plan.additions.len(), 1);
    assert_eq!(plan.plan.additions[0].linked_memory_ids, vec!["m-1"]);
    assert!(plan.plan.suppressed.is_empty());
}

#[tokio::test]
async fn the_write_plan_withholds_one_bad_emission_without_losing_the_rest() {
    let provider = ScriptedLanguageProvider::new(vec![r#"{"memory":[
            {"id":"0","text":"User keeps a garden.","attributed_to":"robot"},
            {"id":"1","text":"User drinks tea.","attributed_to":"user"}
        ]}"#
    .to_string()]);
    let runtime = runtime_with_language_model(provider);

    let plan = runtime
        .plan_write(&request(&["I keep a garden and drink tea."]), &[])
        .await
        .expect("write plan");

    assert_eq!(plan.report.memories.len(), 1);
    assert_eq!(plan.report.refused.len(), 1);
    assert_eq!(
        plan.report.refused[0].rejection.label(),
        "unrecognised_attribution"
    );
    assert_eq!(plan.plan.additions.len(), 1);
    assert_eq!(plan.plan.additions[0].text, "User drinks tea.");
}

#[tokio::test]
async fn the_write_plan_refuses_when_no_language_model_is_bound() {
    let runtime = SearchFirstVectorRuntime::without_providers(SearchFirstVectorConfig::default())
        .expect("runtime");

    let error = runtime
        .plan_write(&request(&["anything"]), &[])
        .await
        .expect_err("the additive write path needs a language model");

    assert!(matches!(
        error,
        SearchFirstVectorError::RequiredPortMissing {
            port: "LanguageModelPort"
        }
    ));
}

#[test]
fn planning_is_available_without_any_provider() {
    let runtime = SearchFirstVectorRuntime::without_providers(SearchFirstVectorConfig::default())
        .expect("runtime");

    let extracted = vec![sdkwork_memory_plugin_search_first_vector::ExtractedMemory {
        index: "0".to_string(),
        text: "User drinks tea.".to_string(),
        attributed_to: Some(AttributedTo::User),
        linked_memory_ids: Vec::new(),
    }];

    let plan = runtime.plan_additions(&extracted, &[]);

    assert_eq!(plan.additions.len(), 1);
}
