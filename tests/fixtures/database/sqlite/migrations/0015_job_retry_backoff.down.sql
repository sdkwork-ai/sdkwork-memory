-- Reverse of 0015_job_retry_backoff: drop the retry-backoff visibility
-- columns from the job queues.

ALTER TABLE ai_learning_job DROP COLUMN next_attempt_at;
ALTER TABLE ai_eval_run DROP COLUMN next_attempt_at;
ALTER TABLE ai_eval_run DROP COLUMN error_json;
