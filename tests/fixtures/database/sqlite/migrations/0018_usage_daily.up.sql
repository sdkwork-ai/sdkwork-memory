-- Usage metering facts (SQLite parity of PostgreSQL migration
-- 0005_usage_daily): one cumulative counter row per (tenant, day, metric),
-- charged inside the same transaction as the journaled mutation that produced
-- it, or best-effort after a stored retrieval trace. Rows are append-only
-- aggregates; retention never cleans them because the table stays naturally
-- bounded by days x metrics per tenant.

CREATE TABLE IF NOT EXISTS ai_usage_daily (
  id BIGINT NOT NULL PRIMARY KEY,
  tenant_id INTEGER NOT NULL,
  day TEXT NOT NULL,
  metric TEXT NOT NULL,
  delta INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_usage_daily_day_metric
  ON ai_usage_daily (tenant_id, day, metric);

CREATE INDEX IF NOT EXISTS idx_ai_usage_daily_tenant_day
  ON ai_usage_daily (tenant_id, day);
