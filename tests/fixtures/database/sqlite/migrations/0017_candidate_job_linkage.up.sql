-- Extraction idempotency linkage for candidates: records which learning job
-- produced each ai_candidate row (SQLite parity of PostgreSQL migration
-- 0004_candidate_job_linkage). The column stays NULL for candidates created
-- outside a background extraction job, and the partial index only covers the
-- linked rows so per-job lookups and dedup checks stay small.
--
-- Also adds ai_eval_run.version (SQLite parity of the same PostgreSQL
-- migration): update_eval_run_state charges it on every terminal transition,
-- matching ai_learning_job's optimistic-count semantics.

ALTER TABLE ai_candidate ADD COLUMN learning_job_uuid TEXT;

CREATE INDEX IF NOT EXISTS idx_ai_candidate_learning_job
  ON ai_candidate (tenant_id, learning_job_uuid)
  WHERE learning_job_uuid IS NOT NULL;

ALTER TABLE ai_eval_run ADD COLUMN version INTEGER NOT NULL DEFAULT 0;
