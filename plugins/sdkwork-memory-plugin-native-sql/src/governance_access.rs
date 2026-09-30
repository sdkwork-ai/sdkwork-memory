//! Bounded governance facts for service-owned authorization and quota policy.

use crate::sqlx_compat as sqlx;
use async_trait::async_trait;
use sdkwork_memory_spi::{
    CountActiveMemoryRecordsQuery, CountUserOwnedMemorySpacesQuery, MemoryActorSpaceBindingFact,
    MemoryCapabilityBindingFact, MemoryGovernanceAccessPort, MemorySpaceGovernanceFact,
    MemorySpaceGovernanceFacts, MemorySpiError, MemorySpiResult, ResolveMemorySpaceGovernanceQuery,
    MAX_MEMORY_GOVERNANCE_FACTS,
};
use sqlx::Row;

use crate::store::{NativeSqlMemoryStore, NativeSqlStoreError};

const GOVERNANCE_PORT: &str = "MemoryGovernanceAccessPort";

impl NativeSqlMemoryStore {
    async fn resolve_bounded_space_governance(
        &self,
        query: &ResolveMemorySpaceGovernanceQuery,
    ) -> Result<MemorySpaceGovernanceFacts, NativeSqlStoreError> {
        let fact_limit = validate_fact_limit(query.fact_limit)?;
        if query.scope.tenant_id < 0 || query.scope.space_id < 0 {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "governance scope identifiers must be non-negative".to_string(),
            });
        }
        if query
            .capability_code
            .as_deref()
            .is_some_and(|capability_code| capability_code.trim().is_empty())
        {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: "governance capability code must not be blank".to_string(),
            });
        }

        let mut transaction = self.pool().begin().await?;
        let space = sqlx::query(
            r#"
            SELECT id, organization_id, owner_subject_type, owner_subject_id, lifecycle_status
            FROM ai_space
            WHERE tenant_id = ? AND id = ?
            "#,
        )
        .bind(query.scope.tenant_id)
        .bind(query.scope.space_id)
        .fetch_optional(&mut *transaction)
        .await?
        .map(|row| MemorySpaceGovernanceFact {
            space_id: row.get("id"),
            organization_id: row.get("organization_id"),
            owner_subject_type: row.get("owner_subject_type"),
            owner_subject_id: row.get("owner_subject_id"),
            lifecycle_status: row.get("lifecycle_status"),
        });

        let mut complete = true;
        let actor_bindings = if let Some(actor) = &query.actor {
            if actor.subject_id.trim().is_empty()
                || actor
                    .subject_type
                    .as_deref()
                    .is_some_and(|subject_type| subject_type.trim().is_empty())
            {
                return Err(NativeSqlStoreError::InvariantViolation {
                    message: "governance actor type and id must not be blank".to_string(),
                });
            }
            if actor.subject_type.is_none() {
                let subject_count = sqlx::query_scalar::<_, i64>(
                    r#"
                    SELECT COUNT(*)
                    FROM ai_subject
                    WHERE tenant_id = ?
                      AND subject_ref = ?
                      AND status = 'active'
                      AND deleted_at IS NULL
                    "#,
                )
                .bind(query.scope.tenant_id)
                .bind(&actor.subject_id)
                .fetch_one(&mut *transaction)
                .await?;
                if subject_count > 1 {
                    complete = false;
                }
            }
            let rows = sqlx::query(
                r#"
                SELECT b.uuid, b.binding_kind, b.binding_role, b.status,
                       b.valid_from, b.valid_to
                FROM ai_memory_binding b
                INNER JOIN ai_subject s
                  ON s.tenant_id = b.tenant_id
                 AND s.id = b.source_subject_id
                 AND (? IS NULL OR s.subject_type = ?)
                 AND s.subject_ref = ?
                 AND s.status = 'active'
                 AND s.deleted_at IS NULL
                WHERE b.tenant_id = ?
                  AND b.target_space_id = ?
                  AND b.deleted_at IS NULL
                ORDER BY b.uuid ASC
                LIMIT ?
                "#,
            )
            .bind(actor.subject_type.as_deref())
            .bind(actor.subject_type.as_deref())
            .bind(&actor.subject_id)
            .bind(query.scope.tenant_id)
            .bind(query.scope.space_id)
            .bind(fact_limit + 1)
            .fetch_all(&mut *transaction)
            .await?;
            if rows.len() > fact_limit as usize {
                complete = false;
            }
            rows.into_iter()
                .take(fact_limit as usize)
                .map(|row| MemoryActorSpaceBindingFact {
                    binding_id: row.get("uuid"),
                    binding_kind: row.get("binding_kind"),
                    binding_role: row.get("binding_role"),
                    status: row.get("status"),
                    valid_from: row.get("valid_from"),
                    valid_to: row.get("valid_to"),
                })
                .collect()
        } else {
            Vec::new()
        };

        let capability_bindings = if let Some(capability_code) = &query.capability_code {
            // A capability may be bound to the space (governing every actor in
            // it) or directly to the actor's subject row. The subject branch
            // resolves the actor the same way the binding facts above do —
            // `ai_subject.subject_ref` (plus the trusted subject type when the
            // context carries one) — with the binding's `target_id` joined as
            // `ai_subject.id`; the create side stores exactly that internal
            // numeric id, and a subject's uuid is the same numeric string. A
            // binding targeting any other subject therefore never applies.
            // Without an actor the NULL subject_ref join excludes the subject
            // branch entirely. The combined `LIMIT fact_limit + 1` keeps the
            // truncation semantics of the previous space-only query intact,
            // and `idx_ai_capability_target (tenant_id, target_type, ...)`
            // stays usable for both OR branches.
            let actor = query.actor.as_ref();
            let actor_subject_ref = actor.map(|actor| actor.subject_id.as_str());
            let actor_subject_type = actor.and_then(|actor| actor.subject_type.as_deref());
            let rows = sqlx::query(
                r#"
                SELECT cb.uuid, cb.capability_code, cb.mode, cb.priority, cb.status,
                       cb.valid_from, cb.valid_to
                FROM ai_capability_binding cb
                LEFT JOIN ai_subject s
                  ON s.tenant_id = cb.tenant_id
                 AND s.id = cb.target_id
                 AND (? IS NULL OR s.subject_type = ?)
                 AND s.subject_ref = ?
                 AND s.status = 'active'
                 AND s.deleted_at IS NULL
                WHERE cb.tenant_id = ?
                  AND cb.capability_code = ?
                  AND cb.deleted_at IS NULL
                  AND (
                    (cb.target_type = 'space' AND cb.target_id = ?)
                    OR (cb.target_type = 'subject' AND s.id IS NOT NULL)
                  )
                ORDER BY cb.priority DESC, cb.uuid ASC
                LIMIT ?
                "#,
            )
            .bind(actor_subject_type)
            .bind(actor_subject_type)
            .bind(actor_subject_ref)
            .bind(query.scope.tenant_id)
            .bind(capability_code)
            .bind(query.scope.space_id)
            .bind(fact_limit + 1)
            .fetch_all(&mut *transaction)
            .await?;
            if rows.len() > fact_limit as usize {
                complete = false;
            }
            rows.into_iter()
                .take(fact_limit as usize)
                .map(|row| MemoryCapabilityBindingFact {
                    binding_id: row.get("uuid"),
                    capability_code: row.get("capability_code"),
                    mode: row.get("mode"),
                    priority: row.get("priority"),
                    status: row.get("status"),
                    valid_from: row.get("valid_from"),
                    valid_to: row.get("valid_to"),
                })
                .collect()
        } else {
            Vec::new()
        };

        transaction.commit().await?;
        Ok(MemorySpaceGovernanceFacts {
            space,
            actor_bindings,
            capability_bindings,
            complete,
        })
    }
}

#[async_trait]
impl MemoryGovernanceAccessPort for NativeSqlMemoryStore {
    fn supports_bounded_governance_access(&self) -> bool {
        true
    }

    async fn resolve_space_governance(
        &self,
        query: ResolveMemorySpaceGovernanceQuery,
    ) -> MemorySpiResult<MemorySpaceGovernanceFacts> {
        self.resolve_bounded_space_governance(&query)
            .await
            .map_err(governance_port_error)
    }

    async fn count_active_records(
        &self,
        query: CountActiveMemoryRecordsQuery,
    ) -> MemorySpiResult<u64> {
        let count = self
            .count_active_records_for_scope(&query.scope)
            .await
            .map_err(governance_port_error)?;
        u64::try_from(count).map_err(|_| MemorySpiError::PortOperationFailed {
            port: GOVERNANCE_PORT.to_string(),
            message: "active memory count must not be negative".to_string(),
        })
    }

    async fn count_user_owned_spaces(
        &self,
        query: CountUserOwnedMemorySpacesQuery,
    ) -> MemorySpiResult<u64> {
        let count = self
            .count_user_owned_spaces_for_tenant(query.tenant_id, &query.owner_subject_id)
            .await
            .map_err(governance_port_error)?;
        u64::try_from(count).map_err(|_| MemorySpiError::PortOperationFailed {
            port: GOVERNANCE_PORT.to_string(),
            message: "user-owned memory space count must not be negative".to_string(),
        })
    }
}

fn validate_fact_limit(fact_limit: u32) -> Result<i64, NativeSqlStoreError> {
    if fact_limit == 0 || fact_limit > MAX_MEMORY_GOVERNANCE_FACTS {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: format!(
                "governance fact limit must be between 1 and {MAX_MEMORY_GOVERNANCE_FACTS}"
            ),
        });
    }
    Ok(i64::from(fact_limit))
}

fn governance_port_error(error: NativeSqlStoreError) -> MemorySpiError {
    MemorySpiError::PortOperationFailed {
        port: GOVERNANCE_PORT.to_string(),
        message: error.to_string(),
    }
}
