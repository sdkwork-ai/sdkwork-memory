-- sdkwork:migration
-- id: 0002_claim_order_and_keyset_indexes
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Reverse of 0002_claim_order_and_keyset_indexes: restore the wider
--   learning-job state CHECK and drop the claim-order/keyset indexes.
-- reversible: true
-- rollback: down-migration (this file is the down migration)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE ai_learning_job DROP CONSTRAINT ai_learning_job_state_check;

ALTER TABLE ai_learning_job ADD CONSTRAINT ai_learning_job_state_check
  CHECK (state IN ('accepted', 'queued', 'running', 'succeeded', 'failed', 'dead'));

ALTER TABLE ai_learning_job DROP COLUMN IF EXISTS next_attempt_at;
ALTER TABLE ai_eval_run DROP COLUMN IF EXISTS next_attempt_at;
ALTER TABLE ai_eval_run DROP COLUMN IF EXISTS error_json;

DROP INDEX IF EXISTS idx_ai_learning_job_claim_order;
DROP INDEX IF EXISTS idx_ai_eval_run_claim_order;
DROP INDEX IF EXISTS idx_ai_retrieval_trace_tenant_id;
DROP INDEX IF EXISTS idx_ai_retrieval_trace_space_id;
DROP INDEX IF EXISTS idx_ai_event_space_uuid;
DROP INDEX IF EXISTS idx_ai_candidate_space_uuid;
DROP INDEX IF EXISTS idx_ai_record_space_user;

COMMIT;
