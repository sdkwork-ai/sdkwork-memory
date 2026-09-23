DROP INDEX IF EXISTS idx_ai_eval_run_execution_lease;
DROP INDEX IF EXISTS idx_ai_learning_job_execution_lease;

ALTER TABLE ai_eval_run DROP COLUMN lease_expires_at;
ALTER TABLE ai_eval_run DROP COLUMN lease_token;
ALTER TABLE ai_eval_run DROP COLUMN lease_owner;
ALTER TABLE ai_learning_job DROP COLUMN lease_expires_at;
ALTER TABLE ai_learning_job DROP COLUMN lease_token;
ALTER TABLE ai_learning_job DROP COLUMN lease_owner;
