-- List/keyset pagination indexes (SQLite parity of the PostgreSQL baseline).
-- Matches the documented list contracts' WHERE + ORDER BY so keyset seeks stay
-- on the index path. The outbox claim index leads with publish_state by
-- design: outbox workers scan across tenants.

CREATE INDEX IF NOT EXISTS idx_ai_record_space_uuid
  ON ai_record (tenant_id, space_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_space_created
  ON ai_retrieval_trace (tenant_id, space_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_ai_audit_resource_id
  ON ai_audit_log (tenant_id, resource_type, id DESC);

CREATE INDEX IF NOT EXISTS idx_ai_learning_job_history
  ON ai_learning_job (tenant_id, job_type, id DESC);

CREATE INDEX IF NOT EXISTS idx_ai_outbox_claim_order
  ON ai_outbox_event (publish_state, created_at, id);
