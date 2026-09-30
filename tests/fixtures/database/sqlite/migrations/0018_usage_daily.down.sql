-- Reverse of 0018_usage_daily: drop the usage daily indexes and the metering
-- table. Data-destroying by definition (the accumulated usage facts are
-- dropped with the table), which is the only way to reverse a created table.

DROP INDEX IF EXISTS idx_ai_usage_daily_tenant_day;
DROP INDEX IF EXISTS uk_ai_usage_daily_day_metric;

DROP TABLE IF EXISTS ai_usage_daily;
