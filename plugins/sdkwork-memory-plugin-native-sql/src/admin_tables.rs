use crate::sqlx_compat as sqlx;
use sqlx::Row;

use crate::store::{now_text, NativeSqlMemoryStore, NativeSqlStoreError};

const PROVIDER_HEALTH_ADVISORY_LOCK_ID: i64 = 0x4D45_4D48_5052_4F42;

const RETENTION_ADVISORY_LOCK_ID: i64 = 0x4D45_4D48_5245_5431;

/// Session-scoped retention sweep lock, same shape as the provider health
/// lease: the sweep walks every high-churn table in bounded batches, so it
/// takes a session-level advisory lock on a dedicated connection instead of
/// holding a SQL transaction open for the whole pass.
pub struct NativeSqlRetentionLease<'a> {
    connection: Option<::sqlx::pool::PoolConnection<::sqlx::Any>>,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl NativeSqlRetentionLease<'_> {
    pub async fn release(mut self) -> Result<(), NativeSqlStoreError> {
        if let Some(connection) = self.connection.as_mut() {
            sqlx::query("SELECT pg_advisory_unlock(?)")
                .bind(RETENTION_ADVISORY_LOCK_ID)
                .execute(&mut **connection)
                .await?;
        }
        self.connection = None;
        Ok(())
    }
}

impl Drop for NativeSqlRetentionLease<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            // Session advisory locks survive a connection returning to the
            // pool, so a lease dropped without release must NOT go back:
            // detaching closes the connection, which releases the lock.
            let _detached = connection.detach();
            tracing::warn!("retention lease dropped without explicit release");
        }
    }
}

/// Session-scoped provider health probe lock.
///
/// The probe pass walks every tenant/binding page with real HTTP probes, so
/// holding it open inside a SQL transaction pinned a pooled connection in
/// 'idle in transaction' for minutes. The lease instead takes a
/// session-level advisory lock on a dedicated connection: no transaction is
/// open, and release (or, as a safety net, Drop) closes the connection,
/// which releases session locks by definition.
pub struct NativeSqlProviderHealthLease<'a> {
    connection: Option<::sqlx::pool::PoolConnection<::sqlx::Any>>,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

impl NativeSqlProviderHealthLease<'_> {
    /// Releases the advisory lock and returns the connection to the pool.
    pub async fn release(mut self) -> Result<(), NativeSqlStoreError> {
        if let Some(connection) = self.connection.as_mut() {
            sqlx::query("SELECT pg_advisory_unlock(?)")
                .bind(PROVIDER_HEALTH_ADVISORY_LOCK_ID)
                .execute(&mut **connection)
                .await?;
        }
        self.connection = None;
        Ok(())
    }
}

impl Drop for NativeSqlProviderHealthLease<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            // Session advisory locks survive a connection returning to the
            // pool, so a lease dropped without release must NOT go back:
            // detaching closes the connection, which releases the lock.
            let _detached = connection.detach();
            tracing::warn!("provider health lease dropped without explicit release");
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlMemoryIndexRow {
    pub index_uuid: String,
    pub space_id: Option<i64>,
    pub index_kind: String,
    pub implementation_profile_id: Option<i64>,
    pub provider_binding_id: Option<i64>,
    pub schema_version: String,
    pub status: String,
    pub config_json: Option<String>,
    pub last_rebuilt_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlRetrievalProfileRow {
    pub profile_uuid: String,
    pub space_id: Option<i64>,
    pub name: String,
    pub strategy: String,
    pub retrievers_json: String,
    pub fusion_policy_json: Option<String>,
    pub rerank_policy_json: Option<String>,
    pub top_k: i32,
    pub context_budget_tokens: i32,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlImplementationProfileRow {
    pub profile_uuid: String,
    pub name: String,
    pub implementation_kind: String,
    pub role: String,
    pub status: String,
    pub capability_json: String,
    pub config_json: Option<String>,
    pub rollout_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlProviderBindingRow {
    pub binding_uuid: String,
    pub provider_kind: String,
    pub provider_code: String,
    pub display_name: String,
    pub endpoint_ref: Option<String>,
    pub secret_ref: Option<String>,
    pub model_ref: Option<String>,
    pub capabilities_json: String,
    pub config_json: Option<String>,
    pub health_state: String,
    pub last_health_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlEvalRunRow {
    pub eval_run_uuid: String,
    pub eval_type: String,
    pub state: String,
    pub dataset_ref: Option<String>,
    pub profile_ref: Option<String>,
    pub metrics_json: Option<String>,
    pub result_json: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy)]
pub struct InsertMemoryEvalRunCommand<'a> {
    pub tenant_id: i64,
    pub eval_run_uuid: &'a str,
    pub eval_type: &'a str,
    pub state: &'a str,
    pub dataset_ref: Option<&'a str>,
    pub profile_ref: Option<&'a str>,
    pub config_json: Option<&'a str>,
}

pub struct ApplyImplementationProfileSwitchCommand<'a> {
    pub preference_id: i64,
    pub tenant_id: i64,
    pub source_id: &'a str,
    pub target_id: &'a str,
    pub demote_source_primary: bool,
    pub target_implementation_kind: &'a str,
    pub target_implementation_profile_id: u64,
}

impl NativeSqlMemoryStore {
    pub async fn ensure_default_keyword_index_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<(), NativeSqlStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_index WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_one(self.pool())
            .await?;
        if count == 0 {
            // Same idempotent-seeding race treatment as the implementation
            // profile above: a concurrent seeder's unique violation is fine.
            if let Err(error) = self
                .insert_mem_index(
                    tenant_id,
                    "1",
                    None,
                    "keyword",
                    "2026-06-10",
                    "active",
                    None,
                    None,
                    None,
                )
                .await
            {
                if error
                    .inner_sqlx_error()
                    .is_none_or(crate::store::is_unique_violation)
                {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_mem_index(
        &self,
        tenant_id: i64,
        index_uuid: &str,
        space_id: Option<i64>,
        index_kind: &str,
        schema_version: &str,
        status: &str,
        config_json: Option<&str>,
        implementation_profile_id: Option<i64>,
        provider_binding_id: Option<i64>,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_index (
              id,
              uuid,
              tenant_id,
              space_id,
              index_kind,
              implementation_profile_id,
              provider_binding_id,
              schema_version,
              status,
              config_json,
              created_at,
              updated_at,
              version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(index_uuid)
        .bind(tenant_id)
        .bind(space_id)
        .bind(index_kind)
        .bind(implementation_profile_id)
        .bind(provider_binding_id)
        .bind(schema_version)
        .bind(status)
        .bind(config_json)
        .bind(now_text())
        .bind(now_text())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_mem_indexes_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlMemoryIndexRow>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, space_id, index_kind, implementation_profile_id, provider_binding_id,
                   schema_version, status, config_json,
                   last_rebuilt_at, created_at, updated_at, version
            FROM ai_index
            WHERE tenant_id = ?
              AND (? IS NULL OR space_id = ?)
              AND (
                ? = ''
                -- a purged cursor row ends the window (fail-closed)
                OR id > COALESCE(
                  (SELECT id FROM ai_index i2 WHERE i2.tenant_id = ? AND i2.uuid = ? LIMIT 1),
                  9223372036854775807
                )
              )
            ORDER BY id ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(space_id)
        .bind(cursor)
        .bind(tenant_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_index_row).collect())
    }

    pub async fn list_ai_indexes_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlMemoryIndexRow>, NativeSqlStoreError> {
        self.list_mem_indexes_for_tenant(tenant_id, space_id, page_size, cursor)
            .await
    }

    pub async fn retrieve_mem_index_for_tenant(
        &self,
        tenant_id: i64,
        index_uuid: &str,
    ) -> Result<Option<NativeSqlMemoryIndexRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, space_id, index_kind, implementation_profile_id, provider_binding_id,
                   schema_version, status, config_json,
                   last_rebuilt_at, created_at, updated_at, version
            FROM ai_index
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(index_uuid)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_index_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_mem_index_for_tenant(
        &self,
        tenant_id: i64,
        index_uuid: &str,
        status: Option<&str>,
        config_json: Option<&str>,
        last_rebuilt_at: Option<&str>,
        implementation_profile_id: Option<Option<i64>>,
        provider_binding_id: Option<Option<i64>>,
    ) -> Result<Option<NativeSqlMemoryIndexRow>, NativeSqlStoreError> {
        let existing = self
            .retrieve_mem_index_for_tenant(tenant_id, index_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: "memory index not found".to_string(),
            })?;

        let implementation_profile_id =
            implementation_profile_id.unwrap_or(existing.implementation_profile_id);
        let provider_binding_id = provider_binding_id.unwrap_or(existing.provider_binding_id);

        sqlx::query(
            r#"
            UPDATE ai_index
            SET status = ?,
                config_json = ?,
                last_rebuilt_at = ?,
                implementation_profile_id = ?,
                provider_binding_id = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(status.unwrap_or(&existing.status))
        .bind(config_json.or(existing.config_json.as_deref()))
        .bind(last_rebuilt_at.or(existing.last_rebuilt_at.as_deref()))
        .bind(implementation_profile_id)
        .bind(provider_binding_id)
        .bind(now_text())
        .bind(tenant_id)
        .bind(index_uuid)
        .execute(self.pool())
        .await?;

        self.retrieve_mem_index_for_tenant(tenant_id, index_uuid)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn ensure_default_retrieval_profile_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<(), NativeSqlStoreError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ai_retrieval_profile WHERE tenant_id = ?")
                .bind(tenant_id)
                .fetch_one(self.pool())
                .await?;
        if count == 0 {
            self.insert_mem_retrieval_profile(
                tenant_id,
                "1",
                None,
                "keyword-default",
                "deterministic",
                r#"{"keyword":{"weight":1.0},"dictionary":{"weight":0.85},"time":{"weight":0.5},"event":{"weight":0.6},"sql":{"weight":0.75}}"#,
                None,
                None,
                10,
                2048,
                "active",
            )
            .await?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_mem_retrieval_profile(
        &self,
        tenant_id: i64,
        profile_uuid: &str,
        space_id: Option<i64>,
        name: &str,
        strategy: &str,
        retrievers_json: &str,
        fusion_policy_json: Option<&str>,
        rerank_policy_json: Option<&str>,
        top_k: i32,
        context_budget_tokens: i32,
        status: &str,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_retrieval_profile (
              id, uuid, tenant_id, space_id, name, strategy, retrievers_json,
              fusion_policy_json, rerank_policy_json,
              top_k, context_budget_tokens, status, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(profile_uuid)
        .bind(tenant_id)
        .bind(space_id)
        .bind(name)
        .bind(strategy)
        .bind(retrievers_json)
        .bind(fusion_policy_json)
        .bind(rerank_policy_json)
        .bind(top_k)
        .bind(context_budget_tokens)
        .bind(status)
        .bind(now_text())
        .bind(now_text())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_mem_retrieval_profiles_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlRetrievalProfileRow>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, space_id, name, strategy, retrievers_json,
                   fusion_policy_json, rerank_policy_json,
                   top_k, context_budget_tokens, status, created_at, updated_at, version
            FROM ai_retrieval_profile
            WHERE tenant_id = ?
              AND (? IS NULL OR space_id = ?)
              AND (
                ? = ''
                -- a purged cursor row ends the window (fail-closed)
                OR id > COALESCE(
                  (SELECT id FROM ai_retrieval_profile p2 WHERE p2.tenant_id = ? AND p2.uuid = ? LIMIT 1),
                  9223372036854775807
                )
              )
            ORDER BY id ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(space_id)
        .bind(cursor)
        .bind(tenant_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_retrieval_profile_row).collect())
    }

    pub async fn retrieve_mem_retrieval_profile_for_tenant(
        &self,
        tenant_id: i64,
        profile_uuid: &str,
    ) -> Result<Option<NativeSqlRetrievalProfileRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, space_id, name, strategy, retrievers_json,
                   fusion_policy_json, rerank_policy_json,
                   top_k, context_budget_tokens, status, created_at, updated_at, version
            FROM ai_retrieval_profile
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(profile_uuid)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_retrieval_profile_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_mem_retrieval_profile_for_tenant(
        &self,
        tenant_id: i64,
        profile_uuid: &str,
        name: Option<&str>,
        strategy: Option<&str>,
        retrievers_json: Option<&str>,
        fusion_policy_json: Option<Option<&str>>,
        rerank_policy_json: Option<Option<&str>>,
        top_k: Option<i32>,
        context_budget_tokens: Option<i32>,
        status: Option<&str>,
    ) -> Result<Option<NativeSqlRetrievalProfileRow>, NativeSqlStoreError> {
        let existing = self
            .retrieve_mem_retrieval_profile_for_tenant(tenant_id, profile_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: "retrieval profile not found".to_string(),
            })?;

        let fusion_policy_json = fusion_policy_json
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.fusion_policy_json.clone());
        let rerank_policy_json = rerank_policy_json
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.rerank_policy_json.clone());

        sqlx::query(
            r#"
            UPDATE ai_retrieval_profile
            SET name = ?,
                strategy = ?,
                retrievers_json = ?,
                fusion_policy_json = ?,
                rerank_policy_json = ?,
                top_k = ?,
                context_budget_tokens = ?,
                status = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(name.unwrap_or(&existing.name))
        .bind(strategy.unwrap_or(&existing.strategy))
        .bind(retrievers_json.unwrap_or(&existing.retrievers_json))
        .bind(fusion_policy_json.as_deref())
        .bind(rerank_policy_json.as_deref())
        .bind(top_k.unwrap_or(existing.top_k))
        .bind(context_budget_tokens.unwrap_or(existing.context_budget_tokens))
        .bind(status.unwrap_or(&existing.status))
        .bind(now_text())
        .bind(tenant_id)
        .bind(profile_uuid)
        .execute(self.pool())
        .await?;

        self.retrieve_mem_retrieval_profile_for_tenant(tenant_id, profile_uuid)
            .await
    }

    pub async fn ensure_default_implementation_profile_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<(), NativeSqlStoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_implementation_profile WHERE tenant_id = ?",
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        if count == 0 {
            let (name, implementation_kind, capability_json) = match self.dialect() {
                crate::MemorySqlDialect::Postgres => (
                    "native-sql-phase1",
                    "native_sql",
                    r#"{"keyword":true,"embedding":false,"productionQualified":true}"#,
                ),
                crate::MemorySqlDialect::Sqlite => (
                    "local-embedded-phase1",
                    "local_embedded",
                    r#"{"keyword":true,"embedding":false,"productionQualified":true}"#,
                ),
            };
            // Two concurrent first requests can both pass the count check;
            // the tenant-unique uuid turns the loser into a unique violation,
            // which for a default-seeding idempotent insert IS success.
            if let Err(error) = self
                .insert_mem_implementation_profile(
                    tenant_id,
                    "1",
                    name,
                    implementation_kind,
                    "primary",
                    "active",
                    capability_json,
                    None,
                    None,
                )
                .await
            {
                if error
                    .inner_sqlx_error()
                    .is_none_or(crate::store::is_unique_violation)
                {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_mem_implementation_profile(
        &self,
        tenant_id: i64,
        profile_uuid: &str,
        name: &str,
        implementation_kind: &str,
        role: &str,
        status: &str,
        capability_json: &str,
        config_json: Option<&str>,
        rollout_json: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_implementation_profile (
              id, uuid, tenant_id, name, implementation_kind, role, status,
              capability_json, config_json, rollout_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(profile_uuid)
        .bind(tenant_id)
        .bind(name)
        .bind(implementation_kind)
        .bind(role)
        .bind(status)
        .bind(capability_json)
        .bind(config_json)
        .bind(rollout_json)
        .bind(now_text())
        .bind(now_text())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn count_implementation_profiles_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<i64, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM ai_implementation_profile
            WHERE tenant_id = ?
            "#,
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        Ok(row.get("total"))
    }

    pub async fn list_mem_implementation_profiles_for_tenant(
        &self,
        tenant_id: i64,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlImplementationProfileRow>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, name, implementation_kind, role, status, capability_json,
                   config_json, rollout_json,
                   created_at, updated_at, version
            FROM ai_implementation_profile
            WHERE tenant_id = ?
              AND (
                ? = ''
                -- a purged cursor row ends the window (fail-closed)
                OR id > COALESCE(
                  (SELECT id FROM ai_implementation_profile p2 WHERE p2.tenant_id = ? AND p2.uuid = ? LIMIT 1),
                  9223372036854775807
                )
              )
            ORDER BY id ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(cursor)
        .bind(tenant_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(map_implementation_profile_row)
            .collect())
    }

    pub async fn retrieve_mem_implementation_profile_for_tenant(
        &self,
        tenant_id: i64,
        profile_uuid: &str,
    ) -> Result<Option<NativeSqlImplementationProfileRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, name, implementation_kind, role, status, capability_json,
                   config_json, rollout_json,
                   created_at, updated_at, version
            FROM ai_implementation_profile
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(profile_uuid)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_implementation_profile_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_mem_implementation_profile_for_tenant(
        &self,
        tenant_id: i64,
        profile_uuid: &str,
        name: Option<&str>,
        implementation_kind: Option<&str>,
        role: Option<&str>,
        status: Option<&str>,
        capability_json: Option<&str>,
        config_json: Option<Option<&str>>,
        rollout_json: Option<Option<&str>>,
    ) -> Result<Option<NativeSqlImplementationProfileRow>, NativeSqlStoreError> {
        let existing = self
            .retrieve_mem_implementation_profile_for_tenant(tenant_id, profile_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: "implementation profile not found".to_string(),
            })?;

        let config_json = config_json
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.config_json.clone());
        let rollout_json = rollout_json
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.rollout_json.clone());

        sqlx::query(
            r#"
            UPDATE ai_implementation_profile
            SET name = ?,
                implementation_kind = ?,
                role = ?,
                status = ?,
                capability_json = ?,
                config_json = ?,
                rollout_json = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(name.unwrap_or(&existing.name))
        .bind(implementation_kind.unwrap_or(&existing.implementation_kind))
        .bind(role.unwrap_or(&existing.role))
        .bind(status.unwrap_or(&existing.status))
        .bind(capability_json.unwrap_or(&existing.capability_json))
        .bind(config_json.as_deref())
        .bind(rollout_json.as_deref())
        .bind(now_text())
        .bind(tenant_id)
        .bind(profile_uuid)
        .execute(self.pool())
        .await?;

        self.retrieve_mem_implementation_profile_for_tenant(tenant_id, profile_uuid)
            .await
    }

    pub async fn apply_implementation_profile_switch(
        &self,
        command: ApplyImplementationProfileSwitchCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        use crate::store::now_text;
        const ACTIVE_IMPLEMENTATION_PROFILE_KEY: &str = "implementation_profile.active";
        let mut tx = self.begin_tx().await?;
        let now = now_text();

        if command.demote_source_primary {
            sqlx::query(
                r#"
                UPDATE ai_implementation_profile
                SET role = 'standby',
                    status = 'active',
                    updated_at = ?,
                    version = version + 1
                WHERE tenant_id = ? AND uuid = ? AND role = 'primary'
                "#,
            )
            .bind(&now)
            .bind(command.tenant_id)
            .bind(command.source_id)
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query(
            r#"
            UPDATE ai_implementation_profile
            SET role = 'primary',
                status = 'active',
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(&now)
        .bind(command.tenant_id)
        .bind(command.target_id)
        .execute(&mut *tx)
        .await?;

        let active_profile_json = serde_json::json!({
            "implementationProfileId": command.target_implementation_profile_id,
            "implementationKind": command.target_implementation_kind,
            "migratedAt": now,
        });
        let preference_json = serde_json::to_string(&active_profile_json).map_err(|error| {
            NativeSqlStoreError::InvariantViolation {
                message: format!("active profile preference encode failed: {error}"),
            }
        })?;
        let bound_user_id = crate::store::preference_scope_user_binding(None, self.dialect());
        sqlx::query(
            r#"
            INSERT INTO ai_tenant_preference (
              id, tenant_id, user_id, preference_key, preference_json, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, 0)
            ON CONFLICT(tenant_id, user_id, preference_key) DO UPDATE SET
              preference_json = excluded.preference_json,
              updated_at = excluded.updated_at,
              version = ai_tenant_preference.version + 1
            "#,
        )
        .bind(command.preference_id)
        .bind(command.tenant_id)
        .bind(bound_user_id)
        .bind(ACTIVE_IMPLEMENTATION_PROFILE_KEY)
        .bind(&preference_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_mem_provider_binding(
        &self,
        tenant_id: i64,
        binding_uuid: &str,
        provider_kind: &str,
        provider_code: &str,
        display_name: &str,
        capabilities_json: &str,
        endpoint_ref: Option<&str>,
        secret_ref: Option<&str>,
        model_ref: Option<&str>,
        config_json: Option<&str>,
        health_state: &str,
        last_health_at: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_provider_binding (
              id, uuid, tenant_id, provider_kind, provider_code, display_name,
              endpoint_ref, secret_ref, model_ref,
              capabilities_json, config_json, health_state, last_health_at,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(binding_uuid)
        .bind(tenant_id)
        .bind(provider_kind)
        .bind(provider_code)
        .bind(display_name)
        .bind(endpoint_ref)
        .bind(secret_ref)
        .bind(model_ref)
        .bind(capabilities_json)
        .bind(config_json)
        .bind(health_state)
        .bind(last_health_at)
        .bind(now_text())
        .bind(now_text())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn try_acquire_provider_health_lease(
        &self,
    ) -> Result<Option<NativeSqlProviderHealthLease<'_>>, NativeSqlStoreError> {
        if self.dialect() == crate::MemorySqlDialect::Sqlite {
            return Ok(Some(NativeSqlProviderHealthLease {
            connection: None,
            _lifetime: std::marker::PhantomData,
        }));
        }

        let mut connection = self.pool().acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(?)")
            .bind(PROVIDER_HEALTH_ADVISORY_LOCK_ID)
            .fetch_one(&mut *connection)
            .await?;
        if !acquired {
            return Ok(None);
        }
        Ok(Some(NativeSqlProviderHealthLease {
            connection: Some(connection),
            _lifetime: std::marker::PhantomData,
        }))
    }

    /// Cluster-wide retention sweep admission: one replica sweeps, the rest
    /// skip their tick. SQLite (the single-process local plane) always
    /// "acquires" — there is no second replica to collide with.
    pub async fn try_acquire_retention_lease(
        &self,
    ) -> Result<Option<NativeSqlRetentionLease<'_>>, NativeSqlStoreError> {
        if self.dialect() == crate::MemorySqlDialect::Sqlite {
            return Ok(Some(NativeSqlRetentionLease {
                connection: None,
                _lifetime: std::marker::PhantomData,
            }));
        }

        let mut connection = self.pool().acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(?)")
            .bind(RETENTION_ADVISORY_LOCK_ID)
            .fetch_one(&mut *connection)
            .await?;
        if !acquired {
            return Ok(None);
        }
        Ok(Some(NativeSqlRetentionLease {
            connection: Some(connection),
            _lifetime: std::marker::PhantomData,
        }))
    }

    pub async fn list_tenant_ids_with_provider_bindings_page(
        &self,
        after_tenant_id: Option<i64>,
        page_size: i32,
    ) -> Result<Vec<i64>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT tenant_id
            FROM ai_provider_binding
            WHERE tenant_id > ?
            ORDER BY tenant_id ASC
            LIMIT ?
            "#,
        )
        .bind(after_tenant_id.unwrap_or(-1))
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| sqlx::Row::try_get::<i64, _>(&row, "tenant_id").map_err(Into::into))
            .collect()
    }

    pub async fn list_mem_provider_bindings_for_tenant(
        &self,
        tenant_id: i64,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlProviderBindingRow>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, provider_kind, provider_code, display_name,
                   endpoint_ref, secret_ref, model_ref,
                   capabilities_json, config_json, health_state, last_health_at,
                   created_at, updated_at, version
            FROM ai_provider_binding
            WHERE tenant_id = ?
              AND (
                ? = ''
                -- a purged cursor row ends the window (fail-closed)
                OR id > COALESCE(
                  (SELECT id FROM ai_provider_binding b2 WHERE b2.tenant_id = ? AND b2.uuid = ? LIMIT 1),
                  9223372036854775807
                )
              )
            ORDER BY id ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(cursor)
        .bind(tenant_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_provider_binding_row).collect())
    }

    pub async fn retrieve_mem_provider_binding_for_tenant(
        &self,
        tenant_id: i64,
        binding_uuid: &str,
    ) -> Result<Option<NativeSqlProviderBindingRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, provider_kind, provider_code, display_name,
                   endpoint_ref, secret_ref, model_ref,
                   capabilities_json, config_json, health_state, last_health_at,
                   created_at, updated_at, version
            FROM ai_provider_binding
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(binding_uuid)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_provider_binding_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_mem_provider_binding_for_tenant(
        &self,
        tenant_id: i64,
        binding_uuid: &str,
        display_name: Option<&str>,
        provider_code: Option<&str>,
        health_state: Option<&str>,
        capabilities_json: Option<&str>,
        endpoint_ref: Option<Option<&str>>,
        secret_ref: Option<Option<&str>>,
        model_ref: Option<Option<&str>>,
        config_json: Option<Option<&str>>,
        last_health_at: Option<Option<&str>>,
    ) -> Result<Option<NativeSqlProviderBindingRow>, NativeSqlStoreError> {
        let existing = self
            .retrieve_mem_provider_binding_for_tenant(tenant_id, binding_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: "provider binding not found".to_string(),
            })?;

        let endpoint_ref = endpoint_ref
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.endpoint_ref.clone());
        let secret_ref = secret_ref
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.secret_ref.clone());
        let model_ref = model_ref
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.model_ref.clone());
        let config_json = config_json
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.config_json.clone());
        let last_health_at = last_health_at
            .map(|value| value.map(str::to_string))
            .unwrap_or_else(|| existing.last_health_at.clone());

        sqlx::query(
            r#"
            UPDATE ai_provider_binding
            SET display_name = ?,
                provider_code = ?,
                health_state = ?,
                capabilities_json = ?,
                endpoint_ref = ?,
                secret_ref = ?,
                model_ref = ?,
                config_json = ?,
                last_health_at = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(display_name.unwrap_or(&existing.display_name))
        .bind(provider_code.unwrap_or(&existing.provider_code))
        .bind(health_state.unwrap_or(&existing.health_state))
        .bind(capabilities_json.unwrap_or(&existing.capabilities_json))
        .bind(endpoint_ref.as_deref())
        .bind(secret_ref.as_deref())
        .bind(model_ref.as_deref())
        .bind(config_json.as_deref())
        .bind(last_health_at.as_deref())
        .bind(now_text())
        .bind(tenant_id)
        .bind(binding_uuid)
        .execute(self.pool())
        .await?;

        self.retrieve_mem_provider_binding_for_tenant(tenant_id, binding_uuid)
            .await
    }

    pub async fn insert_mem_eval_run(
        &self,
        tenant_id: i64,
        eval_run_uuid: &str,
        eval_type: &str,
        state: &str,
        metrics_json: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_eval_run (
              id, uuid, tenant_id, eval_type, state, metrics_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(eval_run_uuid)
        .bind(tenant_id)
        .bind(eval_type)
        .bind(state)
        .bind(metrics_json)
        .bind(now_text())
        .bind(now_text())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn insert_mem_eval_run_request(
        &self,
        command: InsertMemoryEvalRunCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        let timestamp = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_eval_run (
              id, uuid, tenant_id, eval_type, state, dataset_ref, profile_ref,
              result_json, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(command.eval_run_uuid)
        .bind(command.tenant_id)
        .bind(command.eval_type)
        .bind(command.state)
        .bind(command.dataset_ref)
        .bind(command.profile_ref)
        .bind(command.config_json)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn list_mem_eval_runs_for_tenant(
        &self,
        tenant_id: i64,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlEvalRunRow>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, eval_type, state, dataset_ref, profile_ref, metrics_json,
                   result_json, started_at, finished_at, created_at, updated_at
            FROM ai_eval_run
            WHERE tenant_id = ?
              AND (
                ? = ''
                -- a purged cursor row ends the window (fail-closed)
                OR id > COALESCE(
                  (SELECT id FROM ai_eval_run r2 WHERE r2.tenant_id = ? AND r2.uuid = ? LIMIT 1),
                  9223372036854775807
                )
              )
            ORDER BY id ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(cursor)
        .bind(tenant_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;

        Ok(rows.into_iter().map(map_eval_run_row).collect())
    }

    pub async fn retrieve_mem_eval_run_for_tenant(
        &self,
        tenant_id: i64,
        eval_run_uuid: &str,
    ) -> Result<Option<NativeSqlEvalRunRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, eval_type, state, dataset_ref, profile_ref, metrics_json,
                   result_json, started_at, finished_at, created_at, updated_at
            FROM ai_eval_run
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(eval_run_uuid)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(map_eval_run_row))
    }
}

fn map_index_row(row: sqlx::any::AnyRow) -> NativeSqlMemoryIndexRow {
    NativeSqlMemoryIndexRow {
        index_uuid: row.get("uuid"),
        space_id: row.get("space_id"),
        index_kind: row.get("index_kind"),
        implementation_profile_id: row.get("implementation_profile_id"),
        provider_binding_id: row.get("provider_binding_id"),
        schema_version: row.get("schema_version"),
        status: row.get("status"),
        config_json: row.get("config_json"),
        last_rebuilt_at: row.get("last_rebuilt_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_retrieval_profile_row(row: sqlx::any::AnyRow) -> NativeSqlRetrievalProfileRow {
    NativeSqlRetrievalProfileRow {
        profile_uuid: row.get("uuid"),
        space_id: row.get("space_id"),
        name: row.get("name"),
        strategy: row.get("strategy"),
        retrievers_json: row.get("retrievers_json"),
        fusion_policy_json: row.get("fusion_policy_json"),
        rerank_policy_json: row.get("rerank_policy_json"),
        top_k: row.get("top_k"),
        context_budget_tokens: row.get("context_budget_tokens"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_implementation_profile_row(row: sqlx::any::AnyRow) -> NativeSqlImplementationProfileRow {
    NativeSqlImplementationProfileRow {
        profile_uuid: row.get("uuid"),
        name: row.get("name"),
        implementation_kind: row.get("implementation_kind"),
        role: row.get("role"),
        status: row.get("status"),
        capability_json: row.get("capability_json"),
        config_json: row.get("config_json"),
        rollout_json: row.get("rollout_json"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_provider_binding_row(row: sqlx::any::AnyRow) -> NativeSqlProviderBindingRow {
    NativeSqlProviderBindingRow {
        binding_uuid: row.get("uuid"),
        provider_kind: row.get("provider_kind"),
        provider_code: row.get("provider_code"),
        display_name: row.get("display_name"),
        endpoint_ref: row.get("endpoint_ref"),
        secret_ref: row.get("secret_ref"),
        model_ref: row.get("model_ref"),
        capabilities_json: row.get("capabilities_json"),
        config_json: row.get("config_json"),
        health_state: row.get("health_state"),
        last_health_at: row.get("last_health_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_eval_run_row(row: sqlx::any::AnyRow) -> NativeSqlEvalRunRow {
    NativeSqlEvalRunRow {
        eval_run_uuid: row.get("uuid"),
        eval_type: row.get("eval_type"),
        state: row.get("state"),
        dataset_ref: row.get("dataset_ref"),
        profile_ref: row.get("profile_ref"),
        metrics_json: row.get("metrics_json"),
        result_json: row.get("result_json"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
