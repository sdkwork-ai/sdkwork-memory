//! Graph entity and edge store methods for commercial memory management.

use crate::sqlx_compat as sqlx;
use sdkwork_memory_spi::{MemoryMutationJournal, MemoryScopeContext};
use sdkwork_utils_rust::MAX_LIST_PAGE_SIZE;
use sqlx::Row;

use crate::store::{now_text, NativeSqlMemoryStore, NativeSqlStoreError};

#[derive(Debug, Clone)]
pub struct NativeSqlEntityRow {
    pub id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub space_id: i64,
    pub entity_type: String,
    pub canonical_name: String,
    pub aliases_json: Option<String>,
    pub attributes_json: Option<String>,
    pub sensitivity_level: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct NativeSqlEdgeRow {
    pub id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub space_id: i64,
    pub source_entity_id: i64,
    pub target_entity_id: i64,
    pub source_entity_uuid: String,
    pub target_entity_uuid: String,
    pub relation_type: String,
    pub source_memory_uuid: Option<String>,
    pub weight: Option<f64>,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

pub struct InsertEntityCommand<'a> {
    pub id: i64,
    pub uuid: &'a str,
    pub tenant_id: i64,
    pub space_id: i64,
    pub entity_type: &'a str,
    pub canonical_name: &'a str,
    pub aliases_json: Option<&'a str>,
    pub attributes_json: Option<&'a str>,
    pub sensitivity_level: &'a str,
}

pub struct UpdateEntityCommand<'a> {
    pub canonical_name: Option<&'a str>,
    pub aliases_json: Option<&'a str>,
    pub attributes_json: Option<&'a str>,
    pub sensitivity_level: Option<&'a str>,
    pub status: Option<&'a str>,
}

pub struct InsertEdgeCommand<'a> {
    pub id: i64,
    pub uuid: &'a str,
    pub tenant_id: i64,
    pub space_id: i64,
    pub source_entity_id: i64,
    pub target_entity_id: i64,
    pub relation_type: &'a str,
    /// Internal `ai_record.id` of the canonical memory evidencing this edge;
    /// persisted into `ai_edge.source_memory_id` so deleting that memory can
    /// clear exactly the edges it supported.
    pub source_record_id: Option<i64>,
    pub weight: Option<f64>,
    pub valid_from: Option<&'a str>,
    pub valid_to: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

pub struct UpdateEdgeCommand<'a> {
    pub relation_type: Option<&'a str>,
    pub weight: Option<f64>,
    pub status: Option<&'a str>,
    pub valid_from: Option<&'a str>,
    pub valid_to: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

impl NativeSqlMemoryStore {
    pub async fn insert_entity_with_journal(
        &self,
        cmd: InsertEntityCommand<'_>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        validate_graph_journal(cmd.uuid, scope, cmd.tenant_id, Some(cmd.space_id), journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        sqlx::query(
            r#"
            INSERT INTO ai_entity (
              id, uuid, tenant_id, space_id, entity_type, canonical_name,
              aliases_json, attributes_json, sensitivity_level, status,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, 1)
            "#,
        )
        .bind(cmd.id)
        .bind(cmd.uuid)
        .bind(cmd.tenant_id)
        .bind(cmd.space_id)
        .bind(cmd.entity_type)
        .bind(cmd.canonical_name)
        .bind(cmd.aliases_json)
        .bind(cmd.attributes_json)
        .bind(cmd.sensitivity_level)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_entity_with_journal(
        &self,
        tenant_id: i64,
        entity_uuid: &str,
        cmd: UpdateEntityCommand<'_>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_graph_journal(entity_uuid, scope, tenant_id, None, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_entity
            SET canonical_name = COALESCE(?, canonical_name),
                aliases_json = COALESCE(?, aliases_json),
                attributes_json = COALESCE(?, attributes_json),
                sensitivity_level = COALESCE(?, sensitivity_level),
                status = COALESCE(?, status),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(cmd.canonical_name)
        .bind(cmd.aliases_json)
        .bind(cmd.attributes_json)
        .bind(cmd.sensitivity_level)
        .bind(cmd.status)
        .bind(&now)
        .bind(tenant_id)
        .bind(entity_uuid)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn insert_edge_with_journal(
        &self,
        cmd: InsertEdgeCommand<'_>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        validate_graph_journal(cmd.uuid, scope, cmd.tenant_id, Some(cmd.space_id), journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        sqlx::query(
            r#"
            INSERT INTO ai_edge (
              id, uuid, tenant_id, space_id, source_entity_id, target_entity_id,
              relation_type, source_memory_id, weight, status, valid_from,
              valid_to, metadata_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(cmd.id)
        .bind(cmd.uuid)
        .bind(cmd.tenant_id)
        .bind(cmd.space_id)
        .bind(cmd.source_entity_id)
        .bind(cmd.target_entity_id)
        .bind(cmd.relation_type)
        .bind(cmd.source_record_id)
        .bind(cmd.weight)
        .bind(cmd.valid_from)
        .bind(cmd.valid_to)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_edge_with_journal(
        &self,
        tenant_id: i64,
        edge_uuid: &str,
        cmd: UpdateEdgeCommand<'_>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_graph_journal(edge_uuid, scope, tenant_id, None, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_edge
            SET relation_type = COALESCE(?, relation_type),
                weight = COALESCE(?, weight),
                status = COALESCE(?, status),
                valid_from = COALESCE(?, valid_from),
                valid_to = COALESCE(?, valid_to),
                metadata_json = COALESCE(?, metadata_json),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(cmd.relation_type)
        .bind(cmd.weight)
        .bind(cmd.status)
        .bind(cmd.valid_from)
        .bind(cmd.valid_to)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(tenant_id)
        .bind(edge_uuid)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_edge_with_journal(
        &self,
        tenant_id: i64,
        edge_uuid: &str,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_graph_journal(edge_uuid, scope, tenant_id, None, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_edge
            SET status = 'deleted', updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(&now)
        .bind(tenant_id)
        .bind(edge_uuid)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() > 0 {
            crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        }
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn resolve_entity_internal_id(
        &self,
        tenant_id: i64,
        entity_uuid: &str,
    ) -> Result<i64, NativeSqlStoreError> {
        self.resolve_entity_internal_id_in_space(tenant_id, entity_uuid, None)
            .await
    }

    pub async fn resolve_entity_internal_id_in_space(
        &self,
        tenant_id: i64,
        entity_uuid: &str,
        expected_space_id: Option<i64>,
    ) -> Result<i64, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, space_id
            FROM ai_entity
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .bind(entity_uuid)
        .fetch_optional(self.pool())
        .await?;

        let Some(row) = row else {
            return Err(NativeSqlStoreError::InvariantViolation {
                message: format!("entity {entity_uuid} not found"),
            });
        };
        let entity_space_id: i64 = row.get("space_id");
        if let Some(expected) = expected_space_id {
            if entity_space_id != expected {
                return Err(NativeSqlStoreError::InvariantViolation {
                    message: format!("entity {entity_uuid} does not belong to space {expected}"),
                });
            }
        }
        Ok(row.get("id"))
    }

    pub async fn insert_entity(
        &self,
        cmd: InsertEntityCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        let now = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_entity (
              id, uuid, tenant_id, space_id, entity_type, canonical_name,
              aliases_json, attributes_json, sensitivity_level, status,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, 1)
            "#,
        )
        .bind(cmd.id)
        .bind(cmd.uuid)
        .bind(cmd.tenant_id)
        .bind(cmd.space_id)
        .bind(cmd.entity_type)
        .bind(cmd.canonical_name)
        .bind(cmd.aliases_json)
        .bind(cmd.attributes_json)
        .bind(cmd.sensitivity_level)
        .bind(&now)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn retrieve_entity(
        &self,
        tenant_id: i64,
        entity_uuid: &str,
    ) -> Result<Option<NativeSqlEntityRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, space_id, entity_type, canonical_name,
                   aliases_json, attributes_json, sensitivity_level, status,
                   created_at, updated_at, version
            FROM ai_entity
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .bind(entity_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(map_entity_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_entities(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        entity_type: Option<&str>,
        status: Option<&str>,
        cursor: Option<&str>,
        page_size: i32,
        sensitivity_read_scope: i32,
    ) -> Result<Vec<NativeSqlEntityRow>, NativeSqlStoreError> {
        let limit = page_size.clamp(1, MAX_LIST_PAGE_SIZE) + 1;
        let cursor = cursor.unwrap_or("");
        let status_filter = status.unwrap_or("active");
        let sensitivity_read_scope = sensitivity_read_scope.clamp(
            crate::store::SENSITIVITY_READ_PUBLIC,
            crate::store::SENSITIVITY_READ_OWNER,
        );
        let sensitivity_sql = crate::store::sensitivity_level_filter_sql("e");
        let sql = format!(
            r#"
            SELECT id, uuid, tenant_id, space_id, entity_type, canonical_name,
                   aliases_json, attributes_json, sensitivity_level, status,
                   created_at, updated_at, version
            FROM ai_entity e
            WHERE tenant_id = ?
              AND status <> 'deleted'
              AND (? IS NULL OR space_id = ?)
              AND (? IS NULL OR entity_type = ?)
              AND (? = 'all' OR status = ?)
              AND uuid > ?
              {sensitivity_sql}
            ORDER BY uuid ASC
            LIMIT ?
            "#
        );
        let rows = sqlx::query(&sql)
            .bind(tenant_id)
            .bind(space_id)
            .bind(space_id)
            .bind(entity_type)
            .bind(entity_type)
            .bind(status_filter)
            .bind(status_filter)
            .bind(cursor)
            .bind(sensitivity_read_scope)
            .bind(sensitivity_read_scope)
            .bind(limit)
            .fetch_all(self.pool())
            .await?;
        Ok(rows.into_iter().map(map_entity_row).collect())
    }

    pub async fn update_entity(
        &self,
        tenant_id: i64,
        entity_uuid: &str,
        cmd: UpdateEntityCommand<'_>,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_entity
            SET canonical_name = COALESCE(?, canonical_name),
                aliases_json = COALESCE(?, aliases_json),
                attributes_json = COALESCE(?, attributes_json),
                sensitivity_level = COALESCE(?, sensitivity_level),
                status = COALESCE(?, status),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(cmd.canonical_name)
        .bind(cmd.aliases_json)
        .bind(cmd.attributes_json)
        .bind(cmd.sensitivity_level)
        .bind(cmd.status)
        .bind(&now)
        .bind(tenant_id)
        .bind(entity_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn insert_edge(&self, cmd: InsertEdgeCommand<'_>) -> Result<(), NativeSqlStoreError> {
        let now = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_edge (
              id, uuid, tenant_id, space_id, source_entity_id, target_entity_id,
              relation_type, source_memory_id, weight, status, valid_from,
              valid_to, metadata_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(cmd.id)
        .bind(cmd.uuid)
        .bind(cmd.tenant_id)
        .bind(cmd.space_id)
        .bind(cmd.source_entity_id)
        .bind(cmd.target_entity_id)
        .bind(cmd.relation_type)
        .bind(cmd.source_record_id)
        .bind(cmd.weight)
        .bind(cmd.valid_from)
        .bind(cmd.valid_to)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn retrieve_edge(
        &self,
        tenant_id: i64,
        edge_uuid: &str,
    ) -> Result<Option<NativeSqlEdgeRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT
              edge.id,
              edge.uuid,
              edge.tenant_id,
              edge.space_id,
              edge.source_entity_id,
              edge.target_entity_id,
              source_entity.uuid AS source_entity_uuid,
              target_entity.uuid AS target_entity_uuid,
              edge.relation_type,
              provenance.uuid AS source_memory_uuid,
              edge.weight,
              edge.status,
              edge.valid_from,
              edge.valid_to,
              edge.metadata_json,
              edge.created_at,
              edge.updated_at,
              edge.version
            FROM ai_edge edge
            JOIN ai_entity source_entity
              ON source_entity.id = edge.source_entity_id
             AND source_entity.tenant_id = edge.tenant_id
            JOIN ai_entity target_entity
              ON target_entity.id = edge.target_entity_id
             AND target_entity.tenant_id = edge.tenant_id
            LEFT JOIN ai_record provenance
              ON provenance.id = edge.source_memory_id
             AND provenance.tenant_id = edge.tenant_id
            WHERE edge.tenant_id = ? AND edge.uuid = ? AND edge.status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .bind(edge_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(map_edge_row))
    }

    pub async fn list_edges(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
        relation_type: Option<&str>,
        source_entity_uuid: Option<&str>,
        cursor: Option<&str>,
        page_size: i32,
    ) -> Result<Vec<NativeSqlEdgeRow>, NativeSqlStoreError> {
        let limit = page_size.clamp(1, MAX_LIST_PAGE_SIZE) + 1;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT
              edge.id,
              edge.uuid,
              edge.tenant_id,
              edge.space_id,
              edge.source_entity_id,
              edge.target_entity_id,
              source_entity.uuid AS source_entity_uuid,
              target_entity.uuid AS target_entity_uuid,
              edge.relation_type,
              provenance.uuid AS source_memory_uuid,
              edge.weight,
              edge.status,
              edge.valid_from,
              edge.valid_to,
              edge.metadata_json,
              edge.created_at,
              edge.updated_at,
              edge.version
            FROM ai_edge edge
            JOIN ai_entity source_entity
              ON source_entity.id = edge.source_entity_id
             AND source_entity.tenant_id = edge.tenant_id
            JOIN ai_entity target_entity
              ON target_entity.id = edge.target_entity_id
             AND target_entity.tenant_id = edge.tenant_id
            LEFT JOIN ai_record provenance
              ON provenance.id = edge.source_memory_id
             AND provenance.tenant_id = edge.tenant_id
            WHERE edge.tenant_id = ?
              AND edge.status <> 'deleted'
              AND (? IS NULL OR edge.space_id = ?)
              AND (? IS NULL OR edge.relation_type = ?)
              AND (? IS NULL OR source_entity.uuid = ?)
              AND edge.uuid > ?
            ORDER BY edge.uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(space_id)
        .bind(space_id)
        .bind(relation_type)
        .bind(relation_type)
        .bind(source_entity_uuid)
        .bind(source_entity_uuid)
        .bind(cursor)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_edge_row).collect())
    }

    pub async fn update_edge(
        &self,
        tenant_id: i64,
        edge_uuid: &str,
        cmd: UpdateEdgeCommand<'_>,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_edge
            SET relation_type = COALESCE(?, relation_type),
                weight = COALESCE(?, weight),
                status = COALESCE(?, status),
                valid_from = COALESCE(?, valid_from),
                valid_to = COALESCE(?, valid_to),
                metadata_json = COALESCE(?, metadata_json),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(cmd.relation_type)
        .bind(cmd.weight)
        .bind(cmd.status)
        .bind(cmd.valid_from)
        .bind(cmd.valid_to)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(tenant_id)
        .bind(edge_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_edge(
        &self,
        tenant_id: i64,
        edge_uuid: &str,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_edge
            SET status = 'deleted', updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND status <> 'deleted'
            "#,
        )
        .bind(&now)
        .bind(tenant_id)
        .bind(edge_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn count_entities_for_tenant(
        &self,
        tenant_id: i64,
    ) -> Result<i64, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM ai_entity
            WHERE tenant_id = ? AND status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        Ok(row.get("total"))
    }

    pub async fn count_edges_for_tenant(&self, tenant_id: i64) -> Result<i64, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS total
            FROM ai_edge
            WHERE tenant_id = ? AND status <> 'deleted'
            "#,
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        Ok(row.get("total"))
    }
}

fn validate_graph_journal(
    resource_id: &str,
    scope: &MemoryScopeContext,
    tenant_id: i64,
    space_id: Option<i64>,
    journal: &MemoryMutationJournal,
) -> Result<(), NativeSqlStoreError> {
    if scope.tenant_id != tenant_id
        || space_id.is_some_and(|space_id| scope.space_id != space_id)
        || journal.aggregate_id != resource_id
        || journal.audit_resource_id != resource_id
    {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: "graph mutation journal scope and resource must match the business row"
                .to_string(),
        });
    }
    Ok(())
}

fn map_entity_row(row: sqlx::any::AnyRow) -> NativeSqlEntityRow {
    NativeSqlEntityRow {
        id: row.get("id"),
        uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        space_id: row.get("space_id"),
        entity_type: row.get("entity_type"),
        canonical_name: row.get("canonical_name"),
        aliases_json: row.get("aliases_json"),
        attributes_json: row.get("attributes_json"),
        sensitivity_level: row.get("sensitivity_level"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_edge_row(row: sqlx::any::AnyRow) -> NativeSqlEdgeRow {
    NativeSqlEdgeRow {
        id: row.get("id"),
        uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        space_id: row.get("space_id"),
        source_entity_id: row.get("source_entity_id"),
        target_entity_id: row.get("target_entity_id"),
        source_entity_uuid: row.get("source_entity_uuid"),
        target_entity_uuid: row.get("target_entity_uuid"),
        relation_type: row.get("relation_type"),
        source_memory_uuid: row.try_get("source_memory_uuid").ok(),
        weight: row.get("weight"),
        status: row.get("status"),
        valid_from: row.get("valid_from"),
        valid_to: row.get("valid_to"),
        metadata_json: row.get("metadata_json"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}
