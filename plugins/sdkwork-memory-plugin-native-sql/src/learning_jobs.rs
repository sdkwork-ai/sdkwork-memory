//! Learning job queue persistence (`ai_learning_job`).

use crate::sqlx_compat as sqlx;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::pool_backend::MemorySqlDialect;
use crate::store::{now_text, timestamp_after_seconds, NativeSqlMemoryStore, NativeSqlStoreError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeSqlLearningJobRow {
    pub job_uuid: String,
    pub tenant_id: i64,
    pub space_id: Option<i64>,
    pub job_type: String,
    pub state: String,
    pub priority: i32,
    pub input_json: Option<String>,
    pub result_json: Option<String>,
    pub error_json: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub lease_owner: Option<String>,
    pub lease_token: Option<String>,
    pub lease_expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlClaimedEvalRun {
    pub tenant_id: i64,
    pub eval_run_uuid: String,
    pub eval_type: String,
    pub lease_owner: String,
    pub lease_token: String,
    pub lease_expires_at: String,
}

pub struct InsertLearningJobCommand<'a> {
    pub tenant_id: i64,
    pub job_uuid: &'a str,
    pub space_id: Option<i64>,
    pub job_type: &'a str,
    pub state: &'a str,
    pub priority: i32,
    pub idempotency_key: Option<&'a str>,
    pub input_json: Option<&'a str>,
}

pub struct FinishLearningJobCommand<'a> {
    pub tenant_id: i64,
    pub job_uuid: &'a str,
    pub lease_owner: &'a str,
    pub lease_token: &'a str,
    pub state: &'a str,
    pub result_json: Option<&'a str>,
    pub error_json: Option<&'a str>,
}

pub struct UpdateEvalRunStateCommand<'a> {
    pub tenant_id: i64,
    pub eval_run_uuid: &'a str,
    pub lease_owner: &'a str,
    pub lease_token: &'a str,
    pub state: &'a str,
    pub metrics_json: Option<&'a str>,
    pub result_json: Option<&'a str>,
}

impl NativeSqlMemoryStore {
    pub async fn insert_learning_job(
        &self,
        command: InsertLearningJobCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        let now = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_learning_job (
              id, uuid, tenant_id, space_id, job_type, state, priority,
              idempotency_key, input_json, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(command.job_uuid)
        .bind(command.tenant_id)
        .bind(command.space_id)
        .bind(command.job_type)
        .bind(command.state)
        .bind(command.priority)
        .bind(command.idempotency_key)
        .bind(command.input_json)
        .bind(&now)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn requeue_stale_running_learning_jobs(
        &self,
        max_attempts: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        let timestamp = now_text();
        let max_attempts = i64::try_from(max_attempts).unwrap_or(i64::MAX);
        // Rows past the attempt ceiling converge on the terminal `dead` state so a
        // crash-looping or panicking job cannot consume the queue forever.
        sqlx::query(
            r#"
            UPDATE ai_learning_job
            SET state = 'dead',
                error_json = ?,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?
            WHERE state = 'running'
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
              AND attempt_count >= ?
            "#,
        )
        .bind(r#"{"error":"max_attempts_exceeded"}"#)
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(max_attempts)
        .execute(self.pool())
        .await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_learning_job
            SET state = 'queued',
                attempt_count = attempt_count + 1,
                started_at = NULL,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?
            WHERE state = 'running'
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
              AND attempt_count < ?
            "#,
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(max_attempts)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn claim_queued_learning_jobs(
        &self,
        limit: i32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlLearningJobRow>, NativeSqlStoreError> {
        match self.dialect() {
            MemorySqlDialect::Postgres => {
                self.claim_queued_learning_jobs_postgres(
                    limit,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                )
                .await
            }
            MemorySqlDialect::Sqlite => {
                self.claim_queued_learning_jobs_sqlite(
                    limit,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                )
                .await
            }
        }
    }

    async fn claim_queued_learning_jobs_postgres(
        &self,
        limit: i32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlLearningJobRow>, NativeSqlStoreError> {
        let row_limit = i64::from(limit.max(1));
        let timestamp = now_text();
        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let rows = sqlx::query(
            r#"
            UPDATE ai_learning_job AS j
            SET state = 'running',
                started_at = ?,
                lease_owner = ?,
                lease_token = ?,
                lease_expires_at = ?,
                updated_at = ?,
                version = j.version + 1
            FROM (
                SELECT uuid, tenant_id
                FROM ai_learning_job
                WHERE state = 'queued'
                ORDER BY priority DESC, created_at ASC
                LIMIT ?
                FOR UPDATE SKIP LOCKED
            ) AS picked
            WHERE j.tenant_id = picked.tenant_id
              AND j.uuid = picked.uuid
              AND j.state = 'queued'
            RETURNING j.uuid, j.tenant_id, j.space_id, j.job_type, j.state, j.priority,
                      j.input_json, j.result_json, j.error_json,
                      j.started_at, j.finished_at,
                      j.lease_owner, j.lease_token, j.lease_expires_at,
                      j.created_at, j.updated_at, j.version
            "#,
        )
        .bind(&timestamp)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&lease_expires_at)
        .bind(&timestamp)
        .bind(row_limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_learning_job_row).collect())
    }

    async fn claim_queued_learning_jobs_sqlite(
        &self,
        limit: i32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlLearningJobRow>, NativeSqlStoreError> {
        let mut connection = self.pool().acquire().await?;
        let mut claimed = Vec::new();
        let rows = sqlx::query(
            r#"
            SELECT uuid, tenant_id, space_id, job_type, state, priority,
                   input_json, result_json, error_json,
                   started_at, finished_at,
                   lease_owner, lease_token, lease_expires_at,
                   created_at, updated_at, version
            FROM ai_learning_job
            WHERE state = 'queued'
            ORDER BY priority DESC, created_at ASC
            LIMIT ?
            "#,
        )
        .bind(limit.max(1) as i64)
        .fetch_all(&mut *connection)
        .await?;

        for row in rows {
            let job_uuid: String = row.get("uuid");
            let tenant_id: i64 = row.get("tenant_id");
            let timestamp = now_text();
            let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
            let updated = sqlx::query(
                r#"
                UPDATE ai_learning_job
                SET state = 'running',
                    started_at = ?,
                    lease_owner = ?,
                    lease_token = ?,
                    lease_expires_at = ?,
                    updated_at = ?,
                    version = version + 1
                WHERE tenant_id = ?
                  AND uuid = ?
                  AND state = 'queued'
                "#,
            )
            .bind(&timestamp)
            .bind(lease_owner)
            .bind(lease_token)
            .bind(&lease_expires_at)
            .bind(&timestamp)
            .bind(tenant_id)
            .bind(&job_uuid)
            .execute(&mut *connection)
            .await?;
            if updated.rows_affected() == 0 {
                continue;
            }
            let mut claimed_row = map_learning_job_row(row);
            claimed_row.state = "running".to_string();
            claimed_row.started_at = Some(timestamp.clone());
            claimed_row.lease_owner = Some(lease_owner.to_string());
            claimed_row.lease_token = Some(lease_token.to_string());
            claimed_row.lease_expires_at = Some(lease_expires_at);
            claimed.push(claimed_row);
        }
        Ok(claimed)
    }

    pub async fn retrieve_learning_job_for_tenant(
        &self,
        tenant_id: i64,
        job_uuid: &str,
    ) -> Result<Option<NativeSqlLearningJobRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, tenant_id, space_id, job_type, state, priority,
                   input_json, result_json, error_json,
                   started_at, finished_at,
                   lease_owner, lease_token, lease_expires_at,
                   created_at, updated_at, version
            FROM ai_learning_job
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(job_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(map_learning_job_row))
    }

    pub async fn list_learning_jobs_for_tenant(
        &self,
        tenant_id: i64,
        job_type: &str,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlLearningJobRow>, NativeSqlStoreError> {
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, tenant_id, space_id, job_type, state, priority,
                   input_json, result_json, error_json,
                   started_at, finished_at,
                   lease_owner, lease_token, lease_expires_at,
                   created_at, updated_at, version
            FROM ai_learning_job AS current
            WHERE tenant_id = ?
              AND job_type = ?
              AND (? IS NULL OR space_id = ?)
              AND (
                ? = ''
                OR current.id < COALESCE((
                  SELECT cursor_row.id
                  FROM ai_learning_job AS cursor_row
                  WHERE cursor_row.tenant_id = current.tenant_id
                    AND cursor_row.uuid = ?
                ), 0)
              )
            ORDER BY current.id DESC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(job_type)
        .bind(space_id)
        .bind(space_id)
        .bind(cursor)
        .bind(cursor)
        .bind(i64::from(page_size.clamp(1, 200)) + 1)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_learning_job_row).collect())
    }

    pub async fn finish_learning_job(
        &self,
        command: FinishLearningJobCommand<'_>,
    ) -> Result<Option<NativeSqlLearningJobRow>, NativeSqlStoreError> {
        let now = now_text();
        let updated = sqlx::query(
            r#"
            UPDATE ai_learning_job
            SET state = ?,
                result_json = COALESCE(?, result_json),
                error_json = COALESCE(?, error_json),
                finished_at = ?,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND state = 'running'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#,
        )
        .bind(command.state)
        .bind(command.result_json)
        .bind(command.error_json)
        .bind(&now)
        .bind(&now)
        .bind(command.tenant_id)
        .bind(command.job_uuid)
        .bind(command.lease_owner)
        .bind(command.lease_token)
        .bind(&now)
        .execute(self.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Ok(None);
        }
        self.retrieve_learning_job_for_tenant(command.tenant_id, command.job_uuid)
            .await
    }

    pub async fn renew_learning_job_lease(
        &self,
        tenant_id: i64,
        job_uuid: &str,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<bool, NativeSqlStoreError> {
        let timestamp = now_text();
        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let updated = sqlx::query(
            r#"
            UPDATE ai_learning_job
            SET lease_expires_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND state = 'running'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#,
        )
        .bind(lease_expires_at)
        .bind(&timestamp)
        .bind(tenant_id)
        .bind(job_uuid)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&timestamp)
        .execute(self.pool())
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn requeue_stale_running_eval_runs(
        &self,
        max_attempts: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        let timestamp = now_text();
        let max_attempts = i64::try_from(max_attempts).unwrap_or(i64::MAX);
        // Same bounded-retry contract as learning jobs: past the ceiling the run
        // is terminal `dead` instead of being requeued forever.
        sqlx::query(
            r#"
            UPDATE ai_eval_run
            SET state = 'dead',
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?
            WHERE state = 'running'
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
              AND attempt_count >= ?
            "#,
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(max_attempts)
        .execute(self.pool())
        .await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_eval_run
            SET state = 'queued',
                attempt_count = attempt_count + 1,
                started_at = NULL,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?
            WHERE state = 'running'
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
              AND attempt_count < ?
            "#,
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(max_attempts)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn update_eval_run_state(
        &self,
        command: UpdateEvalRunStateCommand<'_>,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let updated = sqlx::query(
            r#"
            UPDATE ai_eval_run
            SET state = ?,
                metrics_json = COALESCE(?, metrics_json),
                result_json = COALESCE(?, result_json),
                finished_at = ?,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?
            WHERE tenant_id = ? AND uuid = ? AND state = 'running'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#,
        )
        .bind(command.state)
        .bind(command.metrics_json)
        .bind(command.result_json)
        .bind(&now)
        .bind(&now)
        .bind(command.tenant_id)
        .bind(command.eval_run_uuid)
        .bind(command.lease_owner)
        .bind(command.lease_token)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn claim_queued_eval_runs(
        &self,
        limit: i32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlClaimedEvalRun>, NativeSqlStoreError> {
        match self.dialect() {
            MemorySqlDialect::Postgres => {
                self.claim_queued_eval_runs_postgres(
                    limit,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                )
                .await
            }
            MemorySqlDialect::Sqlite => {
                self.claim_queued_eval_runs_sqlite(
                    limit,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                )
                .await
            }
        }
    }

    async fn claim_queued_eval_runs_postgres(
        &self,
        limit: i32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlClaimedEvalRun>, NativeSqlStoreError> {
        let row_limit = i64::from(limit.max(1));
        let timestamp = now_text();
        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let rows = sqlx::query(
            r#"
            UPDATE ai_eval_run AS e
            SET state = 'running',
                started_at = COALESCE(e.started_at, ?),
                lease_owner = ?,
                lease_token = ?,
                lease_expires_at = ?,
                updated_at = ?
            FROM (
                SELECT tenant_id, uuid, eval_type
                FROM ai_eval_run
                WHERE state IN ('accepted', 'queued')
                ORDER BY created_at ASC
                LIMIT ?
                FOR UPDATE SKIP LOCKED
            ) AS picked
            WHERE e.tenant_id = picked.tenant_id
              AND e.uuid = picked.uuid
              AND e.state IN ('accepted', 'queued')
            RETURNING e.tenant_id, e.uuid, e.eval_type
            "#,
        )
        .bind(&timestamp)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&lease_expires_at)
        .bind(&timestamp)
        .bind(row_limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| NativeSqlClaimedEvalRun {
                tenant_id: row.get("tenant_id"),
                eval_run_uuid: row.get("uuid"),
                eval_type: row.get("eval_type"),
                lease_owner: lease_owner.to_string(),
                lease_token: lease_token.to_string(),
                lease_expires_at: lease_expires_at.clone(),
            })
            .collect())
    }

    async fn claim_queued_eval_runs_sqlite(
        &self,
        limit: i32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlClaimedEvalRun>, NativeSqlStoreError> {
        let mut connection = self.pool().acquire().await?;
        let mut claimed = Vec::new();
        let rows = sqlx::query(
            r#"
            SELECT tenant_id, uuid, eval_type
            FROM ai_eval_run
            WHERE state IN ('accepted', 'queued')
            ORDER BY created_at ASC
            LIMIT ?
            "#,
        )
        .bind(limit.max(1) as i64)
        .fetch_all(&mut *connection)
        .await?;

        for row in rows {
            let tenant_id: i64 = row.get("tenant_id");
            let eval_uuid: String = row.get("uuid");
            let eval_type: String = row.get("eval_type");
            let timestamp = now_text();
            let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
            let updated = sqlx::query(
                r#"
                UPDATE ai_eval_run
                SET state = 'running',
                    started_at = COALESCE(started_at, ?),
                    lease_owner = ?,
                    lease_token = ?,
                    lease_expires_at = ?,
                    updated_at = ?
                WHERE tenant_id = ? AND uuid = ? AND state IN ('accepted', 'queued')
                "#,
            )
            .bind(&timestamp)
            .bind(lease_owner)
            .bind(lease_token)
            .bind(&lease_expires_at)
            .bind(&timestamp)
            .bind(tenant_id)
            .bind(&eval_uuid)
            .execute(&mut *connection)
            .await?;
            if updated.rows_affected() == 0 {
                continue;
            }
            claimed.push(NativeSqlClaimedEvalRun {
                tenant_id,
                eval_run_uuid: eval_uuid,
                eval_type,
                lease_owner: lease_owner.to_string(),
                lease_token: lease_token.to_string(),
                lease_expires_at,
            });
        }
        Ok(claimed)
    }

    pub async fn renew_eval_run_lease(
        &self,
        tenant_id: i64,
        eval_run_uuid: &str,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<bool, NativeSqlStoreError> {
        let timestamp = now_text();
        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let updated = sqlx::query(
            r#"
            UPDATE ai_eval_run
            SET lease_expires_at = ?, updated_at = ?
            WHERE tenant_id = ? AND uuid = ? AND state = 'running'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#,
        )
        .bind(lease_expires_at)
        .bind(&timestamp)
        .bind(tenant_id)
        .bind(eval_run_uuid)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&timestamp)
        .execute(self.pool())
        .await?;
        Ok(updated.rows_affected() == 1)
    }
}

fn map_learning_job_row(row: sqlx::any::AnyRow) -> NativeSqlLearningJobRow {
    NativeSqlLearningJobRow {
        job_uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        space_id: row.get("space_id"),
        job_type: row.get("job_type"),
        state: row.get("state"),
        priority: row.get("priority"),
        input_json: row.get("input_json"),
        result_json: row.get("result_json"),
        error_json: row.get("error_json"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        lease_owner: row.get("lease_owner"),
        lease_token: row.get("lease_token"),
        lease_expires_at: row.get("lease_expires_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}
