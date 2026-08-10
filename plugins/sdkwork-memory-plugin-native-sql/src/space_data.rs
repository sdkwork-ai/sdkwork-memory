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

/// PostgreSQL locks one stable migration row with `FOR UPDATE`. SQLite performs
/// a no-op update as the transaction's first write, acquiring the database
/// writer lock before the quota count. This deliberately serializes all space
/// creation until a per-owner quota ledger is introduced through a reviewed
/// schema migration.
async fn lock_space_quota_serialization_row(
    dialect: MemorySqlDialect,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
) -> Result<(), NativeSqlStoreError> {
    let locked =
        match dialect {
            MemorySqlDialect::Postgres => sqlx::query(
                "SELECT version FROM ops_memory_schema_version WHERE version = ? FOR UPDATE",
            )
            .bind(SPACE_QUOTA_LOCK_VERSION)
            .fetch_optional(&mut **tx)
            .await?
            .is_some(),
            MemorySqlDialect::Sqlite => sqlx::query(
                "UPDATE ops_memory_schema_version SET applied_at = applied_at WHERE version = ?",
            )
            .bind(SPACE_QUOTA_LOCK_VERSION)
            .execute(&mut **tx)
            .await?
            .rows_affected()
                == 1,
        };

    if !locked {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: format!(
                "space quota serialization row {SPACE_QUOTA_LOCK_VERSION} is not installed"
            ),
        });
    }
    Ok(())
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
