-- Hard-delete cleanup reverse-reference indexes, mirroring the PostgreSQL 0003
-- migration: the per-record hard-delete cleanup updates ai_habit by promoted
-- memory, ai_edge by source memory, and ai_memory_binding by either endpoint.
-- Without them a forget sweep runs one full table scan per deleted record.

CREATE INDEX IF NOT EXISTS idx_ai_habit_promoted_memory
  ON ai_habit (tenant_id, promoted_memory_id);

CREATE INDEX IF NOT EXISTS idx_ai_edge_source_memory
  ON ai_edge (tenant_id, source_memory_id);

CREATE INDEX IF NOT EXISTS idx_ai_memory_binding_source
  ON ai_memory_binding (tenant_id, source_memory_id);

CREATE INDEX IF NOT EXISTS idx_ai_memory_binding_target
  ON ai_memory_binding (tenant_id, target_memory_id);
