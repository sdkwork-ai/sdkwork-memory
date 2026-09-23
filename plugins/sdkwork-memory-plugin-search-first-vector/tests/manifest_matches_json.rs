//! The Rust manifest and the source-controlled JSON manifest must not drift.
//!
//! The runtime manifest is the artifact operators and the plugin registry read,
//! while the Rust builder is what the crate actually reports. If they disagree,
//! the admitted capability set is a fiction.

use std::fs;

use sdkwork_memory_plugin_search_first_vector::{
    build_search_first_vector_index, build_search_first_vector_retriever,
    search_first_vector_manifest, SearchFirstVectorPortBuilder,
};
use sdkwork_memory_spi::{
    MemoryImplementationKind, MemoryIndexKind, MemoryPluginManifest, MemoryPluginRole,
    MemoryProviderKind, MemoryRetrieverKind,
};

#[test]
fn rust_manifest_matches_source_controlled_json_manifest() {
    let json_path = format!("{}/sdkwork.memory.plugin.json", env!("CARGO_MANIFEST_DIR"));
    let json = fs::read_to_string(json_path).expect("read the runtime manifest");
    let json_manifest: MemoryPluginManifest =
        serde_json::from_str(&json).expect("runtime manifest must deserialize into the SPI type");
    let rust_manifest = search_first_vector_manifest();

    assert_eq!(
        serde_json::to_value(&rust_manifest).expect("serialize the Rust manifest"),
        serde_json::to_value(&json_manifest).expect("serialize the JSON manifest"),
        "the Rust and source-controlled manifests must match in full"
    );
    assert!(rust_manifest.validate().is_ok());
}

#[test]
fn manifest_covers_the_search_first_family_and_the_vector_kinds() {
    let manifest = search_first_vector_manifest();

    assert_eq!(
        manifest.implementation_kinds,
        vec![MemoryImplementationKind::SearchFirst]
    );
    assert_eq!(manifest.retriever_kinds, vec![MemoryRetrieverKind::Vector]);
    assert_eq!(manifest.index_kinds, vec![MemoryIndexKind::Vector]);
}

#[test]
fn manifest_declares_the_roles_the_spi_requires_for_its_kinds() {
    let manifest = search_first_vector_manifest();

    // A non-empty `retrieverKinds` requires the retriever role and a
    // `MemoryRetrieverPort` export; the SPI's own `validate` enforces this, and
    // this test pins it so a future edit cannot quietly drop either.
    assert!(manifest.plugin_roles.contains(&MemoryPluginRole::Retriever));
    assert!(manifest.plugin_roles.contains(&MemoryPluginRole::Index));
    assert!(manifest
        .port_exports
        .iter()
        .any(|export| export.port == "MemoryRetrieverPort"));
}

#[test]
fn manifest_declares_no_canonical_ownership() {
    let capabilities = search_first_vector_manifest().capabilities;

    assert!(!capabilities.canonical_store);
    assert!(!capabilities.event_log);
    assert!(!capabilities.candidate_lifecycle);
    assert!(!capabilities.habit_learning);
    assert!(!capabilities.audit_log);
    assert!(!capabilities.outbox_log);
    assert!(!capabilities.retrieval_trace);
    assert!(capabilities.deletion_propagation);
    assert!(capabilities.embedding_required);
}

#[test]
fn manifest_names_the_injected_provider_surface_without_offering_the_provider_role() {
    let manifest = search_first_vector_manifest();

    assert_eq!(
        manifest.provider_kinds,
        vec![
            MemoryProviderKind::LanguageModel,
            MemoryProviderKind::EmbeddingModel,
            MemoryProviderKind::RerankModel,
        ]
    );
    // This plugin integrates providers; it does not offer provider capability.
    assert!(!manifest.plugin_roles.contains(&MemoryPluginRole::Provider));
}

#[test]
fn manifest_holds_no_secret_reference() {
    // Provider access is injected, so the plugin never carries a credential
    // reference that could be logged or leaked.
    assert!(search_first_vector_manifest().secret_refs.is_empty());
}

#[test]
fn manifest_never_returns_stale_hits() {
    let degradation = search_first_vector_manifest().degradation;

    assert!(!degradation.returns_stale_hits);
    assert_eq!(degradation.mode, "fail_closed_when_embedding_port_unbound");
}

#[test]
fn every_exported_port_has_a_ready_builder() {
    let manifest = search_first_vector_manifest();
    let builders: [SearchFirstVectorPortBuilder; 2] = [
        build_search_first_vector_retriever(),
        build_search_first_vector_index(),
    ];

    for builder in builders {
        assert!(builder.ready, "{} must be implemented", builder.port_name);
        assert!(
            !builder.fail_closed,
            "{} is implemented and reports degradation per request instead",
            builder.port_name
        );
        assert!(
            manifest
                .port_exports
                .iter()
                .any(|export| export.port == builder.port_name
                    && export.builder == builder.builder_name),
            "{} must be declared in portExports with its builder name",
            builder.port_name
        );
    }
}
