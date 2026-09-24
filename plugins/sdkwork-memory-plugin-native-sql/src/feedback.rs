//! Durable user feedback records.
//!
//! PRD names retrieval feedback as a first-class capability. The governance
//! audit row alone cannot answer "what feedback did this tenant give on this
//! target", so each accepted feedback request also lands here as the
//! queryable signal record.

use crate::sqlx_compat as sqlx;
use serde_json::Value;

use sdkwork_memory_spi::MemoryScopeContext;

use crate::store::{now_text, NativeSqlMemoryStore, NativeSqlStoreError};

pub struct NativeSqlInsertFeedbackCommand<'a> {
    pub feedback_id: &'a str,
    pub actor_id: Option<&'a str>,
    pub target_type: &'a str,
    pub target_id: i64,
    pub feedback_type: &'a str,
    pub rating: Option<i32>,
    pub comment: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

impl NativeSqlMemoryStore {
    pub async fn insert_feedback_record(
        &self,
        scope: &MemoryScopeContext,
        command: NativeSqlInsertFeedbackCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        if let Some(metadata) = command.metadata_json {
            let parsed: Value = serde_json::from_str(metadata)?;
            debug_assert!(parsed.is_object() || parsed.is_null());
        }
        sqlx::query(
            r#"
            INSERT INTO ai_feedback (
              id, uuid, tenant_id, space_id, actor_id, target_type, target_id,
              feedback_type, rating, comment, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(command.feedback_id)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(command.actor_id)
        .bind(command.target_type)
        .bind(command.target_id)
        .bind(command.feedback_type)
        .bind(command.rating)
        .bind(command.comment)
        .bind(command.metadata_json)
        .bind(now_text())
        .execute(self.pool())
        .await
        .map_err(|error| {
            // A retried request with the same feedback id is an idempotent
            // hit, not an error; any other duplicate shape stays a conflict.
            if crate::store::is_unique_violation(&error) {
                return NativeSqlStoreError::InvariantViolation {
                    message: format!(
                        "feedback record {} already exists for this tenant",
                        command.feedback_id
                    ),
                };
            }
            error.into()
        })?;
        Ok(())
    }
}
