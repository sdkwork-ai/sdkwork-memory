-- Reverse 0006_ai_eval_run_extend for sqlite.
--
-- SQLite does not support DROP COLUMN prior to 3.35.0 and has no IF EXISTS
-- clause for column drops. The recommended rollback path for sqlite is to
-- recreate the table without the columns; this down migration is intentionally
-- a no-op placeholder to keep the down/up contract symmetric. Operators that
-- need a true rollback on sqlite should rebuild the table manually or restore
-- from a snapshot taken before 0006 was applied.

-- No-op: sqlite cannot DROP COLUMN portably across supported versions.
