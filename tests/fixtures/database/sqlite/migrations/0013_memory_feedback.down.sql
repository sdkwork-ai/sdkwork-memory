-- Reverse 0013_memory_feedback for sqlite.
DROP INDEX IF EXISTS idx_ai_feedback_target;
DROP INDEX IF EXISTS uk_ai_feedback_uuid;
DROP TABLE IF EXISTS ai_feedback;
