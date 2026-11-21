-- sdkwork:migration
-- id: 0005_usage_daily
-- engine: postgres
-- module: sdkwork-memory
-- purpose: Reverse of 0005_usage_daily: drop the usage daily indexes and the
--   metering table. Data-destroying by definition (the accumulated usage
--   facts are dropped with the table), which is the only way to reverse a
--   created table.
-- reversible: true
-- rollback: down-migration (this file is the down migration)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

DROP INDEX IF EXISTS idx_ai_usage_daily_tenant_day;
DROP INDEX IF EXISTS uk_ai_usage_daily_day_metric;

DROP TABLE IF EXISTS ai_usage_daily;

COMMIT;
