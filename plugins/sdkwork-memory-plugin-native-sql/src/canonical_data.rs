//! Coarse-grained canonical memory mutations with durable journal side effects.

use crate::sqlx_compat as sqlx;
use sdkwork_memory_spi::{
    CreateCanonicalMemoryCommand, DeleteCanonicalMemoryCommand, MemoryCanonicalRecord,
    MemoryDeletionReceipt, MemoryMutationJournal, MemoryRecordQuotaAdmission, MemoryScopeContext,
    SupersedeCanonicalMemoryAtomicCommand, UpdateCanonicalMemoryCommand,
};
use serde_json::Value;
use sqlx::{any::AnyRow, Row};

use crate::pool_backend::MemorySqlDialect;
use crate::store::{NativeSqlMemoryRecordDetail, NativeSqlMemoryStore, NativeSqlStoreError};

impl NativeSqlMemoryStore {
    pub async fn create_canonical_memory_atomic(
        &self,
        command: &CreateCanonicalMemoryCommand,
    ) -> Result<MemoryCanonicalRecord, NativeSqlStoreError> {
        match self
            .create_canonical_memory_atomic_with_quota(command, 0)
            .await?
        {
            MemoryRecordQuotaAdmission::Admitted(record) => Ok(record),
            MemoryRecordQuotaAdmission::QuotaExceeded { .. } => {
                Err(NativeSqlStoreError::InvariantViolation {
                    message: "unlimited canonical memory mutation was rejected by quota admission"
                        .to_string(),
                })
            }
        }
    }

    pub async fn create_canonical_memory_atomic_with_quota(
        &self,
        command: &CreateCanonicalMemoryCommand,
        max_active_records: u64,
    ) -> Result<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>, NativeSqlStoreError> {
        validate_journal(&command.memory_id, &command.journal)?;
        let mut tx = self.begin_tx().await?;
        let active_records = self
            .lock_space_and_count_active_records_on_tx(&mut tx, &command.scope)
            .await?;
        if max_active_records > 0 && active_records >= max_active_records {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Ok(MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            });
        }
        self.create_record_on_tx(
            &mut tx,
            &command.scope,
            &command.memory_id,
            &command.scope_label,
            &command.memory_type,
            command.subject.as_deref(),
            command.predicate.as_deref(),
            &command.object_text,
            &command.canonical_text,
            &command.sensitivity_level,
            command.expires_at.as_deref(),
        )
        .await?;
        append_journal_on_tx(self, &mut tx, &command.scope, &command.journal).await?;
        sync_record_fts_on_tx(
            self.dialect(),
            &mut tx,
            FtsRecordProjection {
                scope: &command.scope,
                memory_uuid: &command.memory_id,
                canonical_text: &command.canonical_text,
                object_text: &command.object_text,
                subject: command.subject.as_deref(),
                predicate: command.predicate.as_deref(),
            },
        )
        .await?;
        tx.commit().await.map_err(NativeSqlStoreError::from)?;
        let record = self
            .load_canonical_record(&command.scope, &command.memory_id)
            .await?;
        Ok(MemoryRecordQuotaAdmission::Admitted(record))
    }

    pub async fn supersede_canonical_memory_atomic_with_quota(
        &self,
        command: &SupersedeCanonicalMemoryAtomicCommand,
        max_active_records: u64,
    ) -> Result<MemoryRecordQuotaAdmission<MemoryCanonicalRecord>, NativeSqlStoreError> {
        validate_journal(&command.new_memory_id, &command.created_journal)?;
        validate_journal(&command.old_memory_id, &command.superseded_journal)?;
        if command.created_journal.outbox_id == command.superseded_journal.outbox_id
            || command.created_journal.audit_id == command.superseded_journal.audit_id
        {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "supersede journals must use distinct outbox and audit ids".to_string(),
            });
        }
        if command.old_memory_id == command.new_memory_id {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "supersede source and target memory ids must differ".to_string(),
            });
        }

        let mut tx = self.begin_tx().await?;
        self.lock_space_on_tx(&mut tx, &command.scope).await?;
        let old_row = match self.dialect() {
            MemorySqlDialect::Postgres => {
                sqlx::query(
                    r#"
                    SELECT id, status, superseded_by_memory_id
                    FROM ai_record
                    WHERE tenant_id = ? AND space_id = ? AND uuid = ?
                    FOR UPDATE
                    "#,
                )
                .bind(command.scope.tenant_id)
                .bind(command.scope.space_id)
                .bind(&command.old_memory_id)
                .fetch_optional(&mut *tx)
                .await?
            }
            MemorySqlDialect::Sqlite => {
                sqlx::query(
                    "UPDATE ai_record SET version = version WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
                )
                .bind(command.scope.tenant_id)
                .bind(command.scope.space_id)
                .bind(&command.old_memory_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "SELECT id, status, superseded_by_memory_id FROM ai_record WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
                )
                .bind(command.scope.tenant_id)
                .bind(command.scope.space_id)
                .bind(&command.old_memory_id)
                .fetch_optional(&mut *tx)
                .await?
            }
        };
        let Some(old_row) = old_row else {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "supersede source memory {} not found",
                    command.old_memory_id
                ),
            });
        };
        let old_row_id: i64 = old_row.get("id");
        let old_status: String = old_row.get("status");
        let old_superseded_by: Option<i64> = old_row.try_get("superseded_by_memory_id")?;

        let existing_new = sqlx::query(
            r#"
            SELECT id, status, supersedes_memory_id, superseded_by_memory_id,
                   user_id, scope, memory_type, subject, predicate,
                   object_text, canonical_text, sensitivity_level, expires_at
            FROM ai_record
            WHERE tenant_id = ? AND space_id = ? AND uuid = ?
            "#,
        )
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(&command.new_memory_id)
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing_new) = existing_new {
            let existing_new_id: i64 = existing_new.get("id");
            let existing_status: String = existing_new.get("status");
            let existing_supersedes: Option<i64> = existing_new.try_get("supersedes_memory_id")?;
            let existing_superseded_by: Option<i64> =
                existing_new.try_get("superseded_by_memory_id")?;
            if existing_supersedes == Some(old_row_id)
                && existing_superseded_by.is_none()
                && old_superseded_by == Some(existing_new_id)
                && old_status == "superseded"
                && existing_status == "active"
            {
                let record_matches = supersede_target_matches_command(&existing_new, command)?;
                let journals_match = supersede_journals_match(
                    &mut tx,
                    &command.scope,
                    &command.created_journal,
                    &command.superseded_journal,
                )
                .await?;
                if !record_matches || !journals_match {
                    tx.rollback().await.map_err(NativeSqlStoreError::from)?;
                    return Err(NativeSqlStoreError::IdempotencyConflict {
                        idempotency_key: command.new_memory_id.clone(),
                    });
                }
                tx.rollback().await.map_err(NativeSqlStoreError::from)?;
                let record = self
                    .load_canonical_record(&command.scope, &command.new_memory_id)
                    .await?;
                return Ok(MemoryRecordQuotaAdmission::Admitted(record));
            }
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "supersede target memory {} already exists with an incompatible chain",
                    command.new_memory_id
                ),
            });
        }

        if old_status != "active" {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!(
                    "supersede source memory {} is not active",
                    command.old_memory_id
                ),
            });
        }

        let active_records: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM ai_record
            WHERE tenant_id = ?
              AND space_id = ?
              AND status <> 'deleted'
            "#,
        )
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .fetch_one(&mut *tx)
        .await?;
        let active_records =
            u64::try_from(active_records).map_err(|_| NativeSqlStoreError::InvariantViolation {
                message: "active memory count for supersede was negative".to_string(),
            })?;
        if max_active_records > 0 && active_records >= max_active_records {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Ok(MemoryRecordQuotaAdmission::QuotaExceeded {
                active_records,
                max_active_records,
            });
        }

        self.create_record_on_tx(
            &mut tx,
            &command.scope,
            &command.new_memory_id,
            &command.scope_label,
            &command.memory_type,
            command.subject.as_deref(),
            command.predicate.as_deref(),
            &command.object_text,
            &command.canonical_text,
            &command.sensitivity_level,
            command.expires_at.as_deref(),
        )
        .await?;
        let new_row_id: i64 = sqlx::query_scalar(
            "SELECT id FROM ai_record WHERE tenant_id = ? AND space_id = ? AND uuid = ?",
        )
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .bind(&command.new_memory_id)
        .fetch_one(&mut *tx)
        .await?;
        let timestamp = crate::store::now_text();
        sqlx::query(
            r#"
            UPDATE ai_record
            SET status = 'superseded',
                superseded_by_memory_id = ?,
                updated_at = ?,
                version = version + 1
            WHERE id = ? AND tenant_id = ? AND space_id = ?
            "#,
        )
        .bind(new_row_id)
        .bind(&timestamp)
        .bind(old_row_id)
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE ai_record
            SET supersedes_memory_id = ?,
                updated_at = ?,
                version = version + 1
            WHERE id = ? AND tenant_id = ? AND space_id = ?
            "#,
        )
        .bind(old_row_id)
        .bind(&timestamp)
        .bind(new_row_id)
        .bind(command.scope.tenant_id)
        .bind(command.scope.space_id)
        .execute(&mut *tx)
        .await?;
        append_journal_on_tx(self, &mut tx, &command.scope, &command.superseded_journal).await?;
        append_journal_on_tx(self, &mut tx, &command.scope, &command.created_journal).await?;
        sync_record_fts_on_tx(
            self.dialect(),
            &mut tx,
            FtsRecordProjection {
                scope: &command.scope,
                memory_uuid: &command.new_memory_id,
                canonical_text: &command.canonical_text,
                object_text: &command.object_text,
                subject: command.subject.as_deref(),
                predicate: command.predicate.as_deref(),
            },
        )
        .await?;
        remove_record_fts_on_tx(
            self.dialect(),
            &mut tx,
            &command.scope,
            &command.old_memory_id,
        )
        .await?;
        tx.commit().await.map_err(NativeSqlStoreError::from)?;
        let record = self
            .load_canonical_record(&command.scope, &command.new_memory_id)
            .await?;
        Ok(MemoryRecordQuotaAdmission::Admitted(record))
    }

    pub async fn update_canonical_memory_atomic(
        &self,
        command: &UpdateCanonicalMemoryCommand,
    ) -> Result<Option<MemoryCanonicalRecord>, NativeSqlStoreError> {
        validate_journal(&command.memory_id, &command.journal)?;
        let mut tx = self.begin_tx().await?;
        let updated = Self::update_record_on_tx(
            &mut tx,
            &command.scope,
            &command.memory_id,
            command.canonical_text.as_deref(),
            command.subject.as_deref(),
        )
        .await?;
        if !updated {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Ok(None);
        }
        append_journal_on_tx(self, &mut tx, &command.scope, &command.journal).await?;
        if matches!(self.dialect(), MemorySqlDialect::Sqlite) {
            let row = sqlx::query(
                r#"
                SELECT canonical_text, object_text, subject, predicate
                FROM ai_record
                WHERE tenant_id = ? AND space_id = ? AND uuid = ? AND status <> 'deleted'
                "#,
            )
            .bind(command.scope.tenant_id)
            .bind(command.scope.space_id)
            .bind(&command.memory_id)
            .fetch_one(&mut *tx)
            .await?;
            let canonical_text: String = row.get("canonical_text");
            let object_text: String = row.get("object_text");
            let subject: Option<String> = row.get("subject");
            let predicate: Option<String> = row.get("predicate");
            sync_record_fts_on_tx(
                self.dialect(),
                &mut tx,
                FtsRecordProjection {
                    scope: &command.scope,
                    memory_uuid: &command.memory_id,
                    canonical_text: &canonical_text,
                    object_text: &object_text,
                    subject: subject.as_deref(),
                    predicate: predicate.as_deref(),
                },
            )
            .await?;
        }
        tx.commit().await.map_err(NativeSqlStoreError::from)?;

        let record = self
            .load_canonical_record(&command.scope, &command.memory_id)
            .await?;
        Ok(Some(record))
    }

    pub async fn delete_canonical_memory_atomic(
        &self,
        command: &DeleteCanonicalMemoryCommand,
    ) -> Result<MemoryDeletionReceipt, NativeSqlStoreError> {
        validate_journal(&command.memory_id, &command.journal)?;
        let mut tx = self.begin_tx().await?;
        let deleted =
            Self::mark_record_deleted_on_tx(&mut tx, &command.scope, &command.memory_id).await?;
        if !deleted {
            tx.rollback().await.map_err(NativeSqlStoreError::from)?;
            return Ok(MemoryDeletionReceipt {
                memory_id: command.memory_id.clone(),
                deleted: false,
                already_deleted: false,
            });
        }
        append_journal_on_tx(self, &mut tx, &command.scope, &command.journal).await?;
        remove_record_fts_on_tx(self.dialect(), &mut tx, &command.scope, &command.memory_id)
            .await?;
        tx.commit().await.map_err(NativeSqlStoreError::from)?;
        Ok(MemoryDeletionReceipt {
            memory_id: command.memory_id.clone(),
            deleted: true,
            already_deleted: false,
        })
    }

    pub async fn retrieve_canonical_memory(
        &self,
        scope: &sdkwork_memory_spi::MemoryScopeContext,
        memory_id: &str,
    ) -> Result<Option<MemoryCanonicalRecord>, NativeSqlStoreError> {
        self.retrieve_record_detail(scope, memory_id)
            .await
            .map(|record| record.map(into_canonical_record))
    }

    async fn load_canonical_record(
        &self,
        scope: &sdkwork_memory_spi::MemoryScopeContext,
        memory_id: &str,
    ) -> Result<MemoryCanonicalRecord, NativeSqlStoreError> {
        self.retrieve_canonical_memory(scope, memory_id)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: format!("canonical memory {memory_id} was not readable after mutation"),
            })
    }
}

fn supersede_target_matches_command(
    row: &AnyRow,
    command: &SupersedeCanonicalMemoryAtomicCommand,
) -> Result<bool, NativeSqlStoreError> {
    let stored_user_id: Option<i64> = row.try_get("user_id")?;
    let stored_scope: String = row.get("scope");
    let stored_memory_type: String = row.get("memory_type");
    let stored_subject: Option<String> = row.try_get("subject")?;
    let stored_predicate: Option<String> = row.try_get("predicate")?;
    let stored_object_text: String = row.get("object_text");
    let stored_canonical_text: String = row.get("canonical_text");
    let stored_sensitivity_level: String = row.get("sensitivity_level");
    let stored_expires_at: Option<String> = row.try_get("expires_at")?;

    Ok(stored_user_id == command.scope.user_id
        && stored_scope == command.scope_label
        && stored_memory_type == command.memory_type
        && stored_subject.as_deref() == command.subject.as_deref()
        && stored_predicate.as_deref() == Some(command.predicate.as_deref().unwrap_or("is"))
        && stored_object_text == command.object_text
        && stored_canonical_text == command.canonical_text
        && stored_sensitivity_level == command.sensitivity_level
        && stored_expires_at == command.expires_at)
}

async fn supersede_journals_match(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    scope: &MemoryScopeContext,
    created: &MemoryMutationJournal,
    superseded: &MemoryMutationJournal,
) -> Result<bool, NativeSqlStoreError> {
    for journal in [created, superseded] {
        let Some(outbox) = sqlx::query(
            r#"
            SELECT aggregate_type, aggregate_id, event_type, event_version, payload_json
            FROM ai_outbox_event
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(&journal.outbox_id)
        .fetch_optional(&mut **tx)
        .await?
        else {
            return Ok(false);
        };
        let outbox_matches = outbox.get::<String, _>("aggregate_type") == journal.aggregate_type
            && outbox.get::<String, _>("aggregate_id") == journal.aggregate_id
            && outbox.get::<String, _>("event_type") == journal.event_type
            && outbox.get::<String, _>("event_version") == journal.event_version
            && journal_payload_matches(
                &outbox.get::<String, _>("payload_json"),
                &journal.payload_json,
            );
        if !outbox_matches {
            return Ok(false);
        }

        let Some(audit) = sqlx::query(
            r#"
            SELECT action, resource_type, resource_id, result
            FROM ai_audit_log
            WHERE tenant_id = ? AND uuid = ?
            "#,
        )
        .bind(scope.tenant_id)
        .bind(&journal.audit_id)
        .fetch_optional(&mut **tx)
        .await?
        else {
            return Ok(false);
        };
        let audit_matches = audit.get::<String, _>("action") == journal.audit_action
            && audit.get::<String, _>("resource_type") == journal.audit_resource_type
            && audit.get::<Option<String>, _>("resource_id")
                == Some(journal.audit_resource_id.clone())
            && audit.get::<String, _>("result") == journal.audit_result;
        if !audit_matches {
            return Ok(false);
        }
    }
    Ok(true)
}

fn journal_payload_matches(stored: &str, expected: &str) -> bool {
    match (
        serde_json::from_str::<Value>(stored),
        serde_json::from_str::<Value>(expected),
    ) {
        (Ok(stored), Ok(expected)) => stored == expected,
        _ => stored == expected,
    }
}

pub(crate) struct FtsRecordProjection<'a> {
    pub(crate) scope: &'a MemoryScopeContext,
    pub(crate) memory_uuid: &'a str,
    pub(crate) canonical_text: &'a str,
    pub(crate) object_text: &'a str,
    pub(crate) subject: Option<&'a str>,
    pub(crate) predicate: Option<&'a str>,
}

pub(crate) async fn sync_record_fts_on_tx(
    dialect: MemorySqlDialect,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    record: FtsRecordProjection<'_>,
) -> Result<(), NativeSqlStoreError> {
    if !matches!(dialect, MemorySqlDialect::Sqlite) {
        return Ok(());
    }
    let row_id: i64 =
        sqlx::query("SELECT id FROM ai_record WHERE tenant_id = ? AND space_id = ? AND uuid = ?")
            .bind(record.scope.tenant_id)
            .bind(record.scope.space_id)
            .bind(record.memory_uuid)
            .fetch_one(&mut **tx)
            .await?
            .get("id");
    sqlx::query("DELETE FROM ai_record_fts WHERE rowid = ?")
        .bind(row_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO ai_record_fts(
          rowid, memory_uuid, tenant_id, space_id, canonical_text, object_text, subject, predicate
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(row_id)
    .bind(record.memory_uuid)
    .bind(record.scope.tenant_id)
    .bind(record.scope.space_id)
    .bind(record.canonical_text)
    .bind(record.object_text)
    .bind(record.subject.unwrap_or(""))
    .bind(record.predicate.unwrap_or(""))
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn remove_record_fts_on_tx(
    dialect: MemorySqlDialect,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    scope: &sdkwork_memory_spi::MemoryScopeContext,
    memory_uuid: &str,
) -> Result<(), NativeSqlStoreError> {
    if !matches!(dialect, MemorySqlDialect::Sqlite) {
        return Ok(());
    }
    sqlx::query(
        "DELETE FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
    )
    .bind(scope.tenant_id)
    .bind(scope.space_id)
    .bind(memory_uuid)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn append_journal_on_tx(
    store: &NativeSqlMemoryStore,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    scope: &sdkwork_memory_spi::MemoryScopeContext,
    journal: &MemoryMutationJournal,
) -> Result<(), NativeSqlStoreError> {
    store.append_outbox_on_tx(tx, scope, journal).await?;
    store.append_audit_on_tx(tx, scope, journal).await
}

pub(crate) fn validate_journal(
    memory_id: &str,
    journal: &MemoryMutationJournal,
) -> Result<(), NativeSqlStoreError> {
    if journal.aggregate_id != memory_id || journal.audit_resource_id != memory_id {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: "memory mutation journal resource ids must match the canonical memory id"
                .to_string(),
        });
    }
    Ok(())
}

pub(crate) fn into_canonical_record(row: NativeSqlMemoryRecordDetail) -> MemoryCanonicalRecord {
    MemoryCanonicalRecord {
        memory_id: row.memory_id,
        space_id: row.space_id,
        user_id: row.user_id,
        scope_label: row.scope,
        memory_type: row.memory_type,
        subject: row.subject,
        predicate: row.predicate,
        object_text: row.object_text,
        canonical_text: row.canonical_text,
        confidence: row.confidence,
        evidence_count: row.evidence_count.unwrap_or(0),
        contradiction_count: row.contradiction_count.unwrap_or(0),
        status: row.status,
        sensitivity_level: row.sensitivity_level,
        supersedes_memory_id: row.supersedes_memory_id,
        superseded_by_memory_id: row.superseded_by_memory_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
        version: row.version,
        expires_at: row.expires_at,
    }
}
