-- SQLite FTS5 search index for ai_record keyword retrieval.

CREATE VIRTUAL TABLE IF NOT EXISTS ai_record_fts USING fts5(
  memory_uuid UNINDEXED,
  tenant_id UNINDEXED,
  space_id UNINDEXED,
  canonical_text,
  object_text,
  subject,
  tokenize = 'unicode61 remove_diacritics 1'
);

INSERT INTO ai_record_fts(rowid, memory_uuid, tenant_id, space_id, canonical_text, object_text, subject)
SELECT id, uuid, tenant_id, space_id,
       coalesce(canonical_text, ''), coalesce(object_text, ''), coalesce(subject, '')
FROM ai_record
WHERE status <> 'deleted'
  AND id NOT IN (SELECT rowid FROM ai_record_fts);
