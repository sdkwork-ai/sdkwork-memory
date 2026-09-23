//! Full-text search index maintenance and query helpers.

use crate::sqlx_compat as sqlx;
use sdkwork_memory_spi::{
    MemoryScopeContext, MemorySensitivityReadScope, MetadataFilterExpression,
};
use sdkwork_utils_rust::MAX_LIST_PAGE_SIZE;

use crate::pool_backend::MemorySqlDialect;
use crate::store::{
    record_detail_from_row, NativeSqlMemoryRecordDetail, NativeSqlMemoryStore, NativeSqlStoreError,
};

pub(crate) fn is_recoverable_fulltext_error(error: &NativeSqlStoreError) -> bool {
    let NativeSqlStoreError::Database(sqlx::Error::Database(database_error)) = error else {
        return false;
    };
    is_fulltext_unavailable_message(database_error.message())
}

fn is_fulltext_unavailable_message(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    (message.contains("ai_record_fts")
        && (message.contains("no such table") || message.contains("does not exist")))
        || (message.contains("fts5") && message.contains("no such module"))
        || (message.contains("search_document")
            && (message.contains("no such column") || message.contains("does not exist")))
        || (message.contains("websearch_to_tsquery") && message.contains("does not exist"))
}

impl NativeSqlMemoryStore {
    pub(crate) fn read_scope_level(read_scope: MemorySensitivityReadScope) -> i32 {
        match read_scope {
            MemorySensitivityReadScope::Public => 0,
            MemorySensitivityReadScope::Elevated => 1,
            MemorySensitivityReadScope::Owner => 2,
        }
    }

    pub(crate) fn append_memory_type_filter(
        sql: &mut String,
        table_alias: &str,
        memory_types: &[String],
    ) {
        if memory_types.is_empty() {
            return;
        }
        sql.push_str(&format!(" AND {table_alias}.memory_type IN ("));
        sql.push_str(
            &(0..memory_types.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(","),
        );
        sql.push(')');
    }

    pub(crate) fn append_sensitivity_filter(sql: &mut String, table_alias: &str) {
        sql.push_str(&format!(
            " AND (? >= 2 OR (? >= 1 AND {table_alias}.sensitivity_level IN ('public','internal','private','sensitive')) OR {table_alias}.sensitivity_level IN ('public','internal'))"
        ));
    }

    pub async fn sync_record_fts_entry(
        &self,
        scope: &MemoryScopeContext,
        memory_uuid: &str,
        canonical_text: &str,
        object_text: &str,
        subject: Option<&str>,
        predicate: Option<&str>,
    ) -> Result<(), NativeSqlStoreError> {
        if !matches!(self.dialect(), MemorySqlDialect::Sqlite) {
            return Ok(());
        }
        let row_id = self
            .lookup_record_row_id(scope, memory_uuid)
            .await?
            .ok_or_else(|| NativeSqlStoreError::InvariantViolation {
                message: format!("fts sync memory {memory_uuid} not found"),
            })?;
        sqlx::query("DELETE FROM ai_record_fts WHERE rowid = ?")
            .bind(row_id)
            .execute(self.pool())
            .await?;
        sqlx::query(
            r#"
            INSERT INTO ai_record_fts(
              rowid, memory_uuid, tenant_id, space_id, canonical_text, object_text, subject, predicate
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(row_id)
        .bind(memory_uuid)
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(canonical_text)
        .bind(object_text)
        .bind(subject.unwrap_or(""))
        .bind(predicate.unwrap_or(""))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn remove_record_fts_entry(
        &self,
        scope: &MemoryScopeContext,
        memory_uuid: &str,
    ) -> Result<(), NativeSqlStoreError> {
        if !matches!(self.dialect(), MemorySqlDialect::Sqlite) {
            return Ok(());
        }
        sqlx::query(
            "DELETE FROM ai_record_fts WHERE tenant_id = ? AND space_id = ? AND memory_uuid = ?",
        )
        .bind(scope.tenant_id)
        .bind(scope.space_id)
        .bind(memory_uuid)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn rebuild_all_record_search_indexes(
        &self,
        tenant_id: i64,
    ) -> Result<u32, NativeSqlStoreError> {
        self.rebuild_record_search_indexes(tenant_id, None).await
    }

    pub async fn rebuild_record_search_indexes_for_space(
        &self,
        tenant_id: i64,
        space_id: i64,
    ) -> Result<u32, NativeSqlStoreError> {
        self.rebuild_record_search_indexes(tenant_id, Some(space_id))
            .await
    }

    pub async fn rebuild_record_search_indexes(
        &self,
        tenant_id: i64,
        space_id: Option<i64>,
    ) -> Result<u32, NativeSqlStoreError> {
        match self.dialect() {
            MemorySqlDialect::Postgres => {
                let updated = if let Some(space_id) = space_id {
                    sqlx::query(
                        r#"
                        UPDATE ai_record
                        SET search_document = to_tsvector(
                          'simple',
                          coalesce(canonical_text, '') || ' ' ||
                          coalesce(object_text, '') || ' ' ||
                          coalesce(subject, '') || ' ' ||
                          coalesce(predicate, '')
                        ),
                        updated_at = updated_at
                        WHERE tenant_id = ?
                          AND space_id = ?
                          AND status <> 'deleted'
                        "#,
                    )
                    .bind(tenant_id)
                    .bind(space_id)
                    .execute(self.pool())
                    .await?
                    .rows_affected()
                } else {
                    sqlx::query(
                        r#"
                        UPDATE ai_record
                        SET search_document = to_tsvector(
                          'simple',
                          coalesce(canonical_text, '') || ' ' ||
                          coalesce(object_text, '') || ' ' ||
                          coalesce(subject, '') || ' ' ||
                          coalesce(predicate, '')
                        ),
                        updated_at = updated_at
                        WHERE tenant_id = ?
                          AND status <> 'deleted'
                        "#,
                    )
                    .bind(tenant_id)
                    .execute(self.pool())
                    .await?
                    .rows_affected()
                };
                Ok(updated as u32)
            }
            MemorySqlDialect::Sqlite => {
                if let Some(space_id) = space_id {
                    sqlx::query("DELETE FROM ai_record_fts WHERE tenant_id = ? AND space_id = ?")
                        .bind(tenant_id)
                        .bind(space_id)
                        .execute(self.pool())
                        .await?;
                    let inserted = sqlx::query(
                        r#"
                        INSERT INTO ai_record_fts(
                          rowid, memory_uuid, tenant_id, space_id, canonical_text, object_text, subject, predicate
                        )
                        SELECT id, uuid, tenant_id, space_id,
                               coalesce(canonical_text, ''), coalesce(object_text, ''), coalesce(subject, ''),
                               coalesce(predicate, '')
                        FROM ai_record
                        WHERE tenant_id = ?
                          AND space_id = ?
                          AND status <> 'deleted'
                        "#,
                    )
                    .bind(tenant_id)
                    .bind(space_id)
                    .execute(self.pool())
                    .await?
                    .rows_affected();
                    Ok(inserted as u32)
                } else {
                    sqlx::query("DELETE FROM ai_record_fts WHERE tenant_id = ?")
                        .bind(tenant_id)
                        .execute(self.pool())
                        .await?;
                    let inserted = sqlx::query(
                        r#"
                        INSERT INTO ai_record_fts(
                          rowid, memory_uuid, tenant_id, space_id, canonical_text, object_text, subject, predicate
                        )
                        SELECT id, uuid, tenant_id, space_id,
                               coalesce(canonical_text, ''), coalesce(object_text, ''), coalesce(subject, ''),
                               coalesce(predicate, '')
                        FROM ai_record
                        WHERE tenant_id = ?
                          AND status <> 'deleted'
                        "#,
                    )
                    .bind(tenant_id)
                    .execute(self.pool())
                    .await?
                    .rows_affected();
                    Ok(inserted as u32)
                }
            }
        }
    }

    pub async fn search_record_details_fulltext(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
        top_k: i32,
    ) -> Result<Vec<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        self.search_record_details_fulltext_filtered(
            scope,
            query,
            top_k,
            &[],
            MemorySensitivityReadScope::Owner,
            None,
        )
        .await
    }

    pub async fn search_record_details_fulltext_filtered(
        &self,
        scope: &MemoryScopeContext,
        query: &str,
        top_k: i32,
        memory_types: &[String],
        read_scope: MemorySensitivityReadScope,
        metadata_filter: Option<&MetadataFilterExpression>,
    ) -> Result<Vec<NativeSqlMemoryRecordDetail>, NativeSqlStoreError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        let result_limit = i64::from(top_k.clamp(1, MAX_LIST_PAGE_SIZE));
        let sensitivity_level = Self::read_scope_level(read_scope);
        // Translated once, because a filter that only reached the LIKE fallback would stop
        // applying as soon as the full-text index answered instead.
        let filter_predicate = crate::filter_pushdown::translate_metadata_filter(
            self.dialect(),
            "r",
            metadata_filter,
        )?;
        match self.dialect() {
            MemorySqlDialect::Postgres => {
                let websearch_query = postgres_websearch_or_query(trimmed);
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
                      AND r.search_document @@ websearch_to_tsquery('simple', ?)
                    "#,
                );
                Self::append_sensitivity_filter(&mut sql, "r");
                Self::append_memory_type_filter(&mut sql, "r", memory_types);
                sql.push_str(&filter_predicate.sql);
                sql.push_str(
                    " ORDER BY ts_rank(r.search_document, websearch_to_tsquery('simple', ?)) DESC, r.uuid ASC LIMIT ?",
                );
                let mut query_builder = sqlx::query(&sql)
                    .bind(scope.tenant_id)
                    .bind(scope.space_id)
                    .bind(&websearch_query)
                    .bind(sensitivity_level)
                    .bind(sensitivity_level);
                for memory_type in memory_types {
                    query_builder = query_builder.bind(memory_type);
                }
                for bind in &filter_predicate.binds {
                    query_builder = query_builder.bind(bind);
                }
                let rows = query_builder
                    .bind(&websearch_query)
                    .bind(result_limit)
                    .fetch_all(self.pool())
                    .await?;
                Ok(rows.into_iter().map(record_detail_from_row).collect())
            }
            MemorySqlDialect::Sqlite => {
                let fts_query = escape_fts5_query(trimmed);
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
                      r.version,
                      sup.uuid AS supersedes_uuid,
                      sub.uuid AS superseded_by_uuid
                    FROM ai_record_fts
                    JOIN ai_record r ON r.id = ai_record_fts.rowid
                    LEFT JOIN ai_record sup
                      ON sup.id = r.supersedes_memory_id AND sup.tenant_id = r.tenant_id
                    LEFT JOIN ai_record sub
                      ON sub.id = r.superseded_by_memory_id AND sub.tenant_id = r.tenant_id
                    WHERE ai_record_fts MATCH ?
                      AND r.tenant_id = ?
                      AND r.space_id = ?
                      AND r.status <> 'deleted'
                    "#,
                );
                Self::append_sensitivity_filter(&mut sql, "r");
                Self::append_memory_type_filter(&mut sql, "r", memory_types);
                sql.push_str(&filter_predicate.sql);
                sql.push_str(" ORDER BY rank, r.uuid ASC LIMIT ?");
                let mut query_builder = sqlx::query(&sql)
                    .bind(fts_query)
                    .bind(scope.tenant_id)
                    .bind(scope.space_id)
                    .bind(sensitivity_level)
                    .bind(sensitivity_level);
                for memory_type in memory_types {
                    query_builder = query_builder.bind(memory_type);
                }
                for bind in &filter_predicate.binds {
                    query_builder = query_builder.bind(bind);
                }
                let rows = query_builder
                    .bind(result_limit)
                    .fetch_all(self.pool())
                    .await?;
                Ok(rows.into_iter().map(record_detail_from_row).collect())
            }
        }
    }
}

/// Escape a user-supplied query string for safe use as an FTS5 MATCH expression.
///
/// Each whitespace-delimited term is wrapped in double quotes (FTS5 phrase
/// syntax) with any embedded double quotes doubled (`"` → `""`).  This
/// neutralises all FTS5 operators (`*`, `(`, `)`, `AND`, `OR`, `NOT`, `NEAR`,
/// `^`, `-`, `:`) and prevents query injection.
fn escape_fts5_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| {
            let escaped = term.replace('"', "\"\"");
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn postgres_websearch_or_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| format!("\"{}\"", term.replace('"', " ")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fts5_escape_wraps_terms_in_quotes() {
        let result = escape_fts5_query("hello world");
        assert_eq!(result, "\"hello\" OR \"world\"");
    }

    #[test]
    fn fts5_escape_neutralises_operators() {
        let result = escape_fts5_query("memory OR 1=1");
        assert_eq!(result, "\"memory\" OR \"OR\" OR \"1=1\"");
    }

    #[test]
    fn fts5_escape_doubles_internal_quotes() {
        let result = escape_fts5_query("say \"hello\"");
        assert_eq!(result, "\"say\" OR \"\"\"hello\"\"\"");
    }

    #[test]
    fn fts5_escape_handles_empty_input() {
        let result = escape_fts5_query("   ");
        assert_eq!(result, "");
    }

    #[test]
    fn postgres_websearch_query_matches_sqlite_or_semantics() {
        assert_eq!(
            postgres_websearch_or_query("alpha beta"),
            "\"alpha\" OR \"beta\""
        );
    }

    #[test]
    fn fulltext_result_limit_is_bounded() {
        assert_eq!(i64::from(i32::MIN.clamp(1, MAX_LIST_PAGE_SIZE)), 1);
        assert_eq!(
            i64::from(i32::MAX.clamp(1, MAX_LIST_PAGE_SIZE)),
            i64::from(MAX_LIST_PAGE_SIZE)
        );
    }

    #[test]
    fn only_missing_fulltext_capabilities_are_recoverable() {
        for message in [
            "no such table: ai_record_fts",
            "no such module: fts5",
            "column r.search_document does not exist",
            "function websearch_to_tsquery(regconfig, text) does not exist",
        ] {
            assert!(is_fulltext_unavailable_message(message));
        }
        for message in [
            "permission denied for table ai_record_fts",
            "database connection is closed",
            "syntax error near MATCH",
        ] {
            assert!(!is_fulltext_unavailable_message(message));
        }
    }
}
