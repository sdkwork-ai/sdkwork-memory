-- Reverse of 0017_candidate_job_linkage: drop the candidate-to-learning-job
-- linkage column, its partial index, and the eval-run version counter.
-- Data-destroying by definition (the linkage values are dropped with the
-- column), which is the only way to reverse an added column.

DROP INDEX IF EXISTS idx_ai_candidate_learning_job;
ALTER TABLE ai_candidate DROP COLUMN learning_job_uuid;
ALTER TABLE ai_eval_run DROP COLUMN version;
