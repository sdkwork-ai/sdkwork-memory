use std::sync::Arc;

use crate::store_error::map_memory_spi_error;
use async_trait::async_trait;
use sdkwork_memory_contract::{
    DeleteAllMemoriesRequest, DeleteAllMemoriesResult, ListCandidatesQuery, ListMemoriesQuery,
    MemoryAppRequestContext, MemoryBackendRequestContext, MemoryCandidate, MemoryCandidateList,
    MemoryCapabilities, MemoryContextPack, MemoryContextPackRequest, MemoryEvent,
    MemoryEventRequest, MemoryExtractionRequest, MemoryFeedback, MemoryFeedbackRequest,
    MemoryImplementationKind, MemoryLearningJob, MemoryOpenApi, MemoryOpenApiRequestContext,
    MemoryProviderHealth, MemoryProviderHealthStatus, MemoryProviderInterface, MemoryRecord,
    MemoryRecordList, MemoryRecordPatch, MemoryRecordRequest, MemoryRetrievalHit,
    MemoryRetrievalRequest, MemoryRetrievalResult, MemoryRetrievalTrace, MemoryRetrieverKind,
    MemoryServiceError, MemoryServiceErrorKind, MemoryServiceResult, MemoryType,
};
use sdkwork_memory_plugin_native_sql::{
    build_native_sql_executable_runtime, native_sql_phase1_port_builders,
    NativeSqlMemoryRecordDetail, NativeSqlMemoryStore, NativeSqlOpenApiEventRow,
    NATIVE_SQL_PLUGIN_ID,
};
use sdkwork_memory_retrieval::{
    build_context_pack_from_hits, entity_boosts, extract_entities,
    fuse_retrieval_candidates_with_policy, normalize_entity_text,
    orchestrate_retrieval_candidates_with_entity_boosts, select_query_entities, EntityBoostInput,
    LinkedEntityMatch, MemoryRetrievalStrategy, RetrievalCandidate, RetrievalEventInput,
    RetrievalFusionPolicy, RetrievalRecordInput,
};
use sdkwork_memory_spi::parse_metadata_filter;
use sdkwork_memory_spi::{
    AppendMemoryAuditCommand, AppendMemoryOutboxCommand, AppendMemoryRetrievalTraceCommand,
    CreateCanonicalMemoryCommand, CreateMemoryCandidateCommand, DeleteAllCanonicalMemoryCommand,
    DeleteCanonicalMemoryCommand, ListMemoryCandidatesQuery, MemoryCanonicalRecord,
    MemoryCoreRuntime, MemoryDeploymentMode, MemoryDriveExportUploader, MemoryGraphPort,
    MemoryImplementationKind as SpiMemoryImplementationKind, MemoryMutationJournal,
    MemoryRetrievalHitDraft, MemoryRetrieverKind as SpiMemoryRetrieverKind,
    MemoryRuntimeProfileMetadata, MemoryScopeContext, MemorySensitivityReadScope,
    RetrieveCanonicalMemoryQuery, RetrieveMemoryCandidateDetailQuery,
    RetrieveMemoryRetrievalTraceForTenantQuery, SearchMemoryCandidatesQuery,
    UpdateCanonicalMemoryCommand, MAX_MEMORY_RETRIEVAL_CANDIDATES,
};
use tracing::info;

use crate::access;
use crate::platform;
use crate::runtime_data_plane::MemoryRuntimeDataPlane;
use crate::sensitive_content::assert_memory_text_is_safe;
use crate::store_error::map_native_sql_store_error;

pub struct OpenMemoryService {
    pub(crate) store: Arc<NativeSqlMemoryStore>,
    /// Graph (entity/edge) operations go through the SPI port so the graph
    /// capability stays store-agnostic behind the boundary.
    pub(crate) graph: Arc<dyn MemoryGraphPort>,
    pub(crate) core_runtime: MemoryCoreRuntime,
    pub(crate) runtime_data_plane: MemoryRuntimeDataPlane,
    pub(crate) drive_export_uploader: Option<Arc<dyn MemoryDriveExportUploader>>,
    retrieval_strategy: MemoryRetrievalStrategy,
}

impl OpenMemoryService {
    pub fn new(store: NativeSqlMemoryStore) -> Self {
        let store = Arc::new(store);
        let core_runtime = Self::build_default_core_runtime(store.clone());
        let runtime_data_plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_runtime.clone())
            .expect("built-in native SQL runtime must expose the Phase-1 HTTP data plane");
        let graph: Arc<dyn MemoryGraphPort> = store.clone();
        Self {
            store,
            graph,
            core_runtime,
            runtime_data_plane,
            drive_export_uploader: None,
            retrieval_strategy: MemoryRetrievalStrategy::Balanced,
        }
    }

    pub fn with_runtime_profile(
        store: NativeSqlMemoryStore,
        profile_id: impl Into<String>,
        primary_plugin_id: impl Into<String>,
    ) -> Self {
        let store = Arc::new(store);
        let metadata = Self::qualified_native_runtime_profile_metadata(
            store.as_ref(),
            profile_id.into(),
            primary_plugin_id.into(),
        )
        .expect("runtime profile must be a qualified native SQL profile");
        let core_runtime = Self::build_native_core_runtime(store.clone(), metadata);
        let runtime_data_plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_runtime.clone())
            .expect("built-in native SQL runtime must expose the Phase-1 HTTP data plane");
        let graph: Arc<dyn MemoryGraphPort> = store.clone();
        Self {
            store,
            graph,
            core_runtime,
            runtime_data_plane,
            drive_export_uploader: None,
            retrieval_strategy: MemoryRetrievalStrategy::Balanced,
        }
    }

    fn build_default_core_runtime(store: Arc<NativeSqlMemoryStore>) -> MemoryCoreRuntime {
        let metadata = match store.dialect() {
            sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres => {
                MemoryRuntimeProfileMetadata {
                    profile_id: "native-sql-phase1".to_string(),
                    implementation_kind: SpiMemoryImplementationKind::NativeSql,
                    primary_plugin_id: NATIVE_SQL_PLUGIN_ID.to_string(),
                    deployment_mode: MemoryDeploymentMode::Server,
                }
            }
            sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite => {
                MemoryRuntimeProfileMetadata {
                    profile_id: "local-embedded-phase1".to_string(),
                    implementation_kind: SpiMemoryImplementationKind::LocalEmbedded,
                    primary_plugin_id: NATIVE_SQL_PLUGIN_ID.to_string(),
                    deployment_mode: MemoryDeploymentMode::Local,
                }
            }
        };
        Self::build_native_core_runtime(store, metadata)
    }

    fn build_native_core_runtime(
        store: Arc<NativeSqlMemoryStore>,
        metadata: MemoryRuntimeProfileMetadata,
    ) -> MemoryCoreRuntime {
        let executable = build_native_sql_executable_runtime(store);
        let mut runtime = MemoryCoreRuntime::new(metadata);
        for builder in native_sql_phase1_port_builders() {
            runtime
                .bind_port(NATIVE_SQL_PLUGIN_ID, builder.port_name, &executable)
                .expect("built-in native SQL executable port must bind");
        }
        runtime
    }

    fn qualified_native_runtime_profile_metadata(
        store: &NativeSqlMemoryStore,
        profile_id: String,
        primary_plugin_id: String,
    ) -> Result<MemoryRuntimeProfileMetadata, String> {
        if primary_plugin_id != NATIVE_SQL_PLUGIN_ID {
            return Err(format!(
                "qualified SQL runtime requires primary plugin {NATIVE_SQL_PLUGIN_ID}"
            ));
        }
        let (expected_profile_id, implementation_kind, deployment_mode) = match store.dialect() {
            sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres => (
                "native-sql-phase1",
                SpiMemoryImplementationKind::NativeSql,
                MemoryDeploymentMode::Server,
            ),
            sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite => (
                "local-embedded-phase1",
                SpiMemoryImplementationKind::LocalEmbedded,
                MemoryDeploymentMode::Local,
            ),
        };
        if profile_id != expected_profile_id {
            return Err(format!(
                "profile {profile_id} is not qualified for {:?}; expected {expected_profile_id}",
                store.dialect()
            ));
        }
        Ok(MemoryRuntimeProfileMetadata {
            profile_id,
            implementation_kind,
            primary_plugin_id,
            deployment_mode,
        })
    }

    fn validate_native_sql_core_runtime(runtime: &MemoryCoreRuntime) -> Result<(), String> {
        let profile = runtime.profile();
        if profile.primary_plugin_id != NATIVE_SQL_PLUGIN_ID {
            return Err(format!(
                "native SQL service requires primary plugin {NATIVE_SQL_PLUGIN_ID}, got {}",
                profile.primary_plugin_id
            ));
        }
        if !matches!(
            &profile.implementation_kind,
            SpiMemoryImplementationKind::NativeSql | SpiMemoryImplementationKind::LocalEmbedded
        ) {
            return Err(format!(
                "native SQL service cannot serve implementation kind {:?}",
                profile.implementation_kind
            ));
        }
        for builder in native_sql_phase1_port_builders() {
            if runtime.port_owner(builder.port_name) != Some(NATIVE_SQL_PLUGIN_ID) {
                return Err(format!(
                    "native SQL service requires {} to be bound to {NATIVE_SQL_PLUGIN_ID}",
                    builder.port_name
                ));
            }
        }
        Ok(())
    }

    pub fn with_drive_export_uploader(
        mut self,
        uploader: Arc<dyn MemoryDriveExportUploader>,
    ) -> Self {
        self.drive_export_uploader = Some(uploader);
        self
    }

    pub fn drive_export_uploader(&self) -> Option<&Arc<dyn MemoryDriveExportUploader>> {
        self.drive_export_uploader.as_ref()
    }

    pub fn core_runtime(&self) -> &MemoryCoreRuntime {
        &self.core_runtime
    }

    pub fn retrieval_strategy(&self) -> MemoryRetrievalStrategy {
        self.retrieval_strategy
    }

    pub fn runtime_data_plane(&self) -> &MemoryRuntimeDataPlane {
        &self.runtime_data_plane
    }

    pub fn from_phase1_runtime(
        phase1: sdkwork_memory_plugin_native_sql::NativeSqlPhase1Runtime,
        profile_id: impl Into<String>,
        primary_plugin_id: impl Into<String>,
    ) -> Self {
        let store = phase1.into_arc_store();
        let metadata = Self::qualified_native_runtime_profile_metadata(
            store.as_ref(),
            profile_id.into(),
            primary_plugin_id.into(),
        )
        .expect("runtime profile must be a qualified native SQL profile");
        let core_runtime = Self::build_native_core_runtime(store.clone(), metadata);
        let runtime_data_plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_runtime.clone())
            .expect("built-in native SQL runtime must expose the Phase-1 HTTP data plane");
        let graph: Arc<dyn MemoryGraphPort> = store.clone();
        Self {
            store,
            graph,
            core_runtime,
            runtime_data_plane,
            drive_export_uploader: None,
            retrieval_strategy: MemoryRetrievalStrategy::Balanced,
        }
    }

    pub fn try_from_core_runtime(
        phase1: sdkwork_memory_plugin_native_sql::NativeSqlPhase1Runtime,
        core_runtime: MemoryCoreRuntime,
    ) -> Result<Self, String> {
        Self::try_from_core_runtime_with_retrieval_strategy(
            phase1,
            core_runtime,
            MemoryRetrievalStrategy::Balanced,
        )
    }

    pub fn try_from_core_runtime_with_retrieval_strategy(
        phase1: sdkwork_memory_plugin_native_sql::NativeSqlPhase1Runtime,
        core_runtime: MemoryCoreRuntime,
        retrieval_strategy: MemoryRetrievalStrategy,
    ) -> Result<Self, String> {
        Self::validate_native_sql_core_runtime(&core_runtime)?;
        let runtime_data_plane = MemoryRuntimeDataPlane::try_for_phase1_http(core_runtime.clone())
            .map_err(|error| error.to_string())?;
        let store = phase1.into_arc_store();
        let graph: Arc<dyn MemoryGraphPort> = store.clone();
        Ok(Self {
            store,
            graph,
            core_runtime,
            runtime_data_plane,
            drive_export_uploader: None,
            retrieval_strategy,
        })
    }

    pub async fn ready_check(&self) -> MemoryServiceResult<()> {
        self.store.ping().await.map_err(Self::map_store_error)?;
        self.store
            .verify_canonical_schema()
            .await
            .map_err(Self::map_store_error)?;
        tracing::debug!(
            profile_id = %self.core_runtime.profile().profile_id,
            primary_plugin_id = %self.core_runtime.profile().primary_plugin_id,
            "memory store ready"
        );
        Ok(())
    }

    pub fn runtime_profile_label(&self) -> &'static str {
        match (self.store.dialect(), self.retrieval_strategy) {
            (
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres,
                MemoryRetrievalStrategy::Balanced,
            ) => "postgresql_balanced",
            (
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres,
                MemoryRetrievalStrategy::SearchFirst,
            ) => "postgresql_search_first",
            (
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres,
                MemoryRetrievalStrategy::EventAware,
            ) => "postgresql_event_aware",
            (
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite,
                MemoryRetrievalStrategy::Balanced,
            ) => "sqlite_balanced",
            (
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite,
                MemoryRetrievalStrategy::SearchFirst,
            ) => "sqlite_search_first",
            (
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite,
                MemoryRetrievalStrategy::EventAware,
            ) => "sqlite_event_aware",
        }
    }

    fn active_implementation_kind(&self) -> MemoryImplementationKind {
        match &self.core_runtime.profile().implementation_kind {
            SpiMemoryImplementationKind::NativeSql => MemoryImplementationKind::NativeSql,
            SpiMemoryImplementationKind::EventSourced => MemoryImplementationKind::EventSourced,
            SpiMemoryImplementationKind::SearchFirst => MemoryImplementationKind::SearchFirst,
            SpiMemoryImplementationKind::GraphTemporal => MemoryImplementationKind::GraphTemporal,
            SpiMemoryImplementationKind::LocalEmbedded => MemoryImplementationKind::LocalEmbedded,
            SpiMemoryImplementationKind::ExternalProviderBridge => {
                MemoryImplementationKind::ExternalProviderBridge
            }
            SpiMemoryImplementationKind::HybridPlatform => MemoryImplementationKind::HybridPlatform,
        }
    }

    pub(crate) fn active_implementation_kind_code(&self) -> &'static str {
        match &self.core_runtime.profile().implementation_kind {
            SpiMemoryImplementationKind::NativeSql => "native_sql",
            SpiMemoryImplementationKind::EventSourced => "event_sourced",
            SpiMemoryImplementationKind::SearchFirst => "search_first",
            SpiMemoryImplementationKind::GraphTemporal => "graph_temporal",
            SpiMemoryImplementationKind::LocalEmbedded => "local_embedded",
            SpiMemoryImplementationKind::ExternalProviderBridge => "external_provider_bridge",
            SpiMemoryImplementationKind::HybridPlatform => "hybrid_platform",
        }
    }

    fn deployment_qualification(&self) -> &'static str {
        match &self.core_runtime.profile().deployment_mode {
            MemoryDeploymentMode::Server => "server",
            MemoryDeploymentMode::Container => "container",
            MemoryDeploymentMode::Private => "private",
            MemoryDeploymentMode::Local => "local",
            MemoryDeploymentMode::Test => "test",
            MemoryDeploymentMode::EvalOnly => "evaluation_only",
        }
    }

    /// Spawns background workers and returns a shutdown sender.
    ///
    /// The caller should call `send(true)` on the returned sender during
    /// graceful shutdown so workers can drain in-flight work.
    pub fn spawn_background_workers(
        service: &Arc<Self>,
    ) -> crate::job_worker::MemoryBackgroundWorkers {
        crate::job_worker::spawn_background_workers(service.clone())
    }

    pub fn to_open_context(app: &MemoryAppRequestContext) -> MemoryOpenApiRequestContext {
        MemoryOpenApiRequestContext {
            api_key_id: app
                .session_id
                .clone()
                .unwrap_or_else(|| format!("app-{}", app.actor_id.unwrap_or(0))),
            tenant_id: app.tenant_id,
            actor_id: app.actor_id,
            elevated_tenant_access: false,
        }
    }

    pub fn to_open_context_backend(
        backend: &MemoryBackendRequestContext,
    ) -> MemoryOpenApiRequestContext {
        MemoryOpenApiRequestContext::for_backend_surface(backend.tenant_id, backend.operator_id)
    }

    pub(crate) fn next_id(&self) -> MemoryServiceResult<u64> {
        platform::next_numeric_id()
    }

    pub(crate) fn scope(
        context: &MemoryOpenApiRequestContext,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryScopeContext> {
        Ok(MemoryScopeContext {
            tenant_id: platform::tenant_id_i64(context.tenant_id)?,
            space_id: platform::space_id_i64(space_id)?,
            organization_id: None,
            user_id: context.actor_id.map(|value| value as i64),
        })
    }

    pub(crate) fn map_store_error(
        error: sdkwork_memory_plugin_native_sql::NativeSqlStoreError,
    ) -> MemoryServiceError {
        map_native_sql_store_error(error)
    }

    fn should_drop_stale_retrieval_hit(error: &MemoryServiceError) -> bool {
        matches!(
            &error.kind,
            MemoryServiceErrorKind::NotFound | MemoryServiceErrorKind::Forbidden
        )
    }

    pub(crate) async fn publish_domain_event(
        &self,
        scope: &MemoryScopeContext,
        event_type: &str,
        aggregate_type: &str,
        aggregate_id: &str,
        payload: serde_json::Value,
    ) -> MemoryServiceResult<()> {
        let outbox_id = self.next_id()?.to_string();
        let payload_json = serde_json::to_string(&payload).map_err(|error| {
            MemoryServiceError::storage(format!("domain event payload encode failed: {error}"))
        })?;
        self.runtime_data_plane
            .append_outbox(AppendMemoryOutboxCommand {
                scope: scope.clone(),
                outbox_id,
                aggregate_type: aggregate_type.to_string(),
                aggregate_id: aggregate_id.to_string(),
                event_type: event_type.to_string(),
                event_version: "1.0".to_string(),
                payload_json,
            })
            .await?;
        Ok(())
    }

    pub(crate) fn memory_mutation_journal(
        &self,
        memory_id: &str,
        event_type: &str,
        audit_action: &str,
        payload: serde_json::Value,
    ) -> MemoryServiceResult<MemoryMutationJournal> {
        let payload_json = serde_json::to_string(&payload).map_err(|error| {
            MemoryServiceError::storage(format!("memory mutation payload encode failed: {error}"))
        })?;
        Ok(MemoryMutationJournal {
            outbox_id: self.next_id()?.to_string(),
            aggregate_type: "memory_record".to_string(),
            aggregate_id: memory_id.to_string(),
            event_type: event_type.to_string(),
            event_version: "1.0".to_string(),
            payload_json,
            audit_id: self.next_id()?.to_string(),
            audit_action: audit_action.to_string(),
            audit_resource_type: "memory_record".to_string(),
            audit_resource_id: memory_id.to_string(),
            audit_result: "accepted".to_string(),
        })
    }

    async fn load_scoped_record(
        &self,
        context: &MemoryOpenApiRequestContext,
        space_id: u64,
        memory_id: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        let authorization = access::authorize_actor_for_space_retrieval(
            &self.runtime_data_plane,
            context,
            space_id,
        )
        .await?;
        let scope = Self::scope(context, space_id)?;
        match self
            .runtime_data_plane
            .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
                scope,
                memory_id: memory_id.to_string(),
            })
            .await?
        {
            Some(row) => {
                let record = Self::map_canonical_record(row)?;
                access::assert_actor_may_read_record_sensitivity_for_owner(
                    context,
                    &record.sensitivity_level,
                    authorization.actor_is_space_owner,
                )?;
                Ok(record)
            }
            None => Err(MemoryServiceError::not_found("memory not found")),
        }
    }

    async fn load_scoped_event(
        &self,
        context: &MemoryOpenApiRequestContext,
        space_id: u64,
        event_id: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        access::assert_actor_can_access_space(&self.runtime_data_plane, context, space_id).await?;
        let scope = Self::scope(context, space_id)?;
        match self
            .store
            .retrieve_open_api_event(&scope, &event_id.to_string())
            .await
            .map_err(Self::map_store_error)?
        {
            Some(row) => Self::map_event(row),
            None => Err(MemoryServiceError::not_found("event not found")),
        }
    }

    pub(crate) fn parse_id(value: &str) -> Option<u64> {
        platform::parse_numeric_id(value)
    }

    pub(crate) fn memory_type_to_db(value: MemoryType) -> &'static str {
        match value {
            MemoryType::Working => "working",
            MemoryType::Session => "session",
            MemoryType::Semantic => "semantic",
            MemoryType::Episodic => "episodic",
            MemoryType::Procedural => "procedural",
            MemoryType::Habit => "habit",
            MemoryType::Relationship => "relationship",
            MemoryType::DomainKnowledge => "domain_knowledge",
        }
    }

    pub(crate) fn memory_type_from_db(value: &str) -> MemoryType {
        match value {
            "working" => MemoryType::Working,
            "session" => MemoryType::Session,
            "episodic" => MemoryType::Episodic,
            "procedural" => MemoryType::Procedural,
            "habit" => MemoryType::Habit,
            "relationship" => MemoryType::Relationship,
            "domain_knowledge" => MemoryType::DomainKnowledge,
            _ => MemoryType::Semantic,
        }
    }

    pub(crate) fn map_record(
        detail: NativeSqlMemoryRecordDetail,
    ) -> MemoryServiceResult<MemoryRecord> {
        let memory_id = Self::parse_id(&detail.memory_id)
            .ok_or_else(|| MemoryServiceError::storage("memory id must be numeric"))?;
        let space_id = u64::try_from(detail.space_id)
            .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;
        let version = u64::try_from(detail.version.max(0))
            .map_err(|_| MemoryServiceError::storage("version must be non-negative"))?;
        let supersedes_memory_id = detail
            .supersedes_memory_id
            .as_deref()
            .and_then(Self::parse_id);
        let superseded_by_memory_id = detail
            .superseded_by_memory_id
            .as_deref()
            .and_then(Self::parse_id);

        Ok(MemoryRecord {
            memory_id,
            uuid: Some(detail.memory_id),
            space_id,
            user_id: detail.user_id.and_then(|v| u64::try_from(v).ok()),
            scope: detail.scope,
            memory_type: Self::memory_type_from_db(&detail.memory_type),
            subject: detail.subject,
            predicate: detail.predicate,
            object_text: Some(detail.object_text),
            canonical_text: detail.canonical_text,
            summary_text: None,
            confidence: detail.confidence,
            evidence_count: detail.evidence_count,
            contradiction_count: detail.contradiction_count,
            status: detail.status,
            sensitivity_level: detail.sensitivity_level,
            supersedes_memory_id,
            superseded_by_memory_id,
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            expires_at: detail.expires_at,
            metadata: detail
                .metadata_json
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok()),
            version,
        })
    }

    pub(crate) fn map_canonical_record(
        detail: MemoryCanonicalRecord,
    ) -> MemoryServiceResult<MemoryRecord> {
        let memory_id = Self::parse_id(&detail.memory_id)
            .ok_or_else(|| MemoryServiceError::storage("memory id must be numeric"))?;
        let space_id = u64::try_from(detail.space_id)
            .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;
        let version = u64::try_from(detail.version.max(0))
            .map_err(|_| MemoryServiceError::storage("version must be non-negative"))?;
        Ok(MemoryRecord {
            memory_id,
            uuid: Some(detail.memory_id),
            space_id,
            user_id: detail.user_id.and_then(|value| u64::try_from(value).ok()),
            scope: detail.scope_label,
            memory_type: Self::memory_type_from_db(&detail.memory_type),
            subject: detail.subject,
            predicate: detail.predicate,
            object_text: Some(detail.object_text),
            canonical_text: detail.canonical_text,
            summary_text: None,
            confidence: detail.confidence,
            evidence_count: Some(detail.evidence_count),
            contradiction_count: Some(detail.contradiction_count),
            status: detail.status,
            sensitivity_level: detail.sensitivity_level,
            supersedes_memory_id: detail
                .supersedes_memory_id
                .and_then(|value| Self::parse_id(&value)),
            superseded_by_memory_id: detail
                .superseded_by_memory_id
                .and_then(|value| Self::parse_id(&value)),
            created_at: detail.created_at,
            updated_at: detail.updated_at,
            expires_at: detail.expires_at,
            metadata: detail
                .metadata_json
                .as_deref()
                .and_then(|json| serde_json::from_str(json).ok()),
            version,
        })
    }

    pub(crate) fn map_event(row: NativeSqlOpenApiEventRow) -> MemoryServiceResult<MemoryEvent> {
        let event_id = Self::parse_id(&row.event_id)
            .ok_or_else(|| MemoryServiceError::storage("event id must be numeric"))?;
        let space_id = u64::try_from(row.space_id)
            .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;

        Ok(MemoryEvent {
            event_id,
            uuid: Some(row.event_id),
            space_id,
            user_id: row.user_id.and_then(|v| u64::try_from(v).ok()),
            actor_type: row.actor_type,
            actor_id: row.actor_id,
            event_type: row.event_type,
            source_type: row.source_type,
            event_time: row.event_time,
            payload: Some(row.payload),
            payload_hash: row.payload_hash,
            sensitivity_level: None,
            ingestion_status: row.ingestion_status,
            created_at: row.created_at,
        })
    }

    pub(crate) fn normalize_sensitivity_level(
        value: Option<&str>,
    ) -> MemoryServiceResult<&'static str> {
        match value.unwrap_or("internal") {
            "public" => Ok("public"),
            "internal" => Ok("internal"),
            "private" => Ok("private"),
            "sensitive" => Ok("sensitive"),
            "restricted" => Ok("restricted"),
            other => Err(MemoryServiceError::validation(format!(
                "sensitivityLevel must be one of public, internal, private, sensitive, restricted; got {other}"
            ))),
        }
    }

    /// Validate a contract-declared `expiresAt` and canonicalise it to the UTC
    /// text format the store writes for every other timestamp.
    ///
    /// Rejecting unparseable values keeps the caller's contract (an unparsable
    /// date must not be silently stored and never honoured), and canonicalising
    /// keeps the SQL string comparison in the retrieval path chronological
    /// regardless of the offset notation the caller used.
    pub(crate) fn normalize_expires_at(value: Option<&str>) -> MemoryServiceResult<Option<String>> {
        let Some(value) = value else {
            return Ok(None);
        };
        match sdkwork_utils_rust::parse_datetime(value, None) {
            Some(parsed) => Ok(Some(sdkwork_utils_rust::format_datetime(parsed, None))),
            None => Err(MemoryServiceError::validation(
                "expiresAt must be an RFC 3339 timestamp such as 2027-01-01T00:00:00Z",
            )),
        }
    }

    /// Serialize caller metadata into the stored JSON text form. `None` stays
    /// `None`; a non-serializable value is a storage fault, not a silent drop.
    pub(crate) fn serialize_metadata(
        value: Option<&serde_json::Value>,
    ) -> MemoryServiceResult<Option<String>> {
        match value {
            None => Ok(None),
            Some(value) => serde_json::to_string(value).map(Some).map_err(|error| {
                MemoryServiceError::storage(format!("metadata encode failed: {error}"))
            }),
        }
    }

    fn default_retriever_profile(&self) -> Option<serde_json::Value> {
        Some(self.retrieval_strategy.retriever_profile())
    }

    fn selectable_memory_schemes(&self) -> serde_json::Value {
        let (storage, scheme_prefix) = match self.store.dialect() {
            sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres => {
                ("native_sql", "native")
            }
            sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite => {
                ("local_embedded", "local")
            }
        };
        serde_json::Value::Array(
            MemoryRetrievalStrategy::all()
                .into_iter()
                .map(|strategy| {
                    serde_json::json!({
                        "schemeId": format!("{scheme_prefix}-{}-v1", strategy.code().replace('_', "-")),
                        "implementationKind": storage,
                        "retrievalStrategy": strategy.code(),
                        "productionQualified": true,
                        "canonicalStore": "sql",
                        "embeddingRequired": false,
                    })
                })
                .collect(),
        )
    }

    fn enabled_retriever_kinds(profile: Option<&serde_json::Value>) -> Vec<SpiMemoryRetrieverKind> {
        let definitions = [
            ("sql", SpiMemoryRetrieverKind::Sql),
            ("keyword", SpiMemoryRetrieverKind::Keyword),
            ("dictionary", SpiMemoryRetrieverKind::Dictionary),
            ("time", SpiMemoryRetrieverKind::Time),
            ("event", SpiMemoryRetrieverKind::Event),
        ];
        definitions
            .into_iter()
            .filter(|(name, _kind)| {
                profile
                    .and_then(|value| value.get(*name))
                    .and_then(|value| value.get("weight"))
                    .and_then(serde_json::Value::as_f64)
                    .map(|weight| weight > 0.0)
                    .unwrap_or(profile.is_none())
            })
            .map(|(_name, kind)| kind)
            .collect()
    }

    pub(crate) async fn execute_extraction_work(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<serde_json::Value> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            request.space_id,
        )
        .await?;
        let max_events = platform::max_extraction_input_events();
        if request.input_events.is_empty() {
            return Err(MemoryServiceError::validation(
                "inputEvents must not be empty",
            ));
        }
        if request.input_events.len() > max_events {
            return Err(MemoryServiceError::validation(format!(
                "inputEvents must not exceed {max_events} entries per extraction request"
            )));
        }

        let scope = Self::scope(&context, request.space_id)?;
        let mut created_candidates = 0_u32;
        let mut missing_events = 0_u32;

        for event_id in &request.input_events {
            if let Some(payload) = self
                .store
                .retrieve_event_payload(&scope, &event_id.to_string())
                .await
                .map_err(Self::map_store_error)?
            {
                let proposed = payload
                    .get("content")
                    .and_then(|value| value.as_str())
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        MemoryServiceError::validation(format!(
                            "event {event_id} payload must contain non-empty 'content' for extraction"
                        ))
                    })?
                    .to_string();
                assert_memory_text_is_safe(&[("proposedText", &proposed)])?;
                let candidate_id = self.next_id()?.to_string();
                self.runtime_data_plane
                    .create_candidate(CreateMemoryCandidateCommand {
                        scope: scope.clone(),
                        candidate_id,
                        candidate_type: "extraction".to_string(),
                        memory_type: "semantic".to_string(),
                        proposed_text: proposed,
                        proposed_payload_json: Some(payload.to_string()),
                        evidence_json: Some(format!(r#"["event:{event_id}"]"#)),
                        confidence: 0.7,
                    })
                    .await?;
                created_candidates += 1;
            } else {
                missing_events += 1;
            }
        }

        if created_candidates == 0 {
            return Err(MemoryServiceError::validation(
                "extraction did not produce any candidates from the provided input events",
            ));
        }

        Ok(serde_json::json!({
            "candidateCount": created_candidates,
            "missingEventCount": missing_events,
            "extractionMode": request
                .extraction_mode
                .unwrap_or_else(|| "deterministic".to_string()),
        }))
    }
}

#[async_trait]
impl MemoryOpenApi for OpenMemoryService {
    async fn retrieve_capabilities(
        &self,
        _context: MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<MemoryCapabilities> {
        Ok(MemoryCapabilities {
            embedding_optional: true,
            retrievers: vec![
                MemoryRetrieverKind::Keyword,
                MemoryRetrieverKind::Dictionary,
                MemoryRetrieverKind::Time,
                MemoryRetrieverKind::Event,
                MemoryRetrieverKind::Sql,
            ],
            provider_interfaces: vec![
                MemoryProviderInterface::Memory,
                MemoryProviderInterface::Search,
            ],
            implementation_kinds: vec![self.active_implementation_kind()],
            open_api_prefix: "/mem/v3/api".to_string(),
            sdk_family: "sdkwork-memory-sdk".to_string(),
            checked_at: platform::current_timestamp(),
            metadata: Some(serde_json::json!({
                "activeProfileId": self.core_runtime.profile().profile_id,
                "primaryPluginId": self.core_runtime.profile().primary_plugin_id,
                "activeRetrievalStrategy": self.retrieval_strategy.code(),
                "selectableSchemes": self.selectable_memory_schemes(),
                "implementationSelectionKey": "SDKWORK_MEMORY_IMPLEMENTATION_PROFILE",
                "retrievalStrategySelectionKey": "SDKWORK_MEMORY_RETRIEVAL_STRATEGY",
                "deploymentQualification": self.deployment_qualification(),
                "runtimeComposition": "typed_ports",
                "dynamicProfileCutover": false,
                "runtimeSelectionRequiresRestart": true,
            })),
        })
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            space_id = %request.space_id,
            event_type = %request.event_type,
            otel_kind = "create_event"
        )
    )]
    async fn create_event(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryEventRequest,
    ) -> MemoryServiceResult<MemoryEvent> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            request.space_id,
        )
        .await?;
        let scope = Self::scope(&context, request.space_id)?;
        let event_id = self.next_id()?.to_string();
        let sensitivity = Self::normalize_sensitivity_level(request.sensitivity_level.as_deref())?;
        let payload_json = serde_json::to_string(&request.payload).map_err(|error| {
            MemoryServiceError::storage(format!("payload serialization failed: {error}"))
        })?;
        assert_memory_text_is_safe(&[("eventPayload", &payload_json)])?;
        self.store
            .append_open_api_event(
                &scope,
                &event_id,
                &request.event_type,
                &request.source_type,
                &request.event_time,
                &request.payload,
                sensitivity,
            )
            .await
            .map_err(Self::map_store_error)?;

        let audit_id = self.next_id()?.to_string();
        self.runtime_data_plane
            .append_audit(AppendMemoryAuditCommand {
                scope: scope.clone(),
                audit_id,
                action: "memory.event.create".to_string(),
                resource_type: "memory_event".to_string(),
                resource_id: event_id.clone(),
                result: "accepted".to_string(),
            })
            .await?;

        self.store
            .retrieve_open_api_event(&scope, &event_id)
            .await
            .map_err(Self::map_store_error)?
            .map(Self::map_event)
            .transpose()?
            .ok_or_else(|| MemoryServiceError::storage("created event could not be loaded"))
    }

    async fn retrieve_event(
        &self,
        context: MemoryOpenApiRequestContext,
        event_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryEvent> {
        self.load_scoped_event(&context, space_id, event_id).await
    }

    async fn list_memories(
        &self,
        context: MemoryOpenApiRequestContext,
        query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        let space_id = access::require_list_space_id(query.space_id)?;
        let authorization = access::authorize_actor_for_space_retrieval(
            &self.runtime_data_plane,
            &context,
            space_id,
        )
        .await?;
        let scope = Self::scope(&context, space_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let page_size_usize = usize::try_from(page_size).unwrap_or(20);
        let sensitivity_scope =
            access::sensitivity_read_scope(&context, authorization.actor_is_space_owner);

        let rows = self
            .store
            .list_record_details(
                &scope,
                query.q.as_deref(),
                page_size,
                query.cursor.as_deref(),
                sensitivity_scope,
                query.show_expired.unwrap_or(false),
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() > page_size_usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size_usize).collect();
        let next_cursor = page_rows.last().map(|row| row.memory_id.clone());
        let items = page_rows
            .into_iter()
            .map(Self::map_record)
            .collect::<MemoryServiceResult<Vec<_>>>()?;

        Ok(MemoryRecordList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            space_id = %request.space_id,
            memory_type = ?request.memory_type,
            otel_kind = "create_memory"
        )
    )]
    async fn create_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryRecordRequest,
    ) -> MemoryServiceResult<MemoryRecord> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            request.space_id,
        )
        .await?;
        let scope = Self::scope(&context, request.space_id)?;
        let quota_scope = scope.clone();
        let quota_limits = crate::tenant_quota::MemoryQuotaLimits::from_env();
        let memory_id = self.next_id()?.to_string();
        let object_text = request
            .object_text
            .unwrap_or_else(|| request.canonical_text.clone());
        let sensitivity = Self::normalize_sensitivity_level(request.sensitivity_level.as_deref())?;

        assert_memory_text_is_safe(&[
            ("canonicalText", &request.canonical_text),
            ("objectText", &object_text),
            ("subject", request.subject.as_deref().unwrap_or("")),
            ("predicate", request.predicate.as_deref().unwrap_or("")),
        ])?;

        let event_payload = serde_json::json!({
            "memoryId": memory_id,
            "spaceId": request.space_id,
            "memoryType": request.memory_type,
        });
        let journal = self.memory_mutation_journal(
            &memory_id,
            "memory.record.created",
            "memory.record.create",
            event_payload,
        )?;
        let admission = self
            .runtime_data_plane
            .create_canonical_memory_atomic_with_quota(
                CreateCanonicalMemoryCommand {
                    scope,
                    memory_id,
                    scope_label: request.scope,
                    memory_type: Self::memory_type_to_db(request.memory_type).to_string(),
                    subject: request.subject,
                    predicate: request.predicate,
                    object_text,
                    canonical_text: request.canonical_text,
                    sensitivity_level: sensitivity.to_string(),
                    expires_at: Self::normalize_expires_at(request.expires_at.as_deref())?,
                    metadata_json: Self::serialize_metadata(request.metadata.as_ref())?,
                    journal,
                },
                quota_limits.max_records_per_space,
            )
            .await?;
        let record =
            crate::tenant_quota::resolve_space_record_quota_admission(&quota_scope, admission)?;
        Self::map_canonical_record(record)
    }

    async fn retrieve_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<MemoryRecord> {
        self.load_scoped_record(&context, space_id, memory_id).await
    }

    #[tracing::instrument(
        skip(self, context, patch),
        fields(
            tenant_id = %context.tenant_id,
            space_id = %space_id,
            memory_id = %memory_id,
            otel_kind = "update_memory"
        )
    )]
    async fn update_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        memory_id: u64,
        space_id: u64,
        patch: MemoryRecordPatch,
    ) -> MemoryServiceResult<MemoryRecord> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            space_id,
        )
        .await?;
        let scope = Self::scope(&context, space_id)?;
        let existing = self
            .load_scoped_record(&context, space_id, memory_id)
            .await?;

        if let Some(ref text) = patch.canonical_text {
            assert_memory_text_is_safe(&[("canonicalText", text)])?;
        }
        if let Some(ref subject) = patch.subject {
            assert_memory_text_is_safe(&[("subject", subject)])?;
        }

        // Mirroring mem0's update semantics, a metadata patch shallow-merges
        // into the stored metadata instead of replacing it wholesale; incoming
        // keys win.
        let merged_metadata_json = match patch.metadata.clone() {
            None => Self::serialize_metadata(existing.metadata.as_ref())?,
            Some(incoming) if incoming.is_object() => {
                let merged = match existing.metadata {
                    Some(mut current) if current.is_object() => {
                        if let (Some(current_obj), Some(incoming_obj)) =
                            (current.as_object_mut(), incoming.as_object())
                        {
                            for (key, value) in incoming_obj {
                                current_obj.insert(key.clone(), value.clone());
                            }
                        }
                        current
                    }
                    _ => incoming,
                };
                Self::serialize_metadata(Some(&merged))?
            }
            Some(_) => {
                return Err(MemoryServiceError::validation(
                    "metadata must be a JSON object",
                ))
            }
        };

        let event_payload = serde_json::json!({
            "memoryId": memory_id,
            "spaceId": space_id,
        });
        let memory_id_text = memory_id.to_string();
        let journal = self.memory_mutation_journal(
            &memory_id_text,
            "memory.record.updated",
            "memory.record.update",
            event_payload,
        )?;
        let record = self
            .runtime_data_plane
            .update_canonical_memory_atomic(UpdateCanonicalMemoryCommand {
                scope,
                memory_id: memory_id_text,
                canonical_text: patch.canonical_text,
                subject: patch.subject,
                metadata_json: merged_metadata_json,
                journal,
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("memory not found"))?;
        Self::map_canonical_record(record)
    }

    #[tracing::instrument(
        skip(self, context),
        fields(
            tenant_id = %context.tenant_id,
            space_id = %space_id,
            memory_id = %memory_id,
            otel_kind = "delete_memory"
        )
    )]
    async fn delete_memory(
        &self,
        context: MemoryOpenApiRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<()> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            space_id,
        )
        .await?;
        let scope = Self::scope(&context, space_id)?;
        let _existing = self
            .load_scoped_record(&context, space_id, memory_id)
            .await?;

        let event_payload = serde_json::json!({
            "memoryId": memory_id,
            "spaceId": space_id,
        });
        let memory_id_text = memory_id.to_string();
        let journal = self.memory_mutation_journal(
            &memory_id_text,
            "memory.record.deleted",
            "memory.record.delete",
            event_payload,
        )?;
        self.runtime_data_plane
            .delete_canonical_memory_atomic(DeleteCanonicalMemoryCommand {
                scope,
                memory_id: memory_id_text,
                journal,
            })
            .await?;
        Ok(())
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            space_id = request.space_id,
            otel_kind = "delete_all_memories"
        )
    )]
    async fn delete_all_memories(
        &self,
        context: MemoryOpenApiRequestContext,
        request: DeleteAllMemoriesRequest,
    ) -> MemoryServiceResult<DeleteAllMemoriesResult> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            request.space_id,
        )
        .await?;
        let scope = Self::scope(&context, request.space_id)?;
        let user_id = request.user_id.and_then(|value| i64::try_from(value).ok());
        let receipt = self
            .runtime_data_plane
            .delete_all_canonical_memory_atomic(DeleteAllCanonicalMemoryCommand { scope, user_id })
            .await?;
        let deleted_count = u64::try_from(receipt.deleted_ids.len())
            .map_err(|_| MemoryServiceError::storage("bulk deletion count overflowed"))?;
        Ok(DeleteAllMemoriesResult {
            deleted_count,
            deleted_memory_ids: receipt.deleted_ids,
        })
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            space_count = request.space_ids.len(),
            top_k = request.top_k,
            otel_kind = "create_retrieval"
        )
    )]
    async fn create_retrieval(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryRetrievalRequest,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        if request.space_ids.is_empty() {
            return Err(MemoryServiceError::validation("spaceIds must not be empty"));
        }
        if request.space_ids.len() > platform::MAX_SCOPE_SPACE_IDS {
            return Err(MemoryServiceError::validation(format!(
                "spaceIds must not exceed {} entries per retrieval request",
                platform::MAX_SCOPE_SPACE_IDS
            )));
        }
        if request.query.trim().is_empty() {
            return Err(MemoryServiceError::validation("query must not be blank"));
        }
        crate::retrieval_profile::validate_retrieval_limits(
            request.top_k,
            request.context_budget_tokens,
        )?;

        // Resolve the caller's metadata filter before touching any store, so a filter that
        // cannot be honored exactly fails the request instead of being ignored. A previously
        // shipped version forwarded `filters` into the request and never applied it, which
        // handed the caller unfiltered results with no signal at all.
        let metadata_filter = match request.filters.as_ref() {
            None => None,
            Some(filters) => parse_metadata_filter(filters)
                .map_err(|error| MemoryServiceError::validation(format!("filters: {error}")))?,
        };

        let authorized_spaces = access::authorize_actor_for_retrieval_spaces(
            &self.runtime_data_plane,
            &context,
            &request.space_ids,
        )
        .await?;

        let started = std::time::Instant::now();
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        self.store
            .ensure_default_retrieval_profile_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;

        let (effective_top_k, applied_profile_id, profile_retrievers, fusion_policy) =
            if let Some(profile_id) = request.retrieval_profile_id {
                if let Some(row) = self
                    .store
                    .retrieve_mem_retrieval_profile_for_tenant(tenant_id, &profile_id.to_string())
                    .await
                    .map_err(Self::map_store_error)?
                {
                    let retrievers: serde_json::Value = serde_json::from_str(&row.retrievers_json)
                        .map_err(|_| {
                            MemoryServiceError::storage(
                                "retrieval profile contains invalid retriever configuration",
                            )
                        })?;
                    crate::retrieval_profile::validate_retrieval_retrievers(&retrievers)?;
                    crate::retrieval_profile::validate_retrieval_strategy(
                        &row.strategy,
                        &retrievers,
                    )?;
                    let fusion_policy_value = row
                        .fusion_policy_json
                        .as_deref()
                        .map(serde_json::from_str::<serde_json::Value>)
                        .transpose()
                        .map_err(|_| {
                            MemoryServiceError::storage(
                                "retrieval profile contains invalid fusion policy",
                            )
                        })?;
                    let fusion_policy = crate::retrieval_profile::resolve_retrieval_fusion_policy(
                        fusion_policy_value.as_ref(),
                    )?;
                    (
                        platform::clamp_retrieval_top_k(row.top_k.min(request.top_k)),
                        Some(profile_id),
                        Some(retrievers),
                        fusion_policy,
                    )
                } else {
                    (
                        platform::clamp_retrieval_top_k(request.top_k),
                        None,
                        self.default_retriever_profile(),
                        RetrievalFusionPolicy::default(),
                    )
                }
            } else {
                (
                    platform::clamp_retrieval_top_k(request.top_k),
                    None,
                    self.default_retriever_profile(),
                    RetrievalFusionPolicy::default(),
                )
            };

        let enabled_retriever_kinds = Self::enabled_retriever_kinds(profile_retrievers.as_ref());
        if enabled_retriever_kinds.is_empty() {
            return Err(MemoryServiceError::validation(
                "retrieval profile must enable at least one retriever",
            ));
        }
        // Over-fetch before scoring, mirroring mem0's `internal_limit =
        // max(limit * 4, 60)`: small top_k still builds a candidate pool wide
        // enough for the fusion stage to discriminate. The warehouse-level
        // ceiling (MAX_MEMORY_RETRIEVAL_CANDIDATES) stays as a deliberate
        // bound upstream does not impose.
        let candidate_limit = ((effective_top_k as u32).saturating_mul(4).max(60))
            .clamp(1, MAX_MEMORY_RETRIEVAL_CANDIDATES);

        // mem0 extracts query entities once per search and caps them; the
        // deterministic extractor here stands in for the spaCy/NER pipeline.
        let query_entities = select_query_entities(&extract_entities(&request.query));

        let memory_type_filter = request.memory_types.as_ref().map(|types| {
            types
                .iter()
                .map(|value| Self::memory_type_to_db(*value).to_string())
                .collect::<Vec<_>>()
        });

        let mut candidates: Vec<RetrievalCandidate> = Vec::new();
        let mut retrieval_degraded = false;
        let mut unavailable_retriever_kinds = Vec::new();
        let mut degradation_codes = Vec::new();

        // Step 1: materialize scopes from the single-snapshot governance decisions.
        let mut space_data: Vec<(MemoryScopeContext, bool)> =
            Vec::with_capacity(authorized_spaces.len());
        for authorization in authorized_spaces {
            let scope = Self::scope(&context, authorization.space_id)?;
            space_data.push((scope, authorization.actor_is_space_owner));
        }

        // Step 2: search all spaces in parallel.
        let search_futures: Vec<_> = space_data
            .iter()
            .map(|(scope, actor_is_owner)| {
                let read_scope = if *actor_is_owner {
                    MemorySensitivityReadScope::Owner
                } else if context.elevated_tenant_access {
                    MemorySensitivityReadScope::Elevated
                } else {
                    MemorySensitivityReadScope::Public
                };
                self.runtime_data_plane
                    .search_candidates_scoped(SearchMemoryCandidatesQuery {
                        scope: scope.clone(),
                        query: request.query.clone(),
                        limit: candidate_limit,
                        retriever_kinds: enabled_retriever_kinds.clone(),
                        memory_types: memory_type_filter.clone().unwrap_or_default(),
                        read_scope,
                        metadata_filter: metadata_filter.clone(),
                        include_expired: request.show_expired.unwrap_or(false),
                    })
            })
            .collect();
        let search_results = futures::future::join_all(search_futures).await;

        // Step 3: combine and score results.
        for (space_idx, search_result) in search_results.into_iter().enumerate() {
            let search_result = search_result?;
            let (scope, actor_is_owner) = &space_data[space_idx];
            retrieval_degraded |= search_result.degraded;
            for kind in search_result.unavailable_retriever_kinds {
                if !unavailable_retriever_kinds.contains(&kind) {
                    unavailable_retriever_kinds.push(kind);
                }
            }
            for code in search_result.degradation_codes {
                if !degradation_codes.contains(&code) {
                    degradation_codes.push(code);
                }
            }

            let candidate_ids = search_result
                .records
                .iter()
                .map(|candidate| candidate.memory_id.clone())
                .chain(
                    search_result
                        .events
                        .iter()
                        .map(|candidate| candidate.memory_id.clone()),
                )
                .collect::<std::collections::BTreeSet<_>>();
            let rehydrate_futures = candidate_ids.into_iter().map(|memory_id| {
                let scope = scope.clone();
                async move {
                    let canonical = self
                        .runtime_data_plane
                        .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
                            scope,
                            memory_id: memory_id.clone(),
                        })
                        .await?;
                    Ok::<_, MemoryServiceError>((memory_id, canonical))
                }
            });
            let rehydrated = futures::future::join_all(rehydrate_futures).await;
            let mut canonical_by_id = std::collections::BTreeMap::new();
            for result in rehydrated {
                let (memory_id, Some(canonical)) = result? else {
                    continue;
                };
                if canonical.space_id != scope.space_id {
                    return Err(MemoryServiceError::storage(
                        "retriever returned a candidate outside the requested scope",
                    ));
                }
                if !access::actor_may_read_sensitivity(
                    &context,
                    &canonical.sensitivity_level,
                    *actor_is_owner,
                ) {
                    continue;
                }
                if let Some(filters) = &memory_type_filter {
                    if !filters.contains(&canonical.memory_type) {
                        continue;
                    }
                }
                canonical_by_id.insert(memory_id, canonical);
            }

            let record_inputs = canonical_by_id
                .values()
                .map(|record| RetrievalRecordInput {
                    memory_id: record.memory_id.clone(),
                    subject: record.subject.clone(),
                    predicate: record.predicate.clone(),
                    object_text: record.object_text.clone(),
                    canonical_text: record.canonical_text.clone(),
                    created_at: record.created_at.clone(),
                })
                .collect::<Vec<_>>();
            let event_inputs = search_result
                .events
                .iter()
                .filter(|event| canonical_by_id.contains_key(&event.memory_id))
                .map(|event| RetrievalEventInput {
                    memory_id: Some(event.memory_id.clone()),
                    event_id: event.event_id.clone(),
                    payload_text: event.payload_text.clone(),
                    created_at: event.created_at.clone(),
                })
                .collect::<Vec<_>>();
            // Entity-boost signal: a candidate memory whose provenance edge
            // ties it to an entity that matches a query entity gets a boost on
            // mem0's arithmetic (similarity 1.0 for an exact normalized-text
            // match against the stored canonical name, then the hub-decay
            // weight inside `entity_boosts`). No entity graph data means an
            // empty signal, so lexical rankings stay byte-identical.
            let entity_boost_inputs = if query_entities.is_empty() {
                Vec::new()
            } else {
                let links = self
                    .graph
                    .entity_memory_links(scope.clone())
                    .await
                    .map_err(map_memory_spi_error)?;
                let mut matches: Vec<LinkedEntityMatch> = Vec::with_capacity(query_entities.len());
                for query_entity in &query_entities {
                    let normalized = normalize_entity_text(&query_entity.text);
                    let mut memory_ids = links
                        .iter()
                        .filter(|link| normalize_entity_text(&link.entity_name) == normalized)
                        .map(|link| link.memory_id.clone())
                        .collect::<Vec<_>>();
                    memory_ids.sort();
                    memory_ids.dedup();
                    matches.push(LinkedEntityMatch {
                        similarity: 1.0,
                        memory_ids,
                    });
                }
                entity_boosts(&matches)
                    .into_iter()
                    .map(|(memory_id, boost)| EntityBoostInput { memory_id, boost })
                    .collect()
            };
            let orchestrated = orchestrate_retrieval_candidates_with_entity_boosts(
                &request.query,
                &record_inputs,
                &event_inputs,
                &[],
                &entity_boost_inputs,
                profile_retrievers.as_ref(),
                candidate_limit as usize,
            );

            for candidate in orchestrated {
                let Some(canonical) = canonical_by_id.get(&candidate.record.memory_id).cloned()
                else {
                    continue;
                };
                let memory = Self::map_canonical_record(canonical)?;
                if candidate.raw_score > 0.0 {
                    candidates.push(RetrievalCandidate {
                        memory,
                        retriever_name: candidate.retriever_name,
                        raw_score: candidate.raw_score,
                        rank: candidate.rank,
                    });
                }
            }
        }

        let fused = fuse_retrieval_candidates_with_policy(
            candidates,
            effective_top_k as usize,
            fusion_policy,
        );
        let retrieval_id = self.next_id()?;
        let trace_id = retrieval_id.to_string();
        let primary_scope = Self::scope(&context, request.space_ids[0])?;
        let latency_ms = platform::elapsed_millis_i64(started);
        let query_hash = platform::stable_query_hash(&request.query);
        let retrievers_json = profile_retrievers
            .as_ref()
            .map(|value| value.to_string())
            .unwrap_or_else(|| r#"{"keyword":{"weight":1.0}}"#.to_string());
        let hits: Vec<MemoryRetrievalHit> = fused
            .iter()
            .map(|hit| {
                Ok(MemoryRetrievalHit {
                    hit_id: self.next_id()?,
                    memory: Some(hit.memory.clone()),
                    memory_id: Some(hit.memory.memory_id),
                    retriever_name: hit.retriever_name.clone(),
                    result_rank: hit.rank,
                    raw_score: Some(hit.raw_score),
                    fused_score: Some(hit.fused_score),
                    explanation: Some(serde_json::json!({
                        "fusionAlgorithm": "weighted_rrf",
                        "rankConstant": fusion_policy.rank_constant,
                        "dominantRetriever": hit.retriever_name,
                        "contributingRetrievers": hit.retrievers,
                        "canonicalRehydrated": true,
                    })),
                    status: "accepted".to_string(),
                })
            })
            .collect::<MemoryServiceResult<Vec<_>>>()?;

        let trace_hits: Vec<MemoryRetrievalHitDraft> = hits
            .iter()
            .map(|hit| MemoryRetrievalHitDraft {
                hit_id: hit.hit_id.to_string(),
                memory_id: hit.memory_id.map(|value| value.to_string()),
                space_id: hit
                    .memory
                    .as_ref()
                    .and_then(|memory| i64::try_from(memory.space_id).ok()),
                retriever_name: hit.retriever_name.clone(),
                result_rank: i64::from(hit.result_rank),
                raw_score: hit.raw_score,
                fused_score: hit.fused_score,
                explanation_json: hit.memory.as_ref().map(|memory| {
                    let mut explanation = hit.explanation.clone().unwrap_or_default();
                    if let Some(object) = explanation.as_object_mut() {
                        object.insert("spaceId".to_string(), serde_json::json!(memory.space_id));
                    }
                    explanation.to_string()
                }),
                status: hit.status.clone(),
            })
            .collect();

        let metadata_json = if request.filters.is_some()
            || !unavailable_retriever_kinds.is_empty()
            || !degradation_codes.is_empty()
        {
            let mut metadata = serde_json::Map::new();
            if let Some(filters) = request.filters.clone() {
                metadata.insert("filters".to_string(), filters);
            }
            if !unavailable_retriever_kinds.is_empty() {
                metadata.insert(
                    "unavailableRetrieverKinds".to_string(),
                    serde_json::to_value(&unavailable_retriever_kinds).unwrap_or_default(),
                );
            }
            if !degradation_codes.is_empty() {
                metadata.insert(
                    "degradationCodes".to_string(),
                    serde_json::to_value(&degradation_codes).unwrap_or_default(),
                );
            }
            Some(serde_json::Value::Object(metadata).to_string())
        } else {
            None
        };

        let _ = self
            .runtime_data_plane
            .append_retrieval_trace(AppendMemoryRetrievalTraceCommand {
                scope: primary_scope,
                trace_id: trace_id.clone(),
                actor_id: request.actor_id.clone(),
                query_text: Some(request.query.clone()),
                query_hash: query_hash.clone(),
                retrievers_json: Some(retrievers_json.clone()),
                latency_ms: Some(latency_ms),
                degraded: retrieval_degraded,
                metadata_json,
                hits: trace_hits,
                context_pack: None,
            })
            .await?;

        let trace = if request.include_trace.unwrap_or(false) {
            Some(MemoryRetrievalTrace {
                trace_id: retrieval_id,
                space_id: Some(request.space_ids[0]),
                retrieval_profile_id: applied_profile_id,
                actor_id: request.actor_id,
                query_text: Some(request.query),
                query_hash,
                result_count: hits.len() as i32,
                degraded: retrieval_degraded,
                created_at: platform::current_timestamp(),
            })
        } else {
            None
        };

        info!(
            tenant_id,
            retrieval_id,
            hit_count = hits.len(),
            latency_ms,
            space_count = request.space_ids.len(),
            "memory retrieval completed"
        );
        crate::domain_metrics::memory_domain_metrics().record_retrieval_completed();

        Ok(MemoryRetrievalResult {
            retrieval_id,
            trace,
            hits,
            degraded: retrieval_degraded,
        })
    }

    async fn retrieve_retrieval(
        &self,
        context: MemoryOpenApiRequestContext,
        retrieval_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalResult> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let lookup = self
            .runtime_data_plane
            .retrieve_retrieval_trace_for_tenant(RetrieveMemoryRetrievalTraceForTenantQuery {
                tenant_id,
                trace_id: retrieval_id.to_string(),
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("retrieval not found"))?;
        let trace_space_id_i64 = lookup.scope.space_id;
        access::assert_actor_can_access_space_i64(
            &self.runtime_data_plane,
            &context,
            trace_space_id_i64,
        )
        .await?;
        let trace_created_at = lookup.created_at;
        let trace = lookup.trace;
        let trace_space_id = u64::try_from(trace_space_id_i64.max(0))
            .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;

        let mut hits = Vec::new();
        for (index, hit) in trace.hits.iter().enumerate() {
            let Some(memory_id) = hit.memory_id.as_deref() else {
                continue;
            };
            let hit_space_id = hit
                .space_id
                .and_then(|value| u64::try_from(value).ok())
                .or_else(|| {
                    hit.explanation_json
                        .as_deref()
                        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                        .and_then(|value| value.get("spaceId").and_then(serde_json::Value::as_u64))
                })
                .unwrap_or(trace_space_id);
            let hit_space_id_i64 = i64::try_from(hit_space_id)
                .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;
            if let Err(error) = access::assert_actor_can_access_space_i64(
                &self.runtime_data_plane,
                &context,
                hit_space_id_i64,
            )
            .await
            {
                if Self::should_drop_stale_retrieval_hit(&error) {
                    continue;
                }
                return Err(error);
            }
            let scope = Self::scope(&context, hit_space_id)?;
            let Some(canonical) = self
                .runtime_data_plane
                .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
                    scope,
                    memory_id: memory_id.to_string(),
                })
                .await?
            else {
                // A trace may outlive a deleted or retention-purged canonical record. Do not
                // expose a stale identifier as a readable hit.
                continue;
            };
            if canonical.space_id != hit_space_id_i64 {
                continue;
            }
            if let Err(error) = access::assert_actor_may_read_record_sensitivity(
                &self.runtime_data_plane,
                &context,
                hit_space_id,
                &canonical.sensitivity_level,
            )
            .await
            {
                if Self::should_drop_stale_retrieval_hit(&error) {
                    continue;
                }
                return Err(error);
            }
            let memory = Some(Self::map_canonical_record(canonical)?);
            let visible_rank = i32::try_from(hits.len().saturating_add(1)).unwrap_or(i32::MAX);

            hits.push(MemoryRetrievalHit {
                hit_id: hit
                    .hit_id
                    .parse()
                    .ok()
                    .or_else(|| Self::parse_id(&hit.hit_id))
                    .unwrap_or_else(|| retrieval_id.saturating_add(index as u64 + 1)),
                memory,
                memory_id: hit.memory_id.as_deref().and_then(Self::parse_id),
                retriever_name: hit.retriever_name.clone(),
                result_rank: visible_rank,
                raw_score: hit.raw_score,
                fused_score: hit.fused_score,
                explanation: hit
                    .explanation_json
                    .as_deref()
                    .and_then(|value| serde_json::from_str(value).ok()),
                status: hit.status.clone(),
            });
        }

        Ok(MemoryRetrievalResult {
            retrieval_id,
            trace: Some(MemoryRetrievalTrace {
                trace_id: retrieval_id,
                space_id: Some(trace_space_id),
                retrieval_profile_id: None,
                actor_id: trace.actor_id,
                query_text: trace.query_text,
                query_hash: trace.query_hash,
                result_count: i32::try_from(hits.len()).unwrap_or(i32::MAX),
                degraded: trace.degraded,
                created_at: trace_created_at.unwrap_or_else(platform::current_timestamp),
            }),
            hits,
            degraded: trace.degraded,
        })
    }

    async fn retrieve_provider_health(
        &self,
        context: MemoryOpenApiRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let max_bindings = platform::max_provider_health_bindings();
        let mut cursor = None;
        let mut providers = Vec::new();
        loop {
            let rows = self
                .store
                .list_mem_provider_bindings_for_tenant(
                    tenant_id,
                    sdkwork_utils_rust::MAX_LIST_PAGE_SIZE,
                    cursor.as_deref(),
                )
                .await
                .map_err(Self::map_store_error)?;
            let page_size = usize::try_from(sdkwork_utils_rust::MAX_LIST_PAGE_SIZE).unwrap_or(200);
            let has_more = rows.len() > page_size;
            for row in rows.iter().take(page_size) {
                if providers.len() >= max_bindings {
                    return Err(MemoryServiceError::validation(format!(
                        "provider binding count exceeds health aggregation limit ({max_bindings})"
                    )));
                }
                providers.push(Self::map_provider_binding_public(row)?);
            }
            if !has_more {
                break;
            }
            cursor = rows
                .get(page_size.saturating_sub(1))
                .map(|row| row.binding_uuid.clone());
        }
        let status = if providers.is_empty()
            || providers
                .iter()
                .all(|provider| provider.health_state == "healthy")
        {
            MemoryProviderHealthStatus::Healthy
        } else if providers
            .iter()
            .any(|provider| provider.health_state == "unhealthy")
        {
            MemoryProviderHealthStatus::Unhealthy
        } else {
            MemoryProviderHealthStatus::Degraded
        };
        Ok(MemoryProviderHealth {
            status,
            checked_at: platform::current_timestamp(),
            providers,
        })
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            space_count = request.space_ids.len(),
            context_budget_tokens = request.context_budget_tokens,
            otel_kind = "create_context_pack"
        )
    )]
    async fn create_context_pack(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryContextPackRequest,
    ) -> MemoryServiceResult<MemoryContextPack> {
        if request.space_ids.is_empty() {
            return Err(MemoryServiceError::validation("spaceIds must not be empty"));
        }

        let top_k = if request.context_budget_tokens > 0 {
            (request.context_budget_tokens / 200).clamp(1, 50)
        } else {
            10
        };

        let retrieval = self
            .create_retrieval(
                context.clone(),
                MemoryRetrievalRequest {
                    query: request.query.clone(),
                    space_ids: request.space_ids.clone(),
                    actor_id: request.actor_id.clone(),
                    retrieval_profile_id: request.retrieval_profile_id,
                    memory_types: None,
                    filters: request.filters.clone(),
                    top_k,
                    context_budget_tokens: request.context_budget_tokens,
                    show_expired: None,
                    include_trace: Some(false),
                },
            )
            .await?;

        let (pack, estimated_tokens, truncated) =
            build_context_pack_from_hits(&retrieval.hits, request.context_budget_tokens);
        let context_pack_id = self.next_id()?;
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let primary_space = request.space_ids[0] as i64;

        self.store
            .insert_context_pack_open_api(
                tenant_id,
                primary_space,
                &context_pack_id.to_string(),
                Some(&retrieval.retrieval_id.to_string()),
                request.actor_id.as_deref(),
                Some(&request.query),
                &pack.to_string(),
                i64::from(estimated_tokens),
                truncated,
            )
            .await
            .map_err(Self::map_store_error)?;

        Ok(MemoryContextPack {
            context_pack_id,
            retrieval_id: Some(retrieval.retrieval_id),
            query: Some(request.query),
            pack,
            estimated_tokens,
            truncated,
            created_at: platform::current_timestamp(),
        })
    }

    async fn retrieve_context_pack(
        &self,
        context: MemoryOpenApiRequestContext,
        context_pack_id: u64,
    ) -> MemoryServiceResult<MemoryContextPack> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let row = self
            .store
            .retrieve_context_pack_for_tenant(tenant_id, &context_pack_id.to_string())
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("context pack not found"))?;

        if let Some(space_id) = row.space_id {
            access::assert_actor_can_access_space_i64(&self.runtime_data_plane, &context, space_id)
                .await?;
        } else {
            return Err(MemoryServiceError::forbidden(
                "context pack is not linked to an authorized memory space",
            ));
        }

        let pack = serde_json::from_str(&row.pack_json).map_err(|error| {
            MemoryServiceError::storage(format!("context pack payload is corrupt: {error}"))
        })?;

        Ok(MemoryContextPack {
            context_pack_id,
            retrieval_id: None,
            query: row.query_text,
            pack,
            estimated_tokens: row.estimated_tokens as i32,
            truncated: row.truncated,
            created_at: row.created_at,
        })
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            target_type = %request.target_type,
            target_id = %request.target_id,
            otel_kind = "create_feedback"
        )
    )]
    async fn create_feedback(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryFeedbackRequest,
    ) -> MemoryServiceResult<MemoryFeedback> {
        let feedback_id = self.next_id()?;
        let space_id = if request.target_type == "memory" {
            let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
            let memory = self
                .store
                .retrieve_record_detail_for_tenant(tenant_id, &request.target_id.to_string())
                .await
                .map_err(Self::map_store_error)?
                .ok_or_else(|| MemoryServiceError::not_found("memory not found"))?;
            u64::try_from(memory.space_id)
                .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?
        } else if request.target_type == "retrieval" {
            let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
            let lookup = self
                .runtime_data_plane
                .retrieve_retrieval_trace_for_tenant(RetrieveMemoryRetrievalTraceForTenantQuery {
                    tenant_id,
                    trace_id: request.target_id.to_string(),
                })
                .await?
                .ok_or_else(|| MemoryServiceError::not_found("retrieval not found"))?;
            u64::try_from(lookup.scope.space_id.max(0))
                .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?
        } else if request.target_type == "candidate" {
            let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
            let row = self
                .runtime_data_plane
                .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                    tenant_id,
                    candidate_id: request.target_id.to_string(),
                })
                .await?
                .ok_or_else(|| MemoryServiceError::not_found("candidate not found"))?;
            u64::try_from(row.space_id.max(0))
                .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?
        } else {
            return Err(MemoryServiceError::validation(
                "feedback targetType must be memory, retrieval, or candidate",
            ));
        };
        let scope = Self::scope(&context, space_id)?;
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            space_id,
        )
        .await?;
        self.runtime_data_plane
            .append_audit(AppendMemoryAuditCommand {
                scope,
                audit_id: feedback_id.to_string(),
                action: "feedback.create".to_string(),
                resource_type: request.target_type.clone(),
                resource_id: request.target_id.to_string(),
                result: "accepted".to_string(),
            })
            .await?;

        Ok(MemoryFeedback {
            feedback_id,
            target_type: request.target_type,
            target_id: request.target_id,
            feedback_type: request.feedback_type,
            created_at: platform::current_timestamp(),
        })
    }

    #[tracing::instrument(
        skip(self, context, request),
        fields(
            tenant_id = %context.tenant_id,
            space_id = %request.space_id,
            input_event_count = request.input_events.len(),
            otel_kind = "create_extraction"
        )
    )]
    async fn create_extraction(
        &self,
        context: MemoryOpenApiRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            request.space_id,
        )
        .await?;
        let max_events = platform::max_extraction_input_events();
        if request.input_events.is_empty() {
            return Err(MemoryServiceError::validation(
                "inputEvents must not be empty",
            ));
        }
        if request.input_events.len() > max_events {
            return Err(MemoryServiceError::validation(format!(
                "inputEvents must not exceed {max_events} entries per extraction request"
            )));
        }
        let actor_id = context.actor_id.ok_or_else(|| {
            MemoryServiceError::validation("actorId is required to enqueue extraction jobs")
        })?;

        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let job_id = self.next_id()?;
        let mut input = serde_json::to_value(&request).map_err(|error| {
            MemoryServiceError::storage(format!("extraction input encode failed: {error}"))
        })?;
        if let Some(object) = input.as_object_mut() {
            object.insert("actorId".to_string(), serde_json::json!(actor_id));
        }
        let input_json = serde_json::to_string(&input).map_err(|error| {
            MemoryServiceError::storage(format!("extraction input encode failed: {error}"))
        })?;

        crate::job_worker::enqueue_learning_job(
            &self.store,
            tenant_id,
            job_id,
            "extraction",
            Some(request.space_id),
            &input_json,
            0,
        )
        .await
        .map_err(Self::map_store_error)?;

        let row = self
            .store
            .retrieve_learning_job_for_tenant(tenant_id, &job_id.to_string())
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| {
                MemoryServiceError::storage("queued extraction job could not be loaded")
            })?;

        crate::job_worker::learning_job_from_row(&row)
    }

    async fn list_candidates(
        &self,
        context: MemoryOpenApiRequestContext,
        query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = access::require_list_space_id(query.space_id)?;
        access::assert_actor_can_access_space(&self.runtime_data_plane, &context, space_id).await?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let page = self
            .runtime_data_plane
            .list_candidates(ListMemoryCandidatesQuery {
                tenant_id,
                space_id: Some(platform::space_id_i64(space_id)?),
                page_size: page_size as u32,
                cursor: query.cursor,
            })
            .await?;
        let items = page
            .items
            .into_iter()
            .map(Self::map_candidate)
            .collect::<MemoryServiceResult<Vec<_>>>()?;

        Ok(MemoryCandidateList {
            items,
            page_info: platform::memory_cursor_page_info(
                page_size,
                page.has_more,
                page.next_cursor,
            ),
        })
    }

    async fn retrieve_candidate(
        &self,
        context: MemoryOpenApiRequestContext,
        candidate_id: u64,
    ) -> MemoryServiceResult<MemoryCandidate> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        match self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
        {
            Some(row) => {
                access::assert_actor_can_access_space_i64(
                    &self.runtime_data_plane,
                    &context,
                    row.space_id,
                )
                .await?;
                Self::map_candidate_api_detail(row)
            }
            None => Err(MemoryServiceError::not_found("candidate not found")),
        }
    }
}
