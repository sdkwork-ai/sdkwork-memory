-- sdkwork:migration
-- id: 0004_candidate_job_linkage
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Extraction idempotency linkage for candidates: records which
--   learning job produced each ai_candidate row. The column stays NULL for
--   candidates created outside a background extraction job, and the partial
--   index only covers the linked rows so per-job lookups and dedup checks
--   stay small. Also adds ai_eval_run.version so update_eval_run_state can
--   charge an optimistic counter per terminal transition, matching
--   ai_learning_job. SQLite parity: tests/fixtures/database/sqlite/migrations/
--   0017_candidate_job_linkage.up.sql.
-- reversible: true
-- rollback: down-migration (drops the partial index and the added columns;
--   dropping the linkage column discards its values, which is the only way
--   to reverse an added column)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE ai_candidate ADD COLUMN IF NOT EXISTS learning_job_uuid TEXT;

CREATE INDEX IF NOT EXISTS idx_ai_candidate_learning_job
  ON ai_candidate (tenant_id, learning_job_uuid)
  WHERE learning_job_uuid IS NOT NULL;

ALTER TABLE ai_eval_run ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 0;

COMMIT;
