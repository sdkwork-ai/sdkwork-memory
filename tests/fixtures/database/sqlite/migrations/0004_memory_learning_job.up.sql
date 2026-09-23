-- Async learning/governance job queue (schema-registry 002-memory-learning.yaml).

CREATE TABLE IF NOT EXISTS ai_learning_job (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER REFERENCES ai_space(id),
  job_type TEXT NOT NULL,
  state TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  idempotency_key TEXT,
  input_json TEXT,
  result_json TEXT,
  error_json TEXT,
  started_at TEXT,
  finished_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_learning_job_uuid
  ON ai_learning_job (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_learning_job_idempotency
  ON ai_learning_job (tenant_id, job_type, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_ai_learning_job_state
  ON ai_learning_job (tenant_id, job_type, state, priority DESC, created_at ASC);
