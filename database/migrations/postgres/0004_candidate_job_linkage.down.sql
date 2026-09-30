-- sdkwork:migration
-- id: 0004_candidate_job_linkage
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Reverse of 0004_candidate_job_linkage: drop the partial index, the
--   candidate-to-learning-job linkage column, and the eval-run version
--   counter. Data-destroying by definition (the linkage values are dropped
--   with the column).
-- reversible: true
-- rollback: down-migration (this file is the down migration)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

DROP INDEX IF EXISTS idx_ai_candidate_learning_job;

ALTER TABLE ai_candidate DROP COLUMN IF EXISTS learning_job_uuid;

ALTER TABLE ai_eval_run DROP COLUMN IF EXISTS version;

COMMIT;
