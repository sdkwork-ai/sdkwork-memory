//! Runtime plugin manifest for the search-first vector plugin.
//!
//! The manifest declares what this plugin owns and, just as importantly, what it
//! does not own. The canonical store, event log, candidate lifecycle, habit
//! store, audit log, outbox, and retrieval trace remain with the native SQL
//! plugin, so every corresponding `capabilities` flag is `false` and no port is
//! exported for it.

use sdkwork_memory_spi::{
    MemoryDeploymentMode, MemoryImplementationKind, MemoryIndexKind, MemoryPluginCapabilities,
    MemoryPluginConformanceContract, MemoryPluginDataClass, MemoryPluginDegradationPolicy,
    MemoryPluginManifest, MemoryPluginMigrationCapabilities, MemoryPluginObservabilityContract,
    MemoryPluginPortExport, MemoryPluginRole, MemoryProviderKind, MemoryRetrieverKind,
};

use crate::error::{INDEX_PORT, RETRIEVER_PORT};

/// Canonical runtime plugin id.
pub const SEARCH_FIRST_VECTOR_PLUGIN_ID: &str = "sdkwork-memory-plugin-search-first-vector";

/// Executable builder exported for [`RETRIEVER_PORT`].
pub const SEARCH_FIRST_VECTOR_RETRIEVER_BUILDER: &str = "build_search_first_vector_retriever";

/// Executable builder exported for [`INDEX_PORT`].
pub const SEARCH_FIRST_VECTOR_INDEX_BUILDER: &str = "build_search_first_vector_index";

/// Conformance suite this plugin is verified against.
pub const SEARCH_FIRST_VECTOR_CONFORMANCE_SUITE: &str = "sdkwork-memory-plugin-conformance";

/// The authoritative runtime manifest for this plugin.
pub fn search_first_vector_manifest() -> MemoryPluginManifest {
    MemoryPluginManifest {
        schema_version: 1,
        kind: "sdkwork.memory.plugin".to_string(),
        plugin_id: SEARCH_FIRST_VECTOR_PLUGIN_ID.to_string(),
        package_name: SEARCH_FIRST_VECTOR_PLUGIN_ID.to_string(),
        display_name: "SDKWork Memory Search-First Vector Plugin".to_string(),
        version: "0.1.0".to_string(),
        owner: "sdkwork-memory".to_string(),
        implementation_kinds: vec![MemoryImplementationKind::SearchFirst],
        plugin_roles: vec![
            MemoryPluginRole::Implementation,
            MemoryPluginRole::Retriever,
            MemoryPluginRole::Index,
        ],
        deployment_modes: vec![
            MemoryDeploymentMode::Server,
            MemoryDeploymentMode::Container,
            MemoryDeploymentMode::Private,
            MemoryDeploymentMode::Test,
        ],
        port_exports: vec![
            MemoryPluginPortExport {
                port: RETRIEVER_PORT.to_string(),
                builder: SEARCH_FIRST_VECTOR_RETRIEVER_BUILDER.to_string(),
            },
            MemoryPluginPortExport {
                port: INDEX_PORT.to_string(),
                builder: SEARCH_FIRST_VECTOR_INDEX_BUILDER.to_string(),
            },
        ],
        provider_kinds: vec![
            MemoryProviderKind::LanguageModel,
            MemoryProviderKind::EmbeddingModel,
            MemoryProviderKind::RerankModel,
        ],
        retriever_kinds: vec![MemoryRetrieverKind::Vector],
        index_kinds: vec![MemoryIndexKind::Vector],
        required_core_version: "0.1.0".to_string(),
        config_schema_ref: None,
        // Provider access is injected as an SPI port, so this plugin holds no
        // credential reference of its own and can never leak one.
        secret_refs: vec![],
        data_classes: vec![
            MemoryPluginDataClass::Internal,
            MemoryPluginDataClass::Tenant,
            MemoryPluginDataClass::Personal,
        ],
        capabilities: MemoryPluginCapabilities {
            canonical_store: false,
            event_log: false,
            candidate_lifecycle: false,
            habit_learning: false,
            retrieval_trace: false,
            deletion_propagation: true,
            audit_log: false,
            outbox_log: false,
            embedding_required: true,
        },
        degradation: MemoryPluginDegradationPolicy {
            mode: "fail_closed_when_embedding_port_unbound".to_string(),
            returns_stale_hits: false,
        },
        migration: MemoryPluginMigrationCapabilities {
            export_supported: false,
            import_supported: false,
            dual_write_supported: false,
            shadow_read_supported: false,
        },
        observability: MemoryPluginObservabilityContract {
            metrics_prefix: "sdkwork_memory_search_first_vector".to_string(),
            redacts_payloads: true,
        },
        conformance: MemoryPluginConformanceContract {
            suite: SEARCH_FIRST_VECTOR_CONFORMANCE_SUITE.to_string(),
            suite_version: "0.1.0".to_string(),
        },
    }
}

/// Executable port builder descriptor.
///
/// `ready` reports whether this plugin implements the port at all. Runtime
/// availability of the retriever additionally depends on an injected embedding
/// provider binding, which is reported per request through
/// `MemoryRetrieverSearchResult::degraded` rather than by silently succeeding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFirstVectorPortBuilder {
    /// SPI port this builder satisfies.
    pub port_name: &'static str,
    /// Exported builder function name.
    pub builder_name: &'static str,
    /// Whether the plugin implements the port.
    pub ready: bool,
    /// Whether the port refuses rather than degrades.
    pub fail_closed: bool,
}

/// Builder for [`RETRIEVER_PORT`].
pub fn build_search_first_vector_retriever() -> SearchFirstVectorPortBuilder {
    SearchFirstVectorPortBuilder {
        port_name: RETRIEVER_PORT,
        builder_name: SEARCH_FIRST_VECTOR_RETRIEVER_BUILDER,
        ready: true,
        fail_closed: false,
    }
}

/// Builder for [`INDEX_PORT`].
pub fn build_search_first_vector_index() -> SearchFirstVectorPortBuilder {
    SearchFirstVectorPortBuilder {
        port_name: INDEX_PORT,
        builder_name: SEARCH_FIRST_VECTOR_INDEX_BUILDER,
        ready: true,
        fail_closed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_valid_and_covers_the_search_first_family() {
        let manifest = search_first_vector_manifest();

        assert!(manifest.validate().is_ok());
        assert_eq!(
            manifest.implementation_kinds,
            vec![MemoryImplementationKind::SearchFirst]
        );
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
    fn manifest_holds_no_secret_reference() {
        assert!(search_first_vector_manifest().secret_refs.is_empty());
    }

    #[test]
    fn every_exported_port_has_a_ready_builder() {
        let manifest = search_first_vector_manifest();
        let builders = [
            build_search_first_vector_retriever(),
            build_search_first_vector_index(),
        ];

        for builder in builders {
            assert!(builder.ready);
            assert!(manifest
                .port_exports
                .iter()
                .any(|export| export.port == builder.port_name
                    && export.builder == builder.builder_name));
        }
    }
}
