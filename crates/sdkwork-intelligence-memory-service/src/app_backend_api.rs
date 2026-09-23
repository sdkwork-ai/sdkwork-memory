use async_trait::async_trait;
use sdkwork_memory_contract::ListSpacesQuery;
use sdkwork_memory_contract::{
    ListAdminResourcesQuery, ListAuditLogsQuery, ListCandidatesQuery, ListEventsQuery,
    ListHabitsQuery, ListJobsQuery, ListMemoriesQuery, ListMemorySourcesQuery,
    ListRetrievalTracesQuery, MemoryAppApi, MemoryAppRequestContext, MemoryAuditLog,
    MemoryAuditLogList, MemoryBackendApi, MemoryBackendRequestContext, MemoryCandidate,
    MemoryCandidateList, MemoryEvalRun, MemoryEvalRunList, MemoryEvalRunRequest, MemoryEventList,
    MemoryExportJob, MemoryExportJobList, MemoryExportRequest, MemoryExtractionRequest,
    MemoryForgetJob, MemoryForgetJobList, MemoryForgetRequest, MemoryHabit, MemoryHabitList,
    MemoryHabitRequest, MemoryImplementationProfile, MemoryImplementationProfileList,
    MemoryImplementationProfileRequest, MemoryIndex, MemoryIndexList, MemoryIndexRequest,
    MemoryLearningJob, MemoryLearningJobList, MemoryLearningSettings, MemoryLearningSettingsPatch,
    MemoryMigrationJobRequest, MemoryOpenApi, MemoryProviderBinding, MemoryProviderBindingList,
    MemoryProviderBindingRequest, MemoryProviderHealth, MemoryRecordList, MemoryRecordRequest,
    MemoryRecordSource, MemoryRecordSourceList, MemoryRetentionJobRequest, MemoryRetrievalProfile,
    MemoryRetrievalProfileList, MemoryRetrievalProfileRequest, MemoryRetrievalTrace,
    MemoryRetrievalTraceList, MemoryReviewRequest, MemoryServiceError, MemoryServiceResult,
    MemorySpace, MemorySpaceList, MemorySpaceRequest,
};
use sdkwork_memory_plugin_native_sql::{
    ExportCollectedPayload, NativeSqlAuditLogRow, NativeSqlHabitRow, NativeSqlMemorySpaceRow,
    NativeSqlRecordSourceRow, NativeSqlRetrievalTraceSummaryRow,
};
use sdkwork_memory_spi::{
    CreateMemorySpaceCommand, DecayMemoryHabitCommand, ListMemoryCandidatesQuery,
    MemoryCandidateDetail, MemoryCandidateSummary, MemoryDriveExportUploadRequest,
    MemoryScopeContext, MemorySpaceRecord, PromoteMemoryHabitCommand, RejectMemoryCandidateCommand,
    RetrieveMemoryCandidateDetailQuery, UpsertMemoryHabitCommand,
};
use sdkwork_utils_rust::is_blank;

use tracing::info;

use crate::access;
use crate::open_api::OpenMemoryService;
use crate::platform;

const LEARNING_SETTINGS_KEY: &str = "learning_settings";

/// Events purged by a targeted `scope: "memory"` forget.
///
/// `ai_event` rows are memory *inputs*. The canonical baseline links them to
/// `ai_space` only — never to `ai_record` — so deleting one record cannot
/// cascade to events, and it must not: a single event may have produced several
/// records, and the ones the caller did not name stay live. Event purging is a
/// property of the whole-space and whole-user scopes, which partition an entire
/// footprint and are served by the store's `delete_events_in_space` and
/// `delete_events_for_user_*`.
///
/// This is therefore a structural zero, not a placeholder. It is named so the
/// field cannot drift into a bare literal, and
/// `forget_memory_scope_reports_no_purged_events_while_space_scope_does` pins it.
const TARGETED_FORGET_PURGED_EVENTS: u32 = 0;

fn default_learning_settings() -> MemoryLearningSettings {
    MemoryLearningSettings {
        auto_promote_candidates: false,
        habit_learning_enabled: true,
        updated_at: platform::current_timestamp(),
    }
}

impl OpenMemoryService {
    pub(crate) fn map_space(row: NativeSqlMemorySpaceRow) -> MemoryServiceResult<MemorySpace> {
        Ok(MemorySpace {
            space_id: platform::non_negative_i64_as_u64(row.space_id, "spaceId")?,
            uuid: Some(row.uuid),
            tenant_id: platform::non_negative_i64_as_u64(row.tenant_id, "tenantId")?,
            organization_id: None,
            owner_subject_type: row.owner_subject_type,
            owner_subject_id: row.owner_subject_id,
            space_type: row.space_type,
            display_name: row.display_name,
            default_scope: row.default_scope,
            lifecycle_status: row.lifecycle_status,
            metadata: None,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: platform::non_negative_i64_as_u64(row.version, "version")?,
        })
    }

    pub(crate) fn map_space_record(row: MemorySpaceRecord) -> MemoryServiceResult<MemorySpace> {
        Ok(MemorySpace {
            space_id: platform::non_negative_i64_as_u64(row.space_id, "spaceId")?,
            uuid: Some(row.uuid),
            tenant_id: platform::non_negative_i64_as_u64(row.tenant_id, "tenantId")?,
            organization_id: row
                .organization_id
                .map(|value| platform::non_negative_i64_as_u64(value, "organizationId"))
                .transpose()?,
            owner_subject_type: row.owner_subject_type,
            owner_subject_id: row.owner_subject_id,
            space_type: row.space_type,
            display_name: row.display_name,
            default_scope: Some(row.default_scope),
            lifecycle_status: row.lifecycle_status,
            metadata: None,
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: platform::non_negative_i64_as_u64(row.version, "version")?,
        })
    }

    async fn resolve_space_list_cursor(
        &self,
        tenant_id: i64,
        cursor: Option<&str>,
    ) -> MemoryServiceResult<i64> {
        let Some(cursor) = cursor.map(str::trim).filter(|value| !value.is_empty()) else {
            return Ok(0);
        };
        if let Ok(space_id) = cursor.parse::<i64>() {
            return Ok(space_id);
        }
        self.store
            .resolve_space_internal_id_by_uuid(tenant_id, cursor)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::validation("cursor is not a valid space cursor"))
    }

    pub(crate) fn map_candidate(
        row: MemoryCandidateSummary,
    ) -> MemoryServiceResult<MemoryCandidate> {
        Ok(MemoryCandidate {
            candidate_id: platform::parse_required_numeric_id(&row.candidate_id, "candidateId")?,
            space_id: platform::non_negative_i64_as_u64(row.space_id, "spaceId")?,
            candidate_type: row.candidate_type,
            memory_type: OpenMemoryService::memory_type_from_db(&row.memory_type),
            proposed_text: row.proposed_text,
            confidence: row.confidence,
            decision_state: row.decision_state,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub(crate) fn map_candidate_api_detail(
        row: MemoryCandidateDetail,
    ) -> MemoryServiceResult<MemoryCandidate> {
        Self::map_candidate(MemoryCandidateSummary {
            candidate_id: row.candidate_id,
            space_id: row.space_id,
            candidate_type: row.candidate_type,
            memory_type: row.memory_type,
            proposed_text: row.proposed_text,
            confidence: row.confidence,
            decision_state: row.decision_state,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    pub(crate) fn map_trace_summary(
        row: NativeSqlRetrievalTraceSummaryRow,
    ) -> MemoryServiceResult<MemoryRetrievalTrace> {
        Ok(MemoryRetrievalTrace {
            trace_id: platform::parse_required_numeric_id(&row.trace_id, "traceId")?,
            space_id: Some(platform::non_negative_i64_as_u64(row.space_id, "spaceId")?),
            retrieval_profile_id: None,
            actor_id: None,
            query_text: row.query_text,
            query_hash: row.query_hash,
            result_count: row.result_count as i32,
            degraded: row.degraded,
            created_at: row.created_at,
        })
    }

    fn normalize_habit_stage(stage: &str) -> String {
        match stage {
            "promoted" => "confirmed".to_string(),
            "decayed" => "decaying".to_string(),
            "candidate" => "emerging".to_string(),
            other => other.to_string(),
        }
    }

    fn map_habit(row: NativeSqlHabitRow) -> MemoryServiceResult<MemoryHabit> {
        Ok(MemoryHabit {
            habit_id: platform::parse_required_numeric_id(&row.habit_id, "habitId")?,
            space_id: platform::non_negative_i64_as_u64(row.space_id, "spaceId")?,
            user_id: platform::non_negative_i64_as_u64(row.user_id, "userId")?,
            habit_key: row.habit_key,
            habit_type: row.habit_type,
            description: row.description,
            stage: Self::normalize_habit_stage(&row.stage),
            strength: row.strength,
            confidence: row.confidence,
            support_count: row.support_count as i32,
            last_signal_at: row.last_signal_at,
            promoted_memory_id: row
                .promoted_memory_uuid
                .as_deref()
                .and_then(platform::parse_numeric_id),
            decay_after: row.decay_after,
            metadata: row
                .metadata_json
                .as_deref()
                .and_then(|value| serde_json::from_str(value).ok()),
            created_at: row.created_at,
            updated_at: row.updated_at,
            version: u64::try_from(row.version.max(0)).unwrap_or(0),
        })
    }

    pub(crate) fn map_record_source(
        row: NativeSqlRecordSourceRow,
    ) -> MemoryServiceResult<MemoryRecordSource> {
        let source_id = OpenMemoryService::parse_id(&row.source_uuid)
            .ok_or_else(|| MemoryServiceError::storage("source id must be numeric"))?;
        let memory_id = OpenMemoryService::parse_id(&row.memory_uuid)
            .ok_or_else(|| MemoryServiceError::storage("memory id must be numeric"))?;
        let event_id = OpenMemoryService::parse_id(&row.event_uuid)
            .ok_or_else(|| MemoryServiceError::storage("event id must be numeric"))?;

        Ok(MemoryRecordSource {
            source_id,
            memory_id,
            event_id,
            source_role: row.source_role,
            confidence_delta: row.confidence_delta,
            created_at: row.created_at,
        })
    }

    async fn load_habit_row(
        &self,
        tenant_id: i64,
        habit_id: u64,
    ) -> MemoryServiceResult<NativeSqlHabitRow> {
        self.store
            .retrieve_habit_for_tenant(tenant_id, &habit_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("habit not found"))
    }

    pub(crate) fn governance_scope(tenant_id: i64) -> MemoryScopeContext {
        MemoryScopeContext {
            tenant_id,
            space_id: 1,
            organization_id: None,
            user_id: None,
        }
    }

    pub(crate) async fn persist_governance_job<T: serde::Serialize>(
        &self,
        tenant_id: i64,
        actor_id: Option<u64>,
        job_id: u64,
        resource_type: &str,
        action: &str,
        job: &T,
    ) -> MemoryServiceResult<()> {
        let metadata = serde_json::to_string(job).map_err(|error| {
            MemoryServiceError::storage(format!("governance job metadata encode failed: {error}"))
        })?;
        self.store
            .append_audit_with_metadata(
                &Self::governance_scope(tenant_id),
                &job_id.to_string(),
                action,
                resource_type,
                &job_id.to_string(),
                "accepted",
                &metadata,
                actor_id.map(|value| value.to_string()).as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        Ok(())
    }

    pub(crate) async fn load_governance_job<T: serde::de::DeserializeOwned>(
        &self,
        tenant_id: i64,
        job_id: u64,
        resource_type: &str,
    ) -> MemoryServiceResult<T> {
        let row = self
            .store
            .retrieve_governance_job_for_tenant(tenant_id, &job_id.to_string(), resource_type)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("governance job not found"))?;
        let metadata = row
            .metadata_json
            .ok_or_else(|| MemoryServiceError::storage("governance job metadata is missing"))?;
        serde_json::from_str(&metadata).map_err(|error| {
            MemoryServiceError::storage(format!("governance job metadata decode failed: {error}"))
        })
    }

    pub(crate) async fn load_governance_job_for_app<T: serde::de::DeserializeOwned>(
        &self,
        context: &MemoryAppRequestContext,
        tenant_id: i64,
        job_id: u64,
        resource_type: &str,
    ) -> MemoryServiceResult<T> {
        let row = self
            .store
            .retrieve_governance_job_for_tenant(tenant_id, &job_id.to_string(), resource_type)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("governance job not found"))?;
        let request_actor = context
            .actor_id
            .map(|value| value.to_string())
            .ok_or_else(|| MemoryServiceError::forbidden("authenticated actor is required"))?;
        if row.actor_id.as_deref() != Some(request_actor.as_str()) {
            crate::domain_metrics::memory_domain_metrics().record_authz_denied();
            return Err(MemoryServiceError::forbidden(
                "governance job is not accessible to this actor",
            ));
        }
        let metadata = row
            .metadata_json
            .ok_or_else(|| MemoryServiceError::storage("governance job metadata is missing"))?;
        serde_json::from_str(&metadata).map_err(|error| {
            MemoryServiceError::storage(format!("governance job metadata decode failed: {error}"))
        })
    }

    async fn filter_export_payload_by_sensitivity(
        &self,
        context: &MemoryAppRequestContext,
        mut payload: ExportCollectedPayload,
    ) -> MemoryServiceResult<ExportCollectedPayload> {
        let open_context = Self::to_open_context(context);
        let mut owner_cache = std::collections::HashMap::new();
        let mut filtered_records = Vec::with_capacity(payload.records.len());
        for record in payload.records {
            let space_id = record
                .get("spaceId")
                .and_then(|value| value.as_i64())
                .and_then(|value| u64::try_from(value).ok())
                .unwrap_or(0);
            let sensitivity = record
                .get("sensitivityLevel")
                .and_then(|value| value.as_str())
                .unwrap_or("internal");
            let actor_is_owner = if let Some(cached) = owner_cache.get(&space_id) {
                *cached
            } else {
                let is_owner = access::actual_actor_is_space_owner(
                    &self.runtime_data_plane,
                    &open_context,
                    space_id,
                )
                .await?;
                owner_cache.insert(space_id, is_owner);
                is_owner
            };
            if access::actor_may_read_sensitivity(&open_context, sensitivity, actor_is_owner) {
                filtered_records.push(record);
            }
        }
        payload.records = filtered_records;
        let mut filtered_events = Vec::with_capacity(payload.events.len());
        for event in payload.events {
            let space_id = event
                .get("spaceId")
                .and_then(|value| value.as_i64())
                .and_then(|value| u64::try_from(value).ok())
                .unwrap_or(0);
            let actor_is_owner = if let Some(cached) = owner_cache.get(&space_id) {
                *cached
            } else {
                let is_owner = access::actual_actor_is_space_owner(
                    &self.runtime_data_plane,
                    &open_context,
                    space_id,
                )
                .await?;
                owner_cache.insert(space_id, is_owner);
                is_owner
            };
            if actor_is_owner {
                filtered_events.push(event);
            }
        }
        payload.events = filtered_events;
        Ok(payload)
    }

    fn encode_export_payload(
        format: &str,
        payload: &ExportCollectedPayload,
    ) -> MemoryServiceResult<serde_json::Value> {
        match format {
            "json" => serde_json::to_value(payload).map_err(|error| {
                MemoryServiceError::storage(format!("export encode failed: {error}"))
            }),
            "jsonl" => {
                let mut lines = Vec::new();
                for record in &payload.records {
                    lines.push(serde_json::to_string(record).map_err(|error| {
                        MemoryServiceError::storage(format!("export jsonl encode failed: {error}"))
                    })?);
                }
                for event in &payload.events {
                    lines.push(serde_json::to_string(event).map_err(|error| {
                        MemoryServiceError::storage(format!("export jsonl encode failed: {error}"))
                    })?);
                }
                Ok(serde_json::Value::String(lines.join("\n")))
            }
            "markdown" => {
                let mut body = String::from("# Memory Export\n\n");
                for record in &payload.records {
                    let text = record
                        .get("canonicalText")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    body.push_str(&format!("- {text}\n"));
                }
                Ok(serde_json::Value::String(body))
            }
            other => Err(MemoryServiceError::validation(format!(
                "unsupported export format: {other}"
            ))),
        }
    }

    async fn assert_habit_actor_access(
        &self,
        context: &MemoryAppRequestContext,
        row: &NativeSqlHabitRow,
    ) -> MemoryServiceResult<()> {
        crate::access::assert_actor_can_access_space_i64(
            &self.runtime_data_plane,
            &Self::to_open_context(context),
            row.space_id,
        )
        .await
    }
}

#[async_trait]
impl MemoryAppApi for OpenMemoryService {
    async fn list_spaces(
        &self,
        context: MemoryAppRequestContext,
        query: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let cursor_space_id = self
            .resolve_space_list_cursor(tenant_id, query.cursor.as_deref())
            .await?;
        let actor_scope = context.actor_id.map(|value| value.to_string());
        let rows = self
            .store
            .list_spaces_for_tenant(
                tenant_id,
                page_size,
                cursor_space_id,
                actor_scope.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let items = rows
            .into_iter()
            .take(page_size as usize)
            .map(Self::map_space)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = items.last().and_then(|space| space.uuid.clone());
        Ok(MemorySpaceList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn create_space(
        &self,
        context: MemoryAppRequestContext,
        request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        crate::access::validate_user_space_owner(
            &Self::to_open_context(&context),
            &request.owner_subject_type,
            &request.owner_subject_id,
        )?;
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = i64::try_from(self.next_id()?)
            .map_err(|_| MemoryServiceError::storage("generated space id out of range"))?;
        let owner_subject_id = request.owner_subject_id.clone();
        let quota_limits = crate::tenant_quota::MemoryQuotaLimits::from_env();
        let admission = self
            .runtime_data_plane
            .create_space_atomic_with_quota(
                CreateMemorySpaceCommand {
                    tenant_id,
                    space_id,
                    organization_id: context.organization_id.map(|value| value as i64),
                    owner_subject_type: request.owner_subject_type,
                    owner_subject_id: request.owner_subject_id,
                    space_type: request.space_type,
                    display_name: request.display_name,
                    default_scope: request.default_scope.unwrap_or_else(|| "user".to_string()),
                },
                quota_limits.max_spaces_per_user,
            )
            .await?;
        let space =
            crate::tenant_quota::resolve_user_space_quota_admission(&owner_subject_id, admission)?;
        Self::map_space_record(space)
    }

    async fn retrieve_space(
        &self,
        context: MemoryAppRequestContext,
        space_id: u64,
    ) -> MemoryServiceResult<MemorySpace> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        crate::access::assert_actor_can_access_space(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            space_id,
        )
        .await?;
        match self
            .store
            .retrieve_space_for_tenant(tenant_id, space_id as i64)
            .await
            .map_err(OpenMemoryService::map_store_error)?
        {
            Some(row) => Self::map_space(row),
            None => Err(MemoryServiceError::not_found("space not found")),
        }
    }

    async fn update_space(
        &self,
        context: MemoryAppRequestContext,
        space_id: u64,
        request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        crate::access::assert_actor_can_access_space(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            space_id,
        )
        .await?;
        match self
            .store
            .update_space_record(
                tenant_id,
                space_id as i64,
                Some(&request.display_name),
                request.default_scope.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?
        {
            Some(row) => Self::map_space(row),
            None => Err(MemoryServiceError::not_found("space not found")),
        }
    }

    async fn create_event(
        &self,
        context: MemoryAppRequestContext,
        request: sdkwork_memory_contract::MemoryEventRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryEvent> {
        MemoryOpenApi::create_event(self, Self::to_open_context(&context), request).await
    }

    async fn retrieve_event(
        &self,
        context: MemoryAppRequestContext,
        event_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryEvent> {
        MemoryOpenApi::retrieve_event(self, Self::to_open_context(&context), event_id, space_id)
            .await
    }

    async fn list_memories(
        &self,
        context: MemoryAppRequestContext,
        query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        MemoryOpenApi::list_memories(self, Self::to_open_context(&context), query).await
    }

    async fn create_memory(
        &self,
        context: MemoryAppRequestContext,
        request: sdkwork_memory_contract::MemoryRecordRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        MemoryOpenApi::create_memory(self, Self::to_open_context(&context), request).await
    }

    async fn retrieve_memory(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        MemoryOpenApi::retrieve_memory(self, Self::to_open_context(&context), memory_id, space_id)
            .await
    }

    async fn update_memory(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        space_id: u64,
        patch: sdkwork_memory_contract::MemoryRecordPatch,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        MemoryOpenApi::update_memory(
            self,
            Self::to_open_context(&context),
            memory_id,
            space_id,
            patch,
        )
        .await
    }

    async fn delete_memory(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<()> {
        MemoryOpenApi::delete_memory(self, Self::to_open_context(&context), memory_id, space_id)
            .await
    }

    async fn list_memory_sources(
        &self,
        context: MemoryAppRequestContext,
        memory_id: u64,
        query: ListMemorySourcesQuery,
    ) -> MemoryServiceResult<MemoryRecordSourceList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let memory_uuid = memory_id.to_string();
        let memory = self
            .store
            .retrieve_record_detail_for_tenant(tenant_id, &memory_uuid)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("memory not found"))?;
        let space_id = u64::try_from(memory.space_id.max(0)).unwrap_or(0);
        crate::access::assert_actor_can_access_space(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            space_id,
        )
        .await?;

        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_record_sources_for_memory(
                tenant_id,
                &memory_uuid,
                page_size,
                query.cursor.as_deref(),
                query.q.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.source_uuid.clone());
        let items = page_rows
            .into_iter()
            .map(Self::map_record_source)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(MemoryRecordSourceList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn create_forget_request(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryForgetRequest,
    ) -> MemoryServiceResult<MemoryForgetJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let job_id = self.next_id()?;
        let now = platform::current_timestamp();

        let stats = match request.scope.as_str() {
            "memory" => {
                let memory_ids = request.memory_ids.as_ref().ok_or_else(|| {
                    MemoryServiceError::validation("memoryIds is required when scope is memory")
                })?;
                if memory_ids.is_empty() {
                    return Err(MemoryServiceError::validation(
                        "memoryIds must not be empty when scope is memory",
                    ));
                }
                let max_memory_ids = platform::MAX_FORGET_MEMORY_IDS;
                if memory_ids.len() > max_memory_ids {
                    return Err(MemoryServiceError::validation(format!(
                        "memoryIds must not exceed {max_memory_ids} entries per forget request"
                    )));
                }
                let space_id = request.space_id.ok_or_else(|| {
                    MemoryServiceError::validation("spaceId is required when scope is memory")
                })?;
                let scope = MemoryScopeContext {
                    tenant_id,
                    space_id: platform::space_id_i64(space_id)?,
                    organization_id: context.organization_id.map(|value| value as i64),
                    user_id: context.actor_id.map(|value| value as i64),
                };
                crate::access::assert_actor_can_access_space(
                    &self.runtime_data_plane,
                    &Self::to_open_context(&context),
                    space_id,
                )
                .await?;
                let mut deleted_records = 0_u32;
                let mut rejected_candidates = 0_u32;
                for memory_id in memory_ids {
                    let memory_key = memory_id.to_string();
                    if self
                        .store
                        .retrieve_record_detail(&scope, &memory_key)
                        .await
                        .map_err(OpenMemoryService::map_store_error)?
                        .is_none()
                    {
                        continue;
                    }
                    let outcome = self
                        .store
                        .hard_delete_record_with_cleanup(&scope, &memory_key)
                        .await
                        .map_err(OpenMemoryService::map_store_error)?;
                    if outcome.deleted {
                        deleted_records += 1;
                    }
                    rejected_candidates += outcome.rejected_candidates;
                }
                sdkwork_memory_plugin_native_sql::ForgetScopeStats {
                    deleted_records,
                    purged_events: TARGETED_FORGET_PURGED_EVENTS,
                    rejected_candidates,
                }
            }
            "space" => {
                let space_id = request.space_id.ok_or_else(|| {
                    MemoryServiceError::validation("spaceId is required when scope is space")
                })?;
                crate::access::assert_actor_can_access_space(
                    &self.runtime_data_plane,
                    &Self::to_open_context(&context),
                    space_id,
                )
                .await?;
                let scope = MemoryScopeContext {
                    tenant_id,
                    space_id: platform::space_id_i64(space_id)?,
                    organization_id: context.organization_id.map(|value| value as i64),
                    user_id: context.actor_id.map(|value| value as i64),
                };
                self.store
                    .forget_all_records_in_space(&scope)
                    .await
                    .map_err(OpenMemoryService::map_store_error)?
            }
            "user" => {
                let user_id = context.actor_id.ok_or_else(|| {
                    MemoryServiceError::validation(
                        "authenticated user context is required when scope is user",
                    )
                })?;
                let space_id = request.space_id.map(platform::space_id_i64).transpose()?;
                self.store
                    .forget_records_for_user(tenant_id, user_id as i64, space_id)
                    .await
                    .map_err(OpenMemoryService::map_store_error)?
            }
            "query" => {
                let space_id = request.space_id.ok_or_else(|| {
                    MemoryServiceError::validation("spaceId is required when scope is query")
                })?;
                access::assert_actor_can_access_space(
                    &self.runtime_data_plane,
                    &Self::to_open_context(&context),
                    space_id,
                )
                .await?;
                let query = request.query.as_deref().ok_or_else(|| {
                    MemoryServiceError::validation("query is required when scope is query")
                })?;
                if is_blank(Some(query)) {
                    return Err(MemoryServiceError::validation(
                        "query must not be empty when scope is query",
                    ));
                }
                let scope = MemoryScopeContext {
                    tenant_id,
                    space_id: platform::space_id_i64(space_id)?,
                    organization_id: context.organization_id.map(|value| value as i64),
                    user_id: context.actor_id.map(|value| value as i64),
                };
                self.store
                    .forget_records_matching_query(&scope, query)
                    .await
                    .map_err(OpenMemoryService::map_store_error)?
            }
            _ => {
                return Err(MemoryServiceError::validation(
                    "scope must be one of memory, space, user, or query",
                ));
            }
        };

        let state = if stats.deleted_records == 0
            && stats.purged_events == 0
            && stats.rejected_candidates == 0
        {
            "failed"
        } else {
            "succeeded"
        };

        let job = MemoryForgetJob {
            forget_request_id: job_id,
            state: state.to_string(),
            result: Some(serde_json::json!({
                "deletedCount": stats.deleted_records,
                "purgedEvents": stats.purged_events,
                "rejectedCandidates": stats.rejected_candidates,
                "scope": request.scope,
                "reason": request.reason,
            })),
            created_at: now.clone(),
            updated_at: now,
        };
        self.persist_governance_job(
            tenant_id,
            context.actor_id,
            job_id,
            "forget_job",
            "forget.request.create",
            &job,
        )
        .await?;
        info!(
            tenant_id,
            forget_request_id = job_id,
            scope = %request.scope,
            state = %state,
            deleted_records = stats.deleted_records,
            purged_events = stats.purged_events,
            "privacy forget request completed"
        );
        Ok(job)
    }

    async fn list_forget_requests(
        &self,
        context: MemoryAppRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryForgetJobList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let actor_id = context
            .actor_id
            .map(|value| value.to_string())
            .ok_or_else(|| MemoryServiceError::forbidden("authenticated actor is required"))?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_governance_jobs_for_tenant(
                tenant_id,
                "forget_job",
                Some(actor_id.as_str()),
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.job_id.clone());
        let items = page_rows
            .into_iter()
            .map(|row| {
                let metadata = row
                    .metadata_json
                    .ok_or_else(|| MemoryServiceError::storage("forget job metadata is missing"))?;
                serde_json::from_str(&metadata).map_err(|error| {
                    MemoryServiceError::storage(format!(
                        "forget job metadata decode failed: {error}"
                    ))
                })
            })
            .collect::<MemoryServiceResult<Vec<_>>>()?;
        Ok(MemoryForgetJobList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn retrieve_forget_request(
        &self,
        context: MemoryAppRequestContext,
        forget_request_id: u64,
    ) -> MemoryServiceResult<MemoryForgetJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        Self::load_governance_job_for_app(
            self,
            &context,
            tenant_id,
            forget_request_id,
            "forget_job",
        )
        .await
    }

    async fn create_export_job(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryExportRequest,
    ) -> MemoryServiceResult<MemoryExportJob> {
        if request.space_ids.is_empty() {
            return Err(MemoryServiceError::validation("spaceIds must not be empty"));
        }
        if request.space_ids.len() > platform::MAX_SCOPE_SPACE_IDS {
            return Err(MemoryServiceError::validation(format!(
                "spaceIds must not exceed {} entries per export request",
                platform::MAX_SCOPE_SPACE_IDS
            )));
        }
        let open_context = Self::to_open_context(&context);
        let authorizations = access::authorize_actor_for_spaces_access(
            &self.runtime_data_plane,
            &open_context,
            &request.space_ids,
        )
        .await?;
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let job_id = self.next_id()?;
        let now = platform::current_timestamp();
        let space_ids = request
            .space_ids
            .iter()
            .map(|space_id| platform::space_id_i64(*space_id))
            .collect::<Result<Vec<_>, _>>()?;
        let all_owner = authorizations
            .iter()
            .all(|authorization| authorization.actor_is_space_owner);
        let sensitivity_scope = access::sensitivity_read_scope(&open_context, all_owner);
        let max_payload_bytes = export_payload_byte_limit(request.drive_target_ref.is_some());
        let payload = self
            .store
            .collect_export_payload_for_spaces(
                tenant_id,
                &space_ids,
                request.include_events.unwrap_or(false),
                sensitivity_scope,
                max_payload_bytes,
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let payload = self
            .filter_export_payload_by_sensitivity(&context, payload)
            .await?;
        let exported_records = payload.records.len() as u32;
        let exported_events = payload.events.len() as u32;
        let export_body = Self::encode_export_payload(&request.format, &payload)?;
        validate_export_payload_size(&export_body, max_payload_bytes)?;
        drop(payload);

        if let Some(drive_target_ref) = &request.drive_target_ref {
            let uploader = self.drive_export_uploader.as_ref().ok_or_else(|| {
                MemoryServiceError::validation(
                    "drive export requires configured SDKWork Drive uploader integration",
                )
            })?;
            let export_ref = format!("export-job-{job_id}");
            let export_bytes = export_payload_bytes(export_body)?;
            let upload = uploader
                .upload_export(MemoryDriveExportUploadRequest {
                    tenant_id,
                    organization_id: context
                        .organization_id
                        .and_then(|id| i64::try_from(id).ok()),
                    user_id: context.actor_id.and_then(|id| i64::try_from(id).ok()),
                    export_job_id: job_id,
                    format: request.format.clone(),
                    drive_target_ref: drive_target_ref.clone(),
                    body: export_bytes,
                    content_type: export_content_type(&request.format),
                    original_file_name: export_file_name(job_id, &request.format),
                })
                .await
                .map_err(|error| {
                    MemoryServiceError::storage(format!("drive export upload failed: {error}"))
                })?;
            let drive_object_ref = upload.drive_object_ref.clone();

            let scope = MemoryScopeContext {
                tenant_id,
                space_id: space_ids[0],
                organization_id: context
                    .organization_id
                    .and_then(|id| i64::try_from(id).ok()),
                user_id: context.actor_id.and_then(|id| i64::try_from(id).ok()),
            };
            self.publish_domain_event(
                &scope,
                "memory.export.drive_upload_completed",
                "export_job",
                &job_id.to_string(),
                serde_json::json!({
                    "exportRef": export_ref,
                    "driveTargetRef": drive_target_ref,
                    "driveObjectRef": drive_object_ref,
                    "driveNodeId": upload.drive_node_id,
                    "checksumSha256Hex": upload.checksum_sha256_hex,
                    "format": request.format,
                    "exportedRecords": exported_records,
                    "exportedEvents": exported_events,
                    "spaceIds": request.space_ids,
                }),
            )
            .await?;

            let job = MemoryExportJob {
                export_job_id: job_id,
                state: "completed".to_string(),
                format: request.format.clone(),
                drive_object_ref: Some(drive_object_ref.clone()),
                result: Some(serde_json::json!({
                    "exportRef": export_ref,
                    "driveTargetRef": drive_target_ref,
                    "driveObjectRef": drive_object_ref,
                    "exportedRecords": exported_records,
                    "exportedEvents": exported_events,
                    "spaceIds": request.space_ids,
                })),
                created_at: now.clone(),
                updated_at: now,
            };
            self.persist_governance_job(
                tenant_id,
                context.actor_id,
                job_id,
                "export_job",
                "export.job.create",
                &job,
            )
            .await?;
            info!(
                tenant_id,
                export_job_id = job_id,
                exported_records,
                exported_events,
                format = %request.format,
                drive_target_ref = %drive_target_ref,
                drive_object_ref = %drive_object_ref,
                "drive-backed export uploaded through SDKWork Drive"
            );
            return Ok(job);
        }

        let job = MemoryExportJob {
            export_job_id: job_id,
            state: "succeeded".to_string(),
            format: request.format.clone(),
            drive_object_ref: None,
            result: Some(serde_json::json!({
                "exportedRecords": exported_records,
                "exportedEvents": exported_events,
                "spaceIds": request.space_ids,
                "exportPayload": export_body,
            })),
            created_at: now.clone(),
            updated_at: now,
        };
        let stored_job = MemoryExportJob {
            result: Some(serde_json::json!({
                "exportedRecords": exported_records,
                "exportedEvents": exported_events,
                "spaceIds": request.space_ids,
                "exportRef": format!("export-job-{job_id}"),
            })),
            ..job.clone()
        };
        self.persist_governance_job(
            tenant_id,
            context.actor_id,
            job_id,
            "export_job",
            "export.job.create",
            &stored_job,
        )
        .await?;
        info!(
            tenant_id,
            export_job_id = job_id,
            exported_records,
            exported_events,
            format = %request.format,
            "privacy export job completed"
        );
        Ok(job)
    }

    async fn list_export_jobs(
        &self,
        context: MemoryAppRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryExportJobList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let actor_id = context
            .actor_id
            .map(|value| value.to_string())
            .ok_or_else(|| MemoryServiceError::forbidden("authenticated actor is required"))?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_governance_jobs_for_tenant(
                tenant_id,
                "export_job",
                Some(actor_id.as_str()),
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.job_id.clone());
        let items = page_rows
            .into_iter()
            .map(|row| {
                let metadata = row
                    .metadata_json
                    .ok_or_else(|| MemoryServiceError::storage("export job metadata is missing"))?;
                serde_json::from_str(&metadata).map_err(|error| {
                    MemoryServiceError::storage(format!(
                        "export job metadata decode failed: {error}"
                    ))
                })
            })
            .collect::<MemoryServiceResult<Vec<_>>>()?;
        Ok(MemoryExportJobList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn retrieve_export_job(
        &self,
        context: MemoryAppRequestContext,
        export_job_id: u64,
    ) -> MemoryServiceResult<MemoryExportJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        Self::load_governance_job_for_app(self, &context, tenant_id, export_job_id, "export_job")
            .await
    }

    async fn list_candidates(
        &self,
        context: MemoryAppRequestContext,
        query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = crate::access::require_list_space_id(query.space_id)?;
        crate::access::assert_actor_can_access_space(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            space_id,
        )
        .await?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
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
            .collect::<Result<Vec<_>, _>>()?;
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
        context: MemoryAppRequestContext,
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
                crate::access::assert_actor_can_access_space_i64(
                    &self.runtime_data_plane,
                    &Self::to_open_context(&context),
                    row.space_id,
                )
                .await?;
                Self::map_candidate_api_detail(row)
            }
            None => Err(MemoryServiceError::not_found("candidate not found")),
        }
    }

    async fn create_retrieval(
        &self,
        context: MemoryAppRequestContext,
        request: sdkwork_memory_contract::MemoryRetrievalRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRetrievalResult> {
        MemoryOpenApi::create_retrieval(self, Self::to_open_context(&context), request).await
    }

    async fn retrieve_retrieval(
        &self,
        context: MemoryAppRequestContext,
        retrieval_id: u64,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRetrievalResult> {
        MemoryOpenApi::retrieve_retrieval(self, Self::to_open_context(&context), retrieval_id).await
    }

    async fn create_extraction(
        &self,
        context: MemoryAppRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        MemoryOpenApi::create_extraction(self, Self::to_open_context(&context), request).await
    }

    async fn approve_candidate(
        &self,
        context: MemoryAppRequestContext,
        candidate_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("candidate not found"))?;
        crate::access::assert_actor_can_access_space_i64(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            existing.space_id,
        )
        .await?;
        MemoryBackendApi::approve_candidate(
            self,
            MemoryBackendRequestContext {
                tenant_id: context.tenant_id,
                operator_id: context.actor_id,
            },
            candidate_id,
            request,
        )
        .await
    }

    async fn reject_candidate(
        &self,
        context: MemoryAppRequestContext,
        candidate_id: u64,
        request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("candidate not found"))?;
        crate::access::assert_actor_can_access_space_i64(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            existing.space_id,
        )
        .await?;
        MemoryBackendApi::reject_candidate(
            self,
            MemoryBackendRequestContext {
                tenant_id: context.tenant_id,
                operator_id: context.actor_id,
            },
            candidate_id,
            request,
        )
        .await
    }

    async fn create_context_pack(
        &self,
        context: MemoryAppRequestContext,
        request: sdkwork_memory_contract::MemoryContextPackRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryContextPack> {
        MemoryOpenApi::create_context_pack(self, Self::to_open_context(&context), request).await
    }

    async fn retrieve_context_pack(
        &self,
        context: MemoryAppRequestContext,
        context_pack_id: u64,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryContextPack> {
        MemoryOpenApi::retrieve_context_pack(self, Self::to_open_context(&context), context_pack_id)
            .await
    }

    async fn create_feedback(
        &self,
        context: MemoryAppRequestContext,
        request: sdkwork_memory_contract::MemoryFeedbackRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryFeedback> {
        MemoryOpenApi::create_feedback(self, Self::to_open_context(&context), request).await
    }

    async fn list_habits(
        &self,
        context: MemoryAppRequestContext,
        query: ListHabitsQuery,
    ) -> MemoryServiceResult<MemoryHabitList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = crate::access::require_list_space_id(query.space_id)?;
        crate::access::assert_actor_can_access_space(
            &self.runtime_data_plane,
            &Self::to_open_context(&context),
            space_id,
        )
        .await?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_habits_for_tenant(
                tenant_id,
                Some(space_id as i64),
                query.stage.as_deref(),
                query.q.as_deref(),
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.habit_id.clone());
        let items = page_rows
            .into_iter()
            .map(Self::map_habit)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryHabitList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn retrieve_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
    ) -> MemoryServiceResult<MemoryHabit> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        match self
            .store
            .retrieve_habit_for_tenant(tenant_id, &habit_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
        {
            Some(row) => {
                crate::access::assert_actor_can_access_space_i64(
                    &self.runtime_data_plane,
                    &Self::to_open_context(&context),
                    row.space_id,
                )
                .await?;
                Self::map_habit(row)
            }
            None => Err(MemoryServiceError::not_found("habit not found")),
        }
    }

    async fn update_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
        request: MemoryHabitRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self.load_habit_row(tenant_id, habit_id).await?;
        self.assert_habit_actor_access(&context, &existing).await?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id: existing.space_id,
            organization_id: context.organization_id.map(|value| value as i64),
            user_id: context.actor_id.map(|value| value as i64),
        };
        let user_id = existing.user_id;
        self.runtime_data_plane
            .upsert_habit(UpsertMemoryHabitCommand {
                scope,
                habit_id: existing.habit_id.clone(),
                user_id,
                habit_key: existing.habit_key.clone(),
                habit_type: existing.habit_type.clone(),
                description: request.description.unwrap_or(existing.description),
                stage: request.stage.unwrap_or(existing.stage),
                strength: existing.strength,
                confidence: existing.confidence,
                support_count: existing.support_count,
                metadata_json: request
                    .metadata
                    .map(|value| value.to_string())
                    .or(existing.metadata_json),
            })
            .await?;
        self.retrieve_habit(context, habit_id).await
    }

    async fn confirm_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self.load_habit_row(tenant_id, habit_id).await?;
        self.assert_habit_actor_access(&context, &existing).await?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id: existing.space_id,
            organization_id: context.organization_id.map(|value| value as i64),
            user_id: context.actor_id.map(|value| value as i64),
        };
        self.runtime_data_plane
            .promote_habit(PromoteMemoryHabitCommand {
                scope,
                user_id: existing.user_id,
                habit_key: existing.habit_key.clone(),
                promoted_memory_id: None,
            })
            .await?;
        self.retrieve_habit(context, habit_id).await
    }

    async fn reject_habit(
        &self,
        context: MemoryAppRequestContext,
        habit_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryHabit> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self.load_habit_row(tenant_id, habit_id).await?;
        self.assert_habit_actor_access(&context, &existing).await?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id: existing.space_id,
            organization_id: context.organization_id.map(|value| value as i64),
            user_id: context.actor_id.map(|value| value as i64),
        };
        self.runtime_data_plane
            .decay_habit(DecayMemoryHabitCommand {
                scope,
                user_id: existing.user_id,
                habit_key: existing.habit_key.clone(),
                strength_delta: existing.strength.max(0.1),
            })
            .await?;
        self.retrieve_habit(context, habit_id).await
    }

    async fn retrieve_learning_settings(
        &self,
        context: MemoryAppRequestContext,
    ) -> MemoryServiceResult<MemoryLearningSettings> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let user_id = context.actor_id.map(|value| value as i64);
        let Some(raw) = self
            .store
            .retrieve_tenant_preference_json(tenant_id, user_id, LEARNING_SETTINGS_KEY)
            .await
            .map_err(OpenMemoryService::map_store_error)?
        else {
            return Ok(default_learning_settings());
        };
        serde_json::from_str(&raw).map_err(|error| {
            MemoryServiceError::storage(format!("learning settings decode failed: {error}"))
        })
    }

    async fn update_learning_settings(
        &self,
        context: MemoryAppRequestContext,
        patch: MemoryLearningSettingsPatch,
    ) -> MemoryServiceResult<MemoryLearningSettings> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let user_id = context.actor_id.map(|value| value as i64);
        let mut settings = self.retrieve_learning_settings(context.clone()).await?;
        if let Some(auto_promote_candidates) = patch.auto_promote_candidates {
            settings.auto_promote_candidates = auto_promote_candidates;
        }
        if let Some(habit_learning_enabled) = patch.habit_learning_enabled {
            settings.habit_learning_enabled = habit_learning_enabled;
        }
        settings.updated_at = platform::current_timestamp();
        let encoded = serde_json::to_string(&settings).map_err(|error| {
            MemoryServiceError::storage(format!("learning settings encode failed: {error}"))
        })?;
        let preference_id = i64::try_from(self.next_id()?)
            .map_err(|_| MemoryServiceError::storage("generated preference id out of range"))?;
        self.store
            .upsert_tenant_preference_json(
                preference_id,
                tenant_id,
                user_id,
                LEARNING_SETTINGS_KEY,
                &encoded,
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        Ok(settings)
    }
}

#[async_trait]
impl MemoryBackendApi for OpenMemoryService {
    async fn list_spaces(
        &self,
        context: MemoryBackendRequestContext,
        query: ListSpacesQuery,
    ) -> MemoryServiceResult<MemorySpaceList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let cursor_space_id = self
            .resolve_space_list_cursor(tenant_id, query.cursor.as_deref())
            .await?;
        let rows = self
            .store
            .list_spaces_for_tenant(tenant_id, page_size, cursor_space_id, None)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let items = rows
            .into_iter()
            .take(page_size as usize)
            .map(Self::map_space)
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = items.last().and_then(|space| space.uuid.clone());
        Ok(MemorySpaceList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn retrieve_space(
        &self,
        context: MemoryBackendRequestContext,
        space_id: u64,
    ) -> MemoryServiceResult<MemorySpace> {
        MemoryAppApi::retrieve_space(
            self,
            MemoryAppRequestContext {
                tenant_id: context.tenant_id,
                actor_id: context.operator_id,
                organization_id: None,
                session_id: None,
            },
            space_id,
        )
        .await
    }

    async fn update_space(
        &self,
        context: MemoryBackendRequestContext,
        space_id: u64,
        request: MemorySpaceRequest,
    ) -> MemoryServiceResult<MemorySpace> {
        MemoryAppApi::update_space(
            self,
            MemoryAppRequestContext {
                tenant_id: context.tenant_id,
                actor_id: context.operator_id,
                organization_id: None,
                session_id: None,
            },
            space_id,
            request,
        )
        .await
    }

    async fn list_memories(
        &self,
        context: MemoryBackendRequestContext,
        query: ListMemoriesQuery,
    ) -> MemoryServiceResult<MemoryRecordList> {
        MemoryOpenApi::list_memories(self, Self::to_open_context_backend(&context), query).await
    }

    async fn retrieve_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        MemoryOpenApi::retrieve_memory(
            self,
            Self::to_open_context_backend(&context),
            memory_id,
            space_id,
        )
        .await
    }

    async fn update_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        space_id: u64,
        patch: sdkwork_memory_contract::MemoryRecordPatch,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        MemoryOpenApi::update_memory(
            self,
            Self::to_open_context_backend(&context),
            memory_id,
            space_id,
            patch,
        )
        .await
    }

    async fn list_events(
        &self,
        context: MemoryBackendRequestContext,
        query: ListEventsQuery,
    ) -> MemoryServiceResult<MemoryEventList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_open_api_events_for_tenant(
                tenant_id,
                query.space_id.map(|value| value as i64),
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.event_id.clone());
        let items = page_rows
            .into_iter()
            .map(OpenMemoryService::map_event)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryEventList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn retrieve_event(
        &self,
        context: MemoryBackendRequestContext,
        event_id: u64,
        space_id: u64,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryEvent> {
        MemoryOpenApi::retrieve_event(
            self,
            Self::to_open_context_backend(&context),
            event_id,
            space_id,
        )
        .await
    }

    async fn list_candidates(
        &self,
        context: MemoryBackendRequestContext,
        query: ListCandidatesQuery,
    ) -> MemoryServiceResult<MemoryCandidateList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let page = self
            .runtime_data_plane
            .list_candidates(ListMemoryCandidatesQuery {
                tenant_id,
                space_id: query.space_id.map(platform::space_id_i64).transpose()?,
                page_size: page_size as u32,
                cursor: query.cursor,
            })
            .await?;
        let items = page
            .items
            .into_iter()
            .map(Self::map_candidate)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryCandidateList {
            items,
            page_info: platform::memory_cursor_page_info(
                page_size,
                page.has_more,
                page.next_cursor,
            ),
        })
    }

    async fn approve_candidate(
        &self,
        context: MemoryBackendRequestContext,
        candidate_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
            .ok_or_else(|| MemoryServiceError::not_found("candidate not found"))?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id: existing.space_id,
            organization_id: None,
            user_id: context.operator_id.map(|value| value as i64),
        };
        self.approve_candidate_with_promotion(tenant_id, scope, candidate_id, context.operator_id)
            .await
    }

    async fn reject_candidate(
        &self,
        context: MemoryBackendRequestContext,
        candidate_id: u64,
        _request: MemoryReviewRequest,
    ) -> MemoryServiceResult<MemoryCandidate> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let existing = self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
            .ok_or_else(|| {
                sdkwork_memory_contract::MemoryServiceError::not_found("candidate not found")
            })?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id: existing.space_id,
            organization_id: None,
            user_id: context.operator_id.map(|value| value as i64),
        };
        self.runtime_data_plane
            .reject_candidate(RejectMemoryCandidateCommand {
                scope,
                candidate_id: candidate_id.to_string(),
                decision_reason: None,
                decided_by: context.operator_id.map(|value| value as i64),
            })
            .await?;
        match self
            .runtime_data_plane
            .retrieve_candidate_detail(RetrieveMemoryCandidateDetailQuery {
                tenant_id,
                candidate_id: candidate_id.to_string(),
            })
            .await?
        {
            Some(row) => Self::map_candidate_api_detail(row),
            None => Err(MemoryServiceError::not_found("candidate not found")),
        }
    }

    async fn retrieve_provider_health(
        &self,
        context: MemoryBackendRequestContext,
    ) -> MemoryServiceResult<MemoryProviderHealth> {
        MemoryOpenApi::retrieve_provider_health(self, Self::to_open_context_backend(&context)).await
    }

    async fn list_retrieval_traces(
        &self,
        context: MemoryBackendRequestContext,
        query: ListRetrievalTracesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalTraceList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_retrieval_traces_for_tenant(
                tenant_id,
                query.space_id.map(|value| value as i64),
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.trace_id.clone());
        let items = page_rows
            .into_iter()
            .map(Self::map_trace_summary)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryRetrievalTraceList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn retrieve_retrieval_trace(
        &self,
        context: MemoryBackendRequestContext,
        trace_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalTrace> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let trace = self
            .store
            .retrieve_retrieval_trace_for_tenant(tenant_id, &trace_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("retrieval trace not found"))?;
        Ok(MemoryRetrievalTrace {
            trace_id,
            space_id: None,
            retrieval_profile_id: None,
            actor_id: trace.actor_id,
            query_text: trace.query_text,
            query_hash: trace.query_hash,
            result_count: trace.result_count as i32,
            degraded: trace.degraded,
            created_at: platform::current_timestamp(),
        })
    }

    async fn list_audit_logs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAuditLogsQuery,
    ) -> MemoryServiceResult<MemoryAuditLogList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = crate::platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_audit_logs_for_tenant(
                tenant_id,
                query.action.as_deref(),
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.audit_id.clone());
        Ok(MemoryAuditLogList {
            items: page_rows
                .into_iter()
                .map(map_audit_log)
                .collect::<MemoryServiceResult<Vec<_>>>()?,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    async fn supersede_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        request: MemoryRecordRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        self.backend_supersede_memory(context, memory_id, request)
            .await
    }

    async fn create_extraction_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_create_extraction_job(context, request).await
    }

    async fn list_extraction_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        self.backend_list_extraction_jobs(context, query).await
    }

    async fn retrieve_extraction_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_retrieve_extraction_job(context, job_id).await
    }

    async fn create_consolidation_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_create_consolidation_job(context, request)
            .await
    }

    async fn list_consolidation_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        self.backend_list_governance_learning_jobs(context, query, "consolidation_job")
            .await
    }

    async fn retrieve_consolidation_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_retrieve_consolidation_job(context, job_id)
            .await
    }

    async fn list_indexes(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryIndexList> {
        self.backend_list_indexes(context, query).await
    }

    async fn create_index(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        self.backend_create_index(context, request).await
    }

    async fn retrieve_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
    ) -> MemoryServiceResult<MemoryIndex> {
        self.backend_retrieve_index(context, index_id).await
    }

    async fn update_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
        request: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        self.backend_update_index(context, index_id, request).await
    }

    async fn rebuild_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_rebuild_index(context, index_id).await
    }

    async fn list_retrieval_profiles(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalProfileList> {
        self.backend_list_retrieval_profiles(context, query).await
    }

    async fn create_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        self.backend_create_retrieval_profile(context, request)
            .await
    }

    async fn retrieve_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        self.backend_retrieve_retrieval_profile(context, profile_id)
            .await
    }

    async fn update_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
        request: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        self.backend_update_retrieval_profile(context, profile_id, request)
            .await
    }

    async fn list_implementation_profiles(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryImplementationProfileList> {
        self.backend_list_implementation_profiles(context, query)
            .await
    }

    async fn create_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        self.backend_create_implementation_profile(context, request)
            .await
    }

    async fn retrieve_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        self.backend_retrieve_implementation_profile(context, profile_id)
            .await
    }

    async fn update_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
        request: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        self.backend_update_implementation_profile(context, profile_id, request)
            .await
    }

    async fn list_provider_bindings(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryProviderBindingList> {
        self.backend_list_provider_bindings(context, query).await
    }

    async fn create_provider_binding(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        self.backend_create_provider_binding(context, request).await
    }

    async fn update_provider_binding(
        &self,
        context: MemoryBackendRequestContext,
        binding_id: u64,
        request: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        self.backend_update_provider_binding(context, binding_id, request)
            .await
    }

    async fn list_eval_runs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryEvalRunList> {
        self.backend_list_eval_runs(context, query).await
    }

    async fn create_eval_run(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryEvalRunRequest,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        self.backend_create_eval_run(context, request).await
    }

    async fn retrieve_eval_run(
        &self,
        context: MemoryBackendRequestContext,
        eval_run_id: u64,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        self.backend_retrieve_eval_run(context, eval_run_id).await
    }

    async fn create_retention_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryRetentionJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_create_retention_job(context, request).await
    }

    async fn list_retention_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        self.backend_list_governance_learning_jobs(context, query, "retention_job")
            .await
    }

    async fn retrieve_retention_job(
        &self,
        context: MemoryBackendRequestContext,
        retention_job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_retrieve_retention_job(context, retention_job_id)
            .await
    }

    async fn create_migration_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryMigrationJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_create_migration_job(context, request).await
    }

    async fn list_migration_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        self.backend_list_governance_learning_jobs(context, query, "migration_job")
            .await
    }

    async fn retrieve_migration_job(
        &self,
        context: MemoryBackendRequestContext,
        migration_job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        self.backend_retrieve_migration_job(context, migration_job_id)
            .await
    }
}

fn map_audit_log(row: NativeSqlAuditLogRow) -> MemoryServiceResult<MemoryAuditLog> {
    Ok(MemoryAuditLog {
        audit_log_id: platform::parse_required_numeric_id(&row.audit_id, "auditLogId")?,
        actor_type: row.actor_type,
        actor_id: row.actor_id,
        action: row.action,
        resource_type: row.resource_type,
        resource_id: Some(row.resource_id),
        request_id: None,
        trace_id: None,
        result: row.result,
        reason: None,
        metadata: None,
        created_at: row.created_at,
    })
}

fn export_payload_bytes(payload: serde_json::Value) -> MemoryServiceResult<Vec<u8>> {
    match payload {
        serde_json::Value::String(body) => Ok(body.into_bytes()),
        other => serde_json::to_vec(&other).map_err(|error| {
            MemoryServiceError::storage(format!("export payload encode failed: {error}"))
        }),
    }
}

const ABSOLUTE_MAX_EXPORT_BYTES: usize = 256 * 1024 * 1024;

fn export_payload_byte_limit(drive_export: bool) -> usize {
    let (key, default) = if drive_export {
        ("SDKWORK_MEMORY_DRIVE_EXPORT_MAX_BYTES", 64 * 1024 * 1024)
    } else {
        ("SDKWORK_MEMORY_INLINE_EXPORT_MAX_BYTES", 4 * 1024 * 1024)
    };
    std::env::var(key)
        .ok()
        .and_then(|value| sdkwork_utils_rust::parse_int(&value))
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(default)
        .clamp(1_024, ABSOLUTE_MAX_EXPORT_BYTES)
}

fn validate_export_payload_size(
    payload: &serde_json::Value,
    max_payload_bytes: usize,
) -> MemoryServiceResult<()> {
    let mut writer = ExportSizeGuard {
        bytes: 0,
        max_bytes: max_payload_bytes,
    };
    let encoded = match payload {
        serde_json::Value::String(body) => body.len(),
        other => {
            serde_json::to_writer(&mut writer, other).map_err(|error| {
                if writer.bytes > writer.max_bytes {
                    MemoryServiceError::validation(format!(
                        "export payload exceeds the configured {max_payload_bytes} byte limit"
                    ))
                } else {
                    MemoryServiceError::storage(format!(
                        "export payload size validation failed: {error}"
                    ))
                }
            })?;
            writer.bytes
        }
    };
    if encoded > max_payload_bytes {
        return Err(MemoryServiceError::validation(format!(
            "export payload exceeds the configured {max_payload_bytes} byte limit"
        )));
    }
    Ok(())
}

struct ExportSizeGuard {
    bytes: usize,
    max_bytes: usize,
}

impl std::io::Write for ExportSizeGuard {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buffer.len());
        if self.bytes > self.max_bytes {
            return Err(std::io::Error::other("export payload byte limit exceeded"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn export_content_type(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "json" => "application/json".to_string(),
        "jsonl" => "application/x-ndjson".to_string(),
        "markdown" => "text/markdown".to_string(),
        other => format!("application/{other}"),
    }
}

fn export_file_name(job_id: u64, format: &str) -> String {
    let normalized = format.trim().to_ascii_lowercase();
    let extension = match normalized.as_str() {
        "json" => "json",
        "jsonl" => "jsonl",
        "markdown" => "md",
        other => other,
    };
    format!("memory-export-{job_id}.{extension}")
}
