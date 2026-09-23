use sdkwork_memory_contract::{
    ListAdminResourcesQuery, ListJobsQuery, MemoryBackendRequestContext, MemoryEvalRun,
    MemoryEvalRunList, MemoryEvalRunRequest, MemoryExtractionRequest, MemoryImplementationProfile,
    MemoryImplementationProfileList, MemoryImplementationProfileRequest, MemoryIndex,
    MemoryIndexList, MemoryIndexRequest, MemoryLearningJob, MemoryLearningJobList,
    MemoryMigrationJobRequest, MemoryOpenApi, MemoryProviderBinding, MemoryProviderBindingList,
    MemoryProviderBindingRequest, MemoryRecordRequest, MemoryRetentionJobRequest,
    MemoryRetrievalProfile, MemoryRetrievalProfileList, MemoryRetrievalProfileRequest,
    MemoryServiceError, MemoryServiceResult, PageInfo,
};
use sdkwork_memory_plugin_native_sql::{
    ConsolidateDuplicateRecordsCommand, InsertMemoryEvalRunCommand, NativeSqlEvalRunRow,
    NativeSqlImplementationProfileRow, NativeSqlMemoryIndexRow, NativeSqlProviderBindingRow,
    NativeSqlRetrievalProfileRow,
};
use sdkwork_memory_spi::{
    MemoryImplementationKind, MemoryScopeContext, SupersedeCanonicalMemoryAtomicCommand,
};
use serde::Serialize;
use serde_json::Value;

use crate::open_api::OpenMemoryService;
use crate::platform;
use crate::retrieval_profile;

const RT_EXTRACTION_JOB: &str = "extraction_job";
const RT_CONSOLIDATION_JOB: &str = "consolidation_job";
const RT_RETENTION_JOB: &str = "retention_job";
const RT_MIGRATION_JOB: &str = "migration_job";

impl OpenMemoryService {
    pub(crate) async fn backend_list_governance_learning_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
        resource_type: &str,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        if query.space_id.is_some() {
            return Err(MemoryServiceError::validation(
                "spaceId is not supported for this job collection",
            ));
        }
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_governance_jobs_for_tenant(
                tenant_id,
                resource_type,
                None,
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
                let metadata = row.metadata_json.ok_or_else(|| {
                    MemoryServiceError::storage("learning job metadata is missing")
                })?;
                serde_json::from_str(&metadata).map_err(|error| {
                    MemoryServiceError::storage(format!(
                        "learning job metadata decode failed: {error}"
                    ))
                })
            })
            .collect::<MemoryServiceResult<Vec<_>>>()?;
        Ok(MemoryLearningJobList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    fn validate_implementation_profile_request(
        request: &MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<()> {
        if request.name.trim().is_empty() {
            return Err(MemoryServiceError::validation(
                "implementation profile name must not be empty",
            ));
        }
        let implementation_kind = serde_json::from_value::<MemoryImplementationKind>(
            Value::String(request.implementation_kind.clone()),
        )
        .map_err(|_| {
            MemoryServiceError::validation(format!(
                "unsupported implementationKind: {}",
                request.implementation_kind
            ))
        })?;
        if !matches!(request.role.as_str(), "primary" | "secondary" | "shadow") {
            return Err(MemoryServiceError::validation(
                "implementation profile role must be primary, secondary, or shadow",
            ));
        }
        if let Some(status) = request.status.as_deref() {
            if !matches!(
                status,
                "active" | "disabled" | "migrating" | "deprecated" | "deleted"
            ) {
                return Err(MemoryServiceError::validation(
                    "implementation profile status is invalid",
                ));
            }
        }
        if !request.capabilities.is_object() {
            return Err(MemoryServiceError::validation(
                "implementation profile capabilities must be an object",
            ));
        }
        let status = request.status.as_deref().unwrap_or("active");
        if !crate::implementation_migration::is_production_qualified_implementation_kind(
            &request.implementation_kind,
        ) {
            if request.role != "shadow" || status != "disabled" {
                return Err(MemoryServiceError::validation(format!(
                    "implementationKind {} is evaluation-only and must use role=shadow with status=disabled",
                    request.implementation_kind
                )));
            }
            if request
                .capabilities
                .get("productionQualified")
                .and_then(Value::as_bool)
                != Some(false)
            {
                return Err(MemoryServiceError::validation(
                    "evaluation-only implementation capabilities must declare productionQualified=false",
                ));
            }
        } else {
            for unsupported_claim in [
                "embeddingRequired",
                "vector",
                "graph",
                "rerank",
                "externalProvider",
                "eventSourced",
                "liveDynamicCutover",
            ] {
                if request
                    .capabilities
                    .get(unsupported_claim)
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    return Err(MemoryServiceError::validation(format!(
                        "qualified {:?} profile cannot claim unavailable capability {unsupported_claim}",
                        implementation_kind
                    )));
                }
            }
        }
        for (name, value) in [
            ("config", request.config.as_ref()),
            ("rollout", request.rollout.as_ref()),
        ] {
            if let Some(value) = value {
                Self::reject_literal_profile_secret(value, name)?;
            }
        }
        Ok(())
    }

    fn reject_literal_profile_secret(value: &Value, path: &str) -> MemoryServiceResult<()> {
        match value {
            Value::Object(object) => {
                for (key, child) in object {
                    let key_lower = key.to_ascii_lowercase();
                    if matches!(
                        key_lower.as_str(),
                        "password" | "token" | "api_key" | "apikey" | "private_key"
                    ) && child.is_string()
                    {
                        return Err(MemoryServiceError::validation(format!(
                            "implementation profile {path}.{key} must reference a secret, not contain a literal"
                        )));
                    }
                    Self::reject_literal_profile_secret(child, &format!("{path}.{key}"))?;
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    Self::reject_literal_profile_secret(child, &format!("{path}[{index}]"))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn admin_page_info(page_size: i32, has_more: bool, next_cursor: Option<String>) -> PageInfo {
        platform::memory_cursor_page_info(page_size, has_more, next_cursor)
    }

    fn page_size_from_query(query: &ListAdminResourcesQuery) -> MemoryServiceResult<i32> {
        crate::platform::validated_page_size(query.page_size)
    }

    fn parse_row_id(id: &str) -> MemoryServiceResult<u64> {
        platform::parse_numeric_id(id).ok_or_else(|| {
            MemoryServiceError::storage("admin resource id must be numeric".to_string())
        })
    }

    fn optional_space_id_i64(space_id: Option<u64>) -> MemoryServiceResult<Option<i64>> {
        platform::optional_u64_as_i64(space_id)
    }

    fn optional_u64_as_i64(value: Option<u64>) -> MemoryServiceResult<Option<i64>> {
        platform::optional_u64_as_i64(value)
    }

    fn optional_i64_as_u64(value: Option<i64>) -> Option<u64> {
        platform::optional_i64_as_u64(value)
    }

    fn encode_optional_json(value: &Option<Value>) -> MemoryServiceResult<Option<String>> {
        value
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| MemoryServiceError::storage(format!("json encode failed: {error}")))
    }

    fn decode_optional_json(raw: Option<&str>) -> Option<Value> {
        raw.and_then(|value| serde_json::from_str(value).ok())
    }

    fn optional_json_patch(value: &Option<Value>) -> Option<Option<String>> {
        Some(
            value
                .as_ref()
                .and_then(|json| serde_json::to_string(json).ok()),
        )
    }

    fn map_memory_index(row: &NativeSqlMemoryIndexRow) -> MemoryServiceResult<MemoryIndex> {
        let config = row
            .config_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok());
        Ok(MemoryIndex {
            index_id: Self::parse_row_id(&row.index_uuid)?,
            space_id: row
                .space_id
                .map(|value| u64::try_from(value.max(0)).unwrap_or(0)),
            index_kind: row.index_kind.clone(),
            implementation_profile_id: Self::optional_i64_as_u64(row.implementation_profile_id),
            provider_binding_id: Self::optional_i64_as_u64(row.provider_binding_id),
            schema_version: row.schema_version.clone(),
            status: row.status.clone(),
            config,
            last_rebuilt_at: row.last_rebuilt_at.clone(),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            version: u64::try_from(row.version.max(0)).unwrap_or(0),
        })
    }

    fn map_retrieval_profile(
        row: &NativeSqlRetrievalProfileRow,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        let retrievers: Value = serde_json::from_str(&row.retrievers_json).map_err(|error| {
            MemoryServiceError::storage(format!(
                "retrieval profile retrievers decode failed: {error}"
            ))
        })?;
        Ok(MemoryRetrievalProfile {
            retrieval_profile_id: Self::parse_row_id(&row.profile_uuid)?,
            space_id: row
                .space_id
                .map(|value| u64::try_from(value.max(0)).unwrap_or(0)),
            name: row.name.clone(),
            strategy: row.strategy.clone(),
            retrievers,
            fusion_policy: Self::decode_optional_json(row.fusion_policy_json.as_deref()),
            rerank_policy: Self::decode_optional_json(row.rerank_policy_json.as_deref()),
            top_k: row.top_k,
            context_budget_tokens: row.context_budget_tokens,
            status: row.status.clone(),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            version: u64::try_from(row.version.max(0)).unwrap_or(0),
        })
    }

    fn map_implementation_profile(
        row: &NativeSqlImplementationProfileRow,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        let capabilities: Value = serde_json::from_str(&row.capability_json).map_err(|error| {
            MemoryServiceError::storage(format!(
                "implementation profile capabilities decode failed: {error}"
            ))
        })?;
        Ok(MemoryImplementationProfile {
            implementation_profile_id: Self::parse_row_id(&row.profile_uuid)?,
            name: row.name.clone(),
            implementation_kind: row.implementation_kind.clone(),
            role: row.role.clone(),
            status: row.status.clone(),
            capabilities,
            config: Self::decode_optional_json(row.config_json.as_deref()),
            rollout: Self::decode_optional_json(row.rollout_json.as_deref()),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            version: u64::try_from(row.version.max(0)).unwrap_or(0),
        })
    }

    pub(crate) fn parse_json_value(raw: &str, field: &str) -> MemoryServiceResult<Value> {
        serde_json::from_str(raw)
            .map_err(|error| MemoryServiceError::storage(format!("{field} decode failed: {error}")))
    }

    pub(crate) fn map_provider_binding(
        row: &NativeSqlProviderBindingRow,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        Ok(MemoryProviderBinding {
            provider_binding_id: Self::parse_row_id(&row.binding_uuid)?,
            provider_kind: row.provider_kind.clone(),
            provider_code: row.provider_code.clone(),
            display_name: row.display_name.clone(),
            endpoint_ref: row.endpoint_ref.clone(),
            secret_ref: row.secret_ref.clone(),
            model_ref: row.model_ref.clone(),
            capabilities: Self::parse_json_value(
                &row.capabilities_json,
                "provider binding capabilities",
            )?,
            config: Self::decode_optional_json(row.config_json.as_deref()),
            health_state: row.health_state.clone(),
            last_health_at: row.last_health_at.clone(),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
            version: u64::try_from(row.version.max(0)).unwrap_or(0),
        })
    }

    pub(crate) fn map_provider_binding_public(
        row: &NativeSqlProviderBindingRow,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        let mut binding = Self::map_provider_binding(row)?;
        binding.secret_ref = None;
        binding.endpoint_ref = None;
        binding.config = None;
        Ok(binding)
    }

    fn map_eval_run(row: &NativeSqlEvalRunRow) -> MemoryServiceResult<MemoryEvalRun> {
        let metrics = Self::decode_optional_json(row.metrics_json.as_deref());
        let result = if matches!(row.state.as_str(), "succeeded" | "failed" | "skipped") {
            Self::decode_optional_json(row.result_json.as_deref())
        } else {
            None
        };
        Ok(MemoryEvalRun {
            eval_run_id: Self::parse_row_id(&row.eval_run_uuid)?,
            eval_type: row.eval_type.clone(),
            state: row.state.clone(),
            dataset_ref: row.dataset_ref.clone(),
            profile_ref: row.profile_ref.clone(),
            metrics,
            result,
            started_at: row.started_at.clone(),
            finished_at: row.finished_at.clone(),
            created_at: row.created_at.clone(),
            updated_at: row.updated_at.clone(),
        })
    }

    fn new_learning_job(
        job_id: u64,
        job_type: &str,
        state: &str,
        space_id: Option<u64>,
        request: &impl Serialize,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let timestamp = platform::current_timestamp();
        let result = serde_json::to_value(request).map_err(|error| {
            MemoryServiceError::storage(format!("governance job request encode failed: {error}"))
        })?;
        Ok(MemoryLearningJob {
            job_id,
            space_id,
            job_type: job_type.to_string(),
            state: state.to_string(),
            priority: 0,
            result: Some(result),
            error: None,
            started_at: None,
            finished_at: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            version: None,
        })
    }

    async fn save_governance_entity(
        &self,
        tenant_id: i64,
        resource_type: &str,
        entity_id: &str,
        entity: &impl Serialize,
    ) -> MemoryServiceResult<()> {
        let metadata = serde_json::to_string(entity).map_err(|error| {
            MemoryServiceError::storage(format!("governance entity encode failed: {error}"))
        })?;
        self.store
            .save_admin_config_entity(tenant_id, resource_type, entity_id, &metadata)
            .await
            .map_err(OpenMemoryService::map_store_error)
    }

    async fn load_governance_entity<T: serde::de::DeserializeOwned>(
        &self,
        tenant_id: i64,
        resource_type: &str,
        entity_id: &str,
    ) -> MemoryServiceResult<T> {
        let metadata = self
            .store
            .retrieve_admin_config_entity(tenant_id, resource_type, entity_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("governance entity not found"))?;
        serde_json::from_str(&metadata).map_err(|error| {
            MemoryServiceError::storage(format!("governance entity decode failed: {error}"))
        })
    }

    pub(crate) async fn backend_list_indexes(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryIndexList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = Self::page_size_from_query(&query)?;
        self.store
            .ensure_default_keyword_index_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let rows = self
            .store
            .list_mem_indexes_for_tenant(
                tenant_id,
                Self::optional_space_id_i64(query.space_id)?,
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.index_uuid.clone());
        let items = page_rows
            .iter()
            .map(Self::map_memory_index)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryIndexList {
            items,
            page_info: Self::admin_page_info(page_size, has_more, next_cursor),
        })
    }

    pub(crate) async fn backend_create_index(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let index_id = self.next_id()?.to_string();
        let config_json = request
            .config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("index config encode failed: {error}"))
            })?;
        self.store
            .insert_mem_index(
                tenant_id,
                &index_id,
                Self::optional_space_id_i64(request.space_id)?,
                &request.index_kind,
                &request.schema_version,
                request.status.as_deref().unwrap_or("active"),
                config_json.as_deref(),
                Self::optional_u64_as_i64(request.implementation_profile_id)?,
                Self::optional_u64_as_i64(request.provider_binding_id)?,
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_index_for_tenant(tenant_id, &index_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("created index could not be loaded"))?;
        Self::map_memory_index(&row)
    }

    pub(crate) async fn backend_retrieve_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
    ) -> MemoryServiceResult<MemoryIndex> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        self.store
            .ensure_default_keyword_index_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_index_for_tenant(tenant_id, &index_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("memory index not found"))?;
        Self::map_memory_index(&row)
    }

    pub(crate) async fn backend_update_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
        request: MemoryIndexRequest,
    ) -> MemoryServiceResult<MemoryIndex> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let config_json = request
            .config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("index config encode failed: {error}"))
            })?;
        let row = self
            .store
            .update_mem_index_for_tenant(
                tenant_id,
                &index_id.to_string(),
                request.status.as_deref(),
                config_json.as_deref(),
                None,
                Some(Self::optional_u64_as_i64(
                    request.implementation_profile_id,
                )?),
                Some(Self::optional_u64_as_i64(request.provider_binding_id)?),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("memory index not found"))?;
        Self::map_memory_index(&row)
    }

    pub(crate) async fn backend_rebuild_index(
        &self,
        context: MemoryBackendRequestContext,
        index_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let rebuilt_at = platform::current_timestamp();
        let row = self
            .store
            .update_mem_index_for_tenant(
                tenant_id,
                &index_id.to_string(),
                Some("active"),
                None,
                Some(rebuilt_at.as_str()),
                None,
                None,
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("memory index not found"))?;
        let index = Self::map_memory_index(&row)?;
        self.store
            .ensure_default_keyword_index_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let rebuilt_records = if let Some(space_id) = row.space_id {
            self.store
                .rebuild_record_search_indexes_for_space(tenant_id, space_id)
                .await
                .map_err(OpenMemoryService::map_store_error)?
        } else {
            self.store
                .rebuild_all_record_search_indexes(tenant_id)
                .await
                .map_err(OpenMemoryService::map_store_error)?
        };
        let job_id = self.next_id()?;
        let finished_at = platform::current_timestamp();
        let mut job =
            Self::new_learning_job(job_id, "index_rebuild", "succeeded", index.space_id, &index)?;
        job.result = Some(serde_json::json!({
            "indexId": index_id,
            "lastRebuiltAt": row.last_rebuilt_at,
            "status": row.status,
            "rebuiltRecords": rebuilt_records,
        }));
        job.finished_at = Some(finished_at.clone());
        job.updated_at = finished_at;
        self.persist_governance_job(
            tenant_id,
            context.operator_id,
            job_id,
            "index_rebuild_job",
            "indexes.rebuild",
            &job,
        )
        .await?;
        Ok(job)
    }

    pub(crate) async fn backend_list_retrieval_profiles(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryRetrievalProfileList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = Self::page_size_from_query(&query)?;
        self.store
            .ensure_default_retrieval_profile_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let rows = self
            .store
            .list_mem_retrieval_profiles_for_tenant(
                tenant_id,
                Self::optional_space_id_i64(query.space_id)?,
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.profile_uuid.clone());
        let items = page_rows
            .iter()
            .map(Self::map_retrieval_profile)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryRetrievalProfileList {
            items,
            page_info: Self::admin_page_info(page_size, has_more, next_cursor),
        })
    }

    pub(crate) async fn backend_create_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        retrieval_profile::validate_retrieval_retrievers(&request.retrievers)?;
        retrieval_profile::validate_retrieval_strategy(&request.strategy, &request.retrievers)?;
        retrieval_profile::validate_retrieval_limits(request.top_k, request.context_budget_tokens)?;
        retrieval_profile::resolve_retrieval_fusion_policy(request.fusion_policy.as_ref())?;
        let profile_id = self.next_id()?.to_string();
        let retrievers_json = serde_json::to_string(&request.retrievers).map_err(|error| {
            MemoryServiceError::storage(format!("retrievers encode failed: {error}"))
        })?;
        let fusion_policy_json = Self::encode_optional_json(&request.fusion_policy)?;
        let rerank_policy_json = Self::encode_optional_json(&request.rerank_policy)?;
        self.store
            .insert_mem_retrieval_profile(
                tenant_id,
                &profile_id,
                Self::optional_space_id_i64(request.space_id)?,
                &request.name,
                &request.strategy,
                &retrievers_json,
                fusion_policy_json.as_deref(),
                rerank_policy_json.as_deref(),
                request.top_k,
                request.context_budget_tokens,
                request.status.as_deref().unwrap_or("active"),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_retrieval_profile_for_tenant(tenant_id, &profile_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("created profile could not be loaded"))?;
        Self::map_retrieval_profile(&row)
    }

    pub(crate) async fn backend_retrieve_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        self.store
            .ensure_default_retrieval_profile_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_retrieval_profile_for_tenant(tenant_id, &profile_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("retrieval profile not found"))?;
        Self::map_retrieval_profile(&row)
    }

    pub(crate) async fn backend_update_retrieval_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
        request: MemoryRetrievalProfileRequest,
    ) -> MemoryServiceResult<MemoryRetrievalProfile> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        retrieval_profile::validate_retrieval_retrievers(&request.retrievers)?;
        retrieval_profile::validate_retrieval_strategy(&request.strategy, &request.retrievers)?;
        retrieval_profile::validate_retrieval_limits(request.top_k, request.context_budget_tokens)?;
        retrieval_profile::resolve_retrieval_fusion_policy(request.fusion_policy.as_ref())?;
        let retrievers_json = serde_json::to_string(&request.retrievers).ok();
        let fusion_policy_json = Self::optional_json_patch(&request.fusion_policy);
        let rerank_policy_json = Self::optional_json_patch(&request.rerank_policy);
        let row = self
            .store
            .update_mem_retrieval_profile_for_tenant(
                tenant_id,
                &profile_id.to_string(),
                Some(request.name.as_str()),
                Some(request.strategy.as_str()),
                retrievers_json.as_deref(),
                fusion_policy_json.as_ref().map(|value| value.as_deref()),
                rerank_policy_json.as_ref().map(|value| value.as_deref()),
                Some(request.top_k),
                Some(request.context_budget_tokens),
                request.status.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("retrieval profile not found"))?;
        Self::map_retrieval_profile(&row)
    }

    pub(crate) async fn backend_list_implementation_profiles(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryImplementationProfileList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = Self::page_size_from_query(&query)?;
        self.store
            .ensure_default_implementation_profile_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let rows = self
            .store
            .list_mem_implementation_profiles_for_tenant(
                tenant_id,
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.profile_uuid.clone());
        let items = page_rows
            .iter()
            .map(Self::map_implementation_profile)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryImplementationProfileList {
            items,
            page_info: Self::admin_page_info(page_size, has_more, next_cursor),
        })
    }

    pub(crate) async fn backend_create_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        Self::validate_implementation_profile_request(&request)?;
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let profile_id = self.next_id()?.to_string();
        let capability_json = serde_json::to_string(&request.capabilities).map_err(|error| {
            MemoryServiceError::storage(format!("capabilities encode failed: {error}"))
        })?;
        let config_json = Self::encode_optional_json(&request.config)?;
        let rollout_json = Self::encode_optional_json(&request.rollout)?;
        self.store
            .insert_mem_implementation_profile(
                tenant_id,
                &profile_id,
                &request.name,
                &request.implementation_kind,
                &request.role,
                request.status.as_deref().unwrap_or("active"),
                &capability_json,
                config_json.as_deref(),
                rollout_json.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_implementation_profile_for_tenant(tenant_id, &profile_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("created profile could not be loaded"))?;
        Self::map_implementation_profile(&row)
    }

    pub(crate) async fn backend_retrieve_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        self.store
            .ensure_default_implementation_profile_for_tenant(tenant_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_implementation_profile_for_tenant(tenant_id, &profile_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("implementation profile not found"))?;
        Self::map_implementation_profile(&row)
    }

    pub(crate) async fn backend_update_implementation_profile(
        &self,
        context: MemoryBackendRequestContext,
        profile_id: u64,
        request: MemoryImplementationProfileRequest,
    ) -> MemoryServiceResult<MemoryImplementationProfile> {
        Self::validate_implementation_profile_request(&request)?;
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let capability_json = serde_json::to_string(&request.capabilities).ok();
        let config_json = Self::optional_json_patch(&request.config);
        let rollout_json = Self::optional_json_patch(&request.rollout);
        let row = self
            .store
            .update_mem_implementation_profile_for_tenant(
                tenant_id,
                &profile_id.to_string(),
                Some(request.name.as_str()),
                Some(request.implementation_kind.as_str()),
                Some(request.role.as_str()),
                request.status.as_deref(),
                capability_json.as_deref(),
                config_json.as_ref().map(|value| value.as_deref()),
                rollout_json.as_ref().map(|value| value.as_deref()),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("implementation profile not found"))?;
        Self::map_implementation_profile(&row)
    }

    pub(crate) async fn backend_list_provider_bindings(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryProviderBindingList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = Self::page_size_from_query(&query)?;
        let rows = self
            .store
            .list_mem_provider_bindings_for_tenant(tenant_id, page_size, query.cursor.as_deref())
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.binding_uuid.clone());
        let items = page_rows
            .iter()
            .map(Self::map_provider_binding)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryProviderBindingList {
            items,
            page_info: Self::admin_page_info(page_size, has_more, next_cursor),
        })
    }

    pub(crate) async fn backend_create_provider_binding(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let binding_id = self.next_id()?.to_string();
        let capabilities_json = serde_json::to_string(&request.capabilities).map_err(|error| {
            MemoryServiceError::storage(format!("provider capabilities encode failed: {error}"))
        })?;
        let config_json = Self::encode_optional_json(&request.config)?;
        self.store
            .insert_mem_provider_binding(
                tenant_id,
                &binding_id,
                &request.provider_kind,
                &request.provider_code,
                &request.display_name,
                &capabilities_json,
                request.endpoint_ref.as_deref(),
                request.secret_ref.as_deref(),
                request.model_ref.as_deref(),
                config_json.as_deref(),
                request.health_state.as_deref().unwrap_or("healthy"),
                None,
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_provider_binding_for_tenant(tenant_id, &binding_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("created binding could not be loaded"))?;
        Self::map_provider_binding(&row)
    }

    pub(crate) async fn backend_update_provider_binding(
        &self,
        context: MemoryBackendRequestContext,
        binding_id: u64,
        request: MemoryProviderBindingRequest,
    ) -> MemoryServiceResult<MemoryProviderBinding> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let capabilities_json = serde_json::to_string(&request.capabilities).ok();
        let config_json = Self::optional_json_patch(&request.config);
        let row = self
            .store
            .update_mem_provider_binding_for_tenant(
                tenant_id,
                &binding_id.to_string(),
                Some(request.display_name.as_str()),
                Some(request.provider_code.as_str()),
                request.health_state.as_deref(),
                capabilities_json.as_deref(),
                Some(request.endpoint_ref.as_deref()),
                Some(request.secret_ref.as_deref()),
                Some(request.model_ref.as_deref()),
                config_json.as_ref().map(|value| value.as_deref()),
                None,
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("provider binding not found"))?;
        Self::map_provider_binding(&row)
    }

    pub(crate) async fn backend_list_eval_runs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListAdminResourcesQuery,
    ) -> MemoryServiceResult<MemoryEvalRunList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let page_size = Self::page_size_from_query(&query)?;
        let rows = self
            .store
            .list_mem_eval_runs_for_tenant(tenant_id, page_size, query.cursor.as_deref())
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.eval_run_uuid.clone());
        let items = page_rows
            .iter()
            .map(Self::map_eval_run)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryEvalRunList {
            items,
            page_info: Self::admin_page_info(page_size, has_more, next_cursor),
        })
    }

    pub(crate) async fn backend_create_eval_run(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryEvalRunRequest,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let eval_type = request.eval_type.trim();
        if eval_type != "retrieval_quality" {
            return Err(MemoryServiceError::validation(
                "evalType must be retrieval_quality; other evaluation engines are not implemented",
            ));
        }
        crate::job_worker::validate_retrieval_quality_eval_request(request.config.as_ref())
            .map_err(MemoryServiceError::validation)?;
        if let Some(profile_ref) = request.profile_ref.as_deref() {
            let profile_id = profile_ref.parse::<u64>().map_err(|_| {
                MemoryServiceError::validation("profileRef must be a numeric retrieval profile id")
            })?;
            let profile_exists = self
                .store
                .retrieve_mem_retrieval_profile_for_tenant(tenant_id, &profile_id.to_string())
                .await
                .map_err(OpenMemoryService::map_store_error)?
                .is_some();
            if !profile_exists {
                return Err(MemoryServiceError::validation(format!(
                    "retrieval profile {profile_id} was not found for the eval tenant"
                )));
            }
        }
        let eval_run_id = self.next_id()?.to_string();
        let config_json = request
            .config
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("eval config encode failed: {error}"))
            })?;
        self.store
            .insert_mem_eval_run_request(InsertMemoryEvalRunCommand {
                tenant_id,
                eval_run_uuid: &eval_run_id,
                eval_type,
                state: "accepted",
                dataset_ref: request.dataset_ref.as_deref(),
                profile_ref: request.profile_ref.as_deref(),
                config_json: config_json.as_deref(),
            })
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let row = self
            .store
            .retrieve_mem_eval_run_for_tenant(tenant_id, &eval_run_id)
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("created eval run could not be loaded"))?;
        Self::map_eval_run(&row)
    }

    pub(crate) async fn backend_retrieve_eval_run(
        &self,
        context: MemoryBackendRequestContext,
        eval_run_id: u64,
    ) -> MemoryServiceResult<MemoryEvalRun> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let row = self
            .store
            .retrieve_mem_eval_run_for_tenant(tenant_id, &eval_run_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("eval run not found"))?;
        Self::map_eval_run(&row)
    }

    pub(crate) async fn backend_create_extraction_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let job = MemoryOpenApi::create_extraction(
            self,
            Self::to_open_context_backend(&context),
            request.clone(),
        )
        .await?;
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        self.save_governance_entity(tenant_id, RT_EXTRACTION_JOB, &job.job_id.to_string(), &job)
            .await?;
        Ok(job)
    }

    pub(crate) async fn backend_retrieve_extraction_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        if let Some(row) = self
            .store
            .retrieve_learning_job_for_tenant(tenant_id, &job_id.to_string())
            .await
            .map_err(OpenMemoryService::map_store_error)?
        {
            return crate::job_worker::learning_job_from_row(&row);
        }
        self.load_governance_entity(tenant_id, RT_EXTRACTION_JOB, &job_id.to_string())
            .await
    }

    pub(crate) async fn backend_list_extraction_jobs(
        &self,
        context: MemoryBackendRequestContext,
        query: ListJobsQuery,
    ) -> MemoryServiceResult<MemoryLearningJobList> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = query.space_id.map(platform::space_id_i64).transpose()?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_learning_jobs_for_tenant(
                tenant_id,
                "extraction",
                space_id,
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let has_more = rows.len() > page_size as usize;
        let page_rows: Vec<_> = rows.into_iter().take(page_size as usize).collect();
        let next_cursor = page_rows.last().map(|row| row.job_uuid.clone());
        let items = page_rows
            .iter()
            .map(crate::job_worker::learning_job_from_row)
            .collect::<MemoryServiceResult<Vec<_>>>()?;
        Ok(MemoryLearningJobList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub(crate) async fn backend_create_consolidation_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryExtractionRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = platform::space_id_i64(request.space_id)?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id,
            organization_id: None,
            user_id: None,
        };
        let job_id = self.next_id()?;
        let operation_id = job_id.to_string();
        let consolidation = self
            .store
            .consolidate_duplicate_records_in_scope_detailed(ConsolidateDuplicateRecordsCommand {
                scope: &scope,
                operation_id: &operation_id,
            })
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let finished_at = platform::current_timestamp();
        let mut job = Self::new_learning_job(
            job_id,
            "consolidation",
            "succeeded",
            Some(request.space_id),
            &request,
        )?;
        job.result = Some(serde_json::json!({
            "mergedDuplicates": consolidation.superseded_records,
            "supersededDuplicates": consolidation.superseded_records,
            "transferredSources": consolidation.transferred_sources,
            "deduplicatedSources": consolidation.deduplicated_sources,
            "consolidationMode": "identity_bounded_supersession",
            "spaceId": request.space_id,
        }));
        job.finished_at = Some(finished_at.clone());
        job.updated_at = finished_at;
        self.persist_governance_job(
            tenant_id,
            context.operator_id,
            job_id,
            RT_CONSOLIDATION_JOB,
            "consolidationJobs.create",
            &job,
        )
        .await?;
        Ok(job)
    }

    pub(crate) async fn backend_retrieve_consolidation_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        Self::load_governance_job(self, tenant_id, job_id, RT_CONSOLIDATION_JOB).await
    }

    pub(crate) async fn backend_create_retention_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryRetentionJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        if request.reason.trim().is_empty() {
            return Err(MemoryServiceError::validation(
                "reason must not be empty for retention jobs",
            ));
        }
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let space_id = request
            .space_id
            .map(platform::space_id_i64)
            .transpose()?
            .ok_or_else(|| {
                MemoryServiceError::validation("spaceId is required for retention jobs")
            })?;
        let scope = MemoryScopeContext {
            tenant_id,
            space_id,
            organization_id: None,
            user_id: None,
        };
        let dry_run = request.dry_run.unwrap_or(false);
        let deleted = self
            .store
            .purge_expired_records_for_scope(&scope, dry_run)
            .await
            .map_err(OpenMemoryService::map_store_error)?;
        let job_id = self.next_id()?;
        let finished_at = platform::current_timestamp();
        let mut job =
            Self::new_learning_job(job_id, "retention", "succeeded", request.space_id, &request)?;
        job.result = Some(serde_json::json!({
            "deletedRecords": deleted,
            "dryRun": dry_run,
            "spaceId": request.space_id,
        }));
        job.finished_at = Some(finished_at.clone());
        job.updated_at = finished_at;
        self.persist_governance_job(
            tenant_id,
            context.operator_id,
            job_id,
            RT_RETENTION_JOB,
            "retentionJobs.create",
            &job,
        )
        .await?;
        Ok(job)
    }

    pub(crate) async fn backend_retrieve_retention_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        Self::load_governance_job(self, tenant_id, job_id, RT_RETENTION_JOB).await
    }

    pub(crate) async fn backend_create_migration_job(
        &self,
        context: MemoryBackendRequestContext,
        request: MemoryMigrationJobRequest,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        if request.reason.trim().is_empty() {
            return Err(MemoryServiceError::validation(
                "reason must not be empty for migration jobs",
            ));
        }
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        let preference_id = i64::try_from(self.next_id()?)
            .map_err(|_| MemoryServiceError::storage("generated preference id out of range"))?;
        let migration_result =
            crate::implementation_migration::execute_implementation_profile_migration(
                &self.store,
                preference_id,
                tenant_id,
                &request,
                &self.core_runtime.profile().profile_id,
                self.active_implementation_kind_code(),
            )
            .await?;
        let job_id = self.next_id()?;
        let finished_at = platform::current_timestamp();
        let mut job = Self::new_learning_job(job_id, "migration", "succeeded", None, &request)?;
        job.result = Some(migration_result);
        job.finished_at = Some(finished_at.clone());
        job.updated_at = finished_at;
        self.persist_governance_job(
            tenant_id,
            context.operator_id,
            job_id,
            RT_MIGRATION_JOB,
            "migrationJobs.create",
            &job,
        )
        .await?;
        Ok(job)
    }

    pub(crate) async fn backend_retrieve_migration_job(
        &self,
        context: MemoryBackendRequestContext,
        job_id: u64,
    ) -> MemoryServiceResult<MemoryLearningJob> {
        let tenant_id = platform::tenant_id_i64(context.tenant_id)?;
        Self::load_governance_job(self, tenant_id, job_id, RT_MIGRATION_JOB).await
    }

    pub(crate) async fn backend_supersede_memory(
        &self,
        context: MemoryBackendRequestContext,
        memory_id: u64,
        request: MemoryRecordRequest,
    ) -> MemoryServiceResult<sdkwork_memory_contract::MemoryRecord> {
        let open_context = Self::to_open_context_backend(&context);
        crate::access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &open_context,
            request.space_id,
        )
        .await?;

        let scope = Self::scope(&open_context, request.space_id)?;
        let old_uuid = memory_id.to_string();
        let new_uuid = self.next_id()?.to_string();
        let object_text = request
            .object_text
            .unwrap_or_else(|| request.canonical_text.clone());
        let sensitivity = Self::normalize_sensitivity_level(request.sensitivity_level.as_deref())?;
        crate::sensitive_content::assert_memory_text_is_safe(&[
            ("canonicalText", &request.canonical_text),
            ("objectText", &object_text),
            ("subject", request.subject.as_deref().unwrap_or("")),
            ("predicate", request.predicate.as_deref().unwrap_or("")),
        ])?;

        let superseded_journal = self.memory_mutation_journal(
            &old_uuid,
            "memory.record.superseded",
            "memory.record.supersede",
            serde_json::json!({
                "memoryId": old_uuid,
                "supersededByMemoryId": new_uuid,
                "spaceId": request.space_id,
            }),
        )?;
        let created_journal = self.memory_mutation_journal(
            &new_uuid,
            "memory.record.created",
            "memory.record.create",
            serde_json::json!({
                "memoryId": new_uuid,
                "supersedesMemoryId": old_uuid,
                "spaceId": request.space_id,
            }),
        )?;
        let quota_limits = crate::tenant_quota::MemoryQuotaLimits::from_env();
        let quota_scope = scope.clone();
        let admission = self
            .runtime_data_plane
            .supersede_canonical_memory_atomic_with_quota(
                SupersedeCanonicalMemoryAtomicCommand {
                    scope,
                    old_memory_id: old_uuid,
                    new_memory_id: new_uuid,
                    scope_label: request.scope,
                    memory_type: Self::memory_type_to_db(request.memory_type).to_string(),
                    subject: request.subject,
                    predicate: request.predicate,
                    object_text,
                    canonical_text: request.canonical_text,
                    sensitivity_level: sensitivity.to_string(),
                    expires_at: Self::normalize_expires_at(request.expires_at.as_deref())?,
                    created_journal,
                    superseded_journal,
                    metadata_json: None,
                },
                quota_limits.max_records_per_space,
            )
            .await?;
        let record =
            crate::tenant_quota::resolve_space_record_quota_admission(&quota_scope, admission)?;
        Self::map_canonical_record(record)
    }
}

#[cfg(test)]
mod implementation_profile_validation_tests {
    use super::*;

    fn request() -> MemoryImplementationProfileRequest {
        MemoryImplementationProfileRequest {
            name: "native sql".to_string(),
            implementation_kind: "native_sql".to_string(),
            role: "primary".to_string(),
            capabilities: serde_json::json!({ "keyword": true, "embedding": false }),
            status: Some("active".to_string()),
            config: Some(serde_json::json!({ "secretRef": "memory/db" })),
            rollout: None,
            version: None,
        }
    }

    #[test]
    fn accepts_typed_native_profile_with_secret_reference() {
        OpenMemoryService::validate_implementation_profile_request(&request())
            .expect("native profile request should be valid");
    }

    #[test]
    fn rejects_unknown_implementation_kind() {
        let mut value = request();
        value.implementation_kind = "made_up_store".to_string();
        let error = OpenMemoryService::validate_implementation_profile_request(&value)
            .expect_err("unknown implementation kinds must fail closed");
        assert_eq!(
            error.kind,
            sdkwork_memory_contract::MemoryServiceErrorKind::Validation
        );
    }

    #[test]
    fn rejects_literal_profile_token() {
        let mut value = request();
        value.config = Some(serde_json::json!({ "token": "literal-token" }));
        let error = OpenMemoryService::validate_implementation_profile_request(&value)
            .expect_err("literal provider tokens must not be persisted");
        assert_eq!(
            error.kind,
            sdkwork_memory_contract::MemoryServiceErrorKind::Validation
        );
    }

    #[test]
    fn rejects_unknown_profile_role() {
        let mut value = request();
        value.role = "active-primary".to_string();
        let error = OpenMemoryService::validate_implementation_profile_request(&value)
            .expect_err("unknown profile roles must fail closed");
        assert_eq!(
            error.kind,
            sdkwork_memory_contract::MemoryServiceErrorKind::Validation
        );
    }

    #[test]
    fn evaluation_only_implementation_cannot_become_primary() {
        let mut value = request();
        value.implementation_kind = "graph_temporal".to_string();
        value.capabilities = serde_json::json!({ "productionQualified": false });
        let error = OpenMemoryService::validate_implementation_profile_request(&value)
            .expect_err("unqualified graph implementation must not become primary");
        assert_eq!(
            error.kind,
            sdkwork_memory_contract::MemoryServiceErrorKind::Validation
        );

        value.role = "shadow".to_string();
        value.status = Some("disabled".to_string());
        OpenMemoryService::validate_implementation_profile_request(&value)
            .expect("disabled shadow metadata may be stored for evaluation");
    }

    #[test]
    fn qualified_profile_cannot_claim_missing_provider_capabilities() {
        let mut value = request();
        value.capabilities = serde_json::json!({ "keyword": true, "graph": true });
        assert!(OpenMemoryService::validate_implementation_profile_request(&value).is_err());
    }
}
