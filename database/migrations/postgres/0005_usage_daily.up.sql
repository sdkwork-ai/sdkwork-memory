-- sdkwork:migration
-- id: 0005_usage_daily
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Usage metering facts for the commercial closed loop: one
--   cumulative counter row per (tenant, day, metric), charged inside the same
--   transaction as the journaled mutation that produced it (canonical record
--   create/delete and other mutation-class journal actions) or best-effort
--   after a stored retrieval trace. Rows are append-only aggregates and
--   retention never cleans them because the table stays naturally bounded by
--   days x metrics per tenant. SQLite parity:
--   tests/fixtures/database/sqlite/migrations/0018_usage_daily.up.sql.
-- reversible: true
-- rollback: down-migration (drops the usage daily indexes and the table;
--   dropping the table discards the accumulated metering facts, which is the
--   only way to reverse a created table)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

CREATE TABLE IF NOT EXISTS ai_usage_daily (
  id BIGINT NOT NULL PRIMARY KEY,
  tenant_id BIGINT NOT NULL,
  day TEXT NOT NULL,
  metric TEXT NOT NULL,
  delta BIGINT NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  version BIGINT NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_usage_daily_day_metric
  ON ai_usage_daily (tenant_id, day, metric);

CREATE INDEX IF NOT EXISTS idx_ai_usage_daily_tenant_day
  ON ai_usage_daily (tenant_id, day);

COMMIT;
