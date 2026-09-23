use crate::sqlx_compat as sqlx;
use async_trait::async_trait;
use sdkwork_database_config::DatabaseConfig;
use sdkwork_database_id::SnowflakeIdGenerator;
use sdkwork_memory_spi::{
    AppendMemoryAuditCommand, AppendMemoryEventCommand, AppendMemoryOutboxCommand,
    AppendMemoryRetrievalTraceCommand, ApproveMemoryCandidateCommand, CreateCanonicalMemoryCommand,
    CreateMemoryCandidateCommand, CreateMemoryRecordCommand, DecayMemoryHabitCommand,
    DeleteCanonicalMemoryCommand, DeleteMemoryRecordCommand, ListMemoryCandidatesQuery,
    ListMemoryRetrievalTracesQuery, ListPendingMemoryOutboxQuery, MarkMemoryOutboxFailedCommand,
    MarkMemoryOutboxPublishedCommand, MemoryAuditRecord, MemoryAuditStorePort, MemoryCandidate,
    MemoryCandidateDetail, MemoryCandidatePage, MemoryCandidatePromotion, MemoryCandidateStorePort,
    MemoryCandidateSummary, MemoryCanonicalRecord, MemoryContextPackSnapshot,
    MemoryDeletionReceipt, MemoryEvent, MemoryEventStorePort, MemoryHabit, MemoryHabitStorePort,
    MemoryMutationJournal, MemoryOutboxEvent, MemoryOutboxStorePort, MemoryRecord,
    MemoryRecordQuotaAdmission, MemoryRecordStorePort, MemoryRetrievalEventCandidate,
    MemoryRetrievalHitDraft, MemoryRetrievalRecordCandidate, MemoryRetrievalTrace,
    MemoryRetrievalTraceStorePort, MemoryRetrieverKind, MemoryRetrieverPort, MemoryRetrieverResult,
    MemoryRetrieverSearchResult, MemoryScopeContext, MemorySensitivityReadScope, MemorySpiError,
    MemorySpiResult, MetadataFilterExpression, PromoteMemoryCandidateAtomicCommand,
    PromoteMemoryCandidateAtomicWithJournalCommand, PromoteMemoryHabitCommand,
    RejectMemoryCandidateCommand, RetrieveCanonicalMemoryQuery, RetrieveMemoryAuditQuery,
    RetrieveMemoryCandidateDetailQuery, RetrieveMemoryCandidateQuery,
    RetrieveMemoryCandidatesCommand, RetrieveMemoryEventQuery, RetrieveMemoryHabitQuery,
    RetrieveMemoryOutboxQuery, RetrieveMemoryRecordQuery,
    RetrieveMemoryRetrievalTraceForTenantQuery, RetrieveMemoryRetrievalTraceQuery,
    ScopedMemoryRetrievalTrace, SearchMemoryCandidatesQuery, SupersedeCanonicalMemoryAtomicCommand,
    UpdateCanonicalMemoryCommand, UpsertMemoryHabitCommand, MAX_MEMORY_RETRIEVAL_CANDIDATES,
};
use serde_json::Value;
use sqlx::any::AnyRow;
use sqlx::{AnyPool, Row};
use std::sync::OnceLock;
use thiserror::Error;

use crate::pool_backend::{connect_any_pool, MemorySqlDialect};
use crate::privacy::like_pattern;
use sdkwork_utils_rust::MAX_LIST_PAGE_SIZE;

use crate::canonical_data::{
    append_journal_on_tx, sync_record_fts_on_tx, validate_journal, FtsRecordProjection,
};

/// Minimum read scope: public and internal records only.
pub const SENSITIVITY_READ_PUBLIC: i32 = 0;
/// Elevated operator scope: includes private and sensitive, excludes restricted.
pub const SENSITIVITY_READ_ELEVATED: i32 = 1;
/// Space owner scope: all sensitivity tiers including restricted.
pub const SENSITIVITY_READ_OWNER: i32 = 2;

fn clamp_list_page_size(page_size: i32) -> i64 {
    page_size.clamp(1, MAX_LIST_PAGE_SIZE) as i64
}

const RECORD_SENSITIVITY_FILTER_SQL: &str = r#"
                  AND (
                    ? >= 2
                    OR (? >= 1 AND r.sensitivity_level IN ('public', 'internal', 'private', 'sensitive'))
                    OR r.sensitivity_level IN ('public', 'internal')
                  )
"#;

/// SQL fragment filtering rows by actor read scope. `table_alias` must be a trusted identifier.
pub fn sensitivity_level_filter_sql(table_alias: &str) -> String {
    format!(
        r#"
                  AND (
                    ? >= 2
                    OR (? >= 1 AND {table_alias}.sensitivity_level IN ('public', 'internal', 'private', 'sensitive'))
                    OR {table_alias}.sensitivity_level IN ('public', 'internal')
                  )
"#
    )
}

pub struct PromoteApprovedCandidateCommand<'a> {
    pub scope: &'a MemoryScopeContext,
    pub tenant_id: i64,
    pub candidate_id: &'a str,
    pub memory_uuid: &'a str,
    pub memory_type: &'a str,
    pub proposed_text: &'a str,
    pub evidence_links: &'a [(String, String, Option<f64>)],
    pub decided_by: Option<i64>,
    pub create_record: bool,
}

pub struct NativeSqlAppendRecordSourceCommand<'a> {
    pub scope: &'a MemoryScopeContext,
    pub source_id: &'a str,
    pub memory_uuid: &'a str,
    pub event_uuid: &'a str,
    pub source_role: &'a str,
    pub confidence_delta: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct NativeSqlMemoryStore {
    pool: AnyPool,
    dialect: MemorySqlDialect,
    id_generator: SnowflakeIdGenerator,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NativeSqlHardDeleteRecordOutcome {
    pub deleted: bool,
    pub rejected_candidates: u32,
}

#[derive(Debug, Clone)]
pub struct TenantRetrievalTraceLookup {
    pub space_id: i64,
    pub trace: MemoryRetrievalTrace,
    pub created_at: String,
}

impl NativeSqlMemoryStore {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self, NativeSqlStoreError> {
        Self::open_pool(config, true).await
    }

    pub async fn open_pool(
        config: &DatabaseConfig,
        apply_migration: bool,
    ) -> Result<Self, NativeSqlStoreError> {
        Self::open_pool_with_id_generator(config, apply_migration, development_id_generator()).await
    }

    pub async fn open_pool_with_id_generator(
        config: &DatabaseConfig,
        apply_migration: bool,
        id_generator: SnowflakeIdGenerator,
    ) -> Result<Self, NativeSqlStoreError> {
        let (pool, dialect) = connect_any_pool(config).await?;
        let store = Self {
            pool,
            dialect,
            id_generator,
        };
        if apply_migration {
            store.apply_phase1_migration().await?;
        }
        Ok(store)
    }

    pub async fn new_in_memory_sqlite() -> Result<Self, NativeSqlStoreError> {
        let config = DatabaseConfig {
            engine: sdkwork_database_config::DatabaseEngine::Sqlite,
            url: "sqlite::memory:".to_string(),
            ..DatabaseConfig::default()
        };
        Self::connect(&config).await
    }

    pub async fn from_any_pool(pool: AnyPool, dialect: MemorySqlDialect) -> Self {
        Self {
            pool,
            dialect,
            id_generator: development_id_generator(),
        }
    }

    pub async fn from_database_pool(
        pool: &sdkwork_database_sqlx::DatabasePool,
    ) -> Result<Self, NativeSqlStoreError> {
        Self::from_database_pool_with_id_generator(pool, development_id_generator()).await
    }

    pub async fn from_database_pool_with_id_generator(
        pool: &sdkwork_database_sqlx::DatabasePool,
        id_generator: SnowflakeIdGenerator,
    ) -> Result<Self, NativeSqlStoreError> {
        let config = crate::pool_backend::normalize_memory_database_config(pool.config().clone());
        Self::open_pool_with_id_generator(&config, false, id_generator).await
    }

    pub async fn install_sqlite_phase1_schema(pool: &AnyPool) -> Result<(), NativeSqlStoreError> {
        let store = Self::from_any_pool(pool.clone(), MemorySqlDialect::Sqlite).await;
        store.apply_phase1_migration().await
    }

    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    pub fn dialect(&self) -> MemorySqlDialect {
        self.dialect
    }

    pub(crate) fn next_row_id(&self) -> Result<i64, NativeSqlStoreError> {
        self.id_generator
            .generate()
            .map_err(|error| NativeSqlStoreError::IdGeneration(error.to_string()))
    }

    async fn apply_phase1_migration(&self) -> Result<(), NativeSqlStoreError> {
        if self.schema_is_initialized().await? {
            return Ok(());
        }
        match self.dialect {
            MemorySqlDialect::Sqlite => self.apply_sqlite_phase1_migration().await,
            MemorySqlDialect::Postgres => self.apply_postgres_phase1_migration().await,
        }
    }

    async fn schema_is_initialized(&self) -> Result<bool, NativeSqlStoreError> {
        let latest_version = match self.dialect() {
            MemorySqlDialect::Sqlite => "0010",
            MemorySqlDialect::Postgres => "0009",
        };
        match sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM ops_memory_schema_version WHERE version = ? LIMIT 1",
        )
        .bind(latest_version)
        .fetch_optional(&self.pool)
        .await
        {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(_) => Ok(false),
        }
    }

    async fn apply_postgres_phase1_migration(&self) -> Result<(), NativeSqlStoreError> {
        // Initialization state: the application-root module keeps the full DDL snapshot in the
        // consolidated baseline (database/ddl/baseline/postgres/0001_memory_baseline.sql);
        // migrations/ is reserved for post-GA changes and is intentionally empty.
        const MIGRATIONS: &[(&str, &str)] = &[(
            "baseline",
            include_str!("../../../database/ddl/baseline/postgres/0001_memory_baseline.sql"),
        )];
        self.apply_embedded_sql_migrations(MIGRATIONS).await
    }

    async fn apply_embedded_sql_migrations(
        &self,
        migrations: &[(&str, &str)],
    ) -> Result<(), NativeSqlStoreError> {
        // This compatibility bootstrap is used by isolated plugin tests. Production uses the
        // application-root sdkwork-database lifecycle and its checksum-backed history tables.
        let create_tracking_table = match self.dialect {
            MemorySqlDialect::Postgres => {
                r"CREATE TABLE IF NOT EXISTS ops_memory_schema_version (
                    version VARCHAR(32) PRIMARY KEY,
                    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                )"
            }
            MemorySqlDialect::Sqlite => {
                r"CREATE TABLE IF NOT EXISTS ops_memory_schema_version (
                    version TEXT PRIMARY KEY,
                    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
                )"
            }
        };
        let mut transaction = self.pool.begin().await?;
        if self.dialect == MemorySqlDialect::Postgres {
            // Serialize the compatibility migration path across concurrent process startups.
            // Production uses the root lifecycle runner, but tests and local tools may still
            // initialize this plugin directly against the same PostgreSQL database.
            sqlx::query("SELECT pg_advisory_xact_lock(714895751361736729)")
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(create_tracking_table)
            .execute(&mut *transaction)
            .await?;

        for (version, migration_sql) in migrations {
            let already_applied = sqlx::query_scalar::<_, i32>(
                "SELECT 1 FROM ops_memory_schema_version WHERE version = ? LIMIT 1",
            )
            .bind(version)
            .fetch_optional(&mut *transaction)
            .await?;

            if already_applied.is_some() {
                continue;
            }

            for statement in split_sql_statements(migration_sql) {
                let statement = statement.trim();
                if statement.is_empty() {
                    continue;
                }
                sqlx::query(statement).execute(&mut *transaction).await?;
            }

            sqlx::query("INSERT INTO ops_memory_schema_version (version) VALUES (?)")
                .bind(version)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn append_open_api_event(
        &self,
        scope: &MemoryScopeContext,
        event_id: &str,
        event_type: &str,
        source_type: &str,
        event_time: &str,
        payload: &Value,
        sensitivity_level: &str,
    ) -> Result<(), NativeSqlStoreError> {
        self.ensure_space(scope).await?;
        let payload_json = payload.to_string();
        let payload_hash = stable_hash(&payload_json);

        if let Some(existing) = self
            .retrieve_event_idempotency_state(scope, event_id)
            .await?
        {
            if existing.space_id == scope.space_id
                && existing.payload_json == payload_json
                && existing.payload_hash == payload_hash
            {
                return Ok(());
            }

            return Err(NativeSqlStoreError::EventConflict {
                tenant_id: scope.tenant_id,
                event_id: event_id.to_string(),
            });
        }

        let (actor_type, actor_id) = match scope.user_id {
            Some(user_id) => ("user", Some(user_id.to_string())),
            None => ("system", None),
        };
        sqlx::query(
            r#"
            INSERT INTO ai_event (
              id,
              uuid,
              tenant_id,
              space_id,
              user_id,
              actor_type,
              actor_id,
              event_type,
              source_type,
              event_time,
              payload_json,
              payload_hash,
              sensitivity_level,
              ingestion_status,
              created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'received', ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(event_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(scope.user_id)
        .bind(actor_type)
        .bind(actor_id.as_deref())
        .bind(event_type)
        .bind(source_type)
        .bind(event_time)
        .bind(payload_json)
        .bind(payload_hash)
        .bind(sensitivity_level)
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn retrieve_open_api_event_for_tenant(
        &self,
        tenant_id: i64,
        event_id: &str,
    ) -> Result<Option<NativeSqlOpenApiEventRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, space_id, user_id, actor_type, actor_id, event_type, source_type, event_time, payload_json, payload_hash, ingestion_status, created_at
            FROM ai_event
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let payload_json: String = row.get("payload_json");
            NativeSqlOpenApiEventRow {
                event_id: row.get("uuid"),
                space_id: row.get("space_id"),
                user_id: row.try_get("user_id").ok(),
                actor_type: row.try_get("actor_type").ok(),
                actor_id: row.try_get("actor_id").ok(),
                event_type: row.get("event_type"),
                source_type: row.get("source_type"),
                event_time: row.get("event_time"),
                payload: parse_event_payload(&payload_json).unwrap_or(Value::Null),
                payload_hash: row.get("payload_hash"),
                ingestion_status: row.get("ingestion_status"),
                created_at: row.get("created_at"),
            }
        }))
    }

    pub async fn retrieve_record_detail_for_tenant(
        &self,
        tenant_id: i64,
        memory_id: &str,
    ) -> Result<Option<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              r.uuid,
              r.space_id,
              r.user_id,
              r.scope,
              r.memory_type,
              r.subject,
              r.predicate,
              r.object_text,
              r.canonical_text,
              r.confidence,
              r.evidence_count,
              r.contradiction_count,
              r.status,
              r.sensitivity_level,
              r.created_at,
              r.updated_at,
              r.expires_at,
              r.version,
              sup.uuid AS supersedes_uuid,
              sub.uuid AS superseded_by_uuid
            FROM ai_record r
            LEFT JOIN ai_record sup
              ON sup.id = r.supersedes_memory_id AND sup.tenant_id = r.tenant_id
            LEFT JOIN ai_record sub
              ON sub.id = r.superseded_by_memory_id AND sub.tenant_id = r.tenant_id
            WHERE r.tenant_id = ?
              AND r.uuid = ?
              AND r.status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(record_detail_from_row))
    }

    pub async fn retrieve_open_api_event(
        &self,
        scope: &MemoryScopeContext,
        event_id: &str,
    ) -> Result<Option<NativeSqlOpenApiEventRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, space_id, user_id, actor_type, actor_id, event_type, source_type, event_time, payload_json, payload_hash, ingestion_status, created_at
            FROM ai_event
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let payload_json: String = row.get("payload_json");
            NativeSqlOpenApiEventRow {
                event_id: row.get("uuid"),
                space_id: row.get("space_id"),
                user_id: row.try_get("user_id").ok(),
                actor_type: row.try_get("actor_type").ok(),
                actor_id: row.try_get("actor_id").ok(),
                event_type: row.get("event_type"),
                source_type: row.get("source_type"),
                event_time: row.get("event_time"),
                payload: parse_event_payload(&payload_json).unwrap_or(Value::Null),
                payload_hash: row.get("payload_hash"),
                ingestion_status: row.get("ingestion_status"),
                created_at: row.get("created_at"),
            }
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_record_open_api(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
        scope_label: &str,
        memory_type: &str,
        subject: Option<&str>,
        predicate: Option<&str>,
        object_text: &str,
        canonical_text: &str,
        sensitivity_level: &str,
        expires_at: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        self.ensure_space(scope).await?;
        sqlx::query(
            r#"
            INSERT INTO ai_record (
              id,
              uuid,
              tenant_id,
              space_id,
              user_id,
              scope,
              memory_type,
              subject,
              predicate,
              object_text,
              canonical_text,
              confidence,
              evidence_count,
              contradiction_count,
              importance_score,
              recency_score,
              status,
              sensitivity_level,
              expires_at,
              created_at,
              updated_at,
              version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1.0, 1, 0, 0.5, 0.5, 'active', ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(memory_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(scope.user_id)
        .bind(scope_label)
        .bind(memory_type)
        .bind(subject)
        .bind(predicate.unwrap_or("is"))
        .bind(object_text)
        .bind(canonical_text)
        .bind(sensitivity_level)
        .bind(expires_at)
        .bind(now_text())
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        self.sync_record_fts_entry(
            scope,
            memory_id,
            canonical_text,
            object_text,
            subject,
            predicate,
        )
        .await?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn supersede_record_open_api(
        &self,
        scope: &MemoryScopeContext,
        old_memory_uuid: &str,
        new_memory_uuid: &str,
        scope_label: &str,
        memory_type: &str,
        subject: Option<&str>,
        predicate: Option<&str>,
        object_text: &str,
        canonical_text: &str,
        sensitivity_level: &str,
        expires_at: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        self.ensure_space(scope).await?;
        let old_row_id = self
            .lookup_record_row_id(scope, old_memory_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: format!("supersede source memory {old_memory_uuid} not found"),
            })?;

        self.create_record_open_api(
            scope,
            new_memory_uuid,
            scope_label,
            memory_type,
            subject,
            predicate,
            object_text,
            canonical_text,
            sensitivity_level,
            expires_at,
        )
        .await?;

        let new_row_id = self
            .lookup_record_row_id(scope, new_memory_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "supersede target memory {new_memory_uuid} not found after create"
                ),
            })?;

        let timestamp = now_text();
        sqlx::query(
            r#"
            UPDATE ai_record
            SET status = 'superseded',
                superseded_by_memory_id = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(new_row_id)
        .bind(&timestamp)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(old_memory_uuid)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            UPDATE ai_record
            SET supersedes_memory_id = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(old_row_id)
        .bind(&timestamp)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(new_memory_uuid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn retrieve_record_detail(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<Option<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              r.uuid,
              r.space_id,
              r.user_id,
              r.scope,
              r.memory_type,
              r.subject,
              r.predicate,
              r.object_text,
              r.canonical_text,
              r.confidence,
              r.evidence_count,
              r.contradiction_count,
              r.status,
              r.sensitivity_level,
              r.created_at,
              r.updated_at,
              r.expires_at,
              r.version,
              sup.uuid AS supersedes_uuid,
              sub.uuid AS superseded_by_uuid
            FROM ai_record r
            LEFT JOIN ai_record sup
              ON sup.id = r.supersedes_memory_id AND sup.tenant_id = r.tenant_id
            LEFT JOIN ai_record sub
              ON sub.id = r.superseded_by_memory_id AND sub.tenant_id = r.tenant_id
            WHERE r.tenant_id = ?
              AND r.space_id = ?
              AND r.uuid = ?
              AND r.status <> 'deleted'
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(record_detail_from_row))
    }

    pub async fn list_record_details(
        &self,
        scope: &MemoryScopeContext,
        query: Option<&str>,
        page_size: i32,
        cursor: Option<&str>,
        sensitivity_read_scope: i32,
    ) -> Result<Vec<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let sensitivity_read_scope =
            sensitivity_read_scope.clamp(SENSITIVITY_READ_PUBLIC, SENSITIVITY_READ_OWNER);
        let cursor = cursor.unwrap_or("");
        let query = query.unwrap_or("").trim();

        let rows = if query.is_empty() {
            sqlx::query(&format!(
                r#"
                SELECT
                  r.uuid,
                  r.space_id,
                  r.scope,
                  r.memory_type,
                  r.subject,
                  r.predicate,
                  r.object_text,
                  r.canonical_text,
                  r.confidence,
                  r.status,
                  r.sensitivity_level,
                  r.created_at,
                  r.updated_at,
                  r.expires_at,
                  r.version,
                  sup.uuid AS supersedes_uuid,
                  sub.uuid AS superseded_by_uuid
                FROM ai_record r
                LEFT JOIN ai_record sup
                  ON sup.id = r.supersedes_memory_id AND sup.tenant_id = r.tenant_id
                LEFT JOIN ai_record sub
                  ON sub.id = r.superseded_by_memory_id AND sub.tenant_id = r.tenant_id
                WHERE r.tenant_id = ?
                  AND r.space_id = ?
                  AND r.status <> 'deleted'
                  AND r.uuid > ?
                {RECORD_SENSITIVITY_FILTER_SQL}
                ORDER BY r.uuid ASC
                LIMIT ?
                "#
            ))
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(cursor)
            .bind(sensitivity_read_scope)
            .bind(sensitivity_read_scope)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        } else {
            let pattern = crate::privacy::like_pattern(query);
            sqlx::query(&format!(
                r#"
                SELECT
                  r.uuid,
                  r.space_id,
                  r.scope,
                  r.memory_type,
                  r.subject,
                  r.predicate,
                  r.object_text,
                  r.canonical_text,
                  r.confidence,
                  r.status,
                  r.sensitivity_level,
                  r.created_at,
                  r.updated_at,
                  r.expires_at,
                  r.version,
                  sup.uuid AS supersedes_uuid,
                  sub.uuid AS superseded_by_uuid
                FROM ai_record r
                LEFT JOIN ai_record sup
                  ON sup.id = r.supersedes_memory_id AND sup.tenant_id = r.tenant_id
                LEFT JOIN ai_record sub
                  ON sub.id = r.superseded_by_memory_id AND sub.tenant_id = r.tenant_id
                WHERE r.tenant_id = ?
                  AND r.space_id = ?
                  AND r.status <> 'deleted'
                  AND r.uuid > ?
                  AND (r.canonical_text LIKE ? ESCAPE '\'
                       OR r.object_text LIKE ? ESCAPE '\'
                       OR COALESCE(r.subject, '') LIKE ? ESCAPE '\')
                {RECORD_SENSITIVITY_FILTER_SQL}
                ORDER BY r.uuid ASC
                LIMIT ?
                "#
            ))
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(cursor)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(sensitivity_read_scope)
            .bind(sensitivity_read_scope)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(record_detail_from_row).collect())
    }

    pub async fn count_active_records_for_scope(
        &self,
        scope: &MemoryScopeContext,
    ) -> Result<i64, NativeSqlStoreError> {
        let row = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM ai_record
            WHERE tenant_id = ?
              AND space_id = ?
              AND status <> 'deleted'
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn count_user_owned_spaces_for_tenant(
        &self,
        tenant_id: i64,
        owner_subject_id: &str,
    ) -> Result<i64, NativeSqlStoreError> {
        let row = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM ai_space
            WHERE tenant_id = ?
              AND owner_subject_type = 'user'
              AND owner_subject_id = ?
              AND lifecycle_status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .bind(owner_subject_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_record_open_api(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
        canonical_text: Option<&str>,
        subject: Option<&str>,
    ) -> Result<Option<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        let existing = self.retrieve_record_detail(scope, memory_id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let canonical_text = canonical_text.unwrap_or(&existing.canonical_text);
        let subject = subject.or(existing.subject.as_deref());

        sqlx::query(
            r#"
            UPDATE ai_record
            SET canonical_text = ?,
                object_text = ?,
                subject = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ?
              AND space_id = ?
              AND uuid = ?
              AND status <> 'deleted'
            "#,
        )
        .bind(canonical_text)
        .bind(canonical_text)
        .bind(subject)
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .execute(&self.pool)
        .await?;

        let updated = self.retrieve_record_detail(scope, memory_id).await?;
        if let Some(ref detail) = updated {
            self.sync_record_fts_entry(
                scope,
                memory_id,
                &detail.canonical_text,
                &detail.object_text,
                detail.subject.as_deref(),
                detail.predicate.as_deref(),
            )
            .await?;
        }

        Ok(updated)
    }

    pub async fn search_record_details_keyword(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
        top_k: i32,
    ) -> Result<Vec<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        self.search_record_details_keyword_filtered(
            scope,
            query,
            top_k,
            &[],
            MemorySensitivityReadScope::Owner,
            None,
        )
        .await
        .map(|(rows, _degraded)| rows)
    }

    pub async fn search_record_details_keyword_filtered(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
        top_k: i32,
        memory_types: &[String],
        read_scope: MemorySensitivityReadScope,
        metadata_filter: Option<&MetadataFilterExpression>,
    ) -> Result<(Vec<NativeSqlMemoryRecordDetail>, bool), NativeSqlStoreError> {
        let fulltext_result = self
            .search_record_details_fulltext_filtered(
                scope,
                query,
                top_k,
                memory_types,
                read_scope,
                metadata_filter,
            )
            .await;
        let degraded = match fulltext_result {
            Ok(rows) if !rows.is_empty() => return Ok((rows, false)),
            Ok(_) => false,
            Err(error) if crate::search_index::is_recoverable_fulltext_error(&error) => true,
            Err(error) => return Err(error),
        };
        let pattern = crate::privacy::like_pattern(query.trim());
        let sensitivity_level = Self::read_scope_level(read_scope);
        let mut sql = String::from(
            r#"
            SELECT
              r.uuid,
              r.space_id,
              r.user_id,
              r.scope,
              r.memory_type,
              r.subject,
              r.predicate,
              r.object_text,
              r.canonical_text,
              r.confidence,
              r.evidence_count,
              r.contradiction_count,
              r.status,
              r.sensitivity_level,
              r.created_at,
              r.updated_at,
              r.expires_at,
              r.version,
              sup.uuid AS supersedes_uuid,
              sub.uuid AS superseded_by_uuid
            FROM ai_record r
            LEFT JOIN ai_record sup
              ON sup.id = r.supersedes_memory_id AND sup.tenant_id = r.tenant_id
            LEFT JOIN ai_record sub
              ON sub.id = r.superseded_by_memory_id AND sub.tenant_id = r.tenant_id
            WHERE r.tenant_id = ?
              AND r.space_id = ?
              AND r.status <> 'deleted'
              AND (r.canonical_text LIKE ? ESCAPE '\'
                   OR r.object_text LIKE ? ESCAPE '\'
                   OR COALESCE(r.subject, '') LIKE ? ESCAPE '\'
                   OR COALESCE(r.predicate, '') LIKE ? ESCAPE '\')
            "#,
        );
        let filter_predicate = crate::filter_pushdown::translate_metadata_filter(
            self.dialect(),
            "r",
            metadata_filter,
        )?;
        Self::append_sensitivity_filter(&mut sql, "r");
        Self::append_memory_type_filter(&mut sql, "r", memory_types);
        sql.push_str(&filter_predicate.sql);
        sql.push_str(" ORDER BY r.updated_at DESC, r.uuid ASC LIMIT ?");
        let mut query_builder = sqlx::query(&sql)
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(&pattern)
            .bind(sensitivity_level)
            .bind(sensitivity_level);
        for memory_type in memory_types {
            query_builder = query_builder.bind(memory_type);
        }
        for bind in &filter_predicate.binds {
            query_builder = query_builder.bind(bind);
        }
        let rows = query_builder
            .bind(top_k.max(1) as i64)
            .fetch_all(&self.pool)
            .await?;

        Ok((
            rows.into_iter().map(record_detail_from_row).collect(),
            degraded,
        ))
    }

    pub async fn search_open_api_events_keyword(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
        top_k: i32,
    ) -> Result<Vec<NativeSqlOpenApiEventRow>, NativeSqlStoreError> {
        let pattern = crate::privacy::like_pattern(query.trim());
        let rows = sqlx::query(
            r#"
            SELECT uuid, space_id, event_type, source_type, event_time, payload_json, payload_hash,
                   ingestion_status, created_at
            FROM ai_event
            WHERE tenant_id = ?
              AND space_id = ?
              AND payload_json LIKE ? ESCAPE '\'
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(&pattern)
        .bind(top_k.max(1) as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlOpenApiEventRow {
                event_id: row.get("uuid"),
                space_id: row.get("space_id"),
                user_id: row.try_get("user_id").ok(),
                actor_type: row.try_get("actor_type").ok(),
                actor_id: row.try_get("actor_id").ok(),
                event_type: row.get("event_type"),
                source_type: row.get("source_type"),
                event_time: row.get("event_time"),
                payload: row
                    .get::<String, _>("payload_json")
                    .parse()
                    .unwrap_or_else(|_| serde_json::json!({})),
                payload_hash: row.get("payload_hash"),
                ingestion_status: row.get("ingestion_status"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn search_linked_event_candidates_keyword(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
        limit: u32,
        memory_types: &[String],
        read_scope: MemorySensitivityReadScope,
        metadata_filter: Option<&MetadataFilterExpression>,
    ) -> Result<Vec<MemoryRetrievalEventCandidate>, NativeSqlStoreError> {
        let pattern = crate::privacy::like_pattern(query.trim());
        let sensitivity_level = Self::read_scope_level(read_scope);
        let filter_predicate = crate::filter_pushdown::translate_metadata_filter(
            self.dialect(),
            "record",
            metadata_filter,
        )?;
        let mut sql = String::from(
            r#"
            SELECT
              record.uuid AS memory_uuid,
              event.uuid AS event_uuid,
              event.payload_json,
              event.created_at
            FROM ai_event event
            INNER JOIN ai_record_source source
              ON source.tenant_id = event.tenant_id
             AND source.event_id = event.id
            INNER JOIN ai_record record
              ON record.tenant_id = source.tenant_id
             AND record.id = source.memory_id
            WHERE event.tenant_id = ?
              AND event.space_id = ?
              AND record.space_id = ?
              AND record.status <> 'deleted'
              AND event.payload_json LIKE ? ESCAPE '\'
            "#,
        );
        Self::append_sensitivity_filter(&mut sql, "record");
        Self::append_memory_type_filter(&mut sql, "record", memory_types);
        sql.push_str(&filter_predicate.sql);
        sql.push_str(" ORDER BY event.created_at DESC, record.uuid ASC, event.uuid ASC LIMIT ?");
        let mut query_builder = sqlx::query(&sql)
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(scope.space_id)
            .bind(&pattern)
            .bind(sensitivity_level)
            .bind(sensitivity_level);
        for memory_type in memory_types {
            query_builder = query_builder.bind(memory_type);
        }
        for bind in &filter_predicate.binds {
            query_builder = query_builder.bind(bind);
        }
        let rows = query_builder
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| MemoryRetrievalEventCandidate {
                memory_id: row.get("memory_uuid"),
                event_id: row.get("event_uuid"),
                payload_text: row.get::<String, _>("payload_json"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn search_memory_candidates(
        &self,
        query: &SearchMemoryCandidatesQuery,
    ) -> Result<MemoryRetrieverSearchResult, NativeSqlStoreError> {
        let trimmed_query = query.query.trim();
        if trimmed_query.is_empty() {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "memory retrieval query must not be blank".to_string(),
            });
        }

        if query.retriever_kinds.is_empty() {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "memory retrieval must select at least one retriever".to_string(),
            });
        }
        if query.limit == 0 || query.limit > MAX_MEMORY_RETRIEVAL_CANDIDATES {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "memory retrieval candidate limit must be between 1 and {MAX_MEMORY_RETRIEVAL_CANDIDATES}"
                ),
            });
        }

        let total_limit = query.limit;
        let unavailable_retriever_kinds = query
            .retriever_kinds
            .iter()
            .filter(|kind| {
                !matches!(
                    kind,
                    MemoryRetrieverKind::Sql
                        | MemoryRetrieverKind::Keyword
                        | MemoryRetrieverKind::Dictionary
                        | MemoryRetrieverKind::Time
                        | MemoryRetrieverKind::Event
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let record_retriever_enabled = query.retriever_kinds.iter().any(|kind| {
            matches!(
                kind,
                MemoryRetrieverKind::Sql
                    | MemoryRetrieverKind::Keyword
                    | MemoryRetrieverKind::Dictionary
                    | MemoryRetrieverKind::Time
            )
        });
        let event_retriever_enabled = query.retriever_kinds.contains(&MemoryRetrieverKind::Event);

        if !record_retriever_enabled && !event_retriever_enabled {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "selected memory retrievers are not supported by native SQL".to_string(),
            });
        }

        let (record_limit, event_limit) = match (record_retriever_enabled, event_retriever_enabled)
        {
            (true, true) => (total_limit.saturating_add(1) / 2, total_limit / 2),
            (true, false) => (total_limit, 0),
            (false, true) => (0, total_limit),
            (false, false) => unreachable!("supported retriever selection was checked above"),
        };

        let (records, record_search_degraded) = if record_retriever_enabled {
            let (rows, degraded) = self
                .search_record_details_keyword_filtered(
                    &query.scope,
                    trimmed_query,
                    i32::try_from(record_limit).unwrap_or(i32::MAX),
                    &query.memory_types,
                    query.read_scope,
                    query.metadata_filter.as_ref(),
                )
                .await?;
            (
                rows.into_iter()
                    .map(|row| MemoryRetrievalRecordCandidate {
                        memory_id: row.memory_id,
                        subject: row.subject,
                        predicate: row.predicate,
                        object_text: row.object_text,
                        canonical_text: row.canonical_text,
                        created_at: row.created_at,
                    })
                    .collect(),
                degraded,
            )
        } else {
            (Vec::new(), false)
        };
        let events = if event_retriever_enabled && event_limit > 0 {
            self.search_linked_event_candidates_keyword(
                &query.scope,
                trimmed_query,
                event_limit,
                &query.memory_types,
                query.read_scope,
                query.metadata_filter.as_ref(),
            )
            .await?
        } else {
            Vec::new()
        };

        let mut degradation_codes = Vec::new();
        if record_search_degraded {
            degradation_codes.push("fulltext_fallback".to_string());
        }
        if !unavailable_retriever_kinds.is_empty() {
            degradation_codes.push("retriever_kind_unavailable".to_string());
        }
        Ok(MemoryRetrieverSearchResult {
            records,
            events,
            degraded: record_search_degraded || !unavailable_retriever_kinds.is_empty(),
            unavailable_retriever_kinds,
            degradation_codes,
        })
    }

    pub async fn append_event(
        &self,
        scope: &MemoryScopeContext,
        event_id: &str,
        content: &str,
    ) -> Result<(), NativeSqlStoreError> {
        self.ensure_space(scope).await?;
        let payload_json = serde_json::json!({ "content": content }).to_string();
        let payload_hash = stable_hash(content);

        if let Some(existing) = self
            .retrieve_event_idempotency_state(scope, event_id)
            .await?
        {
            if existing.space_id == scope.space_id
                && existing.payload_json == payload_json
                && existing.payload_hash == payload_hash
            {
                return Ok(());
            }

            return Err(NativeSqlStoreError::EventConflict {
                tenant_id: scope.tenant_id,
                event_id: event_id.to_string(),
            });
        }

        let (actor_type, actor_id) = match scope.user_id {
            Some(user_id) => ("user", Some(user_id.to_string())),
            None => ("system", None),
        };
        sqlx::query(
            r#"
            INSERT INTO ai_event (
              id,
              uuid,
              tenant_id,
              space_id,
              user_id,
              actor_type,
              actor_id,
              event_type,
              source_type,
              event_time,
              payload_json,
              payload_hash,
              sensitivity_level,
              ingestion_status,
              created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, 'memory.event.appended', 'api', ?, ?, ?, 'internal', 'received', ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(event_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(scope.user_id)
        .bind(actor_type)
        .bind(actor_id.as_deref())
        .bind(now_text())
        .bind(payload_json)
        .bind(payload_hash)
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn retrieve_event(
        &self,
        scope: &MemoryScopeContext,
        event_id: &str,
    ) -> Result<Option<NativeSqlMemoryEvent>, NativeSqlStoreError> {
        let row = sqlx::query(
            "SELECT uuid, payload_json FROM ai_event WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let payload: String = row.get("payload_json");
            let payload = parse_event_payload(&payload).unwrap_or(Value::Null);
            NativeSqlMemoryEvent {
                event_id: row.get("uuid"),
                content: payload
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }
        }))
    }

    pub async fn retrieve_event_payload(
        &self,
        scope: &MemoryScopeContext,
        event_id: &str,
    ) -> Result<Option<Value>, NativeSqlStoreError> {
        let row = sqlx::query(
            "SELECT payload_json FROM ai_event WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .map(|row| {
                let payload: String = row.get("payload_json");
                parse_event_payload(&payload)
            })
            .transpose()?)
    }

    pub async fn create_record(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
        subject: &str,
        content: &str,
    ) -> Result<(), NativeSqlStoreError> {
        self.ensure_space(scope).await?;
        sqlx::query(
            r#"
            INSERT INTO ai_record (
              id,
              uuid,
              tenant_id,
              space_id,
              scope,
              memory_type,
              subject,
              predicate,
              object_text,
              canonical_text,
              confidence,
              evidence_count,
              contradiction_count,
              importance_score,
              recency_score,
              status,
              sensitivity_level,
              created_at,
              updated_at
            )
            VALUES (?, ?, ?, ?, 'user', 'semantic', ?, 'is', ?, ?, 1.0, 1, 0, 0.5, 0.5, 'active', 'internal', ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(memory_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(subject)
        .bind(content)
        .bind(content)
        .bind(now_text())
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        self.sync_record_fts_entry(
            scope,
            memory_id,
            content,
            content,
            Some(subject),
            Some("is"),
        )
        .await?;

        Ok(())
    }

    pub async fn retrieve_record(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<Option<NativeSqlMemoryRecord>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, object_text
            FROM ai_record
            WHERE tenant_id = ?
              AND space_id = ?
              AND uuid = ?
              AND status <> 'deleted'
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlMemoryRecord {
            memory_id: row.get("uuid"),
            content: row.get("object_text"),
        }))
    }

    pub async fn mark_record_deleted(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<MemoryDeletionReceipt, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT status, deleted_at
            FROM ai_record
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(MemoryDeletionReceipt {
                memory_id: memory_id.to_string(),
                deleted: false,
                already_deleted: false,
            });
        };

        let status: String = row.get("status");
        let deleted_at: Option<String> = row.get("deleted_at");

        if status == "deleted" || deleted_at.is_some() {
            return Ok(MemoryDeletionReceipt {
                memory_id: memory_id.to_string(),
                deleted: true,
                already_deleted: true,
            });
        }

        sqlx::query(
            r#"
            UPDATE ai_record
            SET status = 'deleted',
                deleted_at = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(now_text())
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .execute(&self.pool)
        .await?;

        Ok(MemoryDeletionReceipt {
            memory_id: memory_id.to_string(),
            deleted: true,
            already_deleted: false,
        })
    }

    pub async fn hard_delete_record(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<bool, NativeSqlStoreError> {
        Ok(self
            .hard_delete_record_with_cleanup(scope, memory_id)
            .await?
            .deleted)
    }

    /// Permanently remove one canonical record and every physical FK reference
    /// that could otherwise block deletion or retain a stale readable target.
    pub async fn hard_delete_record_with_cleanup(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<NativeSqlHardDeleteRecordOutcome, NativeSqlStoreError> {
        let mut tx = self.begin_tx().await?;
        self.lock_space_on_tx(&mut tx, scope).await?;
        let record_query = match self.dialect() {
            MemorySqlDialect::Postgres => {
                "SELECT id FROM ai_record WHERE tenant_id = ? AND space_id = ? AND uuid = ? FOR UPDATE"
            }
            MemorySqlDialect::Sqlite => {
                "SELECT id FROM ai_record WHERE tenant_id = ? AND space_id = ? AND uuid = ?"
            }
        };
        let record_id = sqlx::query_scalar::<_, i64>(record_query)
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(memory_id)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(record_id) = record_id else {
            if matches!(self.dialect(), MemorySqlDialect::Sqlite) {
                sqlx::query(
                    "DELETE FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
                )
                .bind(scope.tenant_id)
                .bind(scope.space_id)
                .bind(memory_id)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await.map_err(NativeSqlStoreError::from)?;
            return Ok(NativeSqlHardDeleteRecordOutcome::default());
        };

        let now = now_text();
        let rejected_candidates: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM ai_candidate
            WHERE tenant_id = ? AND target_memory_id = ? AND decision_state = 'pending'
            "#,
        )
        .bind(scope.tenant_id)
        .bind(record_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM ai_record_source WHERE tenant_id = ? AND memory_id = ?")
            .bind(scope.tenant_id)
            .bind(record_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            UPDATE ai_candidate
            SET target_memory_id = NULL,
                decision_state = CASE WHEN decision_state = 'pending' THEN 'rejected' ELSE decision_state END,
                decision_reason = CASE WHEN decision_state = 'pending' THEN 'privacy_forget' ELSE decision_reason END,
                decided_at = CASE WHEN decision_state = 'pending' THEN ? ELSE decided_at END,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND target_memory_id = ?
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(record_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE ai_habit
            SET promoted_memory_id = NULL, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND promoted_memory_id = ?
            "#,
        )
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(record_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE ai_retrieval_hit
            SET memory_id = NULL, status = 'suppressed', explanation_json = NULL
            WHERE tenant_id = ? AND memory_id = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(record_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE ai_edge
            SET source_memory_id = NULL, status = 'deleted', updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND source_memory_id = ?
            "#,
        )
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(record_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE ai_memory_binding
            SET source_memory_id = NULL,
                target_memory_id = NULL,
                status = 'deleted',
                deleted_at = COALESCE(deleted_at, ?),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND (source_memory_id = ? OR target_memory_id = ?)
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(record_id)
        .bind(record_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE ai_record
            SET supersedes_memory_id = CASE WHEN supersedes_memory_id = ? THEN NULL ELSE supersedes_memory_id END,
                superseded_by_memory_id = CASE WHEN superseded_by_memory_id = ? THEN NULL ELSE superseded_by_memory_id END,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ?
              AND (supersedes_memory_id = ? OR superseded_by_memory_id = ?)
            "#,
        )
        .bind(record_id)
        .bind(record_id)
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(record_id)
        .bind(record_id)
        .execute(&mut *tx)
        .await?;
        if matches!(self.dialect(), MemorySqlDialect::Sqlite) {
            sqlx::query(
                "DELETE FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
            )
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(memory_id)
            .execute(&mut *tx)
            .await?;
        }
        let deleted = sqlx::query(
            r#"
            DELETE FROM ai_record
            WHERE id = ? AND tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(record_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await.map_err(NativeSqlStoreError::from)?;

        let rejected_candidates = u32::try_from(rejected_candidates).map_err(|_| {
            NativeSqlStoreError::InvariantViolation {
                message: "pending candidate count exceeded u32 range during privacy deletion"
                    .to_string(),
            }
        })?;
        Ok(NativeSqlHardDeleteRecordOutcome {
            deleted: deleted > 0,
            rejected_candidates,
        })
    }

    pub async fn retrieve_record_lifecycle(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<Option<NativeSqlMemoryRecordLifecycle>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, status, deleted_at
            FROM ai_record
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlMemoryRecordLifecycle {
            memory_id: row.get("uuid"),
            status: row.get("status"),
            deleted_at: row.get("deleted_at"),
        }))
    }

    pub async fn append_audit(
        &self,
        scope: &MemoryScopeContext,
        audit_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        result: &str,
    ) -> Result<NativeSqlMemoryAuditRecord, NativeSqlStoreError> {
        let (actor_type, actor_id) = match scope.user_id {
            Some(user_id) => ("user", Some(user_id.to_string())),
            None => ("system", None),
        };
        sqlx::query(
            r#"
            INSERT INTO ai_audit_log (
              id,
              uuid,
              tenant_id,
              actor_type,
              actor_id,
              action,
              resource_type,
              resource_id,
              result,
              created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(audit_id)
        .bind(scope.tenant_id)
        .bind(actor_type)
        .bind(actor_id.as_deref())
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(result)
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(NativeSqlMemoryAuditRecord {
            audit_id: audit_id.to_string(),
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            result: result.to_string(),
        })
    }

    // -----------------------------------------------------------------
    // Transactional helpers ??allow multi-step writes to be atomic.
    // -----------------------------------------------------------------

    /// Begin a database transaction for atomic multi-step operations.
    pub async fn begin_tx(&self) -> Result<sqlx::Transaction<'_, sqlx::Any>, NativeSqlStoreError> {
        self.pool.begin().await.map_err(Into::into)
    }

    /// Serialize per-space record admission and return the active count while
    /// the space row remains locked by the caller's transaction.
    pub(crate) async fn lock_space_and_count_active_records_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
    ) -> Result<u64, NativeSqlStoreError> {
        self.lock_space_on_tx(tx, scope).await?;
        self.count_active_records_on_tx(tx, scope).await
    }

    pub(crate) async fn lock_space_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
    ) -> Result<(), NativeSqlStoreError> {
        let exists = match self.dialect() {
            MemorySqlDialect::Postgres => {
                sqlx::query("SELECT id FROM ai_space WHERE tenant_id = ? AND id = ? FOR UPDATE")
                    .bind(scope.tenant_id)
                    .bind(scope.space_id)
                    .fetch_optional(&mut **tx)
                    .await?
                    .is_some()
            }
            MemorySqlDialect::Sqlite => {
                sqlx::query("UPDATE ai_space SET version = version WHERE tenant_id = ? AND id = ?")
                    .bind(scope.tenant_id)
                    .bind(scope.space_id)
                    .execute(&mut **tx)
                    .await?
                    .rows_affected()
                    == 1
            }
        };
        if !exists {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "memory space {} does not exist for tenant {}",
                    scope.space_id, scope.tenant_id
                ),
            });
        }
        Ok(())
    }

    pub(crate) async fn count_active_records_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
    ) -> Result<u64, NativeSqlStoreError> {
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM ai_record
            WHERE tenant_id = ?
              AND space_id = ?
              AND status <> 'deleted'
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .fetch_one(&mut **tx)
        .await?;
        u64::try_from(count).map_err(|_| NativeSqlStoreError::InvariantViolation {
            message: format!(
                "active memory count for space {} was negative",
                scope.space_id
            ),
        })
    }

    async fn lock_candidate_target_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        candidate_id: &str,
    ) -> Result<(Option<String>, bool, String), NativeSqlStoreError> {
        let candidate_exists = match self.dialect() {
            MemorySqlDialect::Postgres => {
                sqlx::query(
                    "SELECT id FROM ai_candidate WHERE tenant_id = ? AND space_id = ? AND uuid = ? FOR UPDATE",
                )
                .bind(scope.tenant_id)
                .bind(scope.space_id)
                .bind(candidate_id)
                .fetch_optional(&mut **tx)
                .await?
                .is_some()
            }
            MemorySqlDialect::Sqlite => {
                // SQLite has no FOR UPDATE; the no-op UPDATE acquires its writer lock.
                sqlx::query(
                    "UPDATE ai_candidate SET version = version WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
                )
                .bind(scope.tenant_id)
                .bind(scope.space_id)
                .bind(candidate_id)
                .execute(&mut **tx)
                .await?
                .rows_affected()
                    == 1
            }
        };
        if !candidate_exists {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "candidate {candidate_id} does not exist in space {} for tenant {}",
                    scope.space_id, scope.tenant_id
                ),
            });
        }

        let row = sqlx::query(
            r#"
            SELECT
              record.uuid AS target_memory_uuid,
              candidate.target_memory_id AS target_memory_row_id,
              candidate.decision_state
            FROM ai_candidate candidate
            LEFT JOIN ai_record record
              ON record.id = candidate.target_memory_id
             AND record.tenant_id = candidate.tenant_id
             AND record.space_id = candidate.space_id
             AND record.status <> 'deleted'
            WHERE candidate.tenant_id = ?
              AND candidate.space_id = ?
              AND candidate.uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(candidate_id)
        .fetch_one(&mut **tx)
        .await?;
        Ok((
            row.try_get("target_memory_uuid")?,
            row.try_get::<Option<i64>, _>("target_memory_row_id")?
                .is_some(),
            row.try_get("decision_state")?,
        ))
    }

    /// Create a record within an existing transaction (no ensure_space check).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_record_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        memory_id: &str,
        scope_label: &str,
        memory_type: &str,
        subject: Option<&str>,
        predicate: Option<&str>,
        object_text: &str,
        canonical_text: &str,
        sensitivity_level: &str,
        expires_at: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_record (
              id, uuid, tenant_id, space_id, user_id, scope, memory_type,
              subject, predicate, object_text, canonical_text,
              confidence, evidence_count, contradiction_count,
              importance_score, recency_score,
              status, sensitivity_level, expires_at, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1.0, 1, 0, 0.5, 0.5, 'active', ?, ?, ?, ?, 1)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(memory_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(scope.user_id)
        .bind(scope_label)
        .bind(memory_type)
        .bind(subject)
        .bind(predicate.unwrap_or("is"))
        .bind(object_text)
        .bind(canonical_text)
        .bind(sensitivity_level)
        .bind(expires_at)
        .bind(now_text())
        .bind(now_text())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Append an outbox event within an existing transaction (no idempotency check).
    pub async fn append_outbox_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            INSERT INTO ai_outbox_event (
              id, uuid, tenant_id, aggregate_type, aggregate_id,
              event_type, event_version, payload_json,
              publish_state, retry_count, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', 0, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(&journal.outbox_id)
        .bind(scope.tenant_id)
        .bind(&journal.aggregate_type)
        .bind(&journal.aggregate_id)
        .bind(&journal.event_type)
        .bind(&journal.event_version)
        .bind(&journal.payload_json)
        .bind(now_text())
        .bind(now_text())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Append an audit log entry within an existing transaction.
    pub async fn append_audit_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        let (actor_type, actor_id) = match scope.user_id {
            Some(user_id) => ("user", Some(user_id.to_string())),
            None => ("system", None),
        };
        sqlx::query(
            r#"
            INSERT INTO ai_audit_log (
              id, uuid, tenant_id, actor_type, actor_id,
              action, resource_type, resource_id, result, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(&journal.audit_id)
        .bind(scope.tenant_id)
        .bind(actor_type)
        .bind(actor_id.as_deref())
        .bind(&journal.audit_action)
        .bind(&journal.audit_resource_type)
        .bind(&journal.audit_resource_id)
        .bind(&journal.audit_result)
        .bind(now_text())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Mark a record as deleted within an existing transaction.
    pub async fn mark_record_deleted_on_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let affected = sqlx::query(
            r#"
            UPDATE ai_record
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        Ok(affected > 0)
    }

    /// Update a record's text fields within an existing transaction.
    pub async fn update_record_on_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        memory_id: &str,
        canonical_text: Option<&str>,
        subject: Option<&str>,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let affected = sqlx::query(
            r#"
            UPDATE ai_record
            SET canonical_text = COALESCE(?, canonical_text),
                subject = COALESCE(?, subject),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(canonical_text)
        .bind(subject)
        .bind(&now)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        Ok(affected > 0)
    }

    pub async fn append_record_source_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        command: NativeSqlAppendRecordSourceCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        let result = sqlx::query(
            r#"
            INSERT INTO ai_record_source (
              id,
              uuid,
              tenant_id,
              memory_id,
              event_id,
              source_role,
              confidence_delta,
              created_at
            )
            SELECT
              ?,
              ?,
              ?,
              record.id,
              event.id,
              ?,
              ?,
              ?
            FROM ai_record record
            JOIN ai_event event
              ON event.tenant_id = record.tenant_id
             AND event.space_id = record.space_id
             AND event.uuid = ?
            WHERE record.tenant_id = ?
              AND record.space_id = ?
              AND record.uuid = ?
              AND record.status <> 'deleted'
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(command.source_id)
        .bind(command.scope.tenant_id)
        .bind(command.source_role)
        .bind(command.confidence_delta)
        .bind(now_text())
        .bind(command.event_uuid)
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(command.memory_uuid)
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "memory or event not found for record source".to_string(),
            });
        }
        Ok(())
    }

    pub async fn set_candidate_target_memory_on_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        candidate_id: &str,
        memory_uuid: &str,
    ) -> Result<(), NativeSqlStoreError> {
        let result = sqlx::query(
            r#"
            UPDATE ai_candidate
            SET target_memory_id = (
              SELECT record.id
              FROM ai_record record
              WHERE record.tenant_id = ?
                AND record.space_id = ?
                AND record.uuid = ?
                AND record.status <> 'deleted'
            ),
            updated_at = ?
            WHERE tenant_id = ?
              AND space_id = ?
              AND uuid = ?
              AND EXISTS (
                SELECT 1
                FROM ai_record record
                WHERE record.tenant_id = ?
                  AND record.space_id = ?
                  AND record.uuid = ?
                  AND record.status <> 'deleted'
              )
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_uuid)
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(candidate_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_uuid)
        .execute(&mut **tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "candidate or promoted memory not found".to_string(),
            });
        }
        Ok(())
    }

    pub async fn approve_candidate_on_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        scope: &MemoryScopeContext,
        candidate_id: &str,
        decided_by: Option<i64>,
    ) -> Result<(), NativeSqlStoreError> {
        sqlx::query(
            r#"
            UPDATE ai_candidate
            SET decision_state = 'approved',
                decision_reason = NULL,
                decided_by = ?,
                decided_at = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(decided_by)
        .bind(now_text())
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(candidate_id)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn promote_and_approve_candidate(
        &self,
        command: PromoteApprovedCandidateCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        match self
            .promote_and_approve_candidate_with_quota(command, 0)
            .await?
        {
            MemoryRecordQuotaAdmission::Admitted(_) => Ok(()),
            MemoryRecordQuotaAdmission::QuotaExceeded { .. } => {
                Err(NativeSqlStoreError::InvariantViolation {
                    message: "unlimited candidate promotion was rejected by quota admission"
                        .to_string(),
                })
            }
        }
    }

    pub async fn promote_and_approve_candidate_with_quota(
        &self,
        command: PromoteApprovedCandidateCommand<'_>,
        max_active_records: u64,
    ) -> Result<MemoryRecordQuotaAdmission<String>, NativeSqlStoreError> {
        self.promote_and_approve_candidate_with_quota_inner(command, max_active_records, None)
            .await
    }

    pub async fn promote_and_approve_candidate_with_quota_and_journal(
        &self,
        command: PromoteApprovedCandidateCommand<'_>,
        max_active_records: u64,
        journal: &MemoryMutationJournal,
    ) -> Result<MemoryRecordQuotaAdmission<String>, NativeSqlStoreError> {
        validate_journal(command.memory_uuid, journal)?;
        self.promote_and_approve_candidate_with_quota_inner(
            command,
            max_active_records,
            Some(journal),
        )
        .await
    }

    async fn promote_and_approve_candidate_with_quota_inner(
        &self,
        command: PromoteApprovedCandidateCommand<'_>,
        max_active_records: u64,
        journal: Option<&MemoryMutationJournal>,
    ) -> Result<MemoryRecordQuotaAdmission<String>, NativeSqlStoreError> {
        if command.tenant_id != command.scope.tenant_id {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "candidate promotion tenant must match its memory scope".to_string(),
            });
        }
        let mut tx = self.begin_tx().await?;
        if command.create_record {
            // The space lock serializes quota admission; the candidate lock then makes
            // retries for one candidate observe the target created by an earlier request.
            self.lock_space_on_tx(&mut tx, command.scope).await?;
        }
        let (existing_target, has_target_reference, decision_state) = self
            .lock_candidate_target_on_tx(&mut tx, command.scope, command.candidate_id)
            .await?;
        if has_target_reference {
            if decision_state != "approved" {
                tx.rollback().await.map_err(NativeSqlStoreError::from)?;
                return Err(NativeSqlStoreError::InvariantViolation {
                    message: format!(
                        "candidate {} has a target reference with unexpected decision state {}",
                        command.candidate_id, decision_state
                    ),
                });
            }
            let Some(existing_target) = existing_target else {
                tx.rollback().await.map_err(NativeSqlStoreError::from)?;
                return Err(NativeSqlStoreError::InvariantViolation {
                    message: format!(
                        "approved candidate {} references a missing, deleted, or cross-space target",
                        command.candidate_id
                    ),
                });
            };
            tx.commit().await.map_err(NativeSqlStoreError::from)?;
            return Ok(MemoryRecordQuotaAdmission::Admitted(existing_target));
        }
        if decision_state != "pending" {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "target-less candidate {} has unexpected decision state {}",
                    command.candidate_id, decision_state
                ),
            });
        }

        // A target-less candidate still needs the space admission check. The space
        // row remains locked while this count and the complete mutation execute.
        if !command.create_record {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "candidate promotion without a target must create a record".to_string(),
            });
        }

        let active_records = self
            .count_active_records_on_tx(&mut tx, command.scope)
            .await?;
        if max_active_records > 0 && active_records >= max_active_records {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Ok(MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            });
        }

        if command.create_record {
            self.create_record_on_tx(
                &mut tx,
                command.scope,
                command.memory_uuid,
                "user",
                command.memory_type,
                None,
                None,
                command.proposed_text,
                command.proposed_text,
                "internal",
                None,
            )
            .await?;
            for (source_id, event_id, confidence) in command.evidence_links {
                self.append_record_source_on_tx(
                    &mut tx,
                    NativeSqlAppendRecordSourceCommand {
                        scope: command.scope,
                        source_id,
                        memory_uuid: command.memory_uuid,
                        event_uuid: event_id,
                        source_role: "evidence",
                        confidence_delta: *confidence,
                    },
                )
                .await?;
            }
            Self::set_candidate_target_memory_on_tx(
                &mut tx,
                command.scope,
                command.candidate_id,
                command.memory_uuid,
            )
            .await?;
        }
        Self::approve_candidate_on_tx(
            &mut tx,
            command.scope,
            command.candidate_id,
            command.decided_by,
        )
        .await?;
        if command.create_record {
            sync_record_fts_on_tx(
                self.dialect(),
                &mut tx,
                FtsRecordProjection {
                    scope: command.scope,
                    memory_uuid: command.memory_uuid,
                    canonical_text: command.proposed_text,
                    object_text: command.proposed_text,
                    subject: None,
                    predicate: None,
                },
            )
            .await?;
            if let Some(journal) = journal {
                append_journal_on_tx(self, &mut tx, command.scope, journal).await?;
            }
        }
        tx.commit().await.map_err(NativeSqlStoreError::from)?;
        Ok(MemoryRecordQuotaAdmission::Admitted(
            command.memory_uuid.to_string(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn append_audit_with_metadata(
        &self,
        scope: &MemoryScopeContext,
        audit_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        result: &str,
        metadata_json: &str,
        actor_id: Option<&str>,
    ) -> Result<NativeSqlMemoryAuditRecord, NativeSqlStoreError> {
        let actor_type = if actor_id.is_some() { "user" } else { "system" };
        sqlx::query(
            r#"
            INSERT INTO ai_audit_log (
              id,
              uuid,
              tenant_id,
              actor_type,
              actor_id,
              action,
              resource_type,
              resource_id,
              result,
              metadata_json,
              created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(audit_id)
        .bind(scope.tenant_id)
        .bind(actor_type)
        .bind(actor_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(result)
        .bind(metadata_json)
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(NativeSqlMemoryAuditRecord {
            audit_id: audit_id.to_string(),
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            result: result.to_string(),
        })
    }

    pub async fn retrieve_governance_job_for_tenant(
        &self,
        tenant_id: i64,
        job_id: &str,
        resource_type: &str,
    ) -> Result<Option<NativeSqlGovernanceJobRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, actor_id, resource_type, result, metadata_json, created_at
            FROM ai_audit_log
            WHERE tenant_id = ?
              AND uuid = ?
              AND resource_type = ?
            "#,
        )
        .bind(tenant_id)
        .bind(job_id)
        .bind(resource_type)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlGovernanceJobRow {
            job_id: row.get("uuid"),
            actor_id: row.get("actor_id"),
            resource_type: row.get("resource_type"),
            result: row.get("result"),
            metadata_json: row.get("metadata_json"),
            created_at: row.get("created_at"),
        }))
    }

    pub async fn list_governance_jobs_for_tenant(
        &self,
        tenant_id: i64,
        resource_type: &str,
        actor_id: Option<&str>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlGovernanceJobRow>, NativeSqlStoreError> {
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, actor_id, resource_type, result, metadata_json, created_at
            FROM ai_audit_log AS current
            WHERE tenant_id = ?
              AND resource_type = ?
              AND (? IS NULL OR actor_id = ?)
              AND (
                ? = ''
                OR current.id < COALESCE((
                  SELECT cursor_row.id
                  FROM ai_audit_log AS cursor_row
                  WHERE cursor_row.tenant_id = current.tenant_id
                    AND cursor_row.uuid = ?
                ), 0)
              )
            ORDER BY current.id DESC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(actor_id)
        .bind(actor_id)
        .bind(cursor)
        .bind(cursor)
        .bind(clamp_list_page_size(page_size) + 1)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlGovernanceJobRow {
                job_id: row.get("uuid"),
                actor_id: row.get("actor_id"),
                resource_type: row.get("resource_type"),
                result: row.get("result"),
                metadata_json: row.get("metadata_json"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn save_admin_config_entity(
        &self,
        tenant_id: i64,
        resource_type: &str,
        entity_id: &str,
        metadata_json: &str,
    ) -> Result<(), NativeSqlStoreError> {
        let audit_id = format!("{resource_type}:{entity_id}:{}", now_text());
        sqlx::query(
            r#"
            INSERT INTO ai_audit_log (
              id,
              uuid,
              tenant_id,
              actor_type,
              action,
              resource_type,
              resource_id,
              result,
              metadata_json,
              created_at
            )
            VALUES (?, ?, ?, 'system', 'admin.config.save', ?, ?, 'active', ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(audit_id)
        .bind(tenant_id)
        .bind(resource_type)
        .bind(entity_id)
        .bind(metadata_json)
        .bind(now_text())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn retrieve_admin_config_entity(
        &self,
        tenant_id: i64,
        resource_type: &str,
        entity_id: &str,
    ) -> Result<Option<String>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT metadata_json
            FROM ai_audit_log
            WHERE tenant_id = ?
              AND resource_type = ?
              AND resource_id = ?
              AND action = 'admin.config.save'
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.get("metadata_json")))
    }

    pub async fn list_admin_config_entities(
        &self,
        tenant_id: i64,
        resource_type: &str,
        page_size: i32,
    ) -> Result<Vec<(String, String)>, NativeSqlStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT resource_id, metadata_json
            FROM ai_audit_log AS current
            WHERE tenant_id = ?
              AND resource_type = ?
              AND action = 'admin.config.save'
              AND created_at = (
                SELECT MAX(created_at)
                FROM ai_audit_log AS latest
                WHERE latest.tenant_id = current.tenant_id
                  AND latest.resource_type = current.resource_type
                  AND latest.resource_id = current.resource_id
                  AND latest.action = 'admin.config.save'
              )
            ORDER BY created_at DESC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(resource_type)
        .bind(clamp_list_page_size(page_size))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| (row.get("resource_id"), row.get("metadata_json")))
            .collect())
    }

    pub async fn retrieve_audit(
        &self,
        scope: &MemoryScopeContext,
        audit_id: &str,
    ) -> Result<Option<NativeSqlMemoryAuditRecord>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, action, resource_type, resource_id, result
            FROM ai_audit_log
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(audit_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlMemoryAuditRecord {
            audit_id: row.get("uuid"),
            action: row.get("action"),
            resource_type: row.get("resource_type"),
            resource_id: row.get("resource_id"),
            result: row.get("result"),
        }))
    }

    pub async fn append_outbox_event(
        &self,
        command: NativeSqlAppendOutboxEventCommand<'_>,
    ) -> Result<NativeSqlMemoryOutboxEvent, NativeSqlStoreError> {
        let _payload: Value = serde_json::from_str(command.payload_json)?;

        if let Some(existing) = self
            .retrieve_outbox_idempotency_state(command.scope, command.outbox_id)
            .await?
        {
            if existing.aggregate_type == command.aggregate_type
                && existing.aggregate_id == command.aggregate_id
                && existing.event_type == command.event_type
                && existing.event_version == command.event_version
                && existing.payload_json == command.payload_json
            {
                return Ok(existing.into_outbox_event(command.outbox_id));
            }

            return Err(NativeSqlStoreError::OutboxConflict {
                tenant_id: command.scope.tenant_id,
                outbox_id: command.outbox_id.to_string(),
            });
        }

        sqlx::query(
            r#"
            INSERT INTO ai_outbox_event (
              id,
              uuid,
              tenant_id,
              aggregate_type,
              aggregate_id,
              event_type,
              event_version,
              payload_json,
              publish_state,
              created_at,
              updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(command.outbox_id)
        .bind(command.scope.tenant_id)
        .bind(command.aggregate_type)
        .bind(command.aggregate_id)
        .bind(command.event_type)
        .bind(command.event_version)
        .bind(command.payload_json)
        .bind(now_text())
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(NativeSqlMemoryOutboxEvent {
            outbox_id: command.outbox_id.to_string(),
            aggregate_type: command.aggregate_type.to_string(),
            aggregate_id: command.aggregate_id.to_string(),
            event_type: command.event_type.to_string(),
            event_version: command.event_version.to_string(),
            payload_json: command.payload_json.to_string(),
            publish_state: "pending".to_string(),
            published_at: None,
            retry_count: 0,
        })
    }

    pub async fn retrieve_outbox_event(
        &self,
        scope: &MemoryScopeContext,
        outbox_id: &str,
    ) -> Result<Option<NativeSqlMemoryOutboxEvent>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              uuid,
              aggregate_type,
              aggregate_id,
              event_type,
              event_version,
              payload_json,
              publish_state,
              published_at,
              retry_count
            FROM ai_outbox_event
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(outbox_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlMemoryOutboxEvent {
            outbox_id: row.get("uuid"),
            aggregate_type: row.get("aggregate_type"),
            aggregate_id: row.get("aggregate_id"),
            event_type: row.get("event_type"),
            event_version: row.get("event_version"),
            payload_json: row.get("payload_json"),
            publish_state: row.get("publish_state"),
            published_at: row.get("published_at"),
            retry_count: row.get("retry_count"),
        }))
    }

    pub async fn list_pending_outbox_events(
        &self,
        scope: &MemoryScopeContext,
        limit: u32,
    ) -> Result<Vec<NativeSqlMemoryOutboxEvent>, NativeSqlStoreError> {
        let row_limit = i64::from(limit.max(1));
        let rows = sqlx::query(
            r#"
            SELECT
              uuid,
              aggregate_type,
              aggregate_id,
              event_type,
              event_version,
              payload_json,
              publish_state,
              published_at,
              retry_count
            FROM ai_outbox_event
            WHERE tenant_id = ? AND publish_state = 'pending'
            ORDER BY created_at ASC, id ASC
            LIMIT ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(row_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlMemoryOutboxEvent {
                outbox_id: row.get("uuid"),
                aggregate_type: row.get("aggregate_type"),
                aggregate_id: row.get("aggregate_id"),
                event_type: row.get("event_type"),
                event_version: row.get("event_version"),
                payload_json: row.get("payload_json"),
                publish_state: row.get("publish_state"),
                published_at: row.get("published_at"),
                retry_count: row.get("retry_count"),
            })
            .collect())
    }

    pub async fn list_global_pending_outbox_events(
        &self,
        limit: u32,
    ) -> Result<Vec<NativeSqlScopedOutboxEvent>, NativeSqlStoreError> {
        let row_limit = i64::from(limit.max(1));
        let rows = sqlx::query(
            r#"
            SELECT
              tenant_id,
              uuid,
              aggregate_type,
              aggregate_id,
              event_type,
              event_version,
              payload_json,
              publish_state,
              published_at,
              retry_count
            FROM ai_outbox_event
            WHERE publish_state = 'pending'
            ORDER BY created_at ASC, id ASC
            LIMIT ?
            "#,
        )
        .bind(row_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlScopedOutboxEvent {
                tenant_id: row.get("tenant_id"),
                lease_owner: None,
                lease_token: None,
                lease_expires_at: None,
                outbox: NativeSqlMemoryOutboxEvent {
                    outbox_id: row.get("uuid"),
                    aggregate_type: row.get("aggregate_type"),
                    aggregate_id: row.get("aggregate_id"),
                    event_type: row.get("event_type"),
                    event_version: row.get("event_version"),
                    payload_json: row.get("payload_json"),
                    publish_state: row.get("publish_state"),
                    published_at: row.get("published_at"),
                    retry_count: row.get("retry_count"),
                },
            })
            .collect())
    }

    /// Atomically claims pending outbox rows for publish. Postgres uses `FOR UPDATE SKIP LOCKED`
    /// so multiple API server replicas do not double-publish the same event.
    pub async fn claim_global_pending_outbox_events(
        &self,
        limit: u32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlScopedOutboxEvent>, NativeSqlStoreError> {
        match self.dialect {
            MemorySqlDialect::Postgres => {
                self.claim_global_pending_outbox_events_postgres(
                    limit,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                )
                .await
            }
            MemorySqlDialect::Sqlite => {
                self.claim_global_pending_outbox_events_sqlite(
                    limit,
                    lease_owner,
                    lease_token,
                    lease_duration_seconds,
                )
                .await
            }
        }
    }

    async fn claim_global_pending_outbox_events_postgres(
        &self,
        limit: u32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlScopedOutboxEvent>, NativeSqlStoreError> {
        let row_limit = i64::from(limit.max(1));
        let timestamp = now_text();
        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let rows = sqlx::query(
            r#"
            UPDATE ai_outbox_event AS o
            SET publish_state = 'processing',
                lease_owner = ?,
                lease_token = ?,
                lease_expires_at = ?,
                next_attempt_at = NULL,
                updated_at = ?
            FROM (
              SELECT id
              FROM ai_outbox_event
              WHERE publish_state = 'pending'
                AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
              ORDER BY created_at ASC, id ASC
              LIMIT ?
              FOR UPDATE SKIP LOCKED
            ) AS picked
            WHERE o.id = picked.id
            RETURNING
              o.tenant_id,
              o.uuid,
              o.aggregate_type,
              o.aggregate_id,
              o.event_type,
              o.event_version,
              o.payload_json,
              o.publish_state,
              o.published_at,
              o.retry_count
            "#,
        )
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&lease_expires_at)
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(row_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlScopedOutboxEvent {
                tenant_id: row.get("tenant_id"),
                lease_owner: Some(lease_owner.to_string()),
                lease_token: Some(lease_token.to_string()),
                lease_expires_at: Some(lease_expires_at.clone()),
                outbox: NativeSqlMemoryOutboxEvent {
                    outbox_id: row.get("uuid"),
                    aggregate_type: row.get("aggregate_type"),
                    aggregate_id: row.get("aggregate_id"),
                    event_type: row.get("event_type"),
                    event_version: row.get("event_version"),
                    payload_json: row.get("payload_json"),
                    publish_state: row.get("publish_state"),
                    published_at: row.get("published_at"),
                    retry_count: row.get("retry_count"),
                },
            })
            .collect())
    }

    async fn claim_global_pending_outbox_events_sqlite(
        &self,
        limit: u32,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<Vec<NativeSqlScopedOutboxEvent>, NativeSqlStoreError> {
        let row_limit = i64::from(limit.max(1));
        let mut tx = self.pool.begin().await?;
        let timestamp = now_text();
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              tenant_id,
              uuid,
              aggregate_type,
              aggregate_id,
              event_type,
              event_version,
              payload_json,
              publish_state,
              published_at,
              retry_count
            FROM ai_outbox_event
            WHERE publish_state = 'pending'
              AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
            ORDER BY created_at ASC, id ASC
            LIMIT ?
            "#,
        )
        .bind(&timestamp)
        .bind(row_limit)
        .fetch_all(&mut *tx)
        .await?;

        if rows.is_empty() {
            tx.commit().await?;
            return Ok(Vec::new());
        }

        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let mut claimed = Vec::with_capacity(rows.len());
        for row in rows {
            let row_id: i64 = row.get("id");
            let updated = sqlx::query(
                r#"
                UPDATE ai_outbox_event
                SET publish_state = 'processing',
                    lease_owner = ?,
                    lease_token = ?,
                    lease_expires_at = ?,
                    next_attempt_at = NULL,
                    updated_at = ?
                WHERE id = ? AND publish_state = 'pending'
                "#,
            )
            .bind(lease_owner)
            .bind(lease_token)
            .bind(&lease_expires_at)
            .bind(&timestamp)
            .bind(row_id)
            .execute(&mut *tx)
            .await?;
            if updated.rows_affected() > 0 {
                claimed.push(NativeSqlScopedOutboxEvent {
                    tenant_id: row.get("tenant_id"),
                    lease_owner: Some(lease_owner.to_string()),
                    lease_token: Some(lease_token.to_string()),
                    lease_expires_at: Some(lease_expires_at.clone()),
                    outbox: NativeSqlMemoryOutboxEvent {
                        outbox_id: row.get("uuid"),
                        aggregate_type: row.get("aggregate_type"),
                        aggregate_id: row.get("aggregate_id"),
                        event_type: row.get("event_type"),
                        event_version: row.get("event_version"),
                        payload_json: row.get("payload_json"),
                        publish_state: "processing".to_owned(),
                        published_at: None,
                        retry_count: row.get("retry_count"),
                    },
                });
            }
        }
        tx.commit().await?;
        Ok(claimed)
    }

    pub async fn retrieve_tenant_preference_json(
        &self,
        tenant_id: i64,
        user_id: Option<i64>,
        preference_key: &str,
    ) -> Result<Option<String>, NativeSqlStoreError> {
        let bound_user_id = preference_scope_user_binding(user_id, self.dialect);
        let row = sqlx::query(
            r#"
            SELECT preference_json
            FROM ai_tenant_preference
            WHERE tenant_id = ? AND user_id IS NOT DISTINCT FROM ? AND preference_key = ?
            "#,
        )
        .bind(tenant_id)
        .bind(bound_user_id)
        .bind(preference_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.get("preference_json")))
    }

    pub async fn upsert_tenant_preference_json(
        &self,
        preference_id: i64,
        tenant_id: i64,
        user_id: Option<i64>,
        preference_key: &str,
        preference_json: &str,
    ) -> Result<(), NativeSqlStoreError> {
        let bound_user_id = preference_scope_user_binding(user_id, self.dialect);
        let timestamp = now_text();
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
        .bind(preference_id)
        .bind(tenant_id)
        .bind(bound_user_id)
        .bind(preference_key)
        .bind(preference_json)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn purge_expired_records_for_scope(
        &self,
        scope: &MemoryScopeContext,
        dry_run: bool,
    ) -> Result<u32, NativeSqlStoreError> {
        if dry_run {
            let count: i64 = sqlx::query_scalar(
                r#"
                SELECT COUNT(*)
                FROM ai_record
                WHERE tenant_id = ? AND space_id = ? AND status <> 'deleted'
                  AND expires_at IS NOT NULL AND expires_at < ?
                "#,
            )
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(now_text())
            .fetch_one(&self.pool)
            .await?;
            return Ok(u32::try_from(count.max(0)).unwrap_or(0));
        }

        let timestamp = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_record
            SET status = 'deleted',
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND status <> 'deleted'
              AND expires_at IS NOT NULL AND expires_at < ?
            "#,
        )
        .bind(&timestamp)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        Ok(u32::try_from(result.rows_affected()).unwrap_or(0))
    }

    pub async fn mark_outbox_published(
        &self,
        scope: &MemoryScopeContext,
        outbox_id: &str,
    ) -> Result<Option<NativeSqlMemoryOutboxEvent>, NativeSqlStoreError> {
        sqlx::query(
            r#"
            UPDATE ai_outbox_event
            SET publish_state = 'published',
                published_at = ?,
                updated_at = ?
            WHERE tenant_id = ? AND uuid = ? AND publish_state = 'pending'
            "#,
        )
        .bind(now_text())
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(outbox_id)
        .execute(&self.pool)
        .await?;

        self.retrieve_outbox_event(scope, outbox_id).await
    }

    pub async fn mark_outbox_failed(
        &self,
        scope: &MemoryScopeContext,
        outbox_id: &str,
    ) -> Result<Option<NativeSqlMemoryOutboxEvent>, NativeSqlStoreError> {
        sqlx::query(
            r#"
            UPDATE ai_outbox_event
            SET publish_state = 'failed',
                retry_count = retry_count + 1,
                updated_at = ?
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(outbox_id)
        .execute(&self.pool)
        .await?;

        self.retrieve_outbox_event(scope, outbox_id).await
    }

    pub async fn ack_outbox_delivery_success(
        &self,
        tenant_id: i64,
        outbox_id: &str,
        lease_owner: &str,
        lease_token: &str,
    ) -> Result<Option<NativeSqlMemoryOutboxEvent>, NativeSqlStoreError> {
        let timestamp = now_text();
        let updated = sqlx::query(
            r#"
            UPDATE ai_outbox_event
            SET publish_state = 'published',
                published_at = ?,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                next_attempt_at = NULL,
                updated_at = ?
            WHERE tenant_id = ? AND uuid = ? AND publish_state = 'processing'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#,
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .bind(tenant_id)
        .bind(outbox_id)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;

        if updated.rows_affected() == 0 {
            return Ok(None);
        }

        self.retrieve_outbox_event(
            &MemoryScopeContext {
                tenant_id,
                space_id: 0,
                organization_id: None,
                user_id: None,
            },
            outbox_id,
        )
        .await
    }

    pub async fn record_outbox_delivery_failure(
        &self,
        tenant_id: i64,
        outbox_id: &str,
        lease_owner: &str,
        lease_token: &str,
        max_retries: u32,
    ) -> Result<Option<NativeSqlMemoryOutboxEvent>, NativeSqlStoreError> {
        let timestamp = now_text();
        let mut transaction = self.pool.begin().await?;
        let retry_query = if self.dialect == MemorySqlDialect::Postgres {
            r#"
            SELECT retry_count FROM ai_outbox_event
            WHERE tenant_id = ? AND uuid = ? AND publish_state = 'processing'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            FOR UPDATE
            "#
        } else {
            r#"
            SELECT retry_count FROM ai_outbox_event
            WHERE tenant_id = ? AND uuid = ? AND publish_state = 'processing'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#
        };
        let current_retry = sqlx::query_scalar::<_, i64>(retry_query)
            .bind(tenant_id)
            .bind(outbox_id)
            .bind(lease_owner)
            .bind(lease_token)
            .bind(&timestamp)
            .fetch_optional(&mut *transaction)
            .await?;
        let Some(current_retry) = current_retry else {
            transaction.rollback().await?;
            return Ok(None);
        };
        let next_retry = current_retry.saturating_add(1);
        let failed = next_retry >= i64::from(max_retries.max(1));
        let backoff_seconds = 1_u64 << u32::try_from(next_retry.clamp(1, 5)).unwrap_or(5);
        let next_attempt_at = (!failed).then(|| timestamp_after_seconds(backoff_seconds));
        let updated = sqlx::query(
            r#"
            UPDATE ai_outbox_event
            SET retry_count = ?,
                publish_state = ?,
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                next_attempt_at = ?,
                updated_at = ?
            WHERE tenant_id = ? AND uuid = ? AND publish_state = 'processing'
              AND lease_owner = ? AND lease_token = ?
            "#,
        )
        .bind(next_retry)
        .bind(if failed { "failed" } else { "pending" })
        .bind(next_attempt_at.as_deref())
        .bind(&timestamp)
        .bind(tenant_id)
        .bind(outbox_id)
        .bind(lease_owner)
        .bind(lease_token)
        .execute(&mut *transaction)
        .await?;
        if updated.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        transaction.commit().await?;

        self.retrieve_outbox_event(
            &MemoryScopeContext {
                tenant_id,
                space_id: 0,
                organization_id: None,
                user_id: None,
            },
            outbox_id,
        )
        .await
    }

    pub async fn requeue_stale_processing_outbox_events(
        &self,
        _stale_after_seconds: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        let timestamp = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_outbox_event
            SET publish_state = 'pending',
                lease_owner = NULL,
                lease_token = NULL,
                lease_expires_at = NULL,
                updated_at = ?
            WHERE publish_state = 'processing'
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
            "#,
        )
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn renew_outbox_delivery_lease(
        &self,
        tenant_id: i64,
        outbox_id: &str,
        lease_owner: &str,
        lease_token: &str,
        lease_duration_seconds: u64,
    ) -> Result<bool, NativeSqlStoreError> {
        let timestamp = now_text();
        let lease_expires_at = timestamp_after_seconds(lease_duration_seconds.max(5));
        let updated = sqlx::query(
            r#"
            UPDATE ai_outbox_event
            SET lease_expires_at = ?, updated_at = ?
            WHERE tenant_id = ? AND uuid = ? AND publish_state = 'processing'
              AND lease_owner = ? AND lease_token = ? AND lease_expires_at > ?
            "#,
        )
        .bind(lease_expires_at)
        .bind(&timestamp)
        .bind(tenant_id)
        .bind(outbox_id)
        .bind(lease_owner)
        .bind(lease_token)
        .bind(&timestamp)
        .execute(&self.pool)
        .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn create_candidate(
        &self,
        command: &CreateMemoryCandidateCommand,
    ) -> Result<MemoryCandidate, NativeSqlStoreError> {
        self.ensure_space(&command.scope).await?;
        sqlx::query(
            r#"
            INSERT INTO ai_candidate (
              id,
              uuid,
              tenant_id,
              space_id,
              user_id,
              candidate_type,
              memory_type,
              proposed_text,
              proposed_payload_json,
              evidence_json,
              confidence,
              decision_state,
              created_at,
              updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(&command.candidate_id)
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(command.scope.user_id)
        .bind(&command.candidate_type)
        .bind(&command.memory_type)
        .bind(&command.proposed_text)
        .bind(&command.proposed_payload_json)
        .bind(&command.evidence_json)
        .bind(command.confidence)
        .bind(now_text())
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(MemoryCandidate {
            candidate_id: command.candidate_id.clone(),
            candidate_type: command.candidate_type.clone(),
            memory_type: command.memory_type.clone(),
            proposed_text: command.proposed_text.clone(),
            proposed_payload_json: command.proposed_payload_json.clone(),
            evidence_json: command.evidence_json.clone(),
            confidence: command.confidence,
            decision_state: "pending".to_string(),
            decision_reason: None,
            decided_by: None,
            decided_at: None,
        })
    }

    pub async fn retrieve_candidate(
        &self,
        scope: &MemoryScopeContext,
        candidate_id: &str,
    ) -> Result<Option<MemoryCandidate>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              uuid,
              candidate_type,
              memory_type,
              proposed_text,
              proposed_payload_json,
              evidence_json,
              confidence,
              decision_state,
              decision_reason,
              decided_by,
              decided_at
            FROM ai_candidate
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(candidate_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(candidate_from_row))
    }

    pub async fn approve_candidate(
        &self,
        command: &ApproveMemoryCandidateCommand,
    ) -> Result<Option<MemoryCandidate>, NativeSqlStoreError> {
        self.decide_candidate(
            &command.scope,
            &command.candidate_id,
            "approved",
            command.decision_reason.as_deref(),
            command.decided_by,
        )
        .await
    }

    pub async fn reject_candidate(
        &self,
        command: &RejectMemoryCandidateCommand,
    ) -> Result<Option<MemoryCandidate>, NativeSqlStoreError> {
        self.decide_candidate(
            &command.scope,
            &command.candidate_id,
            "rejected",
            command.decision_reason.as_deref(),
            command.decided_by,
        )
        .await
    }

    pub async fn upsert_habit(
        &self,
        command: &UpsertMemoryHabitCommand,
    ) -> Result<MemoryHabit, NativeSqlStoreError> {
        self.ensure_space(&command.scope).await?;
        sqlx::query(
            r#"
            INSERT INTO ai_habit (
              id,
              uuid,
              tenant_id,
              space_id,
              user_id,
              habit_key,
              habit_type,
              description,
              stage,
              strength,
              confidence,
              support_count,
              last_signal_at,
              metadata_json,
              created_at,
              updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (tenant_id, space_id, user_id, habit_key)
            DO UPDATE SET
              uuid = excluded.uuid,
              habit_type = excluded.habit_type,
              description = excluded.description,
              stage = excluded.stage,
              strength = excluded.strength,
              confidence = excluded.confidence,
              support_count = excluded.support_count,
              last_signal_at = excluded.last_signal_at,
              metadata_json = excluded.metadata_json,
              updated_at = excluded.updated_at,
              version = ai_habit.version + 1
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(&command.habit_id)
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(command.user_id)
        .bind(&command.habit_key)
        .bind(&command.habit_type)
        .bind(&command.description)
        .bind(&command.stage)
        .bind(command.strength)
        .bind(command.confidence)
        .bind(command.support_count)
        .bind(now_text())
        .bind(&command.metadata_json)
        .bind(now_text())
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        self.retrieve_habit(&command.scope, command.user_id, &command.habit_key)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: "habit upsert did not return a stored row".to_string(),
            })
    }

    pub async fn retrieve_habit(
        &self,
        scope: &MemoryScopeContext,
        user_id: i64,
        habit_key: &str,
    ) -> Result<Option<MemoryHabit>, NativeSqlStoreError> {
        let row = self.fetch_habit(scope, user_id, habit_key).await?;
        Ok(row.map(habit_from_row))
    }

    pub async fn promote_habit(
        &self,
        command: &PromoteMemoryHabitCommand,
    ) -> Result<Option<MemoryHabit>, NativeSqlStoreError> {
        let promoted_memory_row_id = match command.promoted_memory_id.as_deref() {
            Some(memory_id) => self.lookup_record_row_id(&command.scope, memory_id).await?,
            None => None,
        };

        sqlx::query(
            r#"
            UPDATE ai_habit
            SET stage = 'promoted',
                promoted_memory_id = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND user_id = ? AND habit_key = ?
            "#,
        )
        .bind(promoted_memory_row_id)
        .bind(now_text())
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(command.user_id)
        .bind(&command.habit_key)
        .execute(&self.pool)
        .await?;

        self.retrieve_habit(&command.scope, command.user_id, &command.habit_key)
            .await
    }

    pub async fn decay_habit(
        &self,
        command: &DecayMemoryHabitCommand,
    ) -> Result<Option<MemoryHabit>, NativeSqlStoreError> {
        sqlx::query(
            r#"
            UPDATE ai_habit
            SET stage = 'decayed',
                strength = CASE
                  WHEN strength - ? < 0 THEN 0
                  ELSE strength - ?
                END,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND user_id = ? AND habit_key = ?
            "#,
        )
        .bind(command.strength_delta)
        .bind(command.strength_delta)
        .bind(now_text())
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(command.user_id)
        .bind(&command.habit_key)
        .execute(&self.pool)
        .await?;

        self.retrieve_habit(&command.scope, command.user_id, &command.habit_key)
            .await
    }

    pub async fn append_retrieval_trace(
        &self,
        command: &AppendMemoryRetrievalTraceCommand,
    ) -> Result<MemoryRetrievalTrace, NativeSqlStoreError> {
        self.ensure_space(&command.scope).await?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            r#"
            INSERT INTO ai_retrieval_trace (
              id,
              uuid,
              tenant_id,
              space_id,
              actor_id,
              query_text,
              query_hash,
              retrievers_json,
              latency_ms,
              result_count,
              degraded,
              metadata_json,
              created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(&command.trace_id)
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(&command.actor_id)
        .bind(&command.query_text)
        .bind(&command.query_hash)
        .bind(&command.retrievers_json)
        .bind(command.latency_ms)
        .bind(command.hits.len() as i64)
        .bind(command.degraded)
        .bind(&command.metadata_json)
        .bind(now_text())
        .execute(&mut *transaction)
        .await?;

        let trace_row_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM ai_retrieval_trace WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
        )
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(&command.trace_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
            message: "retrieval trace append did not return a stored row".to_string(),
        })?;

        for hit in &command.hits {
            let memory_row_id = match hit.memory_id.as_deref() {
                Some(memory_id) => {
                    sqlx::query_scalar::<_, i64>(
                        r#"
                        SELECT id
                        FROM ai_record
                        WHERE tenant_id = ?
                          AND space_id = ?
                          AND uuid = ?
                          AND status <> 'deleted'
                        "#,
                    )
                    .bind(command.scope.tenant_id)
                    .bind(hit.space_id.unwrap_or(command.scope.space_id))
                    .bind(memory_id)
                    .fetch_optional(&mut *transaction)
                    .await?
                }
                None => None,
            };
            sqlx::query(
                r#"
                INSERT INTO ai_retrieval_hit (
                  id,
                  uuid,
                  tenant_id,
                  retrieval_trace_id,
                  memory_id,
                  retriever_name,
                  result_rank,
                  raw_score,
                  fused_score,
                  explanation_json,
                  status,
                  created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(self.next_row_id()?)
            .bind(&hit.hit_id)
            .bind(command.scope.tenant_id)
            .bind(trace_row_id)
            .bind(memory_row_id)
            .bind(&hit.retriever_name)
            .bind(hit.result_rank)
            .bind(hit.raw_score)
            .bind(hit.fused_score)
            .bind(&hit.explanation_json)
            .bind(&hit.status)
            .bind(now_text())
            .execute(&mut *transaction)
            .await?;
        }

        if let Some(context_pack) = &command.context_pack {
            sqlx::query(
                r#"
                INSERT INTO ai_context_pack (
                  id,
                  uuid,
                  tenant_id,
                  retrieval_trace_id,
                  actor_id,
                  query_text,
                  pack_json,
                  estimated_tokens,
                  truncated,
                  created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(self.next_row_id()?)
            .bind(&context_pack.context_pack_id)
            .bind(command.scope.tenant_id)
            .bind(trace_row_id)
            .bind(&command.actor_id)
            .bind(&command.query_text)
            .bind(&context_pack.pack_json)
            .bind(context_pack.estimated_tokens)
            .bind(context_pack.truncated)
            .bind(now_text())
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;

        self.retrieve_retrieval_trace(&command.scope, &command.trace_id)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: "retrieval trace append could not retrieve stored row".to_string(),
            })
    }

    pub async fn retrieve_retrieval_trace_lookup_for_tenant(
        &self,
        tenant_id: i64,
        trace_id: &str,
    ) -> Result<Option<TenantRetrievalTraceLookup>, NativeSqlStoreError> {
        let row = sqlx::query(
            "SELECT space_id, created_at FROM ai_retrieval_trace WHERE tenant_id = ? AND uuid = ?",
        )
        .bind(tenant_id)
        .bind(trace_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let space_id: i64 = row.get("space_id");
        let created_at: String = row.get("created_at");
        let scope = MemoryScopeContext {
            tenant_id,
            space_id,
            organization_id: None,
            user_id: None,
        };
        let trace = self.retrieve_retrieval_trace(&scope, trace_id).await?;
        Ok(trace.map(|trace| TenantRetrievalTraceLookup {
            space_id,
            trace,
            created_at,
        }))
    }

    pub async fn retrieve_retrieval_trace_for_tenant(
        &self,
        tenant_id: i64,
        trace_id: &str,
    ) -> Result<Option<MemoryRetrievalTrace>, NativeSqlStoreError> {
        let row =
            sqlx::query("SELECT space_id FROM ai_retrieval_trace WHERE tenant_id = ? AND uuid = ?")
                .bind(tenant_id)
                .bind(trace_id)
                .fetch_optional(&self.pool)
                .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let scope = MemoryScopeContext {
            tenant_id,
            space_id: row.get("space_id"),
            organization_id: None,
            user_id: None,
        };
        self.retrieve_retrieval_trace(&scope, trace_id).await
    }

    pub async fn retrieve_retrieval_trace(
        &self,
        scope: &MemoryScopeContext,
        trace_id: &str,
    ) -> Result<Option<MemoryRetrievalTrace>, NativeSqlStoreError> {
        let row = sqlx::query(retrieval_trace_select_sql())
            .bind(scope.tenant_id)
            .bind(scope.space_id)
            .bind(trace_id)
            .fetch_optional(&self.pool)
            .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        self.retrieval_trace_from_row(scope, row).await.map(Some)
    }

    pub async fn list_recent_retrieval_traces(
        &self,
        scope: &MemoryScopeContext,
        limit: u32,
    ) -> Result<Vec<MemoryRetrievalTrace>, NativeSqlStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT
              id,
              uuid,
              actor_id,
              query_text,
              query_hash,
              retrievers_json,
              latency_ms,
              result_count,
              degraded,
              metadata_json
            FROM ai_retrieval_trace
            WHERE tenant_id = ? AND space_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(i64::from(limit.max(1)))
        .fetch_all(&self.pool)
        .await?;

        let mut traces = Vec::with_capacity(rows.len());
        for row in rows {
            traces.push(self.retrieval_trace_from_row(scope, row).await?);
        }

        Ok(traces)
    }

    async fn apply_sqlite_phase1_migration(&self) -> Result<(), NativeSqlStoreError> {
        const MIGRATIONS: &[(&str, &str)] = &[
            (
                "0001",
                include_str!("../../../tests/fixtures/database/sqlite/migrations/0001_memory_schema.up.sql"),
            ),
            (
                "0002",
                include_str!("../../../tests/fixtures/database/sqlite/migrations/0002_memory_indexes.up.sql"),
            ),
            (
                "0003",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0003_memory_tenant_preference.up.sql"
                ),
            ),
            (
                "0004",
                include_str!("../../../tests/fixtures/database/sqlite/migrations/0004_memory_learning_job.up.sql"),
            ),
            (
                "0005",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0005_memory_record_fulltext_search.up.sql"
                ),
            ),
            (
                "0006",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0006_memory_eval_run_extend.up.sql"
                ),
            ),
            (
                "0007",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0007_memory_commercial_management.up.sql"
                ),
            ),
            (
                "0008",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0008_memory_fts_predicate.up.sql"
                ),
            ),
            (
                "0009",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0009_memory_outbox_delivery_lease.up.sql"
                ),
            ),
            (
                "0010",
                include_str!(
                    "../../../tests/fixtures/database/sqlite/migrations/0010_memory_job_execution_lease.up.sql"
                ),
            ),
        ];
        self.apply_embedded_sql_migrations(MIGRATIONS).await
    }

    async fn retrieve_event_idempotency_state(
        &self,
        scope: &MemoryScopeContext,
        event_id: &str,
    ) -> Result<Option<NativeSqlEventIdempotencyState>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT space_id, payload_json, payload_hash
            FROM ai_event
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlEventIdempotencyState {
            space_id: row.get("space_id"),
            payload_json: row.get("payload_json"),
            payload_hash: row.get("payload_hash"),
        }))
    }

    async fn retrieve_outbox_idempotency_state(
        &self,
        scope: &MemoryScopeContext,
        outbox_id: &str,
    ) -> Result<Option<NativeSqlOutboxIdempotencyState>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              aggregate_type,
              aggregate_id,
              event_type,
              event_version,
              payload_json,
              publish_state,
              published_at,
              retry_count
            FROM ai_outbox_event
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(outbox_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlOutboxIdempotencyState {
            aggregate_type: row.get("aggregate_type"),
            aggregate_id: row.get("aggregate_id"),
            event_type: row.get("event_type"),
            event_version: row.get("event_version"),
            payload_json: row.get("payload_json"),
            publish_state: row.get("publish_state"),
            published_at: row.get("published_at"),
            retry_count: row.get("retry_count"),
        }))
    }

    async fn decide_candidate(
        &self,
        scope: &MemoryScopeContext,
        candidate_id: &str,
        decision_state: &str,
        decision_reason: Option<&str>,
        decided_by: Option<i64>,
    ) -> Result<Option<MemoryCandidate>, NativeSqlStoreError> {
        sqlx::query(
            r#"
            UPDATE ai_candidate
            SET decision_state = ?,
                decision_reason = ?,
                decided_by = ?,
                decided_at = ?,
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(decision_state)
        .bind(decision_reason)
        .bind(decided_by)
        .bind(now_text())
        .bind(now_text())
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(candidate_id)
        .execute(&self.pool)
        .await?;

        self.retrieve_candidate(scope, candidate_id).await
    }

    async fn fetch_habit(
        &self,
        scope: &MemoryScopeContext,
        user_id: i64,
        habit_key: &str,
    ) -> Result<Option<AnyRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              habit.uuid,
              habit.user_id,
              habit.habit_key,
              habit.habit_type,
              habit.description,
              habit.stage,
              habit.strength,
              habit.confidence,
              habit.support_count,
              habit.last_signal_at,
              promoted.uuid AS promoted_memory_uuid,
              habit.decay_after,
              habit.metadata_json
            FROM ai_habit habit
            LEFT JOIN ai_record promoted
              ON promoted.id = habit.promoted_memory_id
             AND promoted.tenant_id = habit.tenant_id
             AND promoted.space_id = habit.space_id
            WHERE habit.tenant_id = ?
              AND habit.space_id = ?
              AND habit.user_id = ?
              AND habit.habit_key = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(user_id)
        .bind(habit_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row)
    }

    pub(crate) async fn lookup_record_row_id(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<Option<i64>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM ai_record
            WHERE tenant_id = ?
              AND space_id = ?
              AND uuid = ?
              AND status <> 'deleted'
            "#,
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.get("id")))
    }

    async fn retrieval_trace_from_row(
        &self,
        scope: &MemoryScopeContext,
        row: AnyRow,
    ) -> Result<MemoryRetrievalTrace, NativeSqlStoreError> {
        let trace_row_id: i64 = row.get("id");
        let hits = self.fetch_retrieval_hits(scope, trace_row_id).await?;
        let context_pack = self.fetch_context_pack(scope, trace_row_id).await?;

        Ok(MemoryRetrievalTrace {
            trace_id: row.get("uuid"),
            actor_id: row.get("actor_id"),
            query_text: row.get("query_text"),
            query_hash: row.get("query_hash"),
            retrievers_json: row.get("retrievers_json"),
            latency_ms: row.get("latency_ms"),
            result_count: row.get("result_count"),
            degraded: self.decode_bool(&row, "degraded"),
            metadata_json: row.get("metadata_json"),
            hits,
            context_pack,
        })
    }

    async fn fetch_retrieval_hits(
        &self,
        scope: &MemoryScopeContext,
        trace_row_id: i64,
    ) -> Result<Vec<MemoryRetrievalHitDraft>, NativeSqlStoreError> {
        let rows = sqlx::query(
            r#"
            SELECT
              hit.uuid,
              record.uuid AS memory_uuid,
              record.space_id AS memory_space_id,
              hit.retriever_name,
              hit.result_rank,
              hit.raw_score,
              hit.fused_score,
              hit.explanation_json,
              hit.status
            FROM ai_retrieval_hit hit
            LEFT JOIN ai_record record
              ON record.id = hit.memory_id
             AND record.tenant_id = hit.tenant_id
            WHERE hit.tenant_id = ?
              AND hit.retrieval_trace_id = ?
            ORDER BY hit.result_rank ASC, hit.id ASC
            LIMIT 1000
            "#,
        )
        .bind(scope.tenant_id)
        .bind(trace_row_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| MemoryRetrievalHitDraft {
                hit_id: row.get("uuid"),
                memory_id: row.get("memory_uuid"),
                space_id: row.get("memory_space_id"),
                retriever_name: row.get("retriever_name"),
                result_rank: row.get("result_rank"),
                raw_score: row.get("raw_score"),
                fused_score: row.get("fused_score"),
                explanation_json: row.get("explanation_json"),
                status: row.get("status"),
            })
            .collect())
    }

    async fn fetch_context_pack(
        &self,
        scope: &MemoryScopeContext,
        trace_row_id: i64,
    ) -> Result<Option<MemoryContextPackSnapshot>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, pack_json, estimated_tokens, truncated
            FROM ai_context_pack
            WHERE tenant_id = ? AND retrieval_trace_id = ?
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(scope.tenant_id)
        .bind(trace_row_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| MemoryContextPackSnapshot {
            context_pack_id: row.get("uuid"),
            pack_json: row.get("pack_json"),
            estimated_tokens: row.get("estimated_tokens"),
            truncated: self.decode_bool(&row, "truncated"),
        }))
    }

    pub async fn list_retrieval_traces_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlRetrievalTraceSummaryRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let cursor = cursor.unwrap_or("");
        let rows = if let Some(space_id) = space_id {
            sqlx::query(
                r#"
                SELECT uuid, space_id, query_text, query_hash, result_count, degraded, created_at
                FROM ai_retrieval_trace
                WHERE tenant_id = ?
                  AND space_id = ?
                  AND id < COALESCE(
                    (SELECT id FROM ai_retrieval_trace t2 WHERE t2.tenant_id = ? AND t2.uuid = ? LIMIT 1),
                    9223372036854775807
                  )
                ORDER BY id DESC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(space_id)
            .bind(tenant_id)
            .bind(cursor)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT uuid, space_id, query_text, query_hash, result_count, degraded, created_at
                FROM ai_retrieval_trace
                WHERE tenant_id = ?
                  AND id < COALESCE(
                    (SELECT id FROM ai_retrieval_trace t2 WHERE t2.tenant_id = ? AND t2.uuid = ? LIMIT 1),
                    9223372036854775807
                  )
                ORDER BY id DESC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(tenant_id)
            .bind(cursor)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlRetrievalTraceSummaryRow {
                trace_id: row.get("uuid"),
                space_id: row.get("space_id"),
                query_text: row.get("query_text"),
                query_hash: row.get("query_hash"),
                result_count: row.get("result_count"),
                degraded: self.decode_bool(&row, "degraded"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_context_pack_open_api(
        &self,
        tenant_id: i64,
        space_id: i64,
        context_pack_id: &str,
        retrieval_trace_id: Option<&str>,
        actor_id: Option<&str>,
        query_text: Option<&str>,
        pack_json: &str,
        estimated_tokens: i64,
        truncated: bool,
    ) -> Result<(), NativeSqlStoreError> {
        let trace_row_id = if let Some(trace_id) = retrieval_trace_id {
            self.lookup_retrieval_trace_row_id_for_tenant(tenant_id, trace_id)
                .await?
        } else {
            None
        };

        sqlx::query(
            r#"
            INSERT INTO ai_context_pack (
              id,
              uuid,
              tenant_id,
              retrieval_trace_id,
              actor_id,
              query_text,
              pack_json,
              estimated_tokens,
              truncated,
              created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(context_pack_id)
        .bind(tenant_id)
        .bind(trace_row_id)
        .bind(actor_id)
        .bind(query_text)
        .bind(pack_json)
        .bind(estimated_tokens)
        .bind(truncated)
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        let _ = space_id;
        Ok(())
    }

    pub async fn retrieve_context_pack_for_tenant(
        &self,
        tenant_id: i64,
        context_pack_id: &str,
    ) -> Result<Option<NativeSqlContextPackRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT cp.uuid, cp.query_text, cp.pack_json, cp.estimated_tokens, cp.truncated,
                   cp.created_at, cp.retrieval_trace_id, rt.space_id
            FROM ai_context_pack cp
            LEFT JOIN ai_retrieval_trace rt
              ON rt.tenant_id = cp.tenant_id AND rt.id = cp.retrieval_trace_id
            WHERE cp.tenant_id = ? AND cp.uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(context_pack_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlContextPackRow {
            context_pack_id: row.get("uuid"),
            query_text: row.get("query_text"),
            pack_json: row.get("pack_json"),
            estimated_tokens: row.get("estimated_tokens"),
            truncated: self.decode_bool(&row, "truncated"),
            created_at: row.get("created_at"),
            retrieval_trace_id: row.get("retrieval_trace_id"),
            space_id: row.get("space_id"),
        }))
    }

    async fn lookup_retrieval_trace_row_id_for_tenant(
        &self,
        tenant_id: i64,
        trace_uuid: &str,
    ) -> Result<Option<i64>, NativeSqlStoreError> {
        let row = sqlx::query("SELECT id FROM ai_retrieval_trace WHERE tenant_id = ? AND uuid = ?")
            .bind(tenant_id)
            .bind(trace_uuid)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|row| row.get("id")))
    }

    pub async fn resolve_space_internal_id_by_uuid(
        &self,
        tenant_id: i64,
        space_uuid: &str,
    ) -> Result<Option<i64>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM ai_space
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|row| row.get("id")))
    }

    pub async fn list_spaces_for_tenant(
        &self,
        tenant_id: i64,
        page_size: i32,
        cursor_space_id: i64,
        actor_id: Option<&str>,
    ) -> Result<Vec<NativeSqlMemorySpaceRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let rows = if let Some(actor_id) = actor_id {
            sqlx::query(
                r#"
                SELECT id, uuid, tenant_id, owner_subject_type, owner_subject_id, space_type,
                       display_name, default_scope, lifecycle_status, created_at, updated_at, version
                FROM ai_space
                WHERE tenant_id = ?
                  AND id > ?
                  AND lifecycle_status <> 'deleted'
                  AND owner_subject_type = 'user'
                  AND owner_subject_id = ?
                ORDER BY id ASC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(cursor_space_id)
            .bind(actor_id)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT id, uuid, tenant_id, owner_subject_type, owner_subject_id, space_type,
                       display_name, default_scope, lifecycle_status, created_at, updated_at, version
                FROM ai_space
                WHERE tenant_id = ? AND id > ? AND lifecycle_status <> 'deleted'
                ORDER BY id ASC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(cursor_space_id)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.into_iter().map(space_row_from_sql).collect())
    }

    pub async fn retrieve_space_for_tenant(
        &self,
        tenant_id: i64,
        space_id: i64,
    ) -> Result<Option<NativeSqlMemorySpaceRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, owner_subject_type, owner_subject_id, space_type,
                   display_name, default_scope, lifecycle_status, created_at, updated_at, version
            FROM ai_space
            WHERE tenant_id = ? AND id = ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(space_row_from_sql))
    }

    pub async fn create_space_record(
        &self,
        tenant_id: i64,
        space_id: i64,
        request: &NativeSqlCreateSpaceCommand,
    ) -> Result<(), NativeSqlStoreError> {
        // `ai_space.organization_id` is `NOT NULL DEFAULT 0` on both engines
        // (`database/migrations/postgres/0001_organization_id_not_null.up.sql` backfilled NULL to
        // 0 and then applied NOT NULL). A personal space legitimately has no organization, so a
        // `None` command field must resolve to the schema's own "no organization" value instead of
        // binding SQL NULL, which the NOT NULL constraint rejects.
        let organization_id = request.organization_id.unwrap_or(0);
        sqlx::query(
            r#"
            INSERT INTO ai_space (
              id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id,
              space_type, display_name, default_scope, lifecycle_status, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, 0)
            "#,
        )
        .bind(space_id)
        .bind(format!("space-{space_id}"))
        .bind(tenant_id)
        .bind(organization_id)
        .bind(&request.owner_subject_type)
        .bind(&request.owner_subject_id)
        .bind(&request.space_type)
        .bind(&request.display_name)
        .bind(&request.default_scope)
        .bind(now_text())
        .bind(now_text())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_space_record(
        &self,
        tenant_id: i64,
        space_id: i64,
        display_name: Option<&str>,
        default_scope: Option<&str>,
    ) -> Result<Option<NativeSqlMemorySpaceRow>, NativeSqlStoreError> {
        let existing = self.retrieve_space_for_tenant(tenant_id, space_id).await?;
        let Some(existing) = existing else {
            return Ok(None);
        };

        let display_name = display_name.unwrap_or(&existing.display_name);
        let default_scope =
            default_scope.unwrap_or(existing.default_scope.as_deref().unwrap_or("user"));

        sqlx::query(
            r#"
            UPDATE ai_space
            SET display_name = ?, default_scope = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND id = ?
            "#,
        )
        .bind(display_name)
        .bind(default_scope)
        .bind(now_text())
        .bind(tenant_id)
        .bind(space_id)
        .execute(&self.pool)
        .await?;

        self.retrieve_space_for_tenant(tenant_id, space_id).await
    }

    pub async fn list_open_api_events_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlOpenApiEventRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let cursor = cursor.unwrap_or("");
        let rows = if let Some(space_id) = space_id {
            sqlx::query(
                r#"
                SELECT uuid, space_id, user_id, actor_type, actor_id, event_type, source_type, event_time, payload_json, payload_hash, ingestion_status, created_at
                FROM ai_event
                WHERE tenant_id = ? AND space_id = ? AND uuid > ?
                ORDER BY uuid ASC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(space_id)
            .bind(cursor)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT uuid, space_id, user_id, actor_type, actor_id, event_type, source_type, event_time, payload_json, payload_hash, ingestion_status, created_at
                FROM ai_event
                WHERE tenant_id = ? AND uuid > ?
                ORDER BY uuid ASC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(cursor)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|row| {
                let payload_json: String = row.get("payload_json");
                NativeSqlOpenApiEventRow {
                    event_id: row.get("uuid"),
                    space_id: row.get("space_id"),
                    user_id: row.try_get("user_id").ok(),
                    actor_type: row.try_get("actor_type").ok(),
                    actor_id: row.try_get("actor_id").ok(),
                    event_type: row.get("event_type"),
                    source_type: row.get("source_type"),
                    event_time: row.get("event_time"),
                    payload: parse_event_payload(&payload_json).unwrap_or(Value::Null),
                    payload_hash: row.get("payload_hash"),
                    ingestion_status: row.get("ingestion_status"),
                    created_at: row.get("created_at"),
                }
            })
            .collect())
    }

    pub async fn list_candidates_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlCandidateRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let cursor = cursor.unwrap_or("");
        let rows = if let Some(space_id) = space_id {
            sqlx::query(
                r#"
                SELECT uuid, space_id, candidate_type, memory_type, proposed_text, confidence,
                       decision_state, created_at, updated_at
                FROM ai_candidate
                WHERE tenant_id = ? AND space_id = ? AND uuid > ?
                ORDER BY uuid ASC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(space_id)
            .bind(cursor)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                r#"
                SELECT uuid, space_id, candidate_type, memory_type, proposed_text, confidence,
                       decision_state, created_at, updated_at
                FROM ai_candidate
                WHERE tenant_id = ? AND uuid > ?
                ORDER BY uuid ASC
                LIMIT ?
                "#,
            )
            .bind(tenant_id)
            .bind(cursor)
            .bind(page_size + 1)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlCandidateRow {
                candidate_id: row.get("uuid"),
                space_id: row.get("space_id"),
                candidate_type: row.get("candidate_type"),
                memory_type: row.get("memory_type"),
                proposed_text: row.get("proposed_text"),
                confidence: row.get("confidence"),
                decision_state: row.get("decision_state"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn retrieve_candidate_for_tenant(
        &self,
        tenant_id: i64,
        candidate_id: &str,
    ) -> Result<Option<NativeSqlCandidateRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT uuid, space_id, candidate_type, memory_type, proposed_text, confidence,
                   decision_state, created_at, updated_at
            FROM ai_candidate
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(candidate_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlCandidateRow {
            candidate_id: row.get("uuid"),
            space_id: row.get("space_id"),
            candidate_type: row.get("candidate_type"),
            memory_type: row.get("memory_type"),
            proposed_text: row.get("proposed_text"),
            confidence: row.get("confidence"),
            decision_state: row.get("decision_state"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn retrieve_candidate_detail_for_tenant(
        &self,
        tenant_id: i64,
        candidate_id: &str,
    ) -> Result<Option<NativeSqlCandidateDetailRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              candidate.uuid,
              candidate.space_id,
              candidate.candidate_type,
              candidate.memory_type,
              candidate.proposed_text,
              candidate.evidence_json,
              candidate.confidence,
              candidate.decision_state,
              candidate.created_at,
              candidate.updated_at,
              record.uuid AS target_memory_uuid
            FROM ai_candidate candidate
            LEFT JOIN ai_record record
              ON record.id = candidate.target_memory_id
             AND record.tenant_id = candidate.tenant_id
             AND record.space_id = candidate.space_id
             AND record.status <> 'deleted'
            WHERE candidate.tenant_id = ?
              AND candidate.uuid = ?
            "#,
        )
        .bind(tenant_id)
        .bind(candidate_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| NativeSqlCandidateDetailRow {
            candidate_id: row.get("uuid"),
            space_id: row.get("space_id"),
            candidate_type: row.get("candidate_type"),
            memory_type: row.get("memory_type"),
            proposed_text: row.get("proposed_text"),
            evidence_json: row.get("evidence_json"),
            confidence: row.get("confidence"),
            decision_state: row.get("decision_state"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            target_memory_uuid: row.get("target_memory_uuid"),
        }))
    }

    pub async fn set_candidate_target_memory_for_tenant(
        &self,
        tenant_id: i64,
        candidate_id: &str,
        memory_uuid: &str,
    ) -> Result<(), NativeSqlStoreError> {
        let result = sqlx::query(
            r#"
            UPDATE ai_candidate
            SET target_memory_id = (
              SELECT record.id
              FROM ai_record record
              WHERE record.tenant_id = ai_candidate.tenant_id
                AND record.space_id = ai_candidate.space_id
                AND record.uuid = ?
                AND record.status <> 'deleted'
            ),
            updated_at = ?
            WHERE ai_candidate.tenant_id = ?
              AND ai_candidate.uuid = ?
              AND EXISTS (
                SELECT 1
                FROM ai_record record
                WHERE record.tenant_id = ai_candidate.tenant_id
                  AND record.space_id = ai_candidate.space_id
                  AND record.uuid = ?
                  AND record.status <> 'deleted'
              )
            "#,
        )
        .bind(memory_uuid)
        .bind(now_text())
        .bind(tenant_id)
        .bind(candidate_id)
        .bind(memory_uuid)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "candidate or promoted memory not found".to_string(),
            });
        }

        Ok(())
    }

    pub async fn list_habits_for_tenant(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        stage: Option<&str>,
        query_text: Option<&str>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlHabitRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let cursor = cursor.unwrap_or("");
        let like_pattern = query_text.map(like_pattern);
        let rows = sqlx::query(
            r#"
            SELECT
              habit.uuid,
              habit.space_id,
              habit.user_id,
              habit.habit_key,
              habit.habit_type,
              habit.description,
              habit.stage,
              habit.strength,
              habit.confidence,
              habit.support_count,
              habit.last_signal_at,
              promoted.uuid AS promoted_memory_uuid,
              habit.decay_after,
              habit.metadata_json,
              habit.created_at,
              habit.updated_at,
              habit.version
            FROM ai_habit habit
            LEFT JOIN ai_record promoted
              ON promoted.id = habit.promoted_memory_id
             AND promoted.tenant_id = habit.tenant_id
             AND promoted.space_id = habit.space_id
            WHERE habit.tenant_id = ?
              AND (? IS NULL OR habit.space_id = ?)
              AND (? IS NULL OR habit.stage = ?)
              AND (
                ? IS NULL
                OR habit.description LIKE ? ESCAPE '\'
                OR habit.habit_key LIKE ? ESCAPE '\'
              )
              AND habit.uuid > ?
            ORDER BY habit.uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(space_id)
        .bind(stage)
        .bind(stage)
        .bind(like_pattern.as_deref())
        .bind(like_pattern.as_deref())
        .bind(like_pattern.as_deref())
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(map_habit_row).collect())
    }

    pub async fn retrieve_habit_for_tenant(
        &self,
        tenant_id: i64,
        habit_id: &str,
    ) -> Result<Option<NativeSqlHabitRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              habit.uuid,
              habit.space_id,
              habit.user_id,
              habit.habit_key,
              habit.habit_type,
              habit.description,
              habit.stage,
              habit.strength,
              habit.confidence,
              habit.support_count,
              habit.last_signal_at,
              promoted.uuid AS promoted_memory_uuid,
              habit.decay_after,
              habit.metadata_json,
              habit.created_at,
              habit.updated_at,
              habit.version
            FROM ai_habit habit
            LEFT JOIN ai_record promoted
              ON promoted.id = habit.promoted_memory_id
             AND promoted.tenant_id = habit.tenant_id
             AND promoted.space_id = habit.space_id
            WHERE habit.tenant_id = ?
              AND (habit.uuid = ? OR CAST(habit.id AS TEXT) = ?)
            "#,
        )
        .bind(tenant_id)
        .bind(habit_id)
        .bind(habit_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(map_habit_row))
    }

    pub async fn append_record_source_for_tenant(
        &self,
        tenant_id: i64,
        source_id: &str,
        memory_uuid: &str,
        event_uuid: &str,
        source_role: &str,
        confidence_delta: Option<f64>,
    ) -> Result<(), NativeSqlStoreError> {
        let result = sqlx::query(
            r#"
            INSERT INTO ai_record_source (
              id,
              uuid,
              tenant_id,
              memory_id,
              event_id,
              source_role,
              confidence_delta,
              created_at
            )
            SELECT
              ?,
              ?,
              ?,
              record.id,
              event.id,
              ?,
              ?,
              ?
            FROM ai_record record
            JOIN ai_event event
              ON event.tenant_id = record.tenant_id
             AND event.uuid = ?
            WHERE record.tenant_id = ?
              AND record.uuid = ?
              AND record.status <> 'deleted'
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(source_id)
        .bind(tenant_id)
        .bind(source_role)
        .bind(confidence_delta)
        .bind(now_text())
        .bind(event_uuid)
        .bind(tenant_id)
        .bind(memory_uuid)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "memory or event not found for record source".to_string(),
            });
        }

        Ok(())
    }

    pub async fn list_record_sources_for_memory(
        &self,
        tenant_id: i64,
        memory_uuid: &str,
        page_size: i32,
        cursor: Option<&str>,
        query_text: Option<&str>,
    ) -> Result<Vec<NativeSqlRecordSourceRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let cursor = cursor.unwrap_or("");
        let like_pattern = query_text.map(like_pattern);
        let rows = sqlx::query(
            r#"
            SELECT
              source.uuid AS source_uuid,
              record.uuid AS memory_uuid,
              event.uuid AS event_uuid,
              source.source_role,
              source.confidence_delta,
              source.created_at
            FROM ai_record_source source
            JOIN ai_record record
              ON record.id = source.memory_id
             AND record.tenant_id = source.tenant_id
            JOIN ai_event event
              ON event.id = source.event_id
             AND event.tenant_id = source.tenant_id
            WHERE source.tenant_id = ?
              AND record.uuid = ?
              AND (
                ? IS NULL
                OR source.source_role LIKE ? ESCAPE '\'
                OR event.uuid LIKE ? ESCAPE '\'
              )
              AND source.id < COALESCE(
                (SELECT s2.id FROM ai_record_source s2 WHERE s2.tenant_id = ? AND s2.uuid = ? LIMIT 1),
                9223372036854775807
              )
            ORDER BY source.id DESC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(memory_uuid)
        .bind(like_pattern.as_deref())
        .bind(like_pattern.as_deref())
        .bind(like_pattern.as_deref())
        .bind(tenant_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlRecordSourceRow {
                source_uuid: row.get("source_uuid"),
                memory_uuid: row.get("memory_uuid"),
                event_uuid: row.get("event_uuid"),
                source_role: row.get("source_role"),
                confidence_delta: row.get("confidence_delta"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn list_audit_logs_for_tenant(
        &self,
        tenant_id: i64,
        action: Option<&str>,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlAuditLogRow>, NativeSqlStoreError> {
        let page_size = clamp_list_page_size(page_size);
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT uuid, actor_type, actor_id, action, resource_type, resource_id, result, created_at
            FROM ai_audit_log
            WHERE tenant_id = ?
              AND (? IS NULL OR action = ?)
              AND uuid > ?
            ORDER BY uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(action)
        .bind(action)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| NativeSqlAuditLogRow {
                audit_id: row.get("uuid"),
                actor_type: row.get("actor_type"),
                actor_id: row.get("actor_id"),
                action: row.get("action"),
                resource_type: row.get("resource_type"),
                resource_id: row.get("resource_id"),
                result: row.get("result"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn count_audit_logs_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<i64, NativeSqlStoreError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ai_audit_log WHERE tenant_id = ?")
                .bind(tenant_id)
                .fetch_one(self.pool())
                .await?;
        Ok(count)
    }

    pub async fn count_eval_runs_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<i64, NativeSqlStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_eval_run WHERE tenant_id = ?")
            .bind(tenant_id)
            .fetch_one(self.pool())
            .await?;
        Ok(count)
    }

    pub async fn count_succeeded_retrieval_quality_evals_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<i64, NativeSqlStoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_eval_run WHERE tenant_id = ? AND eval_type = 'retrieval_quality' AND state = 'succeeded'",
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        Ok(count)
    }

    pub async fn count_succeeded_migration_jobs_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<i64, NativeSqlStoreError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_learning_job WHERE tenant_id = ? AND job_type = 'migration_job' AND state = 'succeeded'",
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        Ok(count)
    }

    pub(crate) async fn ensure_space(
        &self,
        scope: &MemoryScopeContext,
    ) -> Result<(), NativeSqlStoreError> {
        if self
            .retrieve_space_for_tenant(scope.tenant_id, scope.space_id)
            .await?
            .is_some()
        {
            return Ok(());
        }

        Err(NativeSqlStoreError::InvariantViolation {
            message: format!(
                "memory space {} does not exist for tenant {}",
                scope.space_id, scope.tenant_id
            ),
        })
    }
}

#[async_trait]
impl MemoryRetrieverPort for NativeSqlMemoryStore {
    fn retriever_code(&self) -> &str {
        "native_sql_phase1"
    }

    fn supports_bounded_scoped_search(&self) -> bool {
        true
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Err(MemorySpiError::PortOperationFailed {
            port: "MemoryRetrieverPort".to_string(),
            message: "scope-aware retrieval is required; use search_scoped".to_string(),
        })
    }

    async fn retrieve_scoped(
        &self,
        scope: MemoryScopeContext,
        command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        let result = self
            .search_memory_candidates(&SearchMemoryCandidatesQuery {
                scope,
                query: command.query,
                limit: 20,
                retriever_kinds: vec![
                    MemoryRetrieverKind::Sql,
                    MemoryRetrieverKind::Keyword,
                    MemoryRetrieverKind::Dictionary,
                    MemoryRetrieverKind::Time,
                    MemoryRetrieverKind::Event,
                ],
                memory_types: Vec::new(),
                read_scope: MemorySensitivityReadScope::Owner,
                metadata_filter: None,
            })
            .await
            .map_err(|err| port_error("MemoryRetrieverPort", err))?;
        let mut memory_ids = result
            .records
            .into_iter()
            .map(|candidate| candidate.memory_id)
            .chain(
                result
                    .events
                    .into_iter()
                    .map(|candidate| candidate.memory_id),
            )
            .collect::<Vec<_>>();
        memory_ids.sort();
        memory_ids.dedup();
        Ok(MemoryRetrieverResult { memory_ids })
    }

    async fn search_scoped(
        &self,
        query: SearchMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryRetrieverSearchResult> {
        self.search_memory_candidates(&query)
            .await
            .map_err(|err| port_error("MemoryRetrieverPort", err))
    }
}

#[async_trait]
impl MemoryEventStorePort for NativeSqlMemoryStore {
    async fn append(&self, command: AppendMemoryEventCommand) -> MemorySpiResult<MemoryEvent> {
        self.append_event(&command.scope, &command.event_id, &command.content)
            .await
            .map_err(|err| port_error("MemoryEventStorePort", err))?;

        Ok(MemoryEvent {
            event_id: command.event_id,
            content: command.content,
        })
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryEventQuery,
    ) -> MemorySpiResult<Option<MemoryEvent>> {
        let event = self
            .retrieve_event(&query.scope, &query.event_id)
            .await
            .map_err(|err| port_error("MemoryEventStorePort", err))?;

        Ok(event.map(|event| MemoryEvent {
            event_id: event.event_id,
            content: event.content,
        }))
    }
}

#[async_trait]
impl MemoryRecordStorePort for NativeSqlMemoryStore {
    fn supports_canonical_atomic(&self) -> bool {
        true
    }

    fn supports_atomic_record_quota_admission(&self) -> bool {
        true
    }

    fn supports_atomic_supersede(&self) -> bool {
        true
    }

    async fn create(&self, command: CreateMemoryRecordCommand) -> MemorySpiResult<MemoryRecord> {
        self.create_record(&command.scope, &command.memory_id, "spi", &command.content)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))?;

        Ok(MemoryRecord {
            memory_id: command.memory_id,
            content: command.content,
        })
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryRecordQuery,
    ) -> MemorySpiResult<Option<MemoryRecord>> {
        let record = self
            .retrieve_record(&query.scope, &query.memory_id)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))?;

        Ok(record.map(|record| MemoryRecord {
            memory_id: record.memory_id,
            content: record.content,
        }))
    }

    async fn mark_deleted(
        &self,
        command: DeleteMemoryRecordCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        self.mark_record_deleted(&command.scope, &command.memory_id)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }

    async fn create_canonical_atomic(
        &self,
        command: CreateCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryCanonicalRecord> {
        self.create_canonical_memory_atomic(&command)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }

    async fn create_canonical_atomic_with_quota(
        &self,
        command: CreateCanonicalMemoryCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        self.create_canonical_memory_atomic_with_quota(&command, max_active_records)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }

    async fn supersede_canonical_atomic_with_quota(
        &self,
        command: SupersedeCanonicalMemoryAtomicCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>> {
        self.supersede_canonical_memory_atomic_with_quota(&command, max_active_records)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }

    async fn retrieve_canonical(
        &self,
        query: RetrieveCanonicalMemoryQuery,
    ) -> MemorySpiResult<Option<MemoryCanonicalRecord>> {
        self.retrieve_canonical_memory(&query.scope, &query.memory_id)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }

    async fn update_canonical_atomic(
        &self,
        command: UpdateCanonicalMemoryCommand,
    ) -> MemorySpiResult<Option<MemoryCanonicalRecord>> {
        self.update_canonical_memory_atomic(&command)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }

    async fn delete_canonical_atomic(
        &self,
        command: DeleteCanonicalMemoryCommand,
    ) -> MemorySpiResult<MemoryDeletionReceipt> {
        self.delete_canonical_memory_atomic(&command)
            .await
            .map_err(|err| port_error("MemoryRecordStorePort", err))
    }
}

#[async_trait]
impl MemoryAuditStorePort for NativeSqlMemoryStore {
    async fn append(
        &self,
        command: AppendMemoryAuditCommand,
    ) -> MemorySpiResult<MemoryAuditRecord> {
        let audit = self
            .append_audit(
                &command.scope,
                &command.audit_id,
                &command.action,
                &command.resource_type,
                &command.resource_id,
                &command.result,
            )
            .await
            .map_err(|err| port_error("MemoryAuditStorePort", err))?;

        Ok(MemoryAuditRecord {
            audit_id: audit.audit_id,
            action: audit.action,
            resource_type: audit.resource_type,
            resource_id: audit.resource_id,
            result: audit.result,
        })
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryAuditQuery,
    ) -> MemorySpiResult<Option<MemoryAuditRecord>> {
        let audit = self
            .retrieve_audit(&query.scope, &query.audit_id)
            .await
            .map_err(|err| port_error("MemoryAuditStorePort", err))?;

        Ok(audit.map(|audit| MemoryAuditRecord {
            audit_id: audit.audit_id,
            action: audit.action,
            resource_type: audit.resource_type,
            resource_id: audit.resource_id,
            result: audit.result,
        }))
    }
}

#[async_trait]
impl MemoryOutboxStorePort for NativeSqlMemoryStore {
    async fn append(
        &self,
        command: AppendMemoryOutboxCommand,
    ) -> MemorySpiResult<MemoryOutboxEvent> {
        let outbox = self
            .append_outbox_event(NativeSqlAppendOutboxEventCommand {
                scope: &command.scope,
                outbox_id: &command.outbox_id,
                aggregate_type: &command.aggregate_type,
                aggregate_id: &command.aggregate_id,
                event_type: &command.event_type,
                event_version: &command.event_version,
                payload_json: &command.payload_json,
            })
            .await
            .map_err(|err| port_error("MemoryOutboxStorePort", err))?;

        Ok(MemoryOutboxEvent {
            outbox_id: outbox.outbox_id,
            aggregate_type: outbox.aggregate_type,
            aggregate_id: outbox.aggregate_id,
            event_type: outbox.event_type,
            event_version: outbox.event_version,
            payload_json: outbox.payload_json,
            publish_state: outbox.publish_state,
            published_at: outbox.published_at,
            retry_count: outbox.retry_count,
        })
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryOutboxQuery,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        let outbox = self
            .retrieve_outbox_event(&query.scope, &query.outbox_id)
            .await
            .map_err(|err| port_error("MemoryOutboxStorePort", err))?;

        Ok(outbox.map(into_spi_outbox_event))
    }

    async fn list_pending(
        &self,
        query: ListPendingMemoryOutboxQuery,
    ) -> MemorySpiResult<Vec<MemoryOutboxEvent>> {
        let outbox_events = self
            .list_pending_outbox_events(&query.scope, query.limit)
            .await
            .map_err(|err| port_error("MemoryOutboxStorePort", err))?;

        Ok(outbox_events
            .into_iter()
            .map(into_spi_outbox_event)
            .collect())
    }

    async fn mark_published(
        &self,
        command: MarkMemoryOutboxPublishedCommand,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        let outbox = self
            .mark_outbox_published(&command.scope, &command.outbox_id)
            .await
            .map_err(|err| port_error("MemoryOutboxStorePort", err))?;

        Ok(outbox.map(into_spi_outbox_event))
    }

    async fn mark_failed(
        &self,
        command: MarkMemoryOutboxFailedCommand,
    ) -> MemorySpiResult<Option<MemoryOutboxEvent>> {
        let outbox = self
            .mark_outbox_failed(&command.scope, &command.outbox_id)
            .await
            .map_err(|err| port_error("MemoryOutboxStorePort", err))?;

        Ok(outbox.map(into_spi_outbox_event))
    }
}

#[async_trait]
impl MemoryCandidateStorePort for NativeSqlMemoryStore {
    fn supports_candidate_detail_lookup(&self) -> bool {
        true
    }

    fn supports_candidate_listing(&self) -> bool {
        true
    }

    fn supports_atomic_candidate_promotion(&self) -> bool {
        true
    }

    fn supports_atomic_candidate_promotion_journal(&self) -> bool {
        true
    }

    async fn create(
        &self,
        command: CreateMemoryCandidateCommand,
    ) -> MemorySpiResult<MemoryCandidate> {
        self.create_candidate(&command)
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryCandidateQuery,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        self.retrieve_candidate(&query.scope, &query.candidate_id)
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))
    }

    async fn retrieve_detail(
        &self,
        query: RetrieveMemoryCandidateDetailQuery,
    ) -> MemorySpiResult<Option<MemoryCandidateDetail>> {
        self.retrieve_candidate_detail_for_tenant(query.tenant_id, &query.candidate_id)
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))
            .map(|detail| {
                detail.map(|row| MemoryCandidateDetail {
                    candidate_id: row.candidate_id,
                    space_id: row.space_id,
                    candidate_type: row.candidate_type,
                    memory_type: row.memory_type,
                    proposed_text: row.proposed_text,
                    evidence_json: row.evidence_json,
                    confidence: row.confidence,
                    decision_state: row.decision_state,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    target_memory_id: row.target_memory_uuid,
                })
            })
    }

    async fn list_candidates(
        &self,
        query: ListMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryCandidatePage> {
        let page_size = i32::try_from(query.page_size)
            .unwrap_or(MAX_LIST_PAGE_SIZE)
            .clamp(1, MAX_LIST_PAGE_SIZE);
        let rows = self
            .list_candidates_for_tenant(
                query.tenant_id,
                query.space_id,
                page_size,
                query.cursor.as_deref(),
            )
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))?;
        let has_more = rows.len() > page_size as usize;
        let items = rows
            .into_iter()
            .take(page_size as usize)
            .map(|row| MemoryCandidateSummary {
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
            .collect::<Vec<_>>();
        let next_cursor = has_more
            .then(|| items.last().map(|item| item.candidate_id.clone()))
            .flatten();
        Ok(MemoryCandidatePage {
            items,
            has_more,
            next_cursor,
        })
    }

    async fn approve(
        &self,
        command: ApproveMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        self.approve_candidate(&command)
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))
    }

    async fn reject(
        &self,
        command: RejectMemoryCandidateCommand,
    ) -> MemorySpiResult<Option<MemoryCandidate>> {
        self.reject_candidate(&command)
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))
    }

    async fn promote_atomic_with_quota(
        &self,
        command: PromoteMemoryCandidateAtomicCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        let evidence_links = command
            .evidence_links
            .iter()
            .map(|link| {
                (
                    link.source_id.clone(),
                    link.event_id.clone(),
                    link.confidence_delta,
                )
            })
            .collect::<Vec<_>>();
        let candidate_id = command.candidate_id.clone();
        let admission = self
            .promote_and_approve_candidate_with_quota(
                PromoteApprovedCandidateCommand {
                    scope: &command.scope,
                    tenant_id: command.scope.tenant_id,
                    candidate_id: &command.candidate_id,
                    memory_uuid: &command.memory_id,
                    memory_type: &command.memory_type,
                    proposed_text: &command.proposed_text,
                    evidence_links: &evidence_links,
                    decided_by: command.decided_by,
                    create_record: true,
                },
                max_active_records,
            )
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))?;

        Ok(match admission {
            MemoryRecordQuotaAdmission::Admitted(memory_id) => {
                MemoryRecordQuotaAdmission::Admitted(MemoryCandidatePromotion {
                    candidate_id,
                    memory_id,
                })
            }
            MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            } => MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            },
        })
    }

    async fn promote_atomic_with_quota_and_journal(
        &self,
        command: PromoteMemoryCandidateAtomicWithJournalCommand,
        max_active_records: u64,
    ) -> MemorySpiResult<MemoryRecordQuotaAdmission<MemoryCandidatePromotion>> {
        let promotion = command.promotion;
        let evidence_links = promotion
            .evidence_links
            .iter()
            .map(|link| {
                (
                    link.source_id.clone(),
                    link.event_id.clone(),
                    link.confidence_delta,
                )
            })
            .collect::<Vec<_>>();
        let candidate_id = promotion.candidate_id.clone();
        let admission = self
            .promote_and_approve_candidate_with_quota_and_journal(
                PromoteApprovedCandidateCommand {
                    scope: &promotion.scope,
                    tenant_id: promotion.scope.tenant_id,
                    candidate_id: &promotion.candidate_id,
                    memory_uuid: &promotion.memory_id,
                    memory_type: &promotion.memory_type,
                    proposed_text: &promotion.proposed_text,
                    evidence_links: &evidence_links,
                    decided_by: promotion.decided_by,
                    create_record: true,
                },
                max_active_records,
                &command.journal,
            )
            .await
            .map_err(|err| port_error("MemoryCandidateStorePort", err))?;

        Ok(match admission {
            MemoryRecordQuotaAdmission::Admitted(memory_id) => {
                MemoryRecordQuotaAdmission::Admitted(MemoryCandidatePromotion {
                    candidate_id,
                    memory_id,
                })
            }
            MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            } => MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            },
        })
    }
}

#[async_trait]
impl MemoryHabitStorePort for NativeSqlMemoryStore {
    async fn upsert(&self, command: UpsertMemoryHabitCommand) -> MemorySpiResult<MemoryHabit> {
        self.upsert_habit(&command)
            .await
            .map_err(|err| port_error("MemoryHabitStorePort", err))
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryHabitQuery,
    ) -> MemorySpiResult<Option<MemoryHabit>> {
        self.retrieve_habit(&query.scope, query.user_id, &query.habit_key)
            .await
            .map_err(|err| port_error("MemoryHabitStorePort", err))
    }

    async fn promote(
        &self,
        command: PromoteMemoryHabitCommand,
    ) -> MemorySpiResult<Option<MemoryHabit>> {
        self.promote_habit(&command)
            .await
            .map_err(|err| port_error("MemoryHabitStorePort", err))
    }

    async fn decay(
        &self,
        command: DecayMemoryHabitCommand,
    ) -> MemorySpiResult<Option<MemoryHabit>> {
        self.decay_habit(&command)
            .await
            .map_err(|err| port_error("MemoryHabitStorePort", err))
    }
}

#[async_trait]
impl MemoryRetrievalTraceStorePort for NativeSqlMemoryStore {
    fn supports_tenant_trace_lookup(&self) -> bool {
        true
    }

    async fn append(
        &self,
        command: AppendMemoryRetrievalTraceCommand,
    ) -> MemorySpiResult<MemoryRetrievalTrace> {
        self.append_retrieval_trace(&command)
            .await
            .map_err(|err| port_error("MemoryRetrievalTraceStorePort", err))
    }

    async fn retrieve(
        &self,
        query: RetrieveMemoryRetrievalTraceQuery,
    ) -> MemorySpiResult<Option<MemoryRetrievalTrace>> {
        self.retrieve_retrieval_trace(&query.scope, &query.trace_id)
            .await
            .map_err(|err| port_error("MemoryRetrievalTraceStorePort", err))
    }

    async fn retrieve_for_tenant(
        &self,
        query: RetrieveMemoryRetrievalTraceForTenantQuery,
    ) -> MemorySpiResult<Option<ScopedMemoryRetrievalTrace>> {
        self.retrieve_retrieval_trace_lookup_for_tenant(query.tenant_id, &query.trace_id)
            .await
            .map(|lookup| {
                lookup.map(|lookup| ScopedMemoryRetrievalTrace {
                    scope: MemoryScopeContext {
                        tenant_id: query.tenant_id,
                        space_id: lookup.space_id,
                        organization_id: None,
                        user_id: None,
                    },
                    trace: lookup.trace,
                    created_at: Some(lookup.created_at),
                })
            })
            .map_err(|err| port_error("MemoryRetrievalTraceStorePort", err))
    }

    async fn list_recent(
        &self,
        query: ListMemoryRetrievalTracesQuery,
    ) -> MemorySpiResult<Vec<MemoryRetrievalTrace>> {
        self.list_recent_retrieval_traces(&query.scope, query.limit)
            .await
            .map_err(|err| port_error("MemoryRetrievalTraceStorePort", err))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlOpenApiEventRow {
    pub event_id: String,
    pub space_id: i64,
    pub user_id: Option<i64>,
    pub actor_type: Option<String>,
    pub actor_id: Option<String>,
    pub event_type: String,
    pub source_type: String,
    pub event_time: String,
    pub payload: Value,
    pub payload_hash: String,
    pub ingestion_status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlMemoryRecordDetail {
    pub memory_id: String,
    pub space_id: i64,
    pub user_id: Option<i64>,
    pub scope: String,
    pub memory_type: String,
    pub subject: Option<String>,
    pub predicate: Option<String>,
    pub object_text: String,
    pub canonical_text: String,
    pub confidence: f64,
    pub evidence_count: Option<i32>,
    pub contradiction_count: Option<i32>,
    pub status: String,
    pub sensitivity_level: String,
    pub supersedes_memory_id: Option<String>,
    pub superseded_by_memory_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub version: i64,
}

pub(crate) fn record_detail_from_row(row: AnyRow) -> NativeSqlMemoryRecordDetail {
    NativeSqlMemoryRecordDetail {
        memory_id: row.get("uuid"),
        space_id: row.get("space_id"),
        user_id: row.try_get("user_id").ok(),
        scope: row.get("scope"),
        memory_type: row.get("memory_type"),
        subject: row.get("subject"),
        predicate: row.get("predicate"),
        object_text: row.get("object_text"),
        canonical_text: row.get("canonical_text"),
        confidence: row.get("confidence"),
        evidence_count: row.try_get("evidence_count").ok(),
        contradiction_count: row.try_get("contradiction_count").ok(),
        status: row.get("status"),
        sensitivity_level: row.get("sensitivity_level"),
        supersedes_memory_id: row.try_get("supersedes_uuid").ok(),
        superseded_by_memory_id: row.try_get("superseded_by_uuid").ok(),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        expires_at: row.try_get("expires_at").ok(),
        version: row.get("version"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlMemorySpaceRow {
    pub space_id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub space_type: String,
    pub display_name: String,
    pub default_scope: Option<String>,
    pub lifecycle_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlCreateSpaceCommand {
    pub organization_id: Option<i64>,
    pub owner_subject_type: String,
    pub owner_subject_id: String,
    pub space_type: String,
    pub display_name: String,
    pub default_scope: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlCandidateRow {
    pub candidate_id: String,
    pub space_id: i64,
    pub candidate_type: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub confidence: f64,
    pub decision_state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlCandidateDetailRow {
    pub candidate_id: String,
    pub space_id: i64,
    pub candidate_type: String,
    pub memory_type: String,
    pub proposed_text: String,
    pub evidence_json: Option<String>,
    pub confidence: f64,
    pub decision_state: String,
    pub created_at: String,
    pub updated_at: String,
    pub target_memory_uuid: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlHabitRow {
    pub habit_id: String,
    pub space_id: i64,
    pub user_id: i64,
    pub habit_key: String,
    pub habit_type: String,
    pub description: String,
    pub stage: String,
    pub strength: f64,
    pub confidence: f64,
    pub support_count: i64,
    pub last_signal_at: Option<String>,
    pub promoted_memory_uuid: Option<String>,
    pub decay_after: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlRecordSourceRow {
    pub source_uuid: String,
    pub memory_uuid: String,
    pub event_uuid: String,
    pub source_role: String,
    pub confidence_delta: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlAuditLogRow {
    pub audit_id: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub result: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlGovernanceJobRow {
    pub job_id: String,
    pub actor_id: Option<String>,
    pub resource_type: String,
    pub result: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlRetrievalTraceSummaryRow {
    pub trace_id: String,
    pub space_id: i64,
    pub query_text: Option<String>,
    pub query_hash: String,
    pub result_count: i64,
    pub degraded: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeSqlContextPackRow {
    pub context_pack_id: String,
    pub query_text: Option<String>,
    pub pack_json: String,
    pub estimated_tokens: i64,
    pub truncated: bool,
    pub created_at: String,
    pub retrieval_trace_id: Option<i64>,
    pub space_id: Option<i64>,
}

fn space_row_from_sql(row: AnyRow) -> NativeSqlMemorySpaceRow {
    NativeSqlMemorySpaceRow {
        space_id: row.get("id"),
        uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        owner_subject_type: row.get("owner_subject_type"),
        owner_subject_id: row.get("owner_subject_id"),
        space_type: row.get("space_type"),
        display_name: row.get("display_name"),
        default_scope: row.get("default_scope"),
        lifecycle_status: row.get("lifecycle_status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlMemoryEvent {
    pub event_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlMemoryRecord {
    pub memory_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlMemoryRecordLifecycle {
    pub memory_id: String,
    pub status: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlMemoryAuditRecord {
    pub audit_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlScopedOutboxEvent {
    pub tenant_id: i64,
    pub lease_owner: Option<String>,
    pub lease_token: Option<String>,
    pub lease_expires_at: Option<String>,
    pub outbox: NativeSqlMemoryOutboxEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlMemoryOutboxEvent {
    pub outbox_id: String,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub event_version: String,
    pub payload_json: String,
    pub publish_state: String,
    pub published_at: Option<String>,
    pub retry_count: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct NativeSqlAppendOutboxEventCommand<'a> {
    pub scope: &'a MemoryScopeContext,
    pub outbox_id: &'a str,
    pub aggregate_type: &'a str,
    pub aggregate_id: &'a str,
    pub event_type: &'a str,
    pub event_version: &'a str,
    pub payload_json: &'a str,
}

#[derive(Debug, Error)]
pub enum NativeSqlStoreError {
    #[error("native SQL store database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("native SQL store pool error: {0}")]
    Pool(#[from] sdkwork_database_sqlx::PoolError),
    #[error("native SQL store JSON payload error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("native SQL store ID generation error: {0}")]
    IdGeneration(String),
    #[error("native SQL event append conflict for tenant {tenant_id} event {event_id}")]
    EventConflict { tenant_id: i64, event_id: String },
    #[error("native SQL outbox append conflict for tenant {tenant_id} outbox event {outbox_id}")]
    OutboxConflict { tenant_id: i64, outbox_id: String },
    #[error("native SQL idempotency conflict for key {idempotency_key}")]
    IdempotencyConflict { idempotency_key: String },
    #[error("native SQL store invariant violation: {message}")]
    InvariantViolation { message: String },
    #[error("native SQL store cannot honor the metadata filter: {message}")]
    MetadataFilterUnsupported { message: String },
}

fn development_id_generator() -> SnowflakeIdGenerator {
    static GENERATOR: OnceLock<SnowflakeIdGenerator> = OnceLock::new();
    GENERATOR
        .get_or_init(|| {
            let node_id = std::env::var("SDKWORK_MEMORY_SNOWFLAKE_NODE_ID")
                .ok()
                .and_then(|value| sdkwork_utils_rust::parse_int(&value))
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0);
            SnowflakeIdGenerator::new(node_id)
                .expect("validated development snowflake node ID must initialize")
        })
        .clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeSqlEventIdempotencyState {
    space_id: i64,
    payload_json: String,
    payload_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeSqlOutboxIdempotencyState {
    aggregate_type: String,
    aggregate_id: String,
    event_type: String,
    event_version: String,
    payload_json: String,
    publish_state: String,
    published_at: Option<String>,
    retry_count: i64,
}

impl NativeSqlOutboxIdempotencyState {
    fn into_outbox_event(self, outbox_id: &str) -> NativeSqlMemoryOutboxEvent {
        NativeSqlMemoryOutboxEvent {
            outbox_id: outbox_id.to_string(),
            aggregate_type: self.aggregate_type,
            aggregate_id: self.aggregate_id,
            event_type: self.event_type,
            event_version: self.event_version,
            payload_json: self.payload_json,
            publish_state: self.publish_state,
            published_at: self.published_at,
            retry_count: self.retry_count,
        }
    }
}

/// PostgreSQL stores tenant-scoped rows with SQL NULL; SQLite uses -1 because
/// `ON CONFLICT` requires a single non-partial unique index.
pub(crate) fn preference_scope_user_binding(
    user_id: Option<i64>,
    dialect: MemorySqlDialect,
) -> Option<i64> {
    match (user_id, dialect) {
        (None, MemorySqlDialect::Postgres) => None,
        (None, MemorySqlDialect::Sqlite) => Some(-1),
        (Some(id), _) => Some(id),
    }
}

fn into_spi_outbox_event(outbox: NativeSqlMemoryOutboxEvent) -> MemoryOutboxEvent {
    MemoryOutboxEvent {
        outbox_id: outbox.outbox_id,
        aggregate_type: outbox.aggregate_type,
        aggregate_id: outbox.aggregate_id,
        event_type: outbox.event_type,
        event_version: outbox.event_version,
        payload_json: outbox.payload_json,
        publish_state: outbox.publish_state,
        published_at: outbox.published_at,
        retry_count: outbox.retry_count,
    }
}

fn candidate_from_row(row: AnyRow) -> MemoryCandidate {
    MemoryCandidate {
        candidate_id: row.get("uuid"),
        candidate_type: row.get("candidate_type"),
        memory_type: row.get("memory_type"),
        proposed_text: row.get("proposed_text"),
        proposed_payload_json: row.get("proposed_payload_json"),
        evidence_json: row.get("evidence_json"),
        confidence: row.get("confidence"),
        decision_state: row.get("decision_state"),
        decision_reason: row.get("decision_reason"),
        decided_by: row.get("decided_by"),
        decided_at: row.get("decided_at"),
    }
}

fn map_habit_row(row: AnyRow) -> NativeSqlHabitRow {
    NativeSqlHabitRow {
        habit_id: row.get("uuid"),
        space_id: row.get("space_id"),
        user_id: row.get("user_id"),
        habit_key: row.get("habit_key"),
        habit_type: row.get("habit_type"),
        description: row.get("description"),
        stage: row.get("stage"),
        strength: row.get("strength"),
        confidence: row.get("confidence"),
        support_count: row.get("support_count"),
        last_signal_at: row.get("last_signal_at"),
        promoted_memory_uuid: row.get("promoted_memory_uuid"),
        decay_after: row.get("decay_after"),
        metadata_json: row.get("metadata_json"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn habit_from_row(row: AnyRow) -> MemoryHabit {
    MemoryHabit {
        habit_id: row.get("uuid"),
        user_id: row.get("user_id"),
        habit_key: row.get("habit_key"),
        habit_type: row.get("habit_type"),
        description: row.get("description"),
        stage: row.get("stage"),
        strength: row.get("strength"),
        confidence: row.get("confidence"),
        support_count: row.get("support_count"),
        last_signal_at: row.get("last_signal_at"),
        promoted_memory_id: row.get("promoted_memory_uuid"),
        decay_after: row.get("decay_after"),
        metadata_json: row.get("metadata_json"),
    }
}

fn retrieval_trace_select_sql() -> &'static str {
    r#"
    SELECT
      id,
      uuid,
      actor_id,
      query_text,
      query_hash,
      retrievers_json,
      latency_ms,
      result_count,
      degraded,
      metadata_json
    FROM ai_retrieval_trace
    WHERE tenant_id = ? AND space_id = ? AND uuid = ?
    "#
}

impl NativeSqlMemoryStore {
    fn decode_bool(&self, row: &sqlx::any::AnyRow, column: &str) -> bool {
        match self.dialect {
            MemorySqlDialect::Sqlite => sqlite_int_to_bool(row.get(column)),
            MemorySqlDialect::Postgres => row
                .try_get::<bool, _>(column)
                .or_else(|_| row.try_get::<i64, _>(column).map(sqlite_int_to_bool))
                .unwrap_or(false),
        }
    }
}

fn sqlite_int_to_bool(value: i64) -> bool {
    value != 0
}

pub(crate) fn now_text() -> String {
    sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), None)
}

pub(crate) fn timestamp_after_seconds(seconds: u64) -> String {
    let now_millis = sdkwork_utils_rust::to_unix_millis(sdkwork_utils_rust::now());
    let delta_millis = i64::try_from(seconds.saturating_mul(1_000)).unwrap_or(i64::MAX);
    let target = now_millis.saturating_add(delta_millis);
    sdkwork_utils_rust::format_datetime(
        sdkwork_utils_rust::from_unix_millis(target).unwrap_or_else(sdkwork_utils_rust::now),
        None,
    )
}

fn stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn parse_event_payload(payload: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(payload)
}

fn port_error(port: &str, error: NativeSqlStoreError) -> MemorySpiError {
    if let NativeSqlStoreError::IdempotencyConflict { idempotency_key } = error {
        return MemorySpiError::IdempotencyConflict { idempotency_key };
    }
    if let NativeSqlStoreError::EventConflict { event_id, .. } = error {
        return MemorySpiError::IdempotencyConflict {
            idempotency_key: event_id,
        };
    }
    if let NativeSqlStoreError::OutboxConflict { outbox_id, .. } = error {
        return MemorySpiError::IdempotencyConflict {
            idempotency_key: outbox_id,
        };
    }

    MemorySpiError::PortOperationFailed {
        port: port.to_string(),
        message: error.to_string(),
    }
}

/// Split a multi-statement SQL string into individual statements.
///
/// Handles PostgreSQL dollar-quoted strings (`$$ ... $$` or `$tag$ ... $tag$`),
/// single-quoted strings with `''` escapes, line comments (`--`), and block
/// comments (`/* ... */`) so that `;` inside these constructs is not treated
/// as a statement separator.
pub(crate) fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        // Line comment: -- to end of line.
        if ch == '-' && chars.peek() == Some(&'-') {
            current.push_str("--");
            chars.next();
            while let Some(&c) = chars.peek() {
                current.push(chars.next().unwrap());
                if c == '\n' {
                    break;
                }
            }
            continue;
        }

        // Block comment: /* ... */
        if ch == '/' && chars.peek() == Some(&'*') {
            current.push_str("/*");
            chars.next();
            while let Some(c) = chars.next() {
                current.push(c);
                if c == '*' && chars.peek() == Some(&'/') {
                    current.push(chars.next().unwrap());
                    break;
                }
            }
            continue;
        }

        // Single-quoted string with '' escape.
        if ch == '\'' {
            current.push('\'');
            while let Some(c) = chars.next() {
                current.push(c);
                if c == '\'' {
                    if chars.peek() == Some(&'\'') {
                        current.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
            }
            continue;
        }

        // Dollar-quoted string: $$ ... $$ or $tag$ ... $tag$
        if ch == '$' {
            let mut tag = String::from("$");
            let mut found_delimiter = false;
            while let Some(&c) = chars.peek() {
                if c == '$' {
                    tag.push('$');
                    chars.next();
                    found_delimiter = true;
                    break;
                }
                if c.is_ascii_alphanumeric() || c == '_' {
                    tag.push(c);
                    chars.next();
                } else {
                    break;
                }
            }

            if found_delimiter {
                current.push_str(&tag);
                // Read until the closing delimiter is found.
                // Use a manual peek loop instead of by_ref() so we don't
                // lose the chars iterator if the closing tag is never found.
                let mut closing_found = false;
                let mut buffer = String::new();
                for c in chars.by_ref() {
                    buffer.push(c);
                    if buffer.ends_with(&tag) {
                        current.push_str(&buffer);
                        closing_found = true;
                        break;
                    }
                }
                if !closing_found {
                    // Unclosed dollar-quote: push what we have and continue.
                    current.push_str(&buffer);
                }
                continue;
            } else {
                // Not a dollar-quote, just a literal $ character.
                current.push('$');
                current.push_str(&tag[1..]);
                continue;
            }
        }

        // Statement separator.
        if ch == ';' {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                statements.push(trimmed.to_string());
            }
            current.clear();
            continue;
        }

        current.push(ch);
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        statements.push(trimmed.to_string());
    }

    statements
}

#[cfg(test)]
mod split_sql_statements_tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(split_sql_statements(""), Vec::<String>::new());
        assert_eq!(split_sql_statements("   "), Vec::<String>::new());
        assert_eq!(split_sql_statements("\n\n"), Vec::<String>::new());
    }

    #[test]
    fn test_single_statement() {
        let result = split_sql_statements("SELECT 1");
        assert_eq!(result, vec!["SELECT 1"]);
    }

    #[test]
    fn test_multiple_statements() {
        let result = split_sql_statements("SELECT 1; SELECT 2; SELECT 3");
        assert_eq!(result, vec!["SELECT 1", "SELECT 2", "SELECT 3"]);
    }

    #[test]
    fn test_trailing_semicolon() {
        let result = split_sql_statements("SELECT 1;");
        assert_eq!(result, vec!["SELECT 1"]);
    }

    #[test]
    fn test_line_comment() {
        // Semicolon in line comment should not split
        let result = split_sql_statements("SELECT 1; -- comment with ; inside\nSELECT 2");
        assert_eq!(
            result,
            vec!["SELECT 1", "-- comment with ; inside\nSELECT 2"]
        );
    }

    #[test]
    fn test_block_comment() {
        // Semicolon in block comment should not split
        let result = split_sql_statements("SELECT 1; /* comment; with; semicolons */ SELECT 2");
        assert_eq!(
            result,
            vec!["SELECT 1", "/* comment; with; semicolons */ SELECT 2"]
        );
    }

    #[test]
    fn test_single_quoted_string() {
        // Semicolon in string should not split
        let result = split_sql_statements("INSERT INTO t VALUES ('a;b;c'); SELECT 1");
        assert_eq!(result, vec!["INSERT INTO t VALUES ('a;b;c')", "SELECT 1"]);
    }

    #[test]
    fn test_single_quoted_string_with_escape() {
        // Escaped single quote '' should not end string
        let result = split_sql_statements("INSERT INTO t VALUES ('it''s;ok'); SELECT 1");
        assert_eq!(
            result,
            vec!["INSERT INTO t VALUES ('it''s;ok')", "SELECT 1"]
        );
    }

    #[test]
    fn test_dollar_quoted_string() {
        // Dollar-quoted string $$ ... $$
        let result = split_sql_statements("SELECT $$text;with;semicolons$$; SELECT 2");
        assert_eq!(result, vec!["SELECT $$text;with;semicolons$$", "SELECT 2"]);
    }

    #[test]
    fn test_dollar_quoted_string_with_tag() {
        // Dollar-quoted string with tag $tag$ ... $tag$
        let result = split_sql_statements("SELECT $tag$content;here$tag$; SELECT 2");
        assert_eq!(result, vec!["SELECT $tag$content;here$tag$", "SELECT 2"]);
    }

    #[test]
    fn test_mixed_constructs() {
        let sql = r#"CREATE FUNCTION f() RETURNS void AS $$
BEGIN
    -- comment; here
    SELECT 'string; value';
    /* block; comment */
END;
$$ LANGUAGE plpgsql;
SELECT 1;"#;
        let result = split_sql_statements(sql);
        assert_eq!(result.len(), 2);
        assert!(result[0].starts_with("CREATE FUNCTION"));
        assert_eq!(result[1], "SELECT 1");
    }

    #[test]
    fn test_whitespace_handling() {
        let result = split_sql_statements("  SELECT 1  ;  SELECT 2  ");
        assert_eq!(result, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_consecutive_semicolons() {
        let result = split_sql_statements("SELECT 1;; SELECT 2");
        assert_eq!(result, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn test_nested_block_comments() {
        // Note: SQL standard doesn't support nested block comments
        // but we handle them as sequential
        let result = split_sql_statements("/* outer /* inner */ */ SELECT 1");
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_complex_postgres_function() {
        let sql = r#"CREATE OR REPLACE FUNCTION test_fn() RETURNS text AS $func$
DECLARE
    msg text := 'Hello; World';
BEGIN
    -- This is a comment; with semicolon
    RETURN msg;
END;
$func$ LANGUAGE plpgsql;
GRANT EXECUTE ON FUNCTION test_fn() TO PUBLIC;"#;
        let result = split_sql_statements(sql);
        assert_eq!(result.len(), 2);
        assert!(result[0].contains("CREATE OR REPLACE FUNCTION"));
        assert!(result[1].starts_with("GRANT"));
    }
}
