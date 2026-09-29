-- sdkwork:migration
-- id: 0003_hard_delete_cleanup_indexes
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Index the reverse-reference columns the per-record hard-delete
--   cleanup updates (ai_habit.promoted_memory_id, ai_edge.source_memory_id,
--   ai_memory_binding.source_memory_id / target_memory_id). Without them a
--   forget sweep runs one tenant-wide table scan per deleted record while
--   holding the deleted row's locks inside its transaction, which serializes
--   concurrent forgets against live traffic on large tenants.
-- reversible: true
-- rollback: down-migration (drops the added indexes; both are data-preserving
--   structural reversals)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

-- Cleanup predicate: UPDATE ai_habit ... WHERE tenant_id = ? AND
-- promoted_memory_id = ?.
CREATE INDEX IF NOT EXISTS idx_ai_habit_promoted_memory
  ON ai_habit (tenant_id, promoted_memory_id);

-- Cleanup predicate: UPDATE ai_edge ... WHERE tenant_id = ? AND
-- source_memory_id = ?.
CREATE INDEX IF NOT EXISTS idx_ai_edge_source_memory
  ON ai_edge (tenant_id, source_memory_id);

-- Cleanup predicate: UPDATE ai_memory_binding ... WHERE tenant_id = ? AND
-- (source_memory_id = ? OR target_memory_id = ?); each side of the OR is one
-- index seek.
CREATE INDEX IF NOT EXISTS idx_ai_memory_binding_source
  ON ai_memory_binding (tenant_id, source_memory_id);

CREATE INDEX IF NOT EXISTS idx_ai_memory_binding_target
  ON ai_memory_binding (tenant_id, target_memory_id);

COMMIT;
