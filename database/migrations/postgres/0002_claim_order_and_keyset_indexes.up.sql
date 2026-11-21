-- sdkwork:migration
-- id: 0002_claim_order_and_keyset_indexes
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Add claim-order and keyset-list indexes for the hot paths the
--   worker and list workloads exercise (learning/eval claim ordering,
--   retrieval-trace id cursors, space-scoped event/candidate uuid cursors,
--   per-user bulk delete), and narrow the learning-job state CHECK to the
--   states the writer actually produces: jobs are only ever inserted as
--   'queued', so the dead 'accepted' vocabulary diverged from the SQLite
--   schema without ever being writable.
-- reversible: true
-- rollback: down-migration (drops the added indexes and restores the wider
--   learning-job state CHECK; both are data-preserving structural reversals)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE ai_learning_job DROP CONSTRAINT ai_learning_job_state_check;

ALTER TABLE ai_learning_job ADD CONSTRAINT ai_learning_job_state_check
  CHECK (state IN ('queued', 'running', 'succeeded', 'failed', 'dead'));

-- Execution-failure retry visibility: a job whose execution errored requeues
-- as 'queued' with next_attempt_at in the future, and claims skip it until
-- the backoff elapses. Stale-crash requeues leave it NULL (immediately due).
ALTER TABLE ai_learning_job ADD COLUMN IF NOT EXISTS next_attempt_at TEXT;
ALTER TABLE ai_eval_run ADD COLUMN IF NOT EXISTS next_attempt_at TEXT;
-- Dead-letter reason carrier: terminal-dead runs record why they died, on
-- parity with ai_learning_job.error_json.
ALTER TABLE ai_eval_run ADD COLUMN IF NOT EXISTS error_json TEXT;

-- Claim path: WHERE state = 'queued' ORDER BY priority DESC, created_at ASC.
-- The existing (state, lease_expires_at, priority, id) lease index cannot
-- serve this ordering, so every worker poll re-sorted the queued backlog.
CREATE INDEX IF NOT EXISTS idx_ai_learning_job_claim_order
  ON ai_learning_job (state, priority DESC, created_at ASC);

-- Eval claim path: WHERE state IN (...) ORDER BY created_at ASC.
CREATE INDEX IF NOT EXISTS idx_ai_eval_run_claim_order
  ON ai_eval_run (state, created_at ASC);

-- Retrieval-trace keyset cursors: ORDER BY id DESC with tenant (and optional
-- space) predicates. The (tenant, space, created_at DESC, id DESC) index
-- cannot serve an id-ordered cursor.
CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_tenant_id
  ON ai_retrieval_trace (tenant_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_space_id
  ON ai_retrieval_trace (tenant_id, space_id, id DESC);

-- Space-scoped event/candidate lists page with `uuid > ? ORDER BY uuid ASC`;
-- the unique (tenant_id, uuid) indexes leave space_id as a residual filter
-- over the space's full history.
CREATE INDEX IF NOT EXISTS idx_ai_event_space_uuid
  ON ai_event (tenant_id, space_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_candidate_space_uuid
  ON ai_candidate (tenant_id, space_id, uuid);

-- Bulk per-user delete filters tenant + space + user + status; the existing
-- (tenant_id, user_id, ...) index has no space leading path.
CREATE INDEX IF NOT EXISTS idx_ai_record_space_user
  ON ai_record (tenant_id, space_id, user_id);

COMMIT;
