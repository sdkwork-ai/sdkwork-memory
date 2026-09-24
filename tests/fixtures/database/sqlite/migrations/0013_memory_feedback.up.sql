-- Durable user feedback signals (SQLite parity of ai_feedback in the
-- PostgreSQL baseline). The audit log keeps the governance event; this table
-- is the queryable per-target feedback record.

CREATE TABLE IF NOT EXISTS ai_feedback (
  id INTEGER PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL,
  actor_id TEXT,
  target_type TEXT NOT NULL,
  target_id INTEGER NOT NULL,
  feedback_type TEXT NOT NULL,
  rating INTEGER,
  comment TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_feedback_uuid
  ON ai_feedback (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_feedback_target
  ON ai_feedback (tenant_id, space_id, target_type, target_id);
