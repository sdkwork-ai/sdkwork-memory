//! Commercial memory management service layer.
//!
//! Implements subject, binding, and capability management with ID generation,
//! validation, and access control. All operations require backend-level
//! access (elevated tenant access).

use sdkwork_memory_contract::{
    BindingKind, CapabilityMode, CapabilityTargetType, CreateBindingCommand,
    CreateCapabilityBindingCommand, CreateEdgeCommand, CreateEntityCommand,
    CreatePolicyAssignmentCommand, CreatePolicyCommand, CreateSubjectCommand, ListBindingsQuery,
    ListCapabilityBindingsQuery, ListEdgesQuery, ListEntitiesQuery, ListPoliciesQuery,
    ListPolicyAssignmentsQuery, ListSubjectsQuery, MemoryBinding, MemoryBindingList,
    MemoryCapabilityBinding, MemoryCapabilityBindingList, MemoryCommercialReadiness, MemoryEdge,
    MemoryEdgeList, MemoryEntity, MemoryEntityList, MemoryOpenApiRequestContext, MemoryPolicy,
    MemoryPolicyAssignment, MemoryPolicyAssignmentList, MemoryPolicyList,
    MemoryResolvedCapabilityList, MemoryServiceError, MemoryServiceResult, MemorySubject,
    MemorySubjectList, PolicyAssignmentTargetType, PolicyInheritanceMode,
    RebuildCommercialReadinessCommand, ResolveCapabilitiesQuery, ResolvedCapability, SubjectType,
    UpdateEdgeCommand, UpdateEntityCommand, UpdatePolicyAssignmentCommand, UpdatePolicyCommand,
    UpdateSubjectCommand,
};
use sdkwork_memory_plugin_native_sql::{
    InsertCommercialReadinessCommand,
    InsertPolicyAssignmentCommand as StoreInsertPolicyAssignmentCommand,
    InsertPolicyCommand as StoreInsertPolicyCommand, InsertSubjectCommand, NativeSqlBindingRow,
    NativeSqlCapabilityBindingRow, NativeSqlCommercialReadinessRow, NativeSqlPolicyAssignmentRow,
    NativeSqlPolicyRow, NativeSqlSubjectRow,
    UpdatePolicyAssignmentCommand as StoreUpdatePolicyAssignmentCommand,
    UpdatePolicyCommand as StoreUpdatePolicyCommand,
    UpdateSubjectCommand as StoreUpdateSubjectCommand,
};
use sdkwork_memory_spi::{
    CreateGraphEdgeCommand, CreateGraphEntityCommand, DeleteGraphEdgeCommand, GraphEdgeRecord,
    GraphEntityRecord, ListGraphEdgesQuery, ListGraphEntitiesQuery, MemoryMutationJournal,
    MemoryScopeContext, MemorySensitivityReadScope as SpiMemorySensitivityReadScope,
    RetrieveCanonicalMemoryQuery, RetrieveGraphEdgeQuery, RetrieveGraphEntityQuery,
    UpdateGraphEdgeCommand, UpdateGraphEntityCommand,
};

use crate::store_error::map_memory_spi_error;

use crate::access;
use crate::platform;
use crate::sensitive_content::assert_memory_text_is_safe;

impl super::open_api::OpenMemoryService {
    // -----------------------------------------------------------------------
    // Subject management
    // -----------------------------------------------------------------------

    pub async fn create_subject(
        &self,
        cmd: CreateSubjectCommand,
    ) -> MemoryServiceResult<MemorySubject> {
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();
        let subject_type = subject_type_str(cmd.subject_type);
        let metadata_json = cmd
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("metadata serialization failed: {error}"))
            })?;

        let mutation_scope = system_commercial_mutation_scope(tenant_id);
        let journal = commercial_mutation_journal("subject", &uuid, "created")?;
        self.store
            .insert_subject_with_journal(
                InsertSubjectCommand {
                    id: id as i64,
                    uuid: &uuid,
                    tenant_id,
                    organization_id: cmd.organization_id.map(|v| v as i64),
                    subject_type,
                    subject_ref: &cmd.subject_ref,
                    display_name: &cmd.display_name,
                    default_space_id: cmd.default_space_id.map(|v| v as i64),
                    metadata_json: metadata_json.as_deref(),
                },
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        let row = self
            .store
            .retrieve_subject(tenant_id, &uuid)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("subject not found after insert"))?;

        Ok(map_subject_row_to_dto(row))
    }

    pub async fn retrieve_subject(
        &self,
        tenant_id: u64,
        subject_id: &str,
    ) -> MemoryServiceResult<MemorySubject> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let row = self
            .store
            .retrieve_subject(tenant_id_i64, subject_id)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("subject not found"))?;
        Ok(map_subject_row_to_dto(row))
    }

    pub async fn list_subjects(
        &self,
        query: ListSubjectsQuery,
    ) -> MemoryServiceResult<MemorySubjectList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let subject_type = query.subject_type.map(subject_type_str);
        let rows = self
            .store
            .list_subjects(
                tenant_id,
                subject_type,
                query.status.as_deref(),
                platform::decode_list_cursor(query.cursor.as_deref())?.as_deref(),
                page_size,
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() as i32 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(map_subject_row_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.subject_id.clone())
        } else {
            None
        };

        Ok(MemorySubjectList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn update_subject(
        &self,
        tenant_id: u64,
        subject_id: &str,
        cmd: UpdateSubjectCommand,
    ) -> MemoryServiceResult<MemorySubject> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let metadata_json = cmd
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("metadata serialization failed: {error}"))
            })?;

        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("subject", subject_id, "updated")?;
        let updated = self
            .store
            .update_subject_with_journal(
                tenant_id_i64,
                subject_id,
                StoreUpdateSubjectCommand {
                    display_name: cmd.display_name.as_deref(),
                    default_space_id: Some(cmd.default_space_id.map(|v| v as i64)),
                    status: cmd.status.as_deref(),
                    metadata_json: metadata_json.as_deref(),
                },
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        if !updated {
            return Err(MemoryServiceError::not_found("subject not found"));
        }

        let row = self
            .store
            .retrieve_subject(tenant_id_i64, subject_id)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("subject not found after update"))?;

        Ok(map_subject_row_to_dto(row))
    }

    pub async fn delete_subject(
        &self,
        tenant_id: u64,
        subject_id: &str,
    ) -> MemoryServiceResult<()> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("subject", subject_id, "deleted")?;
        let deleted = self
            .store
            .delete_subject_with_journal(tenant_id_i64, subject_id, &mutation_scope, &journal)
            .await
            .map_err(Self::map_store_error)?;

        if !deleted {
            return Err(MemoryServiceError::not_found("subject not found"));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Binding management
    // -----------------------------------------------------------------------

    pub async fn create_binding(
        &self,
        cmd: CreateBindingCommand,
    ) -> MemoryServiceResult<MemoryBinding> {
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();
        let binding_kind = binding_kind_str(cmd.binding_kind);
        let capability_codes_json = cmd
            .capability_codes
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!(
                    "capability_codes serialization failed: {error}"
                ))
            })?;
        let metadata_json = cmd
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("metadata serialization failed: {error}"))
            })?;

        let mutation_scope = system_commercial_mutation_scope(tenant_id);
        let journal = commercial_mutation_journal("binding", &uuid, "created")?;
        self.store
            .insert_binding_with_journal(
                id as i64,
                &uuid,
                tenant_id,
                cmd.space_id.map(|v| v as i64),
                binding_kind,
                &cmd.binding_role,
                cmd.source_subject_id.map(|v| v as i64),
                cmd.target_subject_id.map(|v| v as i64),
                cmd.target_space_id.map(|v| v as i64),
                capability_codes_json.as_deref(),
                cmd.valid_from.as_deref(),
                cmd.valid_to.as_deref(),
                metadata_json.as_deref(),
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        let row = self
            .store
            .retrieve_binding(tenant_id, &uuid)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::storage("binding not found after insert"))?;

        Ok(map_binding_row_to_dto(row))
    }

    pub async fn retrieve_binding(
        &self,
        tenant_id: u64,
        binding_id: &str,
    ) -> MemoryServiceResult<MemoryBinding> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let row = self
            .store
            .retrieve_binding(tenant_id_i64, binding_id)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("binding not found"))?;
        Ok(map_binding_row_to_dto(row))
    }

    pub async fn list_bindings(
        &self,
        query: ListBindingsQuery,
    ) -> MemoryServiceResult<MemoryBindingList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let binding_kind = query.binding_kind.map(binding_kind_str);
        let rows = self
            .store
            .list_bindings(
                tenant_id,
                query.source_subject_id.map(|v| v as i64),
                query.target_subject_id.map(|v| v as i64),
                query.target_space_id.map(|v| v as i64),
                binding_kind,
                query.status.as_deref(),
                platform::decode_list_cursor(query.cursor.as_deref())?.as_deref(),
                page_size,
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() as i32 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(map_binding_row_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.binding_id.clone())
        } else {
            None
        };

        Ok(MemoryBindingList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn delete_binding(
        &self,
        tenant_id: u64,
        binding_id: &str,
    ) -> MemoryServiceResult<()> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("binding", binding_id, "deleted")?;
        let deleted = self
            .store
            .delete_binding_with_journal(tenant_id_i64, binding_id, &mutation_scope, &journal)
            .await
            .map_err(Self::map_store_error)?;

        if !deleted {
            return Err(MemoryServiceError::not_found("binding not found"));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Capability binding management
    // -----------------------------------------------------------------------

    pub async fn create_capability_binding(
        &self,
        cmd: CreateCapabilityBindingCommand,
    ) -> MemoryServiceResult<MemoryCapabilityBinding> {
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();
        let target_type = target_type_str(cmd.target_type);
        let mode = mode_str(cmd.mode);
        let metadata_json = cmd
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("metadata serialization failed: {error}"))
            })?;

        let mutation_scope = system_commercial_mutation_scope(tenant_id);
        let journal = commercial_mutation_journal("capability_binding", &uuid, "created")?;
        self.store
            .insert_capability_binding_with_journal(
                id as i64,
                &uuid,
                tenant_id,
                &cmd.capability_code,
                target_type,
                cmd.target_id as i64,
                mode,
                cmd.priority,
                cmd.valid_from.as_deref(),
                cmd.valid_to.as_deref(),
                metadata_json.as_deref(),
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        let row = self
            .store
            .retrieve_capability_binding(tenant_id, &uuid)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| {
                MemoryServiceError::storage("capability binding not found after insert")
            })?;

        Ok(map_capability_binding_row_to_dto(row))
    }

    pub async fn retrieve_capability_binding(
        &self,
        tenant_id: u64,
        cap_id: &str,
    ) -> MemoryServiceResult<MemoryCapabilityBinding> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let row = self
            .store
            .retrieve_capability_binding(tenant_id_i64, cap_id)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("capability binding not found"))?;
        Ok(map_capability_binding_row_to_dto(row))
    }

    pub async fn list_capability_bindings(
        &self,
        query: ListCapabilityBindingsQuery,
    ) -> MemoryServiceResult<MemoryCapabilityBindingList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let target_type = query.target_type.map(target_type_str);
        let rows = self
            .store
            .list_capability_bindings(
                tenant_id,
                query.capability_code.as_deref(),
                target_type,
                query.target_id.map(|v| v as i64),
                query.status.as_deref(),
                platform::decode_list_cursor(query.cursor.as_deref())?.as_deref(),
                page_size,
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() as i32 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(map_capability_binding_row_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.capability_binding_id.clone())
        } else {
            None
        };

        Ok(MemoryCapabilityBindingList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn delete_capability_binding(
        &self,
        tenant_id: u64,
        cap_id: &str,
    ) -> MemoryServiceResult<()> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("capability_binding", cap_id, "deleted")?;
        let deleted = self
            .store
            .delete_capability_binding_with_journal(
                tenant_id_i64,
                cap_id,
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        if !deleted {
            return Err(MemoryServiceError::not_found(
                "capability binding not found",
            ));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Capability resolution
    // -----------------------------------------------------------------------

    pub async fn resolve_capabilities(
        &self,
        query: ResolveCapabilitiesQuery,
    ) -> MemoryServiceResult<MemoryResolvedCapabilityList> {
        let tenant_id_i64 = platform::tenant_id_i64(query.tenant_id)?;
        let target_type_str = target_type_str(parse_target_type(&query.target_type)?);
        let page_size = platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .resolve_capabilities_for_target(
                tenant_id_i64,
                target_type_str,
                query.target_id as i64,
                page_size,
                platform::decode_list_cursor(query.cursor.as_deref())?.as_deref(),
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() > page_size as usize;
        let items = rows
            .into_iter()
            .take(page_size as usize)
            .map(|row| ResolvedCapability {
                capability_code: row.capability_code,
                mode: parse_mode(&row.mode),
                priority: row.priority,
                source: row.uuid,
            })
            .collect::<Vec<_>>();
        let next_cursor = if has_more {
            items.last().map(|item| item.source.clone())
        } else {
            None
        };

        Ok(MemoryResolvedCapabilityList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    // -----------------------------------------------------------------------
    // Entity management
    // -----------------------------------------------------------------------

    pub async fn create_entity(
        &self,
        context: MemoryOpenApiRequestContext,
        cmd: CreateEntityCommand,
    ) -> MemoryServiceResult<MemoryEntity> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            cmd.space_id,
        )
        .await?;
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let space_id = platform::space_id_i64(cmd.space_id)?;
        if cmd.entity_type.trim().is_empty() || cmd.canonical_name.trim().is_empty() {
            return Err(MemoryServiceError::validation(
                "entityType and canonicalName are required",
            ));
        }
        assert_memory_text_is_safe(&[("canonicalName", &cmd.canonical_name)])?;
        if let Some(ref aliases) = cmd.aliases {
            for alias in aliases {
                assert_memory_text_is_safe(&[("alias", alias.as_str())])?;
            }
        }
        if let Some(ref attributes) = cmd.attributes {
            let attributes_text = serde_json::to_string(attributes).map_err(|error| {
                MemoryServiceError::storage(format!("attributes serialization failed: {error}"))
            })?;
            assert_memory_text_is_safe(&[("attributes", &attributes_text)])?;
        }
        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();
        let aliases_json = optional_json_array(cmd.aliases)?;
        let attributes_json = optional_json_value(cmd.attributes)?;

        let scope = commercial_mutation_scope(&context, tenant_id, space_id);
        let journal = commercial_mutation_journal("entity", &uuid, "created")?;
        self.graph
            .create_entity(CreateGraphEntityCommand {
                scope,
                entity_id: uuid.clone(),
                entity_type: cmd.entity_type,
                canonical_name: cmd.canonical_name,
                aliases_json,
                attributes_json,
                sensitivity_level: cmd.sensitivity_level,
                journal,
            })
            .await
            .map_err(map_memory_spi_error)?;

        self.retrieve_entity(context, tenant_id as u64, &uuid).await
    }

    pub async fn retrieve_entity(
        &self,
        context: MemoryOpenApiRequestContext,
        tenant_id: u64,
        entity_id: &str,
    ) -> MemoryServiceResult<MemoryEntity> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let record = self
            .graph
            .retrieve_entity(RetrieveGraphEntityQuery {
                tenant_id: tenant_id_i64,
                entity_id: entity_id.to_string(),
            })
            .await
            .map_err(map_memory_spi_error)?
            .ok_or_else(|| MemoryServiceError::not_found("entity not found"))?;
        let space_id = u64::try_from(record.space_id.max(0))
            .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;
        let authorization =
            access::authorize_actor_for_space_access(&self.runtime_data_plane, &context, space_id)
                .await?;
        access::assert_actor_may_read_entity_sensitivity(
            &context,
            &record.sensitivity_level,
            authorization.actor_is_space_owner,
        )
        .await?;
        Ok(map_graph_entity_to_dto(record))
    }

    pub async fn list_entities(
        &self,
        context: MemoryOpenApiRequestContext,
        query: ListEntitiesQuery,
    ) -> MemoryServiceResult<MemoryEntityList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let space_filter = access::require_commercial_list_space_id(&context, query.space_id)?;
        let authorization = if let Some(space_id) = space_filter {
            Some(
                access::authorize_actor_for_space_access(
                    &self.runtime_data_plane,
                    &context,
                    space_id,
                )
                .await?,
            )
        } else {
            None
        };
        let page_size = platform::validated_page_size(query.page_size)?;
        let sensitivity_scope = if let Some(authorization) = authorization {
            access::sensitivity_read_scope(&context, authorization.actor_is_space_owner)
        } else {
            use sdkwork_memory_plugin_native_sql::{
                SENSITIVITY_READ_ELEVATED, SENSITIVITY_READ_PUBLIC,
            };
            if context.elevated_tenant_access {
                SENSITIVITY_READ_ELEVATED
            } else {
                SENSITIVITY_READ_PUBLIC
            }
        };
        let records = self
            .graph
            .list_entities(ListGraphEntitiesQuery {
                tenant_id,
                space_id: space_filter.map(|value| value as i64),
                entity_type: query.entity_type,
                status: query.status,
                page_size,
                cursor: platform::decode_list_cursor(query.cursor.as_deref())?,
                sensitivity_read_scope: sensitivity_read_scope_from_i32(sensitivity_scope),
            })
            .await
            .map_err(map_memory_spi_error)?;

        let has_more = records.len() as i32 > page_size;
        let items: Vec<_> = records
            .into_iter()
            .take(page_size as usize)
            .map(map_graph_entity_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.entity_id.clone())
        } else {
            None
        };

        Ok(MemoryEntityList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn update_entity(
        &self,
        context: MemoryOpenApiRequestContext,
        tenant_id: u64,
        entity_id: &str,
        cmd: UpdateEntityCommand,
    ) -> MemoryServiceResult<MemoryEntity> {
        let existing = self
            .retrieve_entity(context.clone(), tenant_id, entity_id)
            .await?;
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            existing.space_id,
        )
        .await?;
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        if let Some(ref name) = cmd.canonical_name {
            assert_memory_text_is_safe(&[("canonicalName", name)])?;
        }
        if let Some(ref aliases) = cmd.aliases {
            for alias in aliases {
                assert_memory_text_is_safe(&[("alias", alias.as_str())])?;
            }
        }
        if let Some(ref attributes) = cmd.attributes {
            let attributes_text = serde_json::to_string(attributes).map_err(|error| {
                MemoryServiceError::storage(format!("attributes serialization failed: {error}"))
            })?;
            assert_memory_text_is_safe(&[("attributes", &attributes_text)])?;
        }
        let aliases_json = optional_json_array(cmd.aliases)?;
        let attributes_json = optional_json_value(cmd.attributes)?;

        let scope = commercial_mutation_scope(
            &context,
            tenant_id_i64,
            platform::space_id_i64(existing.space_id)?,
        );
        let journal = commercial_mutation_journal("entity", entity_id, "updated")?;
        let updated = self
            .graph
            .update_entity(UpdateGraphEntityCommand {
                scope,
                entity_id: entity_id.to_string(),
                canonical_name: cmd.canonical_name,
                aliases_json,
                attributes_json,
                sensitivity_level: cmd.sensitivity_level,
                status: cmd.status,
                journal,
            })
            .await
            .map_err(map_memory_spi_error)?;

        if !updated {
            return Err(MemoryServiceError::not_found("entity not found"));
        }

        self.retrieve_entity(context, tenant_id, entity_id).await
    }

    // -----------------------------------------------------------------------
    // Edge management
    // -----------------------------------------------------------------------

    pub async fn create_edge(
        &self,
        context: MemoryOpenApiRequestContext,
        cmd: CreateEdgeCommand,
    ) -> MemoryServiceResult<MemoryEdge> {
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            cmd.space_id,
        )
        .await?;
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let space_id = platform::space_id_i64(cmd.space_id)?;
        if cmd.relation_type.trim().is_empty() {
            return Err(MemoryServiceError::validation("relationType is required"));
        }

        // The caller names the provenance memory by its uuid; an unknown uuid
        // is a validation error, not a silently dangling provenance link.
        if let Some(memory_uuid) = cmd.source_memory_id.as_deref() {
            let exists = self
                .runtime_data_plane
                .retrieve_canonical_memory(RetrieveCanonicalMemoryQuery {
                    scope: commercial_mutation_scope(&context, tenant_id, space_id),
                    memory_id: memory_uuid.to_string(),
                })
                .await?
                .is_some();
            if !exists {
                return Err(MemoryServiceError::validation(format!(
                    "sourceMemoryId {memory_uuid} does not reference an existing memory"
                )));
            }
        }

        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();
        let metadata_json = optional_json_value(cmd.metadata)?;

        let scope = commercial_mutation_scope(&context, tenant_id, space_id);
        let journal = commercial_mutation_journal("edge", &uuid, "created")?;
        self.graph
            .create_edge(CreateGraphEdgeCommand {
                scope,
                edge_id: uuid.clone(),
                source_entity_id: cmd.source_entity_id,
                target_entity_id: cmd.target_entity_id,
                relation_type: cmd.relation_type,
                source_memory_id: cmd.source_memory_id,
                weight: cmd.weight,
                valid_from: cmd.valid_from,
                valid_to: cmd.valid_to,
                metadata_json,
                journal,
            })
            .await
            .map_err(map_memory_spi_error)?;

        self.retrieve_edge(context, tenant_id as u64, &uuid).await
    }

    pub async fn retrieve_edge(
        &self,
        context: MemoryOpenApiRequestContext,
        tenant_id: u64,
        edge_id: &str,
    ) -> MemoryServiceResult<MemoryEdge> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let record = self
            .graph
            .retrieve_edge(RetrieveGraphEdgeQuery {
                tenant_id: tenant_id_i64,
                edge_id: edge_id.to_string(),
            })
            .await
            .map_err(map_memory_spi_error)?
            .ok_or_else(|| MemoryServiceError::not_found("edge not found"))?;
        let space_id = u64::try_from(record.space_id.max(0))
            .map_err(|_| MemoryServiceError::storage("space id must be non-negative"))?;
        access::assert_actor_can_access_space(&self.runtime_data_plane, &context, space_id).await?;
        Ok(map_graph_edge_to_dto(record))
    }

    pub async fn list_edges(
        &self,
        context: MemoryOpenApiRequestContext,
        query: ListEdgesQuery,
    ) -> MemoryServiceResult<MemoryEdgeList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let space_filter = access::require_commercial_list_space_id(&context, query.space_id)?;
        if let Some(space_id) = space_filter {
            access::assert_actor_can_access_space(&self.runtime_data_plane, &context, space_id)
                .await?;
        }
        let page_size = platform::validated_page_size(query.page_size)?;
        let records = self
            .graph
            .list_edges(ListGraphEdgesQuery {
                tenant_id,
                space_id: space_filter.map(|value| value as i64),
                relation_type: query.relation_type,
                source_entity_id: query.source_entity_id,
                page_size,
                cursor: platform::decode_list_cursor(query.cursor.as_deref())?,
                sensitivity_read_scope: SpiMemorySensitivityReadScope::Owner,
            })
            .await
            .map_err(map_memory_spi_error)?;

        let has_more = records.len() as i32 > page_size;
        let items: Vec<_> = records
            .into_iter()
            .take(page_size as usize)
            .map(map_graph_edge_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.edge_id.clone())
        } else {
            None
        };

        Ok(MemoryEdgeList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn update_edge(
        &self,
        context: MemoryOpenApiRequestContext,
        tenant_id: u64,
        edge_id: &str,
        cmd: UpdateEdgeCommand,
    ) -> MemoryServiceResult<MemoryEdge> {
        let existing = self
            .retrieve_edge(context.clone(), tenant_id, edge_id)
            .await?;
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            existing.space_id,
        )
        .await?;
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let metadata_json = optional_json_value(cmd.metadata)?;

        let scope = commercial_mutation_scope(
            &context,
            tenant_id_i64,
            platform::space_id_i64(existing.space_id)?,
        );
        let journal = commercial_mutation_journal("edge", edge_id, "updated")?;
        let updated = self
            .graph
            .update_edge(UpdateGraphEdgeCommand {
                scope,
                edge_id: edge_id.to_string(),
                relation_type: cmd.relation_type,
                weight: cmd.weight,
                status: cmd.status,
                valid_from: cmd.valid_from,
                valid_to: cmd.valid_to,
                metadata_json,
                journal,
            })
            .await
            .map_err(map_memory_spi_error)?;

        if !updated {
            return Err(MemoryServiceError::not_found("edge not found"));
        }

        self.retrieve_edge(context, tenant_id, edge_id).await
    }

    pub async fn delete_edge(
        &self,
        context: MemoryOpenApiRequestContext,
        tenant_id: u64,
        edge_id: &str,
    ) -> MemoryServiceResult<()> {
        let existing = self
            .retrieve_edge(context.clone(), tenant_id, edge_id)
            .await?;
        access::assert_actor_can_access_space_for_write(
            &self.runtime_data_plane,
            &context,
            existing.space_id,
        )
        .await?;
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let scope = commercial_mutation_scope(
            &context,
            tenant_id_i64,
            platform::space_id_i64(existing.space_id)?,
        );
        let journal = commercial_mutation_journal("edge", edge_id, "deleted")?;
        self.graph
            .delete_edge(DeleteGraphEdgeCommand {
                scope,
                edge_id: edge_id.to_string(),
                journal,
            })
            .await
            .map_err(map_memory_spi_error)?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Policy management (backend)
    // -----------------------------------------------------------------------

    pub async fn create_policy(
        &self,
        cmd: CreatePolicyCommand,
    ) -> MemoryServiceResult<MemoryPolicy> {
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        if cmd.policy_type.trim().is_empty() || cmd.scope.trim().is_empty() {
            return Err(MemoryServiceError::validation(
                "policyType and scope are required",
            ));
        }
        let policy_json = serde_json::to_string(&cmd.policy).map_err(|error| {
            MemoryServiceError::storage(format!("policy serialization failed: {error}"))
        })?;
        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();

        let mutation_scope = system_commercial_mutation_scope(tenant_id);
        let journal = commercial_mutation_journal("policy", &uuid, "created")?;
        self.store
            .insert_policy_with_journal(
                StoreInsertPolicyCommand {
                    id: id as i64,
                    uuid: &uuid,
                    tenant_id,
                    policy_type: &cmd.policy_type,
                    scope: &cmd.scope,
                    scope_ref: cmd.scope_ref.as_deref(),
                    policy_json: &policy_json,
                },
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        self.retrieve_policy(tenant_id as u64, &uuid).await
    }

    pub async fn retrieve_policy(
        &self,
        tenant_id: u64,
        policy_id: &str,
    ) -> MemoryServiceResult<MemoryPolicy> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let row = self
            .store
            .retrieve_policy(tenant_id_i64, policy_id)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("policy not found"))?;
        Ok(map_policy_row_to_dto(row))
    }

    pub async fn list_policies(
        &self,
        query: ListPoliciesQuery,
    ) -> MemoryServiceResult<MemoryPolicyList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let rows = self
            .store
            .list_policies(
                tenant_id,
                query.policy_type.as_deref(),
                query.scope.as_deref(),
                platform::decode_list_cursor(query.cursor.as_deref())?.as_deref(),
                page_size,
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() as i32 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(map_policy_row_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.policy_id.clone())
        } else {
            None
        };

        Ok(MemoryPolicyList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn update_policy(
        &self,
        tenant_id: u64,
        policy_id: &str,
        cmd: UpdatePolicyCommand,
    ) -> MemoryServiceResult<MemoryPolicy> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let policy_json = cmd
            .policy
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| {
                MemoryServiceError::storage(format!("policy serialization failed: {error}"))
            })?;

        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("policy", policy_id, "updated")?;
        let updated = self
            .store
            .update_policy_with_journal(
                tenant_id_i64,
                policy_id,
                StoreUpdatePolicyCommand {
                    policy_type: cmd.policy_type.as_deref(),
                    scope: cmd.scope.as_deref(),
                    scope_ref: cmd.scope_ref.as_deref(),
                    policy_json: policy_json.as_deref(),
                    status: cmd.status.as_deref(),
                },
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        if !updated {
            return Err(MemoryServiceError::not_found("policy not found"));
        }

        self.retrieve_policy(tenant_id, policy_id).await
    }

    pub async fn delete_policy(&self, tenant_id: u64, policy_id: &str) -> MemoryServiceResult<()> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("policy", policy_id, "deleted")?;
        let deleted = self
            .store
            .delete_policy_with_journal(tenant_id_i64, policy_id, &mutation_scope, &journal)
            .await
            .map_err(Self::map_store_error)?;
        if !deleted {
            return Err(MemoryServiceError::not_found("policy not found"));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Policy assignment management
    // -----------------------------------------------------------------------

    pub async fn create_policy_assignment(
        &self,
        cmd: CreatePolicyAssignmentCommand,
    ) -> MemoryServiceResult<MemoryPolicyAssignment> {
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let policy_id = self
            .store
            .resolve_policy_internal_id(tenant_id, &cmd.policy_id)
            .await
            .map_err(Self::map_store_error)?;
        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();

        let mutation_scope = system_commercial_mutation_scope(tenant_id);
        let journal = commercial_mutation_journal("policy_assignment", &uuid, "created")?;
        self.store
            .insert_policy_assignment_with_journal(
                StoreInsertPolicyAssignmentCommand {
                    id: id as i64,
                    uuid: &uuid,
                    tenant_id,
                    policy_id,
                    target_type: policy_target_type_str(cmd.target_type),
                    target_id: cmd.target_id as i64,
                    priority: cmd.priority,
                    inheritance_mode: policy_inheritance_mode_str(cmd.inheritance_mode),
                    valid_from: cmd.valid_from.as_deref(),
                    valid_to: cmd.valid_to.as_deref(),
                },
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        self.retrieve_policy_assignment(tenant_id as u64, &uuid)
            .await
    }

    pub async fn retrieve_policy_assignment(
        &self,
        tenant_id: u64,
        assignment_id: &str,
    ) -> MemoryServiceResult<MemoryPolicyAssignment> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let row = self
            .store
            .retrieve_policy_assignment(tenant_id_i64, assignment_id)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("policy assignment not found"))?;
        Ok(map_policy_assignment_row_to_dto(row))
    }

    pub async fn list_policy_assignments(
        &self,
        query: ListPolicyAssignmentsQuery,
    ) -> MemoryServiceResult<MemoryPolicyAssignmentList> {
        let tenant_id = platform::tenant_id_i64(query.tenant_id)?;
        let page_size = platform::validated_page_size(query.page_size)?;
        let target_type = query.target_type.map(policy_target_type_str);
        let rows = self
            .store
            .list_policy_assignments(
                tenant_id,
                target_type,
                query.target_id.map(|value| value as i64),
                query.policy_id.as_deref(),
                platform::decode_list_cursor(query.cursor.as_deref())?.as_deref(),
                page_size,
            )
            .await
            .map_err(Self::map_store_error)?;

        let has_more = rows.len() as i32 > page_size;
        let items: Vec<_> = rows
            .into_iter()
            .take(page_size as usize)
            .map(map_policy_assignment_row_to_dto)
            .collect();
        let next_cursor = if has_more {
            items.last().map(|item| item.policy_assignment_id.clone())
        } else {
            None
        };

        Ok(MemoryPolicyAssignmentList {
            items,
            page_info: platform::memory_cursor_page_info(page_size, has_more, next_cursor),
        })
    }

    pub async fn update_policy_assignment(
        &self,
        tenant_id: u64,
        assignment_id: &str,
        cmd: UpdatePolicyAssignmentCommand,
    ) -> MemoryServiceResult<MemoryPolicyAssignment> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("policy_assignment", assignment_id, "updated")?;
        let updated = self
            .store
            .update_policy_assignment_with_journal(
                tenant_id_i64,
                assignment_id,
                StoreUpdatePolicyAssignmentCommand {
                    priority: cmd.priority,
                    inheritance_mode: cmd.inheritance_mode.map(policy_inheritance_mode_str),
                    status: cmd.status.as_deref(),
                    valid_from: cmd.valid_from.as_deref(),
                    valid_to: cmd.valid_to.as_deref(),
                },
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;

        if !updated {
            return Err(MemoryServiceError::not_found("policy assignment not found"));
        }

        self.retrieve_policy_assignment(tenant_id, assignment_id)
            .await
    }

    pub async fn delete_policy_assignment(
        &self,
        tenant_id: u64,
        assignment_id: &str,
    ) -> MemoryServiceResult<()> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let mutation_scope = system_commercial_mutation_scope(tenant_id_i64);
        let journal = commercial_mutation_journal("policy_assignment", assignment_id, "deleted")?;
        let deleted = self
            .store
            .delete_policy_assignment_with_journal(
                tenant_id_i64,
                assignment_id,
                &mutation_scope,
                &journal,
            )
            .await
            .map_err(Self::map_store_error)?;
        if !deleted {
            return Err(MemoryServiceError::not_found("policy assignment not found"));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Commercial readiness
    // -----------------------------------------------------------------------

    pub async fn retrieve_commercial_readiness(
        &self,
        tenant_id: u64,
    ) -> MemoryServiceResult<MemoryCommercialReadiness> {
        let tenant_id_i64 = platform::tenant_id_i64(tenant_id)?;
        let row = self
            .store
            .retrieve_latest_commercial_readiness(tenant_id_i64)
            .await
            .map_err(Self::map_store_error)?
            .ok_or_else(|| MemoryServiceError::not_found("commercial readiness not found"))?;
        Ok(map_readiness_row_to_dto(row))
    }

    pub async fn rebuild_commercial_readiness(
        &self,
        cmd: RebuildCommercialReadinessCommand,
    ) -> MemoryServiceResult<MemoryCommercialReadiness> {
        let tenant_id = platform::tenant_id_i64(cmd.tenant_id)?;
        let subject_count = self
            .store
            .count_subjects_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let binding_count = self
            .store
            .count_bindings_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let entity_count = self
            .graph
            .count_entities_for_tenant(tenant_id)
            .await
            .map_err(map_memory_spi_error)?;
        let edge_count = self
            .graph
            .count_edges_for_tenant(tenant_id)
            .await
            .map_err(map_memory_spi_error)?;
        let policy_count = self
            .store
            .count_policies_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let assignment_count = self
            .store
            .count_policy_assignments_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;

        let management_coverage = serde_json::json!({
            "subjects": subject_count,
            "bindings": binding_count,
            "entities": entity_count,
            "edges": edge_count,
            "policies": policy_count,
            "policyAssignments": assignment_count,
        });
        let profile_count = self
            .store
            .count_implementation_profiles_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let audit_count = self
            .store
            .count_audit_logs_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let eval_count = self
            .store
            .count_eval_runs_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let succeeded_eval_count = self
            .store
            .count_succeeded_retrieval_quality_evals_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let succeeded_migration_count = self
            .store
            .count_succeeded_migration_jobs_for_tenant(tenant_id)
            .await
            .map_err(Self::map_store_error)?;
        let has_active_profile = self
            .store
            .retrieve_tenant_preference_json(
                tenant_id,
                None,
                crate::implementation_migration::ACTIVE_IMPLEMENTATION_PROFILE_KEY,
            )
            .await
            .map_err(Self::map_store_error)?
            .is_some();

        let export_disabled = std::env::var("SDKWORK_MEMORY_EXPORT_DISABLED")
            .map(|value| value == "true" || value == "1")
            .unwrap_or(false);
        let export_configured = !export_disabled;
        let snowflake_initialized = crate::platform::snowflake_initialized();
        let outbox_delivery_ready = crate::outbox_delivery::production_outbox_delivery_ready();
        let redis_configured = std::env::var("SDKWORK_MEMORY_WEB_REDIS_URL")
            .or_else(|_| std::env::var("SDKWORK_REDIS_URL"))
            .is_ok_and(|value| !value.trim().is_empty());
        let postgres_runtime =
            self.store.dialect() == sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres;

        let mut blocking_findings = Vec::new();
        let mut warning_findings = Vec::new();
        if subject_count == 0 {
            blocking_findings.push("no_subjects".to_owned());
        }
        if binding_count == 0 {
            warning_findings.push("no_bindings".to_owned());
        }
        if entity_count == 0 {
            warning_findings.push("no_entities".to_owned());
        }
        if audit_count == 0 {
            blocking_findings.push("audit_execution_not_verified".to_owned());
        }
        if succeeded_eval_count == 0 {
            blocking_findings.push("retrieval_quality_not_verified".to_owned());
        }
        if succeeded_migration_count == 0 {
            blocking_findings.push("migration_execution_not_verified".to_owned());
        }
        if platform::is_production_like_environment() && !snowflake_initialized {
            blocking_findings.push("snowflake_not_initialized".to_owned());
        }
        if platform::is_production_like_environment() && !outbox_delivery_ready {
            blocking_findings.push("outbox_delivery_not_configured".to_owned());
        }
        if platform::is_production_like_environment() && !redis_configured {
            blocking_findings.push("distributed_web_stores_not_configured".to_owned());
        }
        if platform::is_production_like_environment() && !postgres_runtime {
            blocking_findings.push("postgres_runtime_not_active".to_owned());
        }
        if !export_configured {
            warning_findings.push("export_disabled".to_owned());
        }
        blocking_findings.push("release_evidence_not_verified_by_runtime".to_owned());
        blocking_findings.push("load_and_recovery_evidence_not_verified_by_runtime".to_owned());

        let contract_checks = [
            subject_count > 0,
            binding_count > 0,
            entity_count > 0,
            edge_count > 0,
            policy_count > 0,
            assignment_count > 0,
        ];
        let contract_score = contract_checks.iter().filter(|&&value| value).count() as f64
            / contract_checks.len() as f64;
        let populated_layers = [subject_count, binding_count, entity_count, edge_count]
            .iter()
            .filter(|&&count| count > 0)
            .count() as f64;
        let data_score = populated_layers / 4.0;
        let runtime_checks = [
            postgres_runtime,
            snowflake_initialized,
            outbox_delivery_ready,
            redis_configured,
            audit_count > 0,
            succeeded_eval_count > 0,
            succeeded_migration_count > 0,
        ];
        let runtime_score = runtime_checks.iter().filter(|&&value| value).count() as f64
            / runtime_checks.len() as f64;
        let score = ((contract_score * 0.4) + (data_score * 0.3) + (runtime_score * 0.3)).min(1.0);
        let state = if !blocking_findings.is_empty() {
            "blocked"
        } else if score >= 0.75 {
            "ready"
        } else {
            "warning"
        };

        let contract_coverage = serde_json::json!({
            "subjects": subject_count > 0,
            "bindings": binding_count > 0,
            "entities": entity_count > 0,
            "edges": edge_count > 0,
            "policies": policy_count > 0,
            "policyAssignments": assignment_count > 0,
            "commercialReadiness": state == "ready",
        });
        let runtime_conformance = serde_json::json!({
            "databaseEngine": match self.store.dialect() {
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Postgres => "postgres",
                sdkwork_memory_plugin_native_sql::MemorySqlDialect::Sqlite => "sqlite",
            },
            "postgresRuntimeActive": postgres_runtime,
            "snowflakeInitialized": snowflake_initialized,
            "outboxDeliveryReady": outbox_delivery_ready,
            "distributedWebStoresConfigured": redis_configured,
        });
        let privacy_coverage = serde_json::json!({
            "exportConfigured": export_configured,
            "exportVerificationState": "not_verified",
            "forgetVerificationState": "not_verified",
        });
        let audit_coverage = serde_json::json!({
            "auditLogCount": audit_count,
            "executionObserved": audit_count > 0,
        });
        let sdk_coverage = serde_json::json!({
            "verificationState": "not_verified",
            "reason": "runtime cannot attest generated SDK ownership, compilation, or publication evidence",
        });
        let evaluation_coverage = serde_json::json!({
            "evalRunCount": eval_count,
            "succeededRetrievalQualityCount": succeeded_eval_count,
            "executionVerified": succeeded_eval_count > 0,
        });
        let prometheus_metrics = crate::domain_metrics::render_memory_domain_prometheus(
            "sdkwork-memory",
            platform::deployment_environment_label(),
            "api",
            "rust",
            "service",
        );
        let observability_coverage = serde_json::json!({
            "prometheusRenderObserved": !prometheus_metrics.is_empty(),
        });
        let migration_coverage = serde_json::json!({
            "implementationProfileCount": profile_count,
            "activeProfileRecorded": has_active_profile,
            "succeededMigrationJobCount": succeeded_migration_count,
            "executionVerified": succeeded_migration_count > 0,
        });

        let blocking_json = if blocking_findings.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&blocking_findings).map_err(|error| {
                MemoryServiceError::storage(format!(
                    "blocking findings serialization failed: {error}"
                ))
            })?)
        };
        let warning_json = if warning_findings.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&warning_findings).map_err(|error| {
                MemoryServiceError::storage(format!(
                    "warning findings serialization failed: {error}"
                ))
            })?)
        };
        let management_json = serde_json::to_string(&management_coverage).map_err(|error| {
            MemoryServiceError::storage(format!(
                "management coverage serialization failed: {error}"
            ))
        })?;
        let contract_json = serde_json::to_string(&contract_coverage).map_err(|error| {
            MemoryServiceError::storage(format!("contract coverage serialization failed: {error}"))
        })?;
        let runtime_json = serde_json::to_string(&runtime_conformance).map_err(|error| {
            MemoryServiceError::storage(format!(
                "runtime conformance serialization failed: {error}"
            ))
        })?;
        let privacy_json = serde_json::to_string(&privacy_coverage).map_err(|error| {
            MemoryServiceError::storage(format!("privacy coverage serialization failed: {error}"))
        })?;
        let audit_json = serde_json::to_string(&audit_coverage).map_err(|error| {
            MemoryServiceError::storage(format!("audit coverage serialization failed: {error}"))
        })?;
        let sdk_json = serde_json::to_string(&sdk_coverage).map_err(|error| {
            MemoryServiceError::storage(format!("sdk coverage serialization failed: {error}"))
        })?;
        let evaluation_json = serde_json::to_string(&evaluation_coverage).map_err(|error| {
            MemoryServiceError::storage(format!(
                "evaluation coverage serialization failed: {error}"
            ))
        })?;
        let observability_json =
            serde_json::to_string(&observability_coverage).map_err(|error| {
                MemoryServiceError::storage(format!(
                    "observability coverage serialization failed: {error}"
                ))
            })?;
        let migration_json = serde_json::to_string(&migration_coverage).map_err(|error| {
            MemoryServiceError::storage(format!("migration coverage serialization failed: {error}"))
        })?;

        let id = platform::next_numeric_id()?;
        let uuid = id.to_string();
        self.store
            .replace_commercial_readiness_snapshot(InsertCommercialReadinessCommand {
                id: id as i64,
                uuid: &uuid,
                tenant_id,
                implementation_profile_id: cmd.implementation_profile_id.map(|value| value as i64),
                score,
                state,
                contract_coverage_json: Some(&contract_json),
                management_coverage_json: Some(&management_json),
                runtime_conformance_json: Some(&runtime_json),
                privacy_coverage_json: Some(&privacy_json),
                audit_coverage_json: Some(&audit_json),
                sdk_coverage_json: Some(&sdk_json),
                evaluation_coverage_json: Some(&evaluation_json),
                observability_coverage_json: Some(&observability_json),
                migration_coverage_json: Some(&migration_json),
                blocking_findings_json: blocking_json.as_deref(),
                warning_findings_json: warning_json.as_deref(),
            })
            .await
            .map_err(Self::map_store_error)?;

        self.retrieve_commercial_readiness(cmd.tenant_id).await
    }
}

fn commercial_mutation_scope(
    context: &MemoryOpenApiRequestContext,
    tenant_id: i64,
    space_id: i64,
) -> MemoryScopeContext {
    MemoryScopeContext {
        tenant_id,
        space_id,
        organization_id: None,
        user_id: context.actor_id.and_then(|value| i64::try_from(value).ok()),
    }
}

fn system_commercial_mutation_scope(tenant_id: i64) -> MemoryScopeContext {
    MemoryScopeContext {
        tenant_id,
        space_id: 0,
        organization_id: None,
        user_id: None,
    }
}

fn commercial_mutation_journal(
    resource_type: &str,
    resource_id: &str,
    mutation: &str,
) -> MemoryServiceResult<MemoryMutationJournal> {
    let event_type = format!("memory.{resource_type}.{mutation}");
    Ok(MemoryMutationJournal {
        outbox_id: platform::next_numeric_id()?.to_string(),
        aggregate_type: format!("ai_{resource_type}"),
        aggregate_id: resource_id.to_string(),
        event_type: event_type.clone(),
        event_version: "1".to_string(),
        payload_json: serde_json::json!({
            "resourceType": resource_type,
            "resourceId": resource_id,
            "mutation": mutation,
        })
        .to_string(),
        audit_id: platform::next_numeric_id()?.to_string(),
        audit_action: event_type,
        audit_resource_type: resource_type.to_string(),
        audit_resource_id: resource_id.to_string(),
        audit_result: "accepted".to_string(),
    })
}

// ---------------------------------------------------------------------------
// Mappers
// ---------------------------------------------------------------------------

fn map_subject_row_to_dto(row: NativeSqlSubjectRow) -> MemorySubject {
    MemorySubject {
        subject_id: row.uuid,
        tenant_id: row.tenant_id as u64,
        organization_id: row.organization_id.map(|v| v as u64),
        subject_type: parse_subject_type(&row.subject_type),
        subject_ref: row.subject_ref,
        display_name: row.display_name,
        default_space_id: row.default_space_id.map(|v| v as u64),
        status: row.status,
        metadata: row
            .metadata_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn map_binding_row_to_dto(row: NativeSqlBindingRow) -> MemoryBinding {
    MemoryBinding {
        binding_id: row.uuid,
        tenant_id: row.tenant_id as u64,
        space_id: row.space_id.map(|v| v as u64),
        binding_kind: parse_binding_kind(&row.binding_kind),
        binding_role: row.binding_role,
        source_subject_id: row.source_subject_id.map(|v| v as u64),
        target_subject_id: row.target_subject_id.map(|v| v as u64),
        target_space_id: row.target_space_id.map(|v| v as u64),
        capability_codes: row
            .capability_codes_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        status: row.status,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
        metadata: row
            .metadata_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn map_capability_binding_row_to_dto(
    row: NativeSqlCapabilityBindingRow,
) -> MemoryCapabilityBinding {
    MemoryCapabilityBinding {
        capability_binding_id: row.uuid,
        tenant_id: row.tenant_id as u64,
        capability_code: row.capability_code,
        target_type: parse_capability_target_type(&row.target_type),
        target_id: row.target_id as u64,
        mode: parse_mode(&row.mode),
        priority: row.priority,
        status: row.status,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
        metadata: row
            .metadata_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn optional_json_value(value: Option<serde_json::Value>) -> MemoryServiceResult<Option<String>> {
    match value {
        Some(json) => serde_json::to_string(&json).map(Some).map_err(|error| {
            MemoryServiceError::storage(format!("json serialization failed: {error}"))
        }),
        None => Ok(None),
    }
}

fn optional_json_array(values: Option<Vec<String>>) -> MemoryServiceResult<Option<String>> {
    match values {
        Some(items) => serde_json::to_string(&items).map(Some).map_err(|error| {
            MemoryServiceError::storage(format!("json serialization failed: {error}"))
        }),
        None => Ok(None),
    }
}

/// The access layer scores read scope numerically; the graph port carries the
/// SPI enum, so translate at the boundary.
fn sensitivity_read_scope_from_i32(value: i32) -> SpiMemorySensitivityReadScope {
    match value {
        0 => SpiMemorySensitivityReadScope::Public,
        1 => SpiMemorySensitivityReadScope::Elevated,
        _ => SpiMemorySensitivityReadScope::Owner,
    }
}

fn map_graph_entity_to_dto(row: GraphEntityRecord) -> MemoryEntity {
    MemoryEntity {
        entity_id: row.entity_id,
        space_id: row.space_id as u64,
        entity_type: row.entity_type,
        canonical_name: row.canonical_name,
        aliases: row
            .aliases_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        attributes: row
            .attributes_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        sensitivity_level: row.sensitivity_level,
        status: row.status,
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn map_graph_edge_to_dto(row: GraphEdgeRecord) -> MemoryEdge {
    MemoryEdge {
        edge_id: row.edge_id,
        space_id: row.space_id as u64,
        source_entity_id: row.source_entity_id,
        target_entity_id: row.target_entity_id,
        relation_type: row.relation_type,
        source_memory_id: row.source_memory_id,
        weight: row.weight,
        status: row.status,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
        metadata: row
            .metadata_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn map_policy_row_to_dto(row: NativeSqlPolicyRow) -> MemoryPolicy {
    MemoryPolicy {
        policy_id: row.uuid,
        tenant_id: row.tenant_id as u64,
        policy_type: row.policy_type,
        scope: row.scope,
        scope_ref: row.scope_ref,
        status: row.status,
        policy: serde_json::from_str(&row.policy_json).unwrap_or(serde_json::Value::Null),
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn map_policy_assignment_row_to_dto(row: NativeSqlPolicyAssignmentRow) -> MemoryPolicyAssignment {
    MemoryPolicyAssignment {
        policy_assignment_id: row.uuid,
        tenant_id: row.tenant_id as u64,
        policy_id: row.policy_uuid,
        target_type: parse_policy_target_type(&row.target_type),
        target_id: row.target_id as u64,
        priority: row.priority,
        inheritance_mode: parse_policy_inheritance_mode(&row.inheritance_mode),
        status: row.status,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version as u64,
    }
}

fn map_readiness_row_to_dto(row: NativeSqlCommercialReadinessRow) -> MemoryCommercialReadiness {
    MemoryCommercialReadiness {
        readiness_id: row.uuid,
        tenant_id: row.tenant_id as u64,
        implementation_profile_id: row.implementation_profile_id.map(|value| value as u64),
        score: row.score,
        state: row.state,
        contract_coverage: row
            .contract_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        management_coverage: row
            .management_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        runtime_conformance: row
            .runtime_conformance_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        privacy_coverage: row
            .privacy_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        audit_coverage: row
            .audit_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        sdk_coverage: row
            .sdk_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        evaluation_coverage: row
            .evaluation_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        observability_coverage: row
            .observability_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        migration_coverage: row
            .migration_coverage_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        blocking_findings: row
            .blocking_findings_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        warning_findings: row
            .warning_findings_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        created_at: row.created_at,
    }
}

// ---------------------------------------------------------------------------
// Enum string conversions
// ---------------------------------------------------------------------------

fn subject_type_str(t: SubjectType) -> &'static str {
    match t {
        SubjectType::Tenant => "tenant",
        SubjectType::Organization => "organization",
        SubjectType::User => "user",
        SubjectType::Application => "application",
        SubjectType::Service => "service",
    }
}

fn parse_subject_type(s: &str) -> SubjectType {
    match s {
        "tenant" => SubjectType::Tenant,
        "organization" => SubjectType::Organization,
        "user" => SubjectType::User,
        "application" => SubjectType::Application,
        "service" => SubjectType::Service,
        _ => SubjectType::User,
    }
}

fn binding_kind_str(k: BindingKind) -> &'static str {
    match k {
        BindingKind::Ownership => "ownership",
        BindingKind::Access => "access",
        BindingKind::Share => "share",
        BindingKind::Reference => "reference",
        BindingKind::Provision => "provision",
    }
}

fn parse_binding_kind(s: &str) -> BindingKind {
    match s {
        "ownership" => BindingKind::Ownership,
        "access" => BindingKind::Access,
        "share" => BindingKind::Share,
        "reference" => BindingKind::Reference,
        "provision" => BindingKind::Provision,
        _ => BindingKind::Access,
    }
}

fn target_type_str(t: CapabilityTargetType) -> &'static str {
    match t {
        CapabilityTargetType::Subject => "subject",
        CapabilityTargetType::Space => "space",
        CapabilityTargetType::Binding => "binding",
        CapabilityTargetType::Memory => "memory",
    }
}

fn parse_capability_target_type(s: &str) -> CapabilityTargetType {
    match s {
        "subject" => CapabilityTargetType::Subject,
        "space" => CapabilityTargetType::Space,
        "binding" => CapabilityTargetType::Binding,
        "memory" => CapabilityTargetType::Memory,
        _ => CapabilityTargetType::Subject,
    }
}

fn parse_target_type(s: &str) -> MemoryServiceResult<CapabilityTargetType> {
    match s {
        "subject" => Ok(CapabilityTargetType::Subject),
        "space" => Ok(CapabilityTargetType::Space),
        "binding" => Ok(CapabilityTargetType::Binding),
        "memory" => Ok(CapabilityTargetType::Memory),
        _ => Err(MemoryServiceError::validation("invalid targetType")),
    }
}

fn mode_str(m: CapabilityMode) -> &'static str {
    match m {
        CapabilityMode::Allow => "allow",
        CapabilityMode::Deny => "deny",
        CapabilityMode::Conditional => "conditional",
    }
}

fn parse_mode(s: &str) -> CapabilityMode {
    match s {
        "allow" => CapabilityMode::Allow,
        "deny" => CapabilityMode::Deny,
        "conditional" => CapabilityMode::Conditional,
        _ => CapabilityMode::Allow,
    }
}

fn policy_target_type_str(value: PolicyAssignmentTargetType) -> &'static str {
    match value {
        PolicyAssignmentTargetType::Subject => "subject",
        PolicyAssignmentTargetType::Space => "space",
        PolicyAssignmentTargetType::Entity => "entity",
        PolicyAssignmentTargetType::Binding => "binding",
        PolicyAssignmentTargetType::CapabilityBinding => "capability_binding",
        PolicyAssignmentTargetType::ImplementationProfile => "implementation_profile",
    }
}

fn parse_policy_target_type(value: &str) -> PolicyAssignmentTargetType {
    match value {
        "subject" => PolicyAssignmentTargetType::Subject,
        "space" => PolicyAssignmentTargetType::Space,
        "entity" => PolicyAssignmentTargetType::Entity,
        "binding" => PolicyAssignmentTargetType::Binding,
        "capability_binding" => PolicyAssignmentTargetType::CapabilityBinding,
        "implementation_profile" => PolicyAssignmentTargetType::ImplementationProfile,
        _ => PolicyAssignmentTargetType::Subject,
    }
}

fn policy_inheritance_mode_str(value: PolicyInheritanceMode) -> &'static str {
    match value {
        PolicyInheritanceMode::Inherit => "inherit",
        PolicyInheritanceMode::Override => "override",
        PolicyInheritanceMode::Deny => "deny",
        PolicyInheritanceMode::Shadow => "shadow",
    }
}

fn parse_policy_inheritance_mode(value: &str) -> PolicyInheritanceMode {
    match value {
        "inherit" => PolicyInheritanceMode::Inherit,
        "override" => PolicyInheritanceMode::Override,
        "deny" => PolicyInheritanceMode::Deny,
        "shadow" => PolicyInheritanceMode::Shadow,
        _ => PolicyInheritanceMode::Inherit,
    }
}
