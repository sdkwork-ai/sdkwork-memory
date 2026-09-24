use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use sdkwork_memory_profile_resolver::{
    MemoryImplementationProfileDraft, MemoryProfilePortBinding, MemoryRuntimeError,
    MemoryRuntimeProfileResolver,
};
use sdkwork_memory_spi::{
    CreateMemoryRecordCommand, DeleteMemoryRecordCommand, MemoryDeletionReceipt,
    MemoryDeploymentMode, MemoryExecutablePluginRuntime, MemoryImplementationKind,
    MemoryPluginManifest, MemoryPluginPorts, MemoryPluginRegistry, MemoryRecord,
    MemoryRecordStorePort, MemoryRetrieverPort, MemoryRetrieverResult, MemoryScopeContext,
    MemorySpiResult, RetrieveMemoryCandidatesCommand, RetrieveMemoryRecordQuery,  MemorySensitivityReadScope};

#[derive(Default)]
struct InMemoryRecordStore {
    records: Mutex<HashMap<String, MemoryRecord>>,
}

#[async_trait]
impl MemoryRecordStorePort for InMemoryRecordStore {
    async fn create(&self, command: CreateMemoryRecordCommand) -> MemorySpiResult<MemoryRecord> {
        let record = MemoryRecord {
            memory_id: command.memory_id.clone(),
            content: command.content,
        };
        self.records
            .lock()
            .unwrap()
            .insert(command.memory_id, record.clone());
        Ok(record)
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryRecordQuery,
    ) -> MemorySpiResult<Option<MemoryRecord>> {
        Ok(self.records.lock().unwrap().get(&query.memory_id).cloned())
    }

    async fn mark_deleted(
        &self,
        command: DeleteMemoryRecordCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        let deleted = self
            .records
            .lock()
            .unwrap()
            .remove(&command.memory_id)
            .is_some();
        Ok(MemoryDeletionReceipt {
            memory_id: command.memory_id,
            deleted,
            already_deleted: false,
        })
    }
}

struct StaticRetriever;

#[async_trait]
impl MemoryRetrieverPort for StaticRetriever {
    fn retriever_code(&self) -> &str {
        "reference-hybrid"
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Ok(MemoryRetrieverResult {
            memory_ids: vec!["retrieved-by-reference".to_string()],
        })
    }
}

fn reference_search_executable_profile() -> MemoryImplementationProfileDraft {
    MemoryImplementationProfileDraft {
        profile_id: "reference-search-executable-test".to_string(),
        implementation_kind: MemoryImplementationKind::SearchFirst,
        primary_plugin_id: "sdkwork-memory-plugin-reference-profiles".to_string(),
        deployment_mode: MemoryDeploymentMode::Test,
        port_bindings: vec![MemoryProfilePortBinding {
            port: "MemoryRecordStorePort".to_string(),
            plugin_id: "sdkwork-memory-plugin-native-sql".to_string(),
        }],
        required_ports: vec![
            "MemoryRecordStorePort".to_string(),
            "MemoryRetrieverPort".to_string(),
        ],
        safe_config_json: serde_json::json!({}),
    }
}

#[tokio::test]
async fn executable_reference_search_profile_dispatches_to_each_bound_plugin() {
    let native_plugin_id = "sdkwork-memory-plugin-native-sql";
    let reference_plugin_id = "sdkwork-memory-plugin-reference-profiles";
    let record_store = Arc::new(InMemoryRecordStore::default());
    let retriever = Arc::new(StaticRetriever);
    let mut registry = MemoryPluginRegistry::default();
    registry
        .register_executable(
            MemoryPluginManifest::native_sql_for_test(),
            MemoryExecutablePluginRuntime::new(
                MemoryPluginPorts::new().with_record_store(record_store),
            ),
        )
        .unwrap();
    registry
        .register_executable(
            MemoryPluginManifest::reference_profiles_for_test(),
            MemoryExecutablePluginRuntime::new(MemoryPluginPorts::new().with_retriever(retriever)),
        )
        .unwrap();

    let runtime = MemoryRuntimeProfileResolver::new(&registry)
        .resolve_executable(reference_search_executable_profile())
        .unwrap();

    assert_eq!(
        runtime.port_owner("MemoryRecordStorePort"),
        Some(native_plugin_id)
    );
    assert_eq!(
        runtime.port_owner("MemoryRetrieverPort"),
        Some(reference_plugin_id)
    );
    assert_eq!(
        runtime.profile().implementation_kind,
        MemoryImplementationKind::SearchFirst
    );

    let scope = MemoryScopeContext::for_test(1, 9);
    let created = runtime
        .record_store()
        .unwrap()
        .create(CreateMemoryRecordCommand {
            scope: scope.clone(),
            memory_id: "record-from-native".to_string(),
            content: "canonical memory".to_string(),
        })
        .await
        .unwrap();
    assert_eq!(created.memory_id, "record-from-native");
    let retrieved = runtime
        .record_store()
        .unwrap()
        .retrieve(RetrieveMemoryRecordQuery {
            scope,
            memory_id: created.memory_id,
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retrieved.content, "canonical memory");

    let retriever = runtime.retriever().unwrap();
    assert_eq!(retriever.retriever_code(), "reference-hybrid");
    assert_eq!(
        retriever
            .retrieve(RetrieveMemoryCandidatesCommand {
                query: "memory".to_string(),
            read_scope: MemorySensitivityReadScope::Public,
            limit: sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES,
            })
            .await
            .unwrap()
            .memory_ids,
        vec!["retrieved-by-reference"]
    );
}

#[test]
fn executable_resolution_rejects_manifest_only_plugin() {
    let mut registry = MemoryPluginRegistry::default();
    registry
        .register(MemoryPluginManifest::native_sql_for_test())
        .unwrap();

    let error = MemoryRuntimeProfileResolver::new(&registry)
        .resolve_executable(MemoryImplementationProfileDraft::native_sql_phase1())
        .unwrap_err();

    assert!(matches!(
        error,
        MemoryRuntimeError::ExecutablePluginMissing(plugin_id)
            if plugin_id == "sdkwork-memory-plugin-native-sql"
    ));
}

#[test]
fn executable_resolution_rejects_declared_port_without_typed_implementation() {
    let mut registry = MemoryPluginRegistry::default();
    registry
        .register_executable(
            MemoryPluginManifest::native_sql_for_test(),
            MemoryExecutablePluginRuntime::default(),
        )
        .unwrap();

    let error = MemoryRuntimeProfileResolver::new(&registry)
        .resolve_executable(MemoryImplementationProfileDraft::native_sql_phase1())
        .unwrap_err();

    assert!(matches!(
        error,
        MemoryRuntimeError::ExecutablePortMissing { plugin_id, port }
            if plugin_id == "sdkwork-memory-plugin-native-sql"
                && port == "MemoryRecordStorePort"
    ));
}
