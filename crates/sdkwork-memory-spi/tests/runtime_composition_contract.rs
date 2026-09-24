use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_memory_spi::{
    AssembleMemoryContextCommand, MemoryContextAssemblerPort, MemoryContextPackDraft,
    MemoryCoreRuntime, MemoryDeploymentMode, MemoryExecutablePluginRuntime,
    MemoryGovernanceAccessPort, MemoryImplementationKind, MemoryPluginManifest, MemoryPluginPorts,
    MemoryPluginRegistry, MemoryRetrieverPort, MemoryRetrieverResult, MemoryRuntimeProfileMetadata,
    MemoryScopeContext, MemorySpiError, MemorySpiResult, ResolveMemorySpaceGovernanceQuery,
    RetrieveMemoryCandidatesCommand, MemorySensitivityReadScope};

struct NamedRetriever {
    code: &'static str,
}

#[async_trait]
impl MemoryRetrieverPort for NamedRetriever {
    fn retriever_code(&self) -> &str {
        self.code
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Ok(MemoryRetrieverResult {
            memory_ids: Vec::new(),
        })
    }
}

struct UnscopedContextAssembler;

#[async_trait]
impl MemoryContextAssemblerPort for UnscopedContextAssembler {
    async fn assemble(
        &self,
        command: AssembleMemoryContextCommand,
    ) -> MemorySpiResult<MemoryContextPackDraft> {
        Ok(MemoryContextPackDraft {
            memory_ids: command.memory_ids,
            context_text: String::new(),
        })
    }
}

struct UnscopedGovernance;

#[async_trait]
impl MemoryGovernanceAccessPort for UnscopedGovernance {}

#[tokio::test]
async fn scoped_port_methods_fail_closed_until_an_implementation_overrides_them() {
    let scope = MemoryScopeContext::for_test(1, 10);
    let retriever = NamedRetriever {
        code: "unscoped-retriever",
    };
    let retrieval_error = MemoryRetrieverPort::retrieve_scoped(
        &retriever,
        scope.clone(),
        RetrieveMemoryCandidatesCommand {
            query: "isolated".to_string(),
            read_scope: MemorySensitivityReadScope::Public,
            limit: sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        retrieval_error,
        MemorySpiError::PortOperationFailed { port, message }
            if port == "MemoryRetrieverPort" && message.contains("scope-aware retrieval")
    ));

    let assembly_error = MemoryContextAssemblerPort::assemble_scoped(
        &UnscopedContextAssembler,
        scope,
        AssembleMemoryContextCommand {
            memory_ids: vec!["memory-1".to_string()],
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        assembly_error,
        MemorySpiError::PortOperationFailed { port, message }
            if port == "MemoryContextAssemblerPort"
                && message.contains("scope-aware context assembly")
    ));

    assert!(!UnscopedGovernance.supports_bounded_governance_access());
    let governance_error = MemoryGovernanceAccessPort::resolve_space_governance(
        &UnscopedGovernance,
        ResolveMemorySpaceGovernanceQuery {
            scope: MemoryScopeContext::for_test(1, 10),
            actor: None,
            capability_code: None,
            fact_limit: 1,
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(
        governance_error,
        MemorySpiError::PortOperationFailed { port, message }
            if port == "MemoryGovernanceAccessPort"
                && message.contains("bounded tenant-scoped governance")
    ));
}

#[test]
fn registry_and_core_runtime_preserve_typed_executable_port_ownership() {
    let plugin_id = "sdkwork-memory-plugin-reference-profiles";
    let executable = MemoryExecutablePluginRuntime::new(MemoryPluginPorts::new().with_retriever(
        Arc::new(NamedRetriever {
            code: "reference-keyword",
        }),
    ));
    let mut registry = MemoryPluginRegistry::default();
    registry
        .register_executable(
            MemoryPluginManifest::reference_profiles_for_test(),
            executable,
        )
        .unwrap();

    assert!(registry.has_executable_runtime(plugin_id));
    assert!(registry
        .require_executable_runtime(plugin_id)
        .unwrap()
        .has_port("MemoryRetrieverPort"));

    let mut runtime = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "hybrid-platform-phase1".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: plugin_id.to_string(),
        deployment_mode: MemoryDeploymentMode::Test,
    });
    runtime
        .bind_port(
            plugin_id,
            "MemoryRetrieverPort",
            registry.require_executable_runtime(plugin_id).unwrap(),
        )
        .unwrap();

    assert_eq!(runtime.profile().profile_id, "hybrid-platform-phase1");
    assert_eq!(runtime.port_owner("MemoryRetrieverPort"), Some(plugin_id));
    assert!(runtime.has_port("MemoryRetrieverPort"));
    assert_eq!(
        runtime.retriever().unwrap().retriever_code(),
        "reference-keyword"
    );
}

#[test]
fn core_runtime_rejects_missing_or_duplicate_executable_ports() {
    let plugin_id = "sdkwork-memory-plugin-reference-profiles";
    let empty = MemoryExecutablePluginRuntime::default();
    let retriever = MemoryExecutablePluginRuntime::new(MemoryPluginPorts::new().with_retriever(
        Arc::new(NamedRetriever {
            code: "reference-keyword",
        }),
    ));
    let profile = MemoryRuntimeProfileMetadata {
        profile_id: "hybrid-platform-phase1".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: plugin_id.to_string(),
        deployment_mode: MemoryDeploymentMode::Test,
    };

    let mut runtime = MemoryCoreRuntime::new(profile);
    let error = runtime
        .bind_port(plugin_id, "MemoryRetrieverPort", &empty)
        .unwrap_err();
    assert!(matches!(
        error,
        MemorySpiError::ExecutablePortMissing { plugin_id: id, port }
            if id == plugin_id && port == "MemoryRetrieverPort"
    ));

    runtime
        .bind_port(plugin_id, "MemoryRetrieverPort", &retriever)
        .unwrap();
    let error = runtime
        .bind_port(plugin_id, "MemoryRetrieverPort", &retriever)
        .unwrap_err();
    assert!(matches!(
        error,
        MemorySpiError::ExecutablePortAlreadyBound(port)
            if port == "MemoryRetrieverPort"
    ));
}
