-- Claim-order and keyset-list indexes for the hot worker and list paths,
-- mirroring the PostgreSQL 0002 migration: learning/eval claim ordering,
-- retrieval-trace id cursors, space-scoped event/candidate uuid cursors, and
-- the per-user bulk-delete path. The learning-job state CHECK already matches
-- the real state machine on SQLite (no dead 'accepted' vocabulary).

CREATE INDEX IF NOT EXISTS idx_ai_learning_job_claim_order
  ON ai_learning_job (state, priority DESC, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_ai_eval_run_claim_order
  ON ai_eval_run (state, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_tenant_id
  ON ai_retrieval_trace (tenant_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_space_id
  ON ai_retrieval_trace (tenant_id, space_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_ai_event_space_uuid
  ON ai_event (tenant_id, space_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_candidate_space_uuid
  ON ai_candidate (tenant_id, space_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_record_space_user
  ON ai_record (tenant_id, space_id, user_id);
