-- Reverse 0011_memory_job_attempt_accounting for sqlite.
ALTER TABLE ai_eval_run DROP COLUMN attempt_count;
ALTER TABLE ai_learning_job DROP COLUMN attempt_count;
