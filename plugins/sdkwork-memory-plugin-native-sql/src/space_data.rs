//! Atomic memory-space mutations and user-owned-space quota admission.

use crate::sqlx_compat as sqlx;
use async_trait::async_trait;
use sdkwork_memory_spi::{
    CreateMemorySpaceCommand, MemorySpaceQuotaAdmission, MemorySpaceRecord, MemorySpaceStorePort,
    MemorySpiError, MemorySpiResult,
};

use crate::pool_backend::MemorySqlDialect;
use crate::store::{now_text, NativeSqlMemoryStore, NativeSqlStoreError};

const SPACE_QUOTA_LOCK_VERSION: &str = "0001";
const SPACE_STORE_PORT: &str = "MemorySpaceStorePort";

impl NativeSqlMemoryStore {
    /// Startup probe for pools adopted through `from_database_pool`: those skip
    /// the embedded migration runner (`apply_migration = false`), so a SQLite
    /// database initialized outside this plugin reaches the first
    /// `create_space` without the quota serialization row and fails there.
    /// Verifying the row once at startup turns that into a named, actionable
    /// admission error instead. PostgreSQL serializes quota admission with a
    /// transaction-scoped advisory lock and never depends on the row, so the
    /// probe is a no-op on that dialect.
    pub(crate) async fn ensure_space_quota_serialization_row_installed(
        &self,
    ) -> Result<(), NativeSqlStoreError> {
        if self.dialect() != MemorySqlDialect::Sqlite {
            return Ok(());
        }
        // `schema_is_initialized` treats any probe error as "not initialized"
        // because an adopted database may lack the bookkeeping table entirely;
        // this probe follows the same rule and reports the driver cause inside
        // the named diagnostic rather than leaking it as a database error.
        let installed = match sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM ops_memory_schema_version WHERE version = ? LIMIT 1",
        )
        .bind(SPACE_QUOTA_LOCK_VERSION)
        .fetch_optional(self.pool())
        .await
        {
            Ok(row) => row,
            Err(error) => return Err(missing_space_quota_serialization_row_error(Some(error))),
        };
        if installed.is_none() {
            return Err(missing_space_quota_serialization_row_error(None));
        }
        Ok(())
    }

    pub async fn create_space_atomic_with_quota(
        &self,
        command: &CreateMemorySpaceCommand,
        max_active_spaces: u64,
    ) -> Result<MemorySpaceQuotaAdmission<MemorySpaceRecord>, NativeSqlStoreError> {
        validate_create_space_command(command)?;

        let mut tx = self.begin_tx().await?;
        lock_space_quota_serialization_row(self.dialect(), &mut tx).await?;

        let active_spaces = if command.owner_subject_type == "user" {
            count_active_user_spaces_on_tx(&mut tx, command.tenant_id, &command.owner_subject_id)
                .await?
        } else {
            0
        };
        if command.owner_subject_type == "user"
            && max_active_spaces > 0
            && active_spaces >= max_active_spaces
        {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Ok(MemorySpaceQuotaAdmission::QuotaExceeded {
                active_spaces,
                max_active_spaces,
            });
        }

        let uuid = format!("space-{}", command.space_id);
        let timestamp = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_space (
              id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id,
              space_type, display_name, default_scope, lifecycle_status, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, 0)
            "#,
        )
        .bind(command.space_id)
        .bind(&uuid)
        .bind(command.tenant_id)
        .bind(command.organization_id.unwrap_or(0))
        .bind(&command.owner_subject_type)
        .bind(&command.owner_subject_id)
        .bind(&command.space_type)
        .bind(&command.display_name)
        .bind(&command.default_scope)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&mut *tx)
        .await?;
        tx.commit().await.map_err(NativeSqlStoreError::from)?;

        Ok(MemorySpaceQuotaAdmission::Admitted(MemorySpaceRecord {
            space_id: command.space_id,
            uuid,
            tenant_id: command.tenant_id,
            organization_id: command.organization_id,
            owner_subject_type: command.owner_subject_type.clone(),
            owner_subject_id: command.owner_subject_id.clone(),
            space_type: command.space_type.clone(),
            display_name: command.display_name.clone(),
            default_scope: command.default_scope.clone(),
            lifecycle_status: "active".to_string(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            version: 0,
        }))
    }
}

#[async_trait]
impl MemorySpaceStorePort for NativeSqlMemoryStore {
    fn supports_atomic_user_space_quota_admission(&self) -> bool {
        true
    }

    async fn create_space_atomic_with_quota(
        &self,
        command: CreateMemorySpaceCommand,
        max_active_spaces: u64,
    ) -> MemorySpiResult<MemorySpaceQuotaAdmission<MemorySpaceRecord>> {
        NativeSqlMemoryStore::create_space_atomic_with_quota(self, &command, max_active_spaces)
            .await
            .map_err(space_store_port_error)
    }
}

/// PostgreSQL serializes space creation with a transaction-scoped advisory lock so
/// quota admission never depends on migration-bookkeeping tables inside the
/// application-root lifecycle schema. SQLite performs a no-op update as the
/// transaction's first write, acquiring the database writer lock before the quota
/// count; the version table it touches is guaranteed to exist because every SQLite
/// database is initialized through this plugin's embedded migration runner, and
/// `from_database_pool` verifies the row at startup instead of trusting that
/// invariant. This deliberately serializes all space creation until a per-owner
/// quota ledger is introduced through a reviewed schema migration.
async fn lock_space_quota_serialization_row(
    dialect: MemorySqlDialect,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
) -> Result<(), NativeSqlStoreError> {
    match dialect {
        MemorySqlDialect::Postgres => {
            sqlx::query("SELECT pg_advisory_xact_lock(714895751361736729)")
                .execute(&mut **tx)
                .await?;
            Ok(())
        }
        MemorySqlDialect::Sqlite => {
            let locked = sqlx::query(
                "UPDATE ops_memory_schema_version SET applied_at = applied_at WHERE version = ?",
            )
            .bind(SPACE_QUOTA_LOCK_VERSION)
            .execute(&mut **tx)
            .await?
            .rows_affected()
                == 1;
            if !locked {
                return Err(NativeSqlStoreError::InvariantViolation {
                    message: format!(
                        "space quota serialization row {SPACE_QUOTA_LOCK_VERSION} is not installed"
                    ),
                });
            }
            Ok(())
        }
    }
}

/// Named startup error for a missing quota serialization row. The diagnostic
/// names both remedies: re-initialize the database through the embedded
/// bootstrap, or seed the row manually into `ops_memory_schema_version`.
fn missing_space_quota_serialization_row_error(cause: Option<sqlx::Error>) -> NativeSqlStoreError {
    let cause = cause
        .map(|error| format!(" (probe failed: {error})"))
        .unwrap_or_default();
    NativeSqlStoreError::InvariantViolation {
        message: format!(
            "space quota serialization row {SPACE_QUOTA_LOCK_VERSION} is not installed; this \
             SQLite database was not initialized through this plugin's embedded bootstrap. \
             Initialize it once through the embedded bootstrap \
             (NativeSqlMemoryStore::connect on the same database file), or seed the row with \
             INSERT INTO ops_memory_schema_version (version) VALUES \
             ('{SPACE_QUOTA_LOCK_VERSION}') before reusing an externally initialized pool{cause}"
        ),
    }
}

async fn count_active_user_spaces_on_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    tenant_id: i64,
    owner_subject_id: &str,
) -> Result<u64, NativeSqlStoreError> {
    let count: i64 = sqlx::query_scalar(
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
    .fetch_one(&mut **tx)
    .await?;

    u64::try_from(count).map_err(|_| NativeSqlStoreError::InvariantViolation {
        message: "active user-owned memory space count must not be negative".to_string(),
    })
}

fn validate_create_space_command(
    command: &CreateMemorySpaceCommand,
) -> Result<(), NativeSqlStoreError> {
    if command.tenant_id < 0 || command.space_id < 0 {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: "memory-space tenant and space identifiers must be non-negative".to_string(),
        });
    }
    for (field, value) in [
        ("owner subject type", command.owner_subject_type.as_str()),
        ("owner subject id", command.owner_subject_id.as_str()),
        ("space type", command.space_type.as_str()),
        ("display name", command.display_name.as_str()),
        ("default scope", command.default_scope.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!("memory-space {field} must not be blank"),
            });
        }
    }
    Ok(())
}

fn space_store_port_error(error: NativeSqlStoreError) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: SPACE_STORE_PORT.to_string(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod space_quota_serialization_probe_tests {
    use super::*;
    use sdkwork_database_config::{DatabaseConfig, DatabaseEngine};

    /// Mirrors the `from_database_pool` adopt path: a SQLite pool opened with
    /// `apply_migration = false`, so the embedded bootstrap never ran.
    async fn adopted_unmigrated_store() -> NativeSqlMemoryStore {
        let config = DatabaseConfig {
            engine: DatabaseEngine::Sqlite,
            url: "sqlite::memory:".to_owned(),
            ..DatabaseConfig::default()
        };
        NativeSqlMemoryStore::open_pool(&config, false)
            .await
            .expect("adopt an uninitialized SQLite database")
    }

    #[tokio::test]
    async fn startup_probe_rejects_missing_serialization_row_with_actionable_diagnostic() {
        let store = adopted_unmigrated_store().await;
        let error = store
            .ensure_space_quota_serialization_row_installed()
            .await
            .expect_err("the missing quota serialization row must fail the startup probe");
        match &error {
            NativeSqlStoreError::InvariantViolation { message } => {
                let missing_row = format!(
                    "space quota serialization row {SPACE_QUOTA_LOCK_VERSION} is not installed"
                );
                assert!(
                    message.contains(&missing_row),
                    "diagnostic must name the missing row, got: {message}"
                );
                assert!(
                    message.contains("embedded bootstrap"),
                    "diagnostic must point at the embedded bootstrap remedy, got: {message}"
                );
                assert!(
                    message.contains("ops_memory_schema_version"),
                    "diagnostic must name the table to seed for the manual remedy, got: {message}"
                );
            }
            other => panic!("expected a named store error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn startup_probe_accepts_database_after_the_embedded_bootstrap_seeds_the_row() {
        let store = adopted_unmigrated_store().await;
        assert!(
            store
                .ensure_space_quota_serialization_row_installed()
                .await
                .is_err(),
            "the serialization row is missing before initialization"
        );

        // The exact remedy the diagnostic names: run the embedded bootstrap
        // against the same database, which seeds `ops_memory_schema_version`.
        NativeSqlMemoryStore::install_sqlite_phase1_schema(store.pool())
            .await
            .expect("embedded bootstrap must seed the serialization row");

        store
            .ensure_space_quota_serialization_row_installed()
            .await
            .expect("the seeded serialization row must pass the startup probe");
    }

    #[tokio::test]
    async fn startup_probe_skips_the_postgres_dialect() {
        // PostgreSQL quota admission serializes on a transaction-scoped
        // advisory lock, so the bookkeeping row never gates startup there. The
        // SQLite pool below proves the dialect short-circuit without a
        // PostgreSQL server.
        let store = adopted_unmigrated_store().await;
        let postgres_adopted =
            NativeSqlMemoryStore::from_any_pool(store.pool().clone(), MemorySqlDialect::Postgres)
                .await;
        postgres_adopted
            .ensure_space_quota_serialization_row_installed()
            .await
            .expect("postgres admission must not depend on the bookkeeping row");
    }
}
