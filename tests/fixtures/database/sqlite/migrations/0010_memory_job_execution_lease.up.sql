ALTER TABLE ai_learning_job ADD COLUMN lease_owner TEXT;
ALTER TABLE ai_learning_job ADD COLUMN lease_token TEXT;
ALTER TABLE ai_learning_job ADD COLUMN lease_expires_at TEXT;

ALTER TABLE ai_eval_run ADD COLUMN lease_owner TEXT;
ALTER TABLE ai_eval_run ADD COLUMN lease_token TEXT;
ALTER TABLE ai_eval_run ADD COLUMN lease_expires_at TEXT;

CREATE INDEX IF NOT EXISTS idx_ai_learning_job_execution_lease
  ON ai_learning_job (state, lease_expires_at, priority, id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_run_execution_lease
  ON ai_eval_run (state, lease_expires_at, id);
