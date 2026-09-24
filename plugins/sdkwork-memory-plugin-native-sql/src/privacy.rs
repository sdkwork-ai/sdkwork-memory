//! Privacy-oriented forget, export, and LIKE helpers for native SQL storage.

use crate::sqlx_compat as sqlx;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;

use sdkwork_memory_spi::MemoryScopeContext;

use crate::store::{now_text, NativeSqlMemoryStore, NativeSqlOpenApiEventRow, NativeSqlStoreError};

/// Escape `%`, `_`, and `\` for SQL `LIKE` patterns.
pub fn escape_like_pattern(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for ch in query.chars() {
        match ch {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            other => escaped.push(other),
        }
    }
    escaped
}

pub fn like_pattern(query: &str) -> String {
    format!("%{}%", escape_like_pattern(query.trim()))
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForgetScopeStats {
    pub deleted_records: u32,
    pub purged_events: u32,
    pub rejected_candidates: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportCollectedPayload {
    pub records: Vec<Value>,
    pub events: Vec<Value>,
}

impl NativeSqlMemoryStore {
    pub async fn ping(&self) -> Result<(), NativeSqlStoreError> {
        sqlx::query("SELECT 1").execute(self.pool()).await?;
        Ok(())
    }

    pub async fn verify_canonical_schema(&self) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            SELECT
              learning.lease_owner,
              learning.lease_token,
              learning.lease_expires_at,
              evaluation.lease_owner,
              evaluation.lease_token,
              evaluation.lease_expires_at,
              outbox.lease_owner,
              outbox.lease_token,
              outbox.lease_expires_at,
              outbox.next_attempt_at,
              readiness.migration_coverage_json
            FROM ai_learning_job AS learning
            CROSS JOIN ai_eval_run AS evaluation
            CROSS JOIN ai_outbox_event AS outbox
            CROSS JOIN ai_commercial_readiness_snapshot AS readiness
            WHERE 1 = 0
            "#,
        )
        .execute(self.pool())
        .await?;

        let search_schema_query = match self.dialect() {
            crate::MemorySqlDialect::Postgres => {
                "SELECT search_document FROM ai_record WHERE 1 = 0"
            }
            crate::MemorySqlDialect::Sqlite => "SELECT predicate FROM ai_record_fts WHERE 1 = 0",
        };
        sqlx::query(search_schema_query)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn forget_all_records_in_space(
        &self,
        scope: &MemoryScopeContext,
    ) -> Result<ForgetScopeStats, NativeSqlStoreError> {
        let mut stats = ForgetScopeStats::default();
        let mut cursor = String::new();
        let batch_size = i64::from(sdkwork_utils_rust::MAX_LIST_PAGE_SIZE);

        loop {
            // Forget is a physical privacy purge: it must enumerate every record in the
            // scope regardless of lifecycle status or sensitivity so soft-deleted rows and
            // their evidence sources cannot survive the workflow.
            let rows = sqlx::query(
                r#"
                SELECT uuid
                FROM ai_record
                WHERE tenant_id = ? AND space_id = ? AND uuid > ?
                ORDER BY uuid ASC
                LIMIT ?
                "#,
            )
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(&cursor)
            .bind(batch_size)
            .fetch_all(self.pool())
            .await?;
            if rows.is_empty() {
                break;
            }

            let page_limit = sdkwork_utils_rust::MAX_LIST_PAGE_SIZE as usize;
            let has_more = rows.len() > page_limit;
            let batch = if has_more {
                &rows[..page_limit]
            } else {
                &rows[..]
            };

            for row in batch {
                let memory_id: String = row.get("uuid");
                let outcome = self
                    .hard_delete_record_with_cleanup(scope, &memory_id)
                    .await?;
                if outcome.deleted {
                    stats.deleted_records += 1;
                }
                stats.rejected_candidates += outcome.rejected_candidates;
            }

            if !has_more {
                break;
            }
            let last_uuid: String = batch.last().expect("batch non-empty").get("uuid");
            cursor.clone_from(&last_uuid);
        }

        stats.purged_events += self
            .delete_events_in_space(scope.tenant_id, scope.space_id)
            .await?;

        Ok(stats)
    }

    pub async fn forget_records_for_user(
        &self,
        tenant_id: i64,
        user_id: i64,
        space_id: Option<i64>,
    ) -> Result<ForgetScopeStats, NativeSqlStoreError> {
        let mut stats = ForgetScopeStats::default();
        let batch_size = i64::from(sdkwork_utils_rust::MAX_LIST_PAGE_SIZE);
        loop {
            let rows = if let Some(space_id) = space_id {
                sqlx::query(
                    r#"
                    SELECT uuid, space_id
                    FROM ai_record
                    WHERE tenant_id = ? AND space_id = ? AND user_id = ?
                    ORDER BY id ASC
                    LIMIT ?
                    "#,
                )
                .bind(tenant_id)
                .bind(space_id)
                .bind(user_id)
                .bind(batch_size)
                .fetch_all(self.pool())
                .await?
            } else {
                sqlx::query(
                    r#"
                    SELECT uuid, space_id
                    FROM ai_record
                    WHERE tenant_id = ? AND user_id = ?
                    ORDER BY id ASC
                    LIMIT ?
                    "#,
                )
                .bind(tenant_id)
                .bind(user_id)
                .bind(batch_size)
                .fetch_all(self.pool())
                .await?
            };
            if rows.is_empty() {
                break;
            }
            for row in rows {
                let memory_id: String = row.get("uuid");
                let scope = MemoryScopeContext {
                    tenant_id,
                    space_id: row.get("space_id"),
                    organization_id: None,
                    user_id: Some(user_id),
                };
                let outcome = self
                    .hard_delete_record_with_cleanup(&scope, &memory_id)
                    .await?;
                if outcome.deleted {
                    stats.deleted_records += 1;
                }
                stats.rejected_candidates += outcome.rejected_candidates;
            }
        }

        let now = now_text();

        let rejected = if let Some(space_id) = space_id {
            sqlx::query(
                r#"
                UPDATE ai_candidate
                SET decision_state = 'rejected',
                    decision_reason = 'privacy_forget',
                    decided_at = ?,
                    updated_at = ?,
                    version = version + 1
                WHERE tenant_id = ? AND space_id = ? AND user_id = ? AND decision_state = 'pending'
                "#,
            )
            .bind(&now)
            .bind(&now)
            .bind(tenant_id)
            .bind(space_id)
            .bind(user_id)
            .execute(self.pool())
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                r#"
                UPDATE ai_candidate
                SET decision_state = 'rejected',
                    decision_reason = 'privacy_forget',
                    decided_at = ?,
                    updated_at = ?,
                    version = version + 1
                WHERE tenant_id = ? AND user_id = ? AND decision_state = 'pending'
                "#,
            )
            .bind(&now)
            .bind(&now)
            .bind(tenant_id)
            .bind(user_id)
            .execute(self.pool())
            .await?
            .rows_affected()
        };
        stats.rejected_candidates += rejected as u32;

        stats.purged_events = if let Some(space_id) = space_id {
            self.delete_events_for_user_in_space(tenant_id, user_id, space_id)
                .await?
        } else {
            self.delete_events_for_user_all_spaces(tenant_id, user_id)
                .await?
        };

        Ok(stats)
    }

    pub async fn forget_records_matching_query(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
    ) -> Result<ForgetScopeStats, NativeSqlStoreError> {
        let pattern = like_pattern(query);
        let batch_size = i64::from(sdkwork_utils_rust::MAX_LIST_PAGE_SIZE);
        let mut stats = ForgetScopeStats::default();

        loop {
            let rows = sqlx::query(
                r#"
                SELECT uuid
                FROM ai_record
                WHERE tenant_id = ?
                  AND space_id = ?
                  AND (
                    canonical_text LIKE ? ESCAPE '\'
                    OR object_text LIKE ? ESCAPE '\'
                    OR COALESCE(subject, '') LIKE ? ESCAPE '\'
                  )
                LIMIT ?
                "#,
            )
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(batch_size)
            .fetch_all(self.pool())
            .await?;

            if rows.is_empty() {
                break;
            }

            for row in rows {
                let memory_id: String = row.get("uuid");
                let outcome = self
                    .hard_delete_record_with_cleanup(scope, &memory_id)
                    .await?;
                if outcome.deleted {
                    stats.deleted_records += 1;
                }
                stats.rejected_candidates += outcome.rejected_candidates;
            }
        }

        Ok(stats)
    }

    pub async fn collect_export_payload_for_spaces(
        &self,
        tenant_id: i64,
        space_ids: &[i64],
        include_events: bool,
        sensitivity_scope: i32,
        max_payload_bytes: usize,
    ) -> Result<ExportCollectedPayload, NativeSqlStoreError> {
        let mut records = Vec::new();
        let mut events = Vec::new();
        let mut payload_bytes = 0_usize;

        let max_export_records = std::env::var("SDKWORK_MEMORY_EXPORT_MAX_RECORDS")
            .ok()
            .and_then(|value| sdkwork_utils_rust::parse_int(&value))
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(100_000);
        let max_export_events = std::env::var("SDKWORK_MEMORY_EXPORT_MAX_EVENTS")
            .ok()
            .and_then(|value| sdkwork_utils_rust::parse_int(&value))
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(100_000);

        for space_id in space_ids {
            let scope = MemoryScopeContext {
                tenant_id,
                space_id: *space_id,
                organization_id: None,
                user_id: None,
            };
            let mut cursor = String::new();
            loop {
                let rows = self
                    .list_record_details(
                        &scope,
                        None,
                        sdkwork_utils_rust::MAX_LIST_PAGE_SIZE,
                        Some(&cursor),
                        sensitivity_scope,
                        false,
                    )
                    .await?;
                if rows.is_empty() {
                    break;
                }
                let page_limit = sdkwork_utils_rust::MAX_LIST_PAGE_SIZE as usize;
                let has_more = rows.len() > page_limit;
                let batch = if has_more {
                    &rows[..page_limit]
                } else {
                    &rows[..]
                };
                for row in batch {
                    if records.len() >= max_export_records {
                        return Err(NativeSqlStoreError::InvariantViolation {
                            message: format!(
                                "export record limit exceeded (max {max_export_records} records per job)"
                            ),
                        });
                    }
                    let value = json!({
                        "memoryId": row.memory_id,
                        "spaceId": row.space_id,
                        "scope": row.scope,
                        "memoryType": row.memory_type,
                        "canonicalText": row.canonical_text,
                        "sensitivityLevel": row.sensitivity_level,
                        "createdAt": row.created_at,
                    });
                    account_export_value(&value, &mut payload_bytes, max_payload_bytes)?;
                    records.push(value);
                }
                if !has_more {
                    break;
                }
                cursor.clone_from(&batch.last().expect("batch non-empty").memory_id);
            }

            if include_events {
                let mut event_cursor = String::new();
                loop {
                    let cursor = if event_cursor.is_empty() {
                        None
                    } else {
                        Some(event_cursor.as_str())
                    };
                    let event_rows = self
                        .list_open_api_events_for_tenant(
                            tenant_id,
                            Some(*space_id),
                            sdkwork_utils_rust::MAX_LIST_PAGE_SIZE,
                            cursor,
                        )
                        .await?;
                    if event_rows.is_empty() {
                        break;
                    }
                    let page_limit = sdkwork_utils_rust::MAX_LIST_PAGE_SIZE as usize;
                    let has_more = event_rows.len() > page_limit;
                    let batch = if has_more {
                        &event_rows[..page_limit]
                    } else {
                        &event_rows[..]
                    };
                    for row in batch {
                        if events.len() >= max_export_events {
                            return Err(NativeSqlStoreError::InvariantViolation {
                                message: format!(
                                    "export event limit exceeded (max {max_export_events} events per job)"
                                ),
                            });
                        }
                        let value = Self::map_export_event(row);
                        account_export_value(&value, &mut payload_bytes, max_payload_bytes)?;
                        events.push(value);
                    }
                    if !has_more {
                        break;
                    }
                    event_cursor.clone_from(&batch.last().expect("batch non-empty").event_id);
                }
            }
        }

        Ok(ExportCollectedPayload { records, events })
    }

    fn map_export_event(row: &NativeSqlOpenApiEventRow) -> Value {
        json!({
            "eventId": row.event_id,
            "spaceId": row.space_id,
            "eventType": row.event_type,
            "payload": row.payload,
            "createdAt": row.created_at,
        })
    }

    async fn delete_events_in_space(
        &self,
        tenant_id: i64,
        space_id: i64,
    ) -> Result<u32, NativeSqlStoreError> {
        self.delete_sources_referencing_space_events(tenant_id, space_id)
            .await?;
        let purged = sqlx::query(
            r#"
            DELETE FROM ai_event
            WHERE tenant_id = ? AND space_id = ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        Ok(purged as u32)
    }

    /// Removes evidence-source rows that reference the events about to be purged,
    /// including sources that point at records outside the forget scope (an event
    /// authored by the forgotten subject can have sourced another actor's record).
    /// Without this purge the `ai_record_source.event_id` foreign key would abort
    /// the event deletion and permanently block the forget workflow.
    async fn delete_sources_referencing_space_events(
        &self,
        tenant_id: i64,
        space_id: i64,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            DELETE FROM ai_record_source
            WHERE tenant_id = ?
              AND event_id IN (
                SELECT id FROM ai_event WHERE tenant_id = ? AND space_id = ?
              )
            "#,
        )
        .bind(tenant_id)
        .bind(tenant_id)
        .bind(space_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn delete_events_for_user_all_spaces(
        &self,
        tenant_id: i64,
        user_id: i64,
    ) -> Result<u32, NativeSqlStoreError> {
        sqlx::query(
            r#"
            DELETE FROM ai_record_source
            WHERE tenant_id = ?
              AND event_id IN (
                SELECT id FROM ai_event WHERE tenant_id = ? AND user_id = ?
              )
            "#,
        )
        .bind(tenant_id)
        .bind(tenant_id)
        .bind(user_id)
        .execute(self.pool())
        .await?;
        let purged = sqlx::query(
            r#"
            DELETE FROM ai_event
            WHERE tenant_id = ? AND user_id = ?
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        Ok(purged as u32)
    }

    async fn delete_events_for_user_in_space(
        &self,
        tenant_id: i64,
        user_id: i64,
        space_id: i64,
    ) -> Result<u32, NativeSqlStoreError> {
        sqlx::query(
            r#"
            DELETE FROM ai_record_source
            WHERE tenant_id = ?
              AND event_id IN (
                SELECT id FROM ai_event WHERE tenant_id = ? AND user_id = ? AND space_id = ?
              )
            "#,
        )
        .bind(tenant_id)
        .bind(tenant_id)
        .bind(user_id)
        .bind(space_id)
        .execute(self.pool())
        .await?;
        let purged = sqlx::query(
            r#"
            DELETE FROM ai_event
            WHERE tenant_id = ? AND user_id = ? AND space_id = ?
            "#,
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(space_id)
        .execute(self.pool())
        .await?
        .rows_affected();
        Ok(purged as u32)
    }
}

fn account_export_value(
    value: &Value,
    payload_bytes: &mut usize,
    max_payload_bytes: usize,
) -> Result<(), NativeSqlStoreError> {
    let value_bytes = serde_json::to_vec(value)?.len();
    *payload_bytes = payload_bytes.saturating_add(value_bytes).saturating_add(1);
    if *payload_bytes > max_payload_bytes {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: format!(
                "export payload byte limit exceeded (max {max_payload_bytes} bytes per job)"
            ),
        });
    }
    Ok(())
}
