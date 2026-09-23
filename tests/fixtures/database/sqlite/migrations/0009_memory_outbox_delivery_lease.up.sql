ALTER TABLE ai_outbox_event ADD COLUMN lease_owner TEXT;
ALTER TABLE ai_outbox_event ADD COLUMN lease_token TEXT;
ALTER TABLE ai_outbox_event ADD COLUMN lease_expires_at TEXT;
ALTER TABLE ai_outbox_event ADD COLUMN next_attempt_at TEXT;

CREATE INDEX IF NOT EXISTS idx_ai_outbox_event_delivery_lease
  ON ai_outbox_event (publish_state, next_attempt_at, lease_expires_at, id);
