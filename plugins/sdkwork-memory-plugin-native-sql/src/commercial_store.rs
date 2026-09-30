//! Commercial memory management store methods (subjects, bindings, capabilities).

use crate::sqlx_compat as sqlx;
use sdkwork_memory_spi::{MemoryMutationJournal, MemoryScopeContext};
use sqlx::Row;

use crate::store::{now_text, NativeSqlMemoryStore, NativeSqlStoreError};

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NativeSqlSubjectRow {
    pub id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub organization_id: Option<i64>,
    pub subject_type: String,
    pub subject_ref: String,
    pub display_name: String,
    pub default_space_id: Option<i64>,
    pub status: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct NativeSqlBindingRow {
    pub id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub space_id: Option<i64>,
    pub binding_kind: String,
    pub binding_role: String,
    pub source_subject_id: Option<i64>,
    pub target_subject_id: Option<i64>,
    pub target_space_id: Option<i64>,
    pub capability_codes_json: Option<String>,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct NativeSqlCapabilityBindingRow {
    pub id: i64,
    pub uuid: String,
    pub tenant_id: i64,
    pub capability_code: String,
    pub target_type: String,
    pub target_id: i64,
    pub mode: String,
    pub priority: i32,
    pub status: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub metadata_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

// ---------------------------------------------------------------------------
// Subject CRUD
// ---------------------------------------------------------------------------

pub struct InsertSubjectCommand<'a> {
    pub id: i64,
    pub uuid: &'a str,
    pub tenant_id: i64,
    pub organization_id: Option<i64>,
    pub subject_type: &'a str,
    pub subject_ref: &'a str,
    pub display_name: &'a str,
    pub default_space_id: Option<i64>,
    pub metadata_json: Option<&'a str>,
}

pub struct UpdateSubjectCommand<'a> {
    pub display_name: Option<&'a str>,
    pub default_space_id: Option<Option<i64>>,
    pub status: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
}

impl NativeSqlMemoryStore {
    pub async fn insert_subject_with_journal(
        &self,
        cmd: InsertSubjectCommand<'_>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        validate_commercial_journal(cmd.uuid, cmd.tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        sqlx::query(
            r#"
            INSERT INTO ai_subject (
              id, uuid, tenant_id, organization_id, subject_type, subject_ref,
              display_name, default_space_id, status, metadata_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, 1)
            "#,
        )
        .bind(cmd.id)
        .bind(cmd.uuid)
        .bind(cmd.tenant_id)
        .bind(cmd.organization_id.unwrap_or(0))
        .bind(cmd.subject_type)
        .bind(cmd.subject_ref)
        .bind(cmd.display_name)
        .bind(cmd.default_space_id)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_subject_with_journal(
        &self,
        tenant_id: i64,
        subject_uuid: &str,
        cmd: UpdateSubjectCommand<'_>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_commercial_journal(subject_uuid, tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_subject
            SET display_name = COALESCE(?, display_name),
                default_space_id = COALESCE(?, default_space_id),
                status = COALESCE(?, status),
                metadata_json = COALESCE(?, metadata_json),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(cmd.display_name)
        .bind(cmd.default_space_id)
        .bind(cmd.status)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(tenant_id)
        .bind(subject_uuid)
        .execute(&mut *tx)
        .await?;
        append_commercial_journal_if_changed(self, &mut tx, scope, journal, result.rows_affected())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_subject_with_journal(
        &self,
        tenant_id: i64,
        subject_uuid: &str,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_commercial_journal(subject_uuid, tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_subject
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(tenant_id)
        .bind(subject_uuid)
        .execute(&mut *tx)
        .await?;
        append_commercial_journal_if_changed(self, &mut tx, scope, journal, result.rows_affected())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    /// Normalizes a binding validity bound to the fixed-width UTC form every
    /// lexicographic window comparison in this store relies on. Writers accept
    /// any RFC3339 spelling (including timezone offsets); storing it verbatim
    /// would silently mis-order `valid_from`/`valid_to` against `now_text()`,
    /// so unparseable input fails closed here.
    fn normalized_window_bound(bound: Option<&str>) -> Result<Option<String>, NativeSqlStoreError> {
        let Some(bound) = bound else {
            return Ok(None);
        };
        match sdkwork_utils_rust::parse_datetime(bound, None) {
            Some(parsed) => Ok(Some(sdkwork_utils_rust::format_datetime(parsed, None))),
            None => Err(NativeSqlStoreError::InvariantViolation {
                message: format!("validity bound is not a parseable timestamp: {bound}"),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// The commercial-API memory-binding create: a tenant-level binding may
    /// anchor to no space at all (`space_id: None`); space-scoped bindings
    /// carry the resolved space id.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_binding_with_journal(
        &self,
        id: i64,
        uuid: &str,
        tenant_id: i64,
        space_id: Option<i64>,
        binding_kind: &str,
        binding_role: &str,
        source_subject_id: Option<i64>,
        target_subject_id: Option<i64>,
        target_space_id: Option<i64>,
        capability_codes_json: Option<&str>,
        valid_from: Option<&str>,
        valid_to: Option<&str>,
        metadata_json: Option<&str>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        let valid_from = Self::normalized_window_bound(valid_from)?;
        let valid_to = Self::normalized_window_bound(valid_to)?;
        validate_commercial_journal(uuid, tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        sqlx::query(
            r#"
            INSERT INTO ai_memory_binding (
              id, uuid, tenant_id, space_id, binding_kind, binding_role,
              source_subject_id, target_subject_id, target_space_id,
              capability_codes_json, status, valid_from, valid_to,
              metadata_json, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(id)
        .bind(uuid)
        .bind(tenant_id)
        .bind(space_id)
        .bind(binding_kind)
        .bind(binding_role)
        .bind(source_subject_id)
        .bind(target_subject_id)
        .bind(target_space_id)
        .bind(capability_codes_json)
        .bind(valid_from)
        .bind(valid_to)
        .bind(metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_binding_with_journal(
        &self,
        tenant_id: i64,
        binding_uuid: &str,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_commercial_journal(binding_uuid, tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_memory_binding
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(tenant_id)
        .bind(binding_uuid)
        .execute(&mut *tx)
        .await?;
        append_commercial_journal_if_changed(self, &mut tx, scope, journal, result.rows_affected())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_capability_binding_with_journal(
        &self,
        id: i64,
        uuid: &str,
        tenant_id: i64,
        capability_code: &str,
        target_type: &str,
        target_id: i64,
        mode: &str,
        priority: i32,
        valid_from: Option<&str>,
        valid_to: Option<&str>,
        metadata_json: Option<&str>,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<(), NativeSqlStoreError> {
        let valid_from = Self::normalized_window_bound(valid_from)?;
        let valid_to = Self::normalized_window_bound(valid_to)?;
        validate_commercial_journal(uuid, tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        sqlx::query(
            r#"
            INSERT INTO ai_capability_binding (
              id, uuid, tenant_id, capability_code, target_type, target_id,
              mode, priority, status, valid_from, valid_to, metadata_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(id)
        .bind(uuid)
        .bind(tenant_id)
        .bind(capability_code)
        .bind(target_type)
        .bind(target_id)
        .bind(mode)
        .bind(priority)
        .bind(valid_from)
        .bind(valid_to)
        .bind(metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        crate::canonical_data::append_journal_on_tx(self, &mut tx, scope, journal).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn delete_capability_binding_with_journal(
        &self,
        tenant_id: i64,
        cap_uuid: &str,
        scope: &MemoryScopeContext,
        journal: &MemoryMutationJournal,
    ) -> Result<bool, NativeSqlStoreError> {
        validate_commercial_journal(cap_uuid, tenant_id, scope, journal)?;
        let now = now_text();
        let mut tx = self.begin_tx().await?;
        let result = sqlx::query(
            r#"
            UPDATE ai_capability_binding
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(tenant_id)
        .bind(cap_uuid)
        .execute(&mut *tx)
        .await?;
        append_commercial_journal_if_changed(self, &mut tx, scope, journal, result.rows_affected())
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn insert_subject(
        &self,
        cmd: InsertSubjectCommand<'_>,
    ) -> Result<(), NativeSqlStoreError> {
        let now = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_subject (
              id, uuid, tenant_id, organization_id, subject_type, subject_ref,
              display_name, default_space_id, status, metadata_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, 1)
            "#,
        )
        .bind(cmd.id)
        .bind(cmd.uuid)
        .bind(cmd.tenant_id)
        .bind(cmd.organization_id.unwrap_or(0))
        .bind(cmd.subject_type)
        .bind(cmd.subject_ref)
        .bind(cmd.display_name)
        .bind(cmd.default_space_id)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn retrieve_subject(
        &self,
        tenant_id: i64,
        subject_uuid: &str,
    ) -> Result<Option<NativeSqlSubjectRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, organization_id, subject_type, subject_ref,
                   display_name, default_space_id, status, metadata_json,
                   created_at, updated_at, version
            FROM ai_subject
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(subject_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(map_subject_row))
    }

    pub async fn list_subjects(
        &self,
        tenant_id: i64,
        subject_type: Option<&str>,
        status: Option<&str>,
        cursor: Option<&str>,
        page_size: i32,
    ) -> Result<Vec<NativeSqlSubjectRow>, NativeSqlStoreError> {
        let limit = clamp_list_page_size(page_size) + 1;
        let rows = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, organization_id, subject_type, subject_ref,
                   display_name, default_space_id, status, metadata_json,
                   created_at, updated_at, version
            FROM ai_subject
            WHERE tenant_id = ?
              AND deleted_at IS NULL
              AND (? IS NULL OR subject_type = ?)
              AND (? IS NULL OR status = ?)
              AND (? IS NULL OR uuid > ?)
            ORDER BY uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(subject_type)
        .bind(subject_type)
        .bind(status)
        .bind(status)
        .bind(cursor)
        .bind(cursor)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_subject_row).collect())
    }

    pub async fn update_subject(
        &self,
        tenant_id: i64,
        subject_uuid: &str,
        cmd: UpdateSubjectCommand<'_>,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_subject
            SET display_name = COALESCE(?, display_name),
                default_space_id = COALESCE(?, default_space_id),
                status = COALESCE(?, status),
                metadata_json = COALESCE(?, metadata_json),
                updated_at = ?,
                version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(cmd.display_name)
        .bind(cmd.default_space_id)
        .bind(cmd.status)
        .bind(cmd.metadata_json)
        .bind(&now)
        .bind(tenant_id)
        .bind(subject_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_subject(
        &self,
        tenant_id: i64,
        subject_uuid: &str,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_subject
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(tenant_id)
        .bind(subject_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Binding CRUD
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_binding(
        &self,
        id: i64,
        uuid: &str,
        tenant_id: i64,
        space_id: Option<i64>,
        binding_kind: &str,
        binding_role: &str,
        source_subject_id: Option<i64>,
        target_subject_id: Option<i64>,
        target_space_id: Option<i64>,
        capability_codes_json: Option<&str>,
        valid_from: Option<&str>,
        valid_to: Option<&str>,
        metadata_json: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        let valid_from = Self::normalized_window_bound(valid_from)?;
        let valid_to = Self::normalized_window_bound(valid_to)?;
        let now = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_memory_binding (
              id, uuid, tenant_id, space_id, binding_kind, binding_role,
              source_subject_id, target_subject_id, target_space_id,
              capability_codes_json, status, valid_from, valid_to,
              metadata_json, created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(id)
        .bind(uuid)
        .bind(tenant_id)
        .bind(space_id)
        .bind(binding_kind)
        .bind(binding_role)
        .bind(source_subject_id)
        .bind(target_subject_id)
        .bind(target_space_id)
        .bind(capability_codes_json)
        .bind(valid_from)
        .bind(valid_to)
        .bind(metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn retrieve_binding(
        &self,
        tenant_id: i64,
        binding_uuid: &str,
    ) -> Result<Option<NativeSqlBindingRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, space_id, binding_kind, binding_role,
                   source_subject_id, target_subject_id, target_space_id,
                   capability_codes_json, status, valid_from, valid_to,
                   metadata_json, created_at, updated_at, version
            FROM ai_memory_binding
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(binding_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(map_binding_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_bindings(
        &self,
        tenant_id: i64,
        source_subject_id: Option<i64>,
        target_subject_id: Option<i64>,
        target_space_id: Option<i64>,
        binding_kind: Option<&str>,
        status: Option<&str>,
        cursor: Option<&str>,
        page_size: i32,
    ) -> Result<Vec<NativeSqlBindingRow>, NativeSqlStoreError> {
        let limit = clamp_list_page_size(page_size) + 1;
        let rows = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, space_id, binding_kind, binding_role,
                   source_subject_id, target_subject_id, target_space_id,
                   capability_codes_json, status, valid_from, valid_to,
                   metadata_json, created_at, updated_at, version
            FROM ai_memory_binding
            WHERE tenant_id = ?
              AND deleted_at IS NULL
              AND (? IS NULL OR source_subject_id = ?)
              AND (? IS NULL OR target_subject_id = ?)
              AND (? IS NULL OR target_space_id = ?)
              AND (? IS NULL OR binding_kind = ?)
              AND (? IS NULL OR status = ?)
              AND (? IS NULL OR uuid > ?)
            ORDER BY uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(source_subject_id)
        .bind(source_subject_id)
        .bind(target_subject_id)
        .bind(target_subject_id)
        .bind(target_space_id)
        .bind(target_space_id)
        .bind(binding_kind)
        .bind(binding_kind)
        .bind(status)
        .bind(status)
        .bind(cursor)
        .bind(cursor)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_binding_row).collect())
    }

    /// Returns true when the actor's subject has an active memory binding granting
    /// access to the target space (`access`, `share`, or `ownership` kinds).
    pub async fn actor_has_active_space_binding(
        &self,
        tenant_id: i64,
        space_id: i64,
        actor_ref: &str,
        require_write: bool,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let row = sqlx::query(
            r#"
            SELECT 1 AS granted
            FROM ai_memory_binding b
            INNER JOIN ai_subject s
              ON s.tenant_id = b.tenant_id
             AND s.id = b.source_subject_id
             AND s.deleted_at IS NULL
             AND s.status = 'active'
            WHERE b.tenant_id = ?
              AND b.deleted_at IS NULL
              AND b.status = 'active'
              AND b.binding_kind IN ('access', 'share', 'ownership')
              AND s.subject_ref = ?
              AND (
                b.target_space_id = ?
                OR (b.target_space_id IS NULL AND b.space_id = ?)
              )
              AND (b.valid_from IS NULL OR b.valid_from <= ?)
              AND (b.valid_to IS NULL OR b.valid_to >= ?)
              AND (
                ? = 0
                OR b.binding_role IN ('owner', 'learner')
              )
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(actor_ref)
        .bind(space_id)
        .bind(space_id)
        .bind(&now)
        .bind(&now)
        .bind(i32::from(require_write))
        .fetch_optional(self.pool())
        .await?;
        Ok(row.is_some())
    }

    pub async fn delete_binding(
        &self,
        tenant_id: i64,
        binding_uuid: &str,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_memory_binding
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(tenant_id)
        .bind(binding_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Capability Binding CRUD
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_capability_binding(
        &self,
        id: i64,
        uuid: &str,
        tenant_id: i64,
        capability_code: &str,
        target_type: &str,
        target_id: i64,
        mode: &str,
        priority: i32,
        valid_from: Option<&str>,
        valid_to: Option<&str>,
        metadata_json: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        let valid_from = Self::normalized_window_bound(valid_from)?;
        let valid_to = Self::normalized_window_bound(valid_to)?;
        let now = now_text();
        sqlx::query(
            r#"
            INSERT INTO ai_capability_binding (
              id, uuid, tenant_id, capability_code, target_type, target_id,
              mode, priority, status, valid_from, valid_to, metadata_json,
              created_at, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(id)
        .bind(uuid)
        .bind(tenant_id)
        .bind(capability_code)
        .bind(target_type)
        .bind(target_id)
        .bind(mode)
        .bind(priority)
        .bind(valid_from)
        .bind(valid_to)
        .bind(metadata_json)
        .bind(&now)
        .bind(&now)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn retrieve_capability_binding(
        &self,
        tenant_id: i64,
        cap_uuid: &str,
    ) -> Result<Option<NativeSqlCapabilityBindingRow>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, capability_code, target_type, target_id,
                   mode, priority, status, valid_from, valid_to, metadata_json,
                   created_at, updated_at, version
            FROM ai_capability_binding
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(tenant_id)
        .bind(cap_uuid)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(map_capability_binding_row))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_capability_bindings(
        &self,
        tenant_id: i64,
        capability_code: Option<&str>,
        target_type: Option<&str>,
        target_id: Option<i64>,
        status: Option<&str>,
        cursor: Option<&str>,
        page_size: i32,
    ) -> Result<Vec<NativeSqlCapabilityBindingRow>, NativeSqlStoreError> {
        let limit = clamp_list_page_size(page_size) + 1;
        let rows = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, capability_code, target_type, target_id,
                   mode, priority, status, valid_from, valid_to, metadata_json,
                   created_at, updated_at, version
            FROM ai_capability_binding
            WHERE tenant_id = ?
              AND deleted_at IS NULL
              AND (? IS NULL OR capability_code = ?)
              AND (? IS NULL OR target_type = ?)
              AND (? IS NULL OR target_id = ?)
              AND (? IS NULL OR status = ?)
              AND (? IS NULL OR uuid > ?)
            ORDER BY uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(capability_code)
        .bind(capability_code)
        .bind(target_type)
        .bind(target_type)
        .bind(target_id)
        .bind(target_id)
        .bind(status)
        .bind(status)
        .bind(cursor)
        .bind(cursor)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_capability_binding_row).collect())
    }

    pub async fn delete_capability_binding(
        &self,
        tenant_id: i64,
        cap_uuid: &str,
    ) -> Result<bool, NativeSqlStoreError> {
        let now = now_text();
        let result = sqlx::query(
            r#"
            UPDATE ai_capability_binding
            SET status = 'deleted', deleted_at = ?, updated_at = ?, version = version + 1
            WHERE tenant_id = ? AND uuid = ? AND deleted_at IS NULL
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(tenant_id)
        .bind(cap_uuid)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Capability resolution for a target
    // -----------------------------------------------------------------------

    pub async fn resolve_capabilities_for_target(
        &self,
        tenant_id: i64,
        target_type: &str,
        target_id: i64,
        page_size: i32,
        cursor: Option<&str>,
    ) -> Result<Vec<NativeSqlCapabilityBindingRow>, NativeSqlStoreError> {
        let page_size = page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64;
        let cursor = cursor.unwrap_or("");
        let rows = sqlx::query(
            r#"
            SELECT id, uuid, tenant_id, capability_code, target_type, target_id,
                   mode, priority, status, valid_from, valid_to, metadata_json,
                   created_at, updated_at, version
            FROM ai_capability_binding
            WHERE tenant_id = ?
              AND target_type = ?
              AND target_id = ?
              AND status = 'active'
              AND deleted_at IS NULL
              AND uuid > ?
            ORDER BY uuid ASC
            LIMIT ?
            "#,
        )
        .bind(tenant_id)
        .bind(target_type)
        .bind(target_id)
        .bind(cursor)
        .bind(page_size + 1)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_capability_binding_row).collect())
    }
}

/// Clamps a caller-supplied page size to the crate-wide list page bound before
/// the `+ 1` continuation lookahead is added, mirroring the private helper in
/// `store.rs`: a raw `page_size + 1` would both overflow on `i32::MAX` and let
/// an unvalidated page widen the `LIMIT` past [`sdkwork_utils_rust::MAX_LIST_PAGE_SIZE`].
fn clamp_list_page_size(page_size: i32) -> i64 {
    page_size.clamp(1, sdkwork_utils_rust::MAX_LIST_PAGE_SIZE) as i64
}

fn validate_commercial_journal(
    resource_id: &str,
    tenant_id: i64,
    scope: &MemoryScopeContext,
    journal: &MemoryMutationJournal,
) -> Result<(), NativeSqlStoreError> {
    if scope.tenant_id != tenant_id
        || journal.aggregate_id != resource_id
        || journal.audit_resource_id != resource_id
    {
        return Err(NativeSqlStoreError::InvariantViolation {
            message: "commercial mutation journal scope and resource must match the business row"
                .to_string(),
        });
    }
    Ok(())
}

async fn append_commercial_journal_if_changed(
    store: &NativeSqlMemoryStore,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    scope: &MemoryScopeContext,
    journal: &MemoryMutationJournal,
    rows_affected: u64,
) -> Result<(), NativeSqlStoreError> {
    if rows_affected > 0 {
        crate::canonical_data::append_journal_on_tx(store, tx, scope, journal).await?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

fn map_subject_row(row: sqlx::any::AnyRow) -> NativeSqlSubjectRow {
    NativeSqlSubjectRow {
        id: row.get("id"),
        uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        organization_id: row.get("organization_id"),
        subject_type: row.get("subject_type"),
        subject_ref: row.get("subject_ref"),
        display_name: row.get("display_name"),
        default_space_id: row.get("default_space_id"),
        status: row.get("status"),
        metadata_json: row.get("metadata_json"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_binding_row(row: sqlx::any::AnyRow) -> NativeSqlBindingRow {
    NativeSqlBindingRow {
        id: row.get("id"),
        uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        space_id: row.get("space_id"),
        binding_kind: row.get("binding_kind"),
        binding_role: row.get("binding_role"),
        source_subject_id: row.get("source_subject_id"),
        target_subject_id: row.get("target_subject_id"),
        target_space_id: row.get("target_space_id"),
        capability_codes_json: row.get("capability_codes_json"),
        status: row.get("status"),
        valid_from: row.get("valid_from"),
        valid_to: row.get("valid_to"),
        metadata_json: row.get("metadata_json"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

fn map_capability_binding_row(row: sqlx::any::AnyRow) -> NativeSqlCapabilityBindingRow {
    NativeSqlCapabilityBindingRow {
        id: row.get("id"),
        uuid: row.get("uuid"),
        tenant_id: row.get("tenant_id"),
        capability_code: row.get("capability_code"),
        target_type: row.get("target_type"),
        target_id: row.get("target_id"),
        mode: row.get("mode"),
        priority: row.get("priority"),
        status: row.get("status"),
        valid_from: row.get("valid_from"),
        valid_to: row.get("valid_to"),
        metadata_json: row.get("metadata_json"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}

// ---------------------------------------------------------------------------
// Tenant quota policy resolution (policy_type = 'memory.quota')
// ---------------------------------------------------------------------------

/// Carrier for an active tenant-scoped `memory.quota` policy. Each limit key
/// is optional: an absent key falls back to the runtime environment default at
/// the service layer, and a present key must be a positive `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantQuotaPolicyLimits {
    pub max_records_per_space: Option<u64>,
    pub max_spaces_per_user: Option<u64>,
}

impl NativeSqlMemoryStore {
    /// Resolves the active `memory.quota` policy of one tenant. The
    /// `scope_ref` column carries the tenant identifier in its decimal string
    /// form, matching the tenant-scope policy convention.
    ///
    /// The predicates line up with `idx_ai_policy_type_scope`
    /// (tenant_id, policy_type, scope, status), and the newest policy wins
    /// (`ORDER BY id DESC`: snowflake ids are allocation-ordered) so
    /// resolution stays deterministic when several active quota policies are
    /// stacked. Malformed policy JSON or non-positive values fail closed with
    /// a named storage error instead of silently downgrading to the
    /// environment defaults.
    pub async fn find_active_tenant_quota_policy(
        &self,
        tenant_id: i64,
    ) -> Result<Option<TenantQuotaPolicyLimits>, NativeSqlStoreError> {
        let row = sqlx::query(
            r#"
            SELECT policy_json
            FROM ai_policy
            WHERE tenant_id = ?
              AND policy_type = 'memory.quota'
              AND scope = 'tenant'
              AND status = 'active'
              AND scope_ref = ?
            ORDER BY id DESC
            LIMIT 1
            "#,
        )
        .bind(tenant_id)
        .bind(tenant_id.to_string())
        .fetch_optional(self.pool())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let policy_json: String = row.get("policy_json");
        parse_tenant_quota_policy_json(&policy_json).map(Some)
    }
}

fn parse_tenant_quota_policy_json(
    policy_json: &str,
) -> Result<TenantQuotaPolicyLimits, NativeSqlStoreError> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TenantQuotaPolicyJson {
        #[serde(default)]
        max_records_per_space: Option<u64>,
        #[serde(default)]
        max_spaces_per_user: Option<u64>,
    }
    let parsed: TenantQuotaPolicyJson = serde_json::from_str(policy_json).map_err(|error| {
        NativeSqlStoreError::InvariantViolation {
            message: format!("tenant memory.quota policy is not valid JSON: {error}"),
        }
    })?;
    let positive_limit = |raw: Option<u64>, key: &str| {
        match raw {
            // Zero would silently disable the quota the policy meant to
            // tighten, so a non-positive limit fails closed instead.
            Some(0) => Err(NativeSqlStoreError::InvariantViolation {
                message: format!("tenant memory.quota policy limit {key} must be positive"),
            }),
            other => Ok(other),
        }
    };
    Ok(TenantQuotaPolicyLimits {
        max_records_per_space: positive_limit(
            parsed.max_records_per_space,
            "maxRecordsPerSpace",
        )?,
        max_spaces_per_user: positive_limit(parsed.max_spaces_per_user, "maxSpacesPerUser")?,
    })
}

// ---------------------------------------------------------------------------
// Usage metering facts (ai_usage_daily)
// ---------------------------------------------------------------------------

/// Journal action `memory.record.create` charges this metric.
pub const USAGE_METRIC_RECORD_CREATE: &str = "record.create";
/// Journal action `memory.record.delete` charges this metric (negative delta).
pub const USAGE_METRIC_RECORD_DELETE: &str = "record.delete";
/// Journal action `memory.event.create` charges this metric.
pub const USAGE_METRIC_EVENT_CREATE: &str = "event.create";
/// Every stored retrieval trace charges this metric (best-effort).
pub const USAGE_METRIC_RETRIEVAL_COUNT: &str = "retrieval.count";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSqlUsageDailyRow {
    pub tenant_id: i64,
    pub day: String,
    pub metric: String,
    pub delta: i64,
    pub updated_at: String,
    pub version: i64,
}

/// UTC calendar day (`YYYY-MM-DD`) for newly created usage facts, derived from
/// the same clock source as [`crate::store::now_text`].
pub(crate) fn usage_day_text() -> String {
    sdkwork_utils_rust::format_datetime(sdkwork_utils_rust::now(), Some("%Y-%m-%d"))
}

impl NativeSqlMemoryStore {
    /// Charges `delta` on the `(tenant, day, metric)` usage counter inside the
    /// caller's transaction. `day` is explicit so a transaction charges one
    /// consistent day and tests can pin day-boundary isolation without
    /// sleeping. The UPSERT is cumulative: repeated charges accumulate into
    /// one bounded row per (tenant, day, metric).
    pub async fn bump_usage_daily_on_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Any>,
        tenant_id: i64,
        metric: &str,
        delta: i64,
        day: &str,
    ) -> Result<(), NativeSqlStoreError> {
        // SQLite and PostgreSQL spell this UPSERT identically through the Any
        // driver: both engines resolve the same single non-partial unique
        // index (`uk_ai_usage_daily_day_metric`), the same pattern the
        // tenant-preference and habit upserts already rely on.
        sqlx::query(
            r#"
            INSERT INTO ai_usage_daily (
              id, tenant_id, day, metric, delta, updated_at, version
            )
            VALUES (?, ?, ?, ?, ?, ?, 0)
            ON CONFLICT (tenant_id, day, metric) DO UPDATE SET
              delta = ai_usage_daily.delta + excluded.delta,
              updated_at = excluded.updated_at,
              version = ai_usage_daily.version + 1
            "#,
        )
        .bind(self.next_row_id()?)
        .bind(tenant_id)
        .bind(day)
        .bind(metric)
        .bind(delta)
        .bind(crate::store::now_text())
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Self-contained variant of [`Self::bump_usage_daily_on_tx`] for
    /// best-effort metering points that do not run inside a caller's
    /// transaction (retrieval usage charges). Charges today's UTC day.
    pub async fn bump_usage_daily(
        &self,
        tenant_id: i64,
        metric: &str,
        delta: i64,
    ) -> Result<(), NativeSqlStoreError> {
        let day = usage_day_text();
        let mut tx = self.begin_tx().await?;
        self.bump_usage_daily_on_tx(&mut tx, tenant_id, metric, delta, &day)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Keyset listing of one tenant's usage facts ordered by (day, metric)
    /// ascending. `cursor` is the exclusive continuation position `(day,
    /// metric)`; an absent cursor starts from the first row.
    pub async fn list_usage_daily_for_tenant(
        &self,
        tenant_id: i64,
        cursor: Option<(&str, &str)>,
        page_size: i32,
    ) -> Result<Vec<NativeSqlUsageDailyRow>, NativeSqlStoreError> {
        let limit = clamp_list_page_size(page_size) + 1;
        let rows = match cursor {
            None => {
                sqlx::query(
                    r#"
                    SELECT tenant_id, day, metric, delta, updated_at, version
                    FROM ai_usage_daily
                    WHERE tenant_id = ?
                    ORDER BY day ASC, metric ASC
                    LIMIT ?
                    "#,
                )
                .bind(tenant_id)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
            }
            Some((cursor_day, cursor_metric)) => {
                sqlx::query(
                    r#"
                    SELECT tenant_id, day, metric, delta, updated_at, version
                    FROM ai_usage_daily
                    WHERE tenant_id = ?
                      AND (day > ? OR (day = ? AND metric > ?))
                    ORDER BY day ASC, metric ASC
                    LIMIT ?
                    "#,
                )
                .bind(tenant_id)
                .bind(cursor_day)
                .bind(cursor_day)
                .bind(cursor_metric)
                .bind(limit)
                .fetch_all(self.pool())
                .await?
            }
        };
        Ok(rows.into_iter().map(map_usage_daily_row).collect())
    }
}

fn map_usage_daily_row(row: sqlx::any::AnyRow) -> NativeSqlUsageDailyRow {
    NativeSqlUsageDailyRow {
        tenant_id: row.get("tenant_id"),
        day: row.get("day"),
        metric: row.get("metric"),
        delta: row.get("delta"),
        updated_at: row.get("updated_at"),
        version: row.get("version"),
    }
}
