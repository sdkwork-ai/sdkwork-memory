-- Reverse of 0016_hard_delete_cleanup_indexes: drop the hard-delete cleanup
-- reverse-reference indexes.

DROP INDEX IF EXISTS idx_ai_habit_promoted_memory;
DROP INDEX IF EXISTS idx_ai_edge_source_memory;
DROP INDEX IF EXISTS idx_ai_memory_binding_source;
DROP INDEX IF EXISTS idx_ai_memory_binding_target;
