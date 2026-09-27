-- Execution-failure retry visibility for the job queues (SQLite parity of
-- PostgreSQL migration 0002): a job whose execution errored requeues as
-- 'queued' with next_attempt_at in the future, and claims skip it until the
-- backoff elapses. Stale-crash requeues leave it NULL (immediately due).

ALTER TABLE ai_learning_job ADD COLUMN next_attempt_at TEXT;
ALTER TABLE ai_eval_run ADD COLUMN next_attempt_at TEXT;
ALTER TABLE ai_eval_run ADD COLUMN error_json TEXT;
