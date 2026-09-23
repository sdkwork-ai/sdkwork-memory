use sdkwork_intelligence_memory_service::OpenMemoryService;
use sdkwork_memory_contract::{MemoryOpenApi, MemoryOpenApiRequestContext};
use sdkwork_memory_plugin_native_sql::{NativeSqlMemoryStore, NativeSqlPhase1Runtime};
use sdkwork_memory_retrieval::MemoryRetrievalStrategy;
use sdkwork_memory_spi::{
    MemoryCoreRuntime, MemoryDeploymentMode, MemoryImplementationKind, MemoryRuntimeProfileMetadata,
};

#[tokio::test]
async fn strict_runtime_constructor_accepts_valid_native_composition() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("in-memory native SQL store must open");
    let prepared = OpenMemoryService::new(store.clone());

    let service = OpenMemoryService::try_from_core_runtime(
        NativeSqlPhase1Runtime::from_store(store),
        prepared.core_runtime().clone(),
    )
    .expect("valid native SQL core runtime must be accepted");

    assert_eq!(
        service.core_runtime().profile().profile_id,
        "local-embedded-phase1"
    );
    assert!(service.core_runtime().has_port("MemoryRecordStorePort"));

    let capabilities = service
        .retrieve_capabilities(MemoryOpenApiRequestContext::for_open_surface(
            "runtime-contract",
            1,
            Some(1),
        ))
        .await
        .expect("capabilities must project the validated core runtime");
    assert_eq!(
        capabilities.metadata.as_ref().and_then(|metadata| {
            metadata
                .get("runtimeComposition")
                .and_then(serde_json::Value::as_str)
        }),
        Some("typed_ports")
    );
    assert_eq!(
        capabilities.metadata.as_ref().and_then(|metadata| {
            metadata
                .get("dynamicProfileCutover")
                .and_then(serde_json::Value::as_bool)
        }),
        Some(false)
    );
}

#[tokio::test]
async fn strict_runtime_constructor_rejects_unvalidated_plugin_identity() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("in-memory native SQL store must open");
    let runtime = MemoryCoreRuntime::new(MemoryRuntimeProfileMetadata {
        profile_id: "hybrid-platform-phase1".to_string(),
        implementation_kind: MemoryImplementationKind::HybridPlatform,
        primary_plugin_id: "sdkwork-memory-plugin-reference-profiles".to_string(),
        deployment_mode: MemoryDeploymentMode::EvalOnly,
    });

    let error = match OpenMemoryService::try_from_core_runtime(
        NativeSqlPhase1Runtime::from_store(store),
        runtime,
    ) {
        Ok(_) => panic!("native SQL service must reject a reference-profile runtime"),
        Err(error) => error,
    };

    assert!(error.contains("requires primary plugin"));
}

#[tokio::test]
async fn qualified_runtime_exposes_honest_selectable_memory_schemes() {
    let store = NativeSqlMemoryStore::new_in_memory_sqlite()
        .await
        .expect("in-memory native SQL store must open");
    let prepared = OpenMemoryService::new(store.clone());
    let service = OpenMemoryService::try_from_core_runtime_with_retrieval_strategy(
        NativeSqlPhase1Runtime::from_store(store),
        prepared.core_runtime().clone(),
        MemoryRetrievalStrategy::EventAware,
    )
    .expect("event-aware retrieval uses qualified native SQL ports");

    let capabilities = service
        .retrieve_capabilities(MemoryOpenApiRequestContext::for_open_surface(
            "scheme-contract",
            1,
            Some(1),
        ))
        .await
        .unwrap();
    let metadata = capabilities.metadata.unwrap();
    assert_eq!(metadata["activeRetrievalStrategy"], "event_aware");
    let schemes = metadata["selectableSchemes"].as_array().unwrap();
    assert_eq!(schemes.len(), 4);
    assert!(schemes
        .iter()
        .all(|scheme| scheme["productionQualified"] == true));
    assert!(schemes
        .iter()
        .all(|scheme| scheme["implementationKind"] == "local_embedded"));
}
