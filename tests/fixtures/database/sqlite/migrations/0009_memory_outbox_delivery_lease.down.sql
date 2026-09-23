DROP INDEX IF EXISTS idx_ai_outbox_event_delivery_lease;

ALTER TABLE ai_outbox_event DROP COLUMN next_attempt_at;
ALTER TABLE ai_outbox_event DROP COLUMN lease_expires_at;
ALTER TABLE ai_outbox_event DROP COLUMN lease_token;
ALTER TABLE ai_outbox_event DROP COLUMN lease_owner;
