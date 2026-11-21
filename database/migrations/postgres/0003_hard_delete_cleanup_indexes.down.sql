-- sdkwork:migration
-- id: 0003_hard_delete_cleanup_indexes
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Reverse of 0003_hard_delete_cleanup_indexes: drop the hard-delete
--   cleanup reverse-reference indexes.
-- reversible: true
-- rollback: down-migration (this file is the down migration)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

DROP INDEX IF EXISTS idx_ai_habit_promoted_memory;
DROP INDEX IF EXISTS idx_ai_edge_source_memory;
DROP INDEX IF EXISTS idx_ai_memory_binding_source;
DROP INDEX IF EXISTS idx_ai_memory_binding_target;

COMMIT;
