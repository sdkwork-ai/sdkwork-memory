use serde::{Deserialize, Serialize};

use crate::MemorySpiError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginManifest {
    pub schema_version: u32,
    pub kind: String,
    pub plugin_id: String,
    pub package_name: String,
    pub display_name: String,
    pub version: String,
    pub owner: String,
    pub implementation_kinds: Vec<MemoryImplementationKind>,
    pub plugin_roles: Vec<MemoryPluginRole>,
    pub deployment_modes: Vec<MemoryDeploymentMode>,
    pub port_exports: Vec<MemoryPluginPortExport>,
    pub provider_kinds: Vec<MemoryProviderKind>,
    pub retriever_kinds: Vec<MemoryRetrieverKind>,
    pub index_kinds: Vec<MemoryIndexKind>,
    pub required_core_version: String,
    #[serde(default)]
    pub config_schema_ref: Option<String>,
    #[serde(default)]
    pub secret_refs: Vec<String>,
    #[serde(default)]
    pub data_classes: Vec<MemoryPluginDataClass>,
    pub capabilities: MemoryPluginCapabilities,
    pub degradation: MemoryPluginDegradationPolicy,
    pub migration: MemoryPluginMigrationCapabilities,
    pub observability: MemoryPluginObservabilityContract,
    pub conformance: MemoryPluginConformanceContract,
}

impl MemoryPluginManifest {
    pub fn validate(&self) -> Result<(), MemorySpiError> {
        if self.schema_version != 1 {
            return Err(MemorySpiError::ManifestInvalid(
                "schemaVersion must be 1".to_string(),
            ));
        }

        if self.kind != "sdkwork.memory.plugin" {
            return Err(MemorySpiError::ManifestInvalid(
                "kind must be sdkwork.memory.plugin".to_string(),
            ));
        }

        if !self.plugin_id.starts_with("sdkwork-memory-plugin-") {
            return Err(MemorySpiError::ManifestInvalid(
                "pluginId must start with sdkwork-memory-plugin-".to_string(),
            ));
        }

        for path_like in [&self.plugin_id, &self.package_name] {
            if path_like.contains(".sdkwork/plugins") || path_like.contains(".sdkwork\\plugins") {
                return Err(MemorySpiError::ManifestInvalid(
                    "runtime plugins must not live under .sdkwork/plugins".to_string(),
                ));
            }
        }

        if self.port_exports.is_empty() {
            return Err(MemorySpiError::ManifestInvalid(
                "portExports must not be empty".to_string(),
            ));
        }

        if self.deployment_modes.is_empty() {
            return Err(MemorySpiError::ManifestInvalid(
                "deploymentModes must not be empty".to_string(),
            ));
        }

        let mut declared_ports = std::collections::HashSet::new();
        for export in &self.port_exports {
            if export.port.trim().is_empty() || export.builder.trim().is_empty() {
                return Err(MemorySpiError::ManifestInvalid(
                    "portExports must declare a non-empty port and executable builder".to_string(),
                ));
            }
            if !declared_ports.insert(&export.port) {
                return Err(MemorySpiError::ManifestInvalid(format!(
                    "portExports contains duplicate port {}",
                    export.port
                )));
            }
        }

        for secret_ref in &self.secret_refs {
            if looks_like_secret_value(secret_ref) {
                return Err(MemorySpiError::ManifestInvalid(
                    "secretRefs must contain references, not literal secret values".to_string(),
                ));
            }
        }

        if self
            .implementation_kinds
            .contains(&MemoryImplementationKind::NativeSql)
            && self.capabilities.embedding_required
        {
            return Err(MemorySpiError::ManifestInvalid(
                "native_sql must not require embeddings".to_string(),
            ));
        }

        self.require_capability_port(
            self.capabilities.canonical_store,
            "MemoryRecordStorePort",
            "canonicalStore",
        )?;
        self.require_capability_port(
            self.capabilities.event_log,
            "MemoryEventStorePort",
            "eventLog",
        )?;
        self.require_capability_port(
            self.capabilities.audit_log,
            "MemoryAuditStorePort",
            "auditLog",
        )?;
        self.require_capability_port(
            self.capabilities.outbox_log,
            "MemoryOutboxStorePort",
            "outboxLog",
        )?;
        self.require_capability_port(
            self.capabilities.candidate_lifecycle,
            "MemoryCandidateStorePort",
            "candidateLifecycle",
        )?;
        self.require_capability_port(
            self.capabilities.habit_learning,
            "MemoryHabitStorePort",
            "habitLearning",
        )?;
        self.require_capability_port(
            self.capabilities.retrieval_trace,
            "MemoryRetrievalTraceStorePort",
            "retrievalTrace",
        )?;
        if !self.retriever_kinds.is_empty() {
            if !self.plugin_roles.contains(&MemoryPluginRole::Retriever) {
                return Err(MemorySpiError::ManifestInvalid(
                    "retrieverKinds requires the retriever plugin role".to_string(),
                ));
            }
            if !self
                .port_exports
                .iter()
                .any(|export| export.port == "MemoryRetrieverPort")
            {
                return Err(MemorySpiError::ManifestInvalid(
                    "retrieverKinds requires MemoryRetrieverPort".to_string(),
                ));
            }
        }

        Ok(())
    }

    fn require_capability_port(
        &self,
        capability_enabled: bool,
        required_port: &str,
        capability_name: &str,
    ) -> Result<(), MemorySpiError> {
        if capability_enabled
            && !self
                .port_exports
                .iter()
                .any(|export| export.port == required_port)
        {
            return Err(MemorySpiError::ManifestInvalid(format!(
                "{capability_name}=true requires {required_port}"
            )));
        }

        Ok(())
    }

    pub fn native_sql_baseline() -> Self {
        Self {
            schema_version: 1,
            kind: "sdkwork.memory.plugin".to_string(),
            plugin_id: "sdkwork-memory-plugin-native-sql".to_string(),
            package_name: "sdkwork-memory-plugin-native-sql".to_string(),
            display_name: "SDKWork Memory Native SQL Plugin".to_string(),
            version: "0.1.0".to_string(),
            owner: "sdkwork-memory".to_string(),
            implementation_kinds: vec![
                MemoryImplementationKind::NativeSql,
                MemoryImplementationKind::LocalEmbedded,
            ],
            plugin_roles: vec![
                MemoryPluginRole::Implementation,
                MemoryPluginRole::Store,
                MemoryPluginRole::Retriever,
            ],
            deployment_modes: vec![
                MemoryDeploymentMode::Server,
                MemoryDeploymentMode::Container,
                MemoryDeploymentMode::Private,
                MemoryDeploymentMode::Local,
                MemoryDeploymentMode::Test,
            ],
            port_exports: vec![
                MemoryPluginPortExport {
                    port: "MemoryRecordStorePort".to_string(),
                    builder: "build_native_sql_record_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryEventStorePort".to_string(),
                    builder: "build_native_sql_event_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryAuditStorePort".to_string(),
                    builder: "build_native_sql_audit_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryOutboxStorePort".to_string(),
                    builder: "build_native_sql_outbox_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryCandidateStorePort".to_string(),
                    builder: "build_native_sql_candidate_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryHabitStorePort".to_string(),
                    builder: "build_native_sql_habit_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryRetrievalTraceStorePort".to_string(),
                    builder: "build_native_sql_retrieval_trace_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryGovernanceAccessPort".to_string(),
                    builder: "build_native_sql_governance_access".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemorySpaceStorePort".to_string(),
                    builder: "build_native_sql_space_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryRetrieverPort".to_string(),
                    builder: "build_native_sql_retriever".to_string(),
                },
            ],
            provider_kinds: vec![],
            retriever_kinds: vec![
                MemoryRetrieverKind::Sql,
                MemoryRetrieverKind::Keyword,
                MemoryRetrieverKind::Dictionary,
                MemoryRetrieverKind::Time,
                MemoryRetrieverKind::Event,
            ],
            index_kinds: vec![],
            required_core_version: "0.1.0".to_string(),
            config_schema_ref: None,
            secret_refs: vec![],
            data_classes: vec![
                MemoryPluginDataClass::Tenant,
                MemoryPluginDataClass::Personal,
            ],
            capabilities: MemoryPluginCapabilities {
                canonical_store: true,
                event_log: true,
                candidate_lifecycle: true,
                habit_learning: true,
                retrieval_trace: true,
                deletion_propagation: true,
                audit_log: true,
                outbox_log: true,
                embedding_required: false,
            },
            degradation: MemoryPluginDegradationPolicy {
                mode: "fail_required_degrade_optional".to_string(),
                returns_stale_hits: false,
            },
            migration: MemoryPluginMigrationCapabilities {
                export_supported: true,
                import_supported: true,
                dual_write_supported: false,
                shadow_read_supported: true,
            },
            observability: MemoryPluginObservabilityContract {
                metrics_prefix: "sdkwork_memory_native_sql".to_string(),
                redacts_payloads: true,
            },
            conformance: MemoryPluginConformanceContract {
                suite: "sdkwork-memory-plugin-conformance".to_string(),
                suite_version: "0.1.0".to_string(),
            },
        }
    }

    pub fn reference_profiles_baseline() -> Self {
        Self {
            schema_version: 1,
            kind: "sdkwork.memory.plugin".to_string(),
            plugin_id: "sdkwork-memory-plugin-reference-profiles".to_string(),
            package_name: "sdkwork-memory-plugin-reference-profiles".to_string(),
            display_name: "SDKWork Memory Reference Profiles Plugin".to_string(),
            version: "0.1.0".to_string(),
            owner: "sdkwork-memory".to_string(),
            implementation_kinds: vec![MemoryImplementationKind::SearchFirst],
            plugin_roles: vec![
                MemoryPluginRole::Implementation,
                MemoryPluginRole::Store,
                MemoryPluginRole::Retriever,
                MemoryPluginRole::Context,
            ],
            deployment_modes: vec![MemoryDeploymentMode::Test, MemoryDeploymentMode::EvalOnly],
            port_exports: vec![
                MemoryPluginPortExport {
                    port: "MemoryRecordStorePort".to_string(),
                    builder: "build_reference_record_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryEventStorePort".to_string(),
                    builder: "build_reference_event_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryAuditStorePort".to_string(),
                    builder: "build_reference_audit_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryOutboxStorePort".to_string(),
                    builder: "build_reference_outbox_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryCandidateStorePort".to_string(),
                    builder: "build_reference_candidate_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryHabitStorePort".to_string(),
                    builder: "build_reference_habit_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryRetrievalTraceStorePort".to_string(),
                    builder: "build_reference_retrieval_trace_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryGovernanceAccessPort".to_string(),
                    builder: "build_reference_governance_access".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemorySpaceStorePort".to_string(),
                    builder: "build_reference_space_store".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryRetrieverPort".to_string(),
                    builder: "build_reference_retriever".to_string(),
                },
                MemoryPluginPortExport {
                    port: "MemoryContextAssemblerPort".to_string(),
                    builder: "build_reference_context_assembler".to_string(),
                },
            ],
            provider_kinds: vec![],
            retriever_kinds: vec![MemoryRetrieverKind::Keyword],
            index_kinds: vec![],
            required_core_version: "0.1.0".to_string(),
            config_schema_ref: None,
            secret_refs: vec![],
            data_classes: vec![
                MemoryPluginDataClass::Tenant,
                MemoryPluginDataClass::Personal,
                MemoryPluginDataClass::Internal,
            ],
            capabilities: MemoryPluginCapabilities {
                canonical_store: true,
                event_log: true,
                candidate_lifecycle: true,
                habit_learning: true,
                retrieval_trace: true,
                deletion_propagation: true,
                audit_log: true,
                outbox_log: true,
                embedding_required: false,
            },
            degradation: MemoryPluginDegradationPolicy {
                mode: "fail_closed_reference_baseline".to_string(),
                returns_stale_hits: false,
            },
            migration: MemoryPluginMigrationCapabilities {
                export_supported: false,
                import_supported: false,
                dual_write_supported: false,
                shadow_read_supported: false,
            },
            observability: MemoryPluginObservabilityContract {
                metrics_prefix: "sdkwork_memory_reference_profiles".to_string(),
                redacts_payloads: true,
            },
            conformance: MemoryPluginConformanceContract {
                suite: "sdkwork-memory-plugin-conformance".to_string(),
                suite_version: "0.1.0".to_string(),
            },
        }
    }

    pub fn phase1_baseline_manifests() -> Vec<Self> {
        vec![
            Self::native_sql_baseline(),
            Self::reference_profiles_baseline(),
        ]
    }

    pub fn native_sql_for_test() -> Self {
        Self::native_sql_baseline()
    }

    pub fn reference_profiles_for_test() -> Self {
        Self::reference_profiles_baseline()
    }

    pub fn phase1_baseline_manifests_for_test() -> Vec<Self> {
        Self::phase1_baseline_manifests()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryImplementationKind {
    NativeSql,
    EventSourced,
    SearchFirst,
    GraphTemporal,
    LocalEmbedded,
    ExternalProviderBridge,
    HybridPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPluginRole {
    Implementation,
    Store,
    Retriever,
    Index,
    Provider,
    Context,
    Evaluation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDeploymentMode {
    Server,
    Container,
    Private,
    Local,
    Test,
    EvalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryProviderKind {
    LanguageModel,
    EmbeddingModel,
    RerankModel,
    SearchEngine,
    GraphEngine,
    ExternalMemory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryRetrieverKind {
    Sql,
    Keyword,
    Dictionary,
    Time,
    Event,
    Vector,
    Graph,
    GrepFile,
    External,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryIndexKind {
    Sql,
    Keyword,
    Dictionary,
    Time,
    Event,
    Vector,
    Graph,
    GrepFile,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPluginDataClass {
    Public,
    Internal,
    Tenant,
    Personal,
    Sensitive,
    Regulated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginPortExport {
    pub port: String,
    pub builder: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginCapabilities {
    pub canonical_store: bool,
    pub event_log: bool,
    pub candidate_lifecycle: bool,
    pub habit_learning: bool,
    pub retrieval_trace: bool,
    pub deletion_propagation: bool,
    pub audit_log: bool,
    pub outbox_log: bool,
    pub embedding_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginDegradationPolicy {
    pub mode: String,
    pub returns_stale_hits: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginMigrationCapabilities {
    pub export_supported: bool,
    pub import_supported: bool,
    pub dual_write_supported: bool,
    pub shadow_read_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginObservabilityContract {
    pub metrics_prefix: String,
    pub redacts_payloads: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPluginConformanceContract {
    pub suite: String,
    pub suite_version: String,
}

fn looks_like_secret_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("literal")
        || lower.contains("password")
        || lower.contains("api_key")
        || lower.contains("private_key")
        || lower.contains("access_token")
        || lower.contains("refresh_token")
        || lower.contains("bearer ")
        || lower.contains("sk-")
        || lower.contains("token-secret")
}
