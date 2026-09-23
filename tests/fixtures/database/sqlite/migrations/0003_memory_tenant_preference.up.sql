-- Tenant-level and user-level preference store (schema-registry 005-memory-governance.yaml).
-- SQLite stores tenant-scoped rows with user_id = -1 (see preference_scope_user_binding).
-- SQLite stores tenant-scoped rows with user_id = -1 (see preference_user_storage_key).

CREATE TABLE IF NOT EXISTS ai_tenant_preference (
  id BIGINT NOT NULL PRIMARY KEY,
  tenant_id INTEGER NOT NULL,
  user_id INTEGER,
  preference_key TEXT NOT NULL,
  preference_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_tenant_preference_scope
  ON ai_tenant_preference (tenant_id, user_id, preference_key);
