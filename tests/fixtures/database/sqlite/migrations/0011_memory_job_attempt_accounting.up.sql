-- Bounded retry accounting for job execution leases (SQLite parity of the
-- PostgreSQL baseline `attempt_count` columns). Workers increment the counter
-- every time a stale running job or eval run is requeued and move rows past
-- the attempt ceiling to the terminal `dead` state instead of looping forever.

ALTER TABLE ai_learning_job ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE ai_eval_run ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0;
