-- Extend ai_eval_run to align with schema-registry 005-memory-governance.yaml.
-- Adds dataset_ref, profile_ref, result_json, started_at, finished_at columns
-- declared in the design contract but absent from 0001_memory_phase1.
--
-- Note: SQLite ALTER TABLE ADD COLUMN has no IF NOT EXISTS clause; the
-- migration runner tracks applied migrations so this runs at most once.

ALTER TABLE ai_eval_run ADD COLUMN dataset_ref TEXT;
ALTER TABLE ai_eval_run ADD COLUMN profile_ref TEXT;
ALTER TABLE ai_eval_run ADD COLUMN result_json TEXT;
ALTER TABLE ai_eval_run ADD COLUMN started_at TEXT;
ALTER TABLE ai_eval_run ADD COLUMN finished_at TEXT;
