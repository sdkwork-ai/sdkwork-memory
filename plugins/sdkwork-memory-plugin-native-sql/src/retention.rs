//! Retention purges for terminal and derived high-churn tables.
//!
//! Every canonical mutation journals one `ai_event`, one `ai_outbox_event`,
//! and one `ai_audit_log` row; every retrieval persists a trace, one hit per
//! hit, and a context pack. Without scheduled purges those tables grow with
//! production traffic forever. The retention worker sweeps them in bounded
//! keyset batches (never one unbounded `DELETE`), ordered so referential
//! integrity holds without cascade support in either dialect.
//!
//! Canonical data (`ai_record`, `ai_event`) is deliberately NOT touched here:
//! its lifetime is product semantics (space retention jobs, privacy forget),
//! not mechanical cleanup.

use crate::store::{NativeSqlMemoryStore, NativeSqlStoreError};

/// Rows hard-deleted from the retrieval-trace family in one sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RetrievalTracePurgeStats {
    pub hits: u64,
    pub context_packs: u64,
    pub traces: u64,
}

/// Upper bound on rows removed per `DELETE` statement inside a purge loop.
const RETENTION_BATCH_LIMIT: i64 = 500;

fn cutoff_text(older_than_seconds: u64) -> String {
    let now_millis = sdkwork_utils_rust::to_unix_millis(sdkwork_utils_rust::now());
    let window_millis = i64::try_from(older_than_seconds.saturating_mul(1_000)).unwrap_or(i64::MAX);
    let cutoff = now_millis.saturating_sub(window_millis);
    sdkwork_utils_rust::format_datetime(
        sdkwork_utils_rust::from_unix_millis(cutoff).unwrap_or_else(sdkwork_utils_rust::now),
        None,
    )
}

impl NativeSqlMemoryStore {
    /// Hard-deletes terminal outbox events (`published`/`failed`) that reached
    /// their terminal state before the cutoff. Loops in bounded batches and
    /// returns the number of rows removed.
    pub async fn purge_terminal_outbox_events(
        &self,
        older_than_seconds: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        const SQL: &str = r#"
            DELETE FROM ai_outbox_event
            WHERE id IN (
                SELECT id FROM ai_outbox_event
                WHERE publish_state IN ('published', 'failed') AND updated_at <= ?
                ORDER BY id
                LIMIT ?
            )
        "#;
        self.purge_batches(SQL, older_than_seconds).await
    }

    /// Hard-deletes terminal learning jobs (`succeeded`/`failed`/`dead`)
    /// including jobs that exhausted their attempt ceiling, past the cutoff.
    pub async fn purge_terminal_learning_jobs(
        &self,
        older_than_seconds: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        const SQL: &str = r#"
            DELETE FROM ai_learning_job
            WHERE id IN (
                SELECT id FROM ai_learning_job
                WHERE state IN ('succeeded', 'failed', 'dead') AND updated_at <= ?
                ORDER BY id
                LIMIT ?
            )
        "#;
        self.purge_batches(SQL, older_than_seconds).await
    }

    /// Hard-deletes terminal evaluation runs past the cutoff.
    pub async fn purge_terminal_eval_runs(
        &self,
        older_than_seconds: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        const SQL: &str = r#"
            DELETE FROM ai_eval_run
            WHERE id IN (
                SELECT id FROM ai_eval_run
                WHERE state IN ('succeeded', 'failed', 'dead') AND updated_at <= ?
                ORDER BY id
                LIMIT ?
            )
        "#;
        self.purge_batches(SQL, older_than_seconds).await
    }

    /// Hard-deletes the retrieval-trace family (hits, then context packs, then
    /// traces) past the cutoff. The children go first because neither dialect
    /// declares the foreign keys with cascade, and the retention window is one
    /// shared value so ordering only has to hold within a sweep.
    pub async fn purge_retrieval_traces(
        &self,
        older_than_seconds: u64,
    ) -> Result<RetrievalTracePurgeStats, NativeSqlStoreError> {
        const HITS_SQL: &str = r#"
            DELETE FROM ai_retrieval_hit
            WHERE id IN (
                SELECT hit.id FROM ai_retrieval_hit hit
                JOIN ai_retrieval_trace trace ON trace.id = hit.retrieval_trace_id
                WHERE trace.created_at <= ?
                ORDER BY hit.id
                LIMIT ?
            )
        "#;
        const PACKS_SQL: &str = r#"
            DELETE FROM ai_context_pack
            WHERE id IN (
                SELECT pack.id FROM ai_context_pack pack
                JOIN ai_retrieval_trace trace ON trace.id = pack.retrieval_trace_id
                WHERE trace.created_at <= ?
                ORDER BY pack.id
                LIMIT ?
            )
        "#;
        const TRACES_SQL: &str = r#"
            DELETE FROM ai_retrieval_trace
            WHERE id IN (
                SELECT id FROM ai_retrieval_trace
                WHERE created_at <= ?
                ORDER BY id
                LIMIT ?
            )
        "#;
        let hits = self.purge_batches(HITS_SQL, older_than_seconds).await?;
        let context_packs = self
            .purge_batches(PACKS_SQL, older_than_seconds)
            .await?;
        let traces = self.purge_batches(TRACES_SQL, older_than_seconds).await?;
        Ok(RetrievalTracePurgeStats {
            hits,
            context_packs,
            traces,
        })
    }

    /// Hard-deletes audit log rows past the cutoff.
    ///
    /// Governance jobs (forget, export, consolidation, index rebuild) load
    /// their typed snapshots back out of this table, so the configured window
    /// defines how long job history stays queryable; operators choose it per
    /// compliance needs.
    pub async fn purge_audit_logs(
        &self,
        older_than_seconds: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        const SQL: &str = r#"
            DELETE FROM ai_audit_log
            WHERE id IN (
                SELECT id FROM ai_audit_log
                WHERE created_at <= ?
                ORDER BY id
                LIMIT ?
            )
        "#;
        self.purge_batches(SQL, older_than_seconds).await
    }

    /// Child-table purge for the retrieval family: the child references its
    /// trace through `reference_column`, so the cutoff predicate joins on the
    /// parent trace's `created_at`.
    /// Bounded-batch delete loop shared by the single-table purges: each
    /// statement removes at most `RETENTION_BATCH_LIMIT` rows selected by the
    /// statement's own inner `ORDER BY id LIMIT ?`, and the loop ends when a
    /// batch comes back under the limit.
    async fn purge_batches(
        &self,
        sql: &'static str,
        older_than_seconds: u64,
    ) -> Result<u64, NativeSqlStoreError> {
        let cutoff = cutoff_text(older_than_seconds);
        let mut purged: u64 = 0;
        loop {
            let result = sqlx::query(sql)
                .bind(&cutoff)
                .bind(RETENTION_BATCH_LIMIT)
                .execute(self.pool())
                .await?;
            let removed: u64 = result.rows_affected();
            purged = purged.saturating_add(removed);
            if removed < u64::try_from(RETENTION_BATCH_LIMIT).unwrap_or(u64::MAX) {
                return Ok(purged);
            }
        }
    }
}
