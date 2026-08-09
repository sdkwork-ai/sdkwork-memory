-- Generated from canonical application-root migrations.
-- Do not edit this folded baseline directly; run `pnpm db:materialize:baseline`.

-- source: database/migrations/sqlite/0001_memory_schema.up.sql
CREATE TABLE IF NOT EXISTS ai_space (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  organization_id INTEGER NOT NULL DEFAULT 0,
  owner_subject_type TEXT NOT NULL,
  owner_subject_id TEXT NOT NULL,
  space_type TEXT NOT NULL,
  display_name TEXT NOT NULL,
  default_scope TEXT NOT NULL,
  lifecycle_status TEXT NOT NULL,
  metadata_json TEXT,
  policy_json TEXT,
  created_by INTEGER,
  updated_by INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_space_uuid
  ON ai_space (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_space_owner_type
  ON ai_space (tenant_id, owner_subject_type, owner_subject_id, space_type);

CREATE TABLE IF NOT EXISTS ai_event (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL,
  user_id INTEGER,
  actor_type TEXT NOT NULL,
  actor_id TEXT,
  session_id TEXT,
  trace_id TEXT,
  request_id TEXT,
  idempotency_key TEXT,
  event_type TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_ref TEXT,
  event_time TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  payload_hash TEXT NOT NULL,
  sensitivity_level TEXT NOT NULL,
  ingestion_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (space_id) REFERENCES ai_space(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_event_uuid
  ON ai_event (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_event_idempotency
  ON ai_event (tenant_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE TABLE IF NOT EXISTS ai_record (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL,
  user_id INTEGER,
  scope TEXT NOT NULL,
  memory_type TEXT NOT NULL,
  subject TEXT,
  predicate TEXT,
  object_text TEXT NOT NULL,
  canonical_text TEXT NOT NULL,
  summary_text TEXT,
  language TEXT,
  confidence REAL NOT NULL,
  evidence_count INTEGER NOT NULL DEFAULT 0,
  contradiction_count INTEGER NOT NULL DEFAULT 0,
  importance_score REAL NOT NULL,
  recency_score REAL NOT NULL,
  habit_strength REAL,
  valid_from TEXT,
  valid_to TEXT,
  expires_at TEXT,
  status TEXT NOT NULL,
  sensitivity_level TEXT NOT NULL,
  metadata_json TEXT,
  tags_json TEXT,
  supersedes_memory_id INTEGER,
  superseded_by_memory_id INTEGER,
  created_by INTEGER,
  updated_by INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (space_id) REFERENCES ai_space(id),
  FOREIGN KEY (supersedes_memory_id) REFERENCES ai_record(id),
  FOREIGN KEY (superseded_by_memory_id) REFERENCES ai_record(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_record_uuid
  ON ai_record (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_record_scope_type_status
  ON ai_record (tenant_id, space_id, scope, memory_type, status, updated_at);

CREATE TABLE IF NOT EXISTS ai_record_source (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  memory_id INTEGER NOT NULL,
  event_id INTEGER NOT NULL,
  source_role TEXT NOT NULL,
  confidence_delta REAL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (memory_id) REFERENCES ai_record(id),
  FOREIGN KEY (event_id) REFERENCES ai_event(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_record_source_pair
  ON ai_record_source (tenant_id, memory_id, event_id, source_role);

CREATE TABLE IF NOT EXISTS ai_candidate (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL,
  user_id INTEGER,
  candidate_type TEXT NOT NULL,
  memory_type TEXT NOT NULL,
  proposed_text TEXT NOT NULL,
  proposed_payload_json TEXT,
  target_memory_id INTEGER,
  evidence_json TEXT,
  confidence REAL NOT NULL,
  novelty_score REAL,
  risk_score REAL,
  decision_state TEXT NOT NULL,
  decision_reason TEXT,
  decided_by INTEGER,
  decided_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (space_id) REFERENCES ai_space(id),
  FOREIGN KEY (target_memory_id) REFERENCES ai_record(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_candidate_uuid
  ON ai_candidate (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_habit (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL,
  user_id INTEGER NOT NULL,
  habit_key TEXT NOT NULL,
  habit_type TEXT NOT NULL,
  description TEXT NOT NULL,
  stage TEXT NOT NULL,
  strength REAL NOT NULL,
  confidence REAL NOT NULL,
  support_count INTEGER NOT NULL DEFAULT 0,
  last_signal_at TEXT,
  promoted_memory_id INTEGER,
  decay_after TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (space_id) REFERENCES ai_space(id),
  FOREIGN KEY (promoted_memory_id) REFERENCES ai_record(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_habit_key
  ON ai_habit (tenant_id, space_id, user_id, habit_key);

CREATE TABLE IF NOT EXISTS ai_retrieval_trace (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER,
  retrieval_profile_id INTEGER,
  actor_id TEXT,
  query_text TEXT,
  query_hash TEXT NOT NULL,
  retrievers_json TEXT,
  latency_ms INTEGER,
  result_count INTEGER NOT NULL DEFAULT 0,
  degraded INTEGER NOT NULL DEFAULT 0,
  metadata_json TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_retrieval_trace_uuid
  ON ai_retrieval_trace (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_retrieval_hit (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  retrieval_trace_id INTEGER NOT NULL,
  memory_id INTEGER,
  retriever_name TEXT NOT NULL,
  result_rank INTEGER NOT NULL,
  raw_score REAL,
  fused_score REAL,
  explanation_json TEXT,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY (retrieval_trace_id) REFERENCES ai_retrieval_trace(id),
  FOREIGN KEY (memory_id) REFERENCES ai_record(id)
);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_hit_trace_rank
  ON ai_retrieval_hit (tenant_id, retrieval_trace_id, result_rank);

CREATE TABLE IF NOT EXISTS ai_context_pack (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  retrieval_trace_id INTEGER,
  actor_id TEXT,
  query_text TEXT,
  pack_json TEXT NOT NULL,
  estimated_tokens INTEGER NOT NULL,
  truncated INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  FOREIGN KEY (retrieval_trace_id) REFERENCES ai_retrieval_trace(id)
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_context_pack_uuid
  ON ai_context_pack (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_index (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER,
  index_kind TEXT NOT NULL,
  implementation_profile_id INTEGER,
  provider_binding_id INTEGER,
  schema_version TEXT NOT NULL,
  status TEXT NOT NULL,
  rebuild_cursor TEXT,
  config_json TEXT,
  last_rebuilt_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_index_uuid
  ON ai_index (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_retrieval_profile (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER,
  name TEXT NOT NULL,
  strategy TEXT NOT NULL,
  retrievers_json TEXT NOT NULL,
  fusion_policy_json TEXT,
  rerank_policy_json TEXT,
  top_k INTEGER NOT NULL,
  context_budget_tokens INTEGER NOT NULL,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_retrieval_profile_uuid
  ON ai_retrieval_profile (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_implementation_profile (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  implementation_kind TEXT NOT NULL,
  role TEXT NOT NULL,
  status TEXT NOT NULL,
  capability_json TEXT NOT NULL,
  config_json TEXT,
  rollout_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_implementation_profile_uuid
  ON ai_implementation_profile (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_provider_binding (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  provider_kind TEXT NOT NULL,
  provider_code TEXT NOT NULL,
  display_name TEXT NOT NULL,
  endpoint_ref TEXT,
  secret_ref TEXT,
  model_ref TEXT,
  capabilities_json TEXT NOT NULL,
  config_json TEXT,
  health_state TEXT NOT NULL,
  last_health_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_provider_binding_uuid
  ON ai_provider_binding (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_eval_run (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  eval_type TEXT NOT NULL,
  state TEXT NOT NULL,
  metrics_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_eval_run_uuid
  ON ai_eval_run (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_audit_log (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  actor_type TEXT NOT NULL,
  actor_id TEXT,
  action TEXT NOT NULL,
  resource_type TEXT NOT NULL,
  resource_id TEXT,
  request_id TEXT,
  trace_id TEXT,
  result TEXT NOT NULL,
  reason TEXT,
  metadata_json TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_audit_log_uuid
  ON ai_audit_log (tenant_id, uuid);

CREATE TABLE IF NOT EXISTS ai_outbox_event (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  event_version TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  publish_state TEXT NOT NULL,
  published_at TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_outbox_event_uuid
  ON ai_outbox_event (tenant_id, uuid);

-- source: database/migrations/sqlite/0002_memory_indexes.up.sql
-- Phase-1 secondary indexes materialized from docs/schema-registry (DATABASE_SPEC alignment).

CREATE INDEX IF NOT EXISTS idx_ai_space_tenant_status
  ON ai_space (tenant_id, lifecycle_status, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_event_space_time
  ON ai_event (tenant_id, space_id, event_time, id);

CREATE INDEX IF NOT EXISTS idx_ai_event_session_time
  ON ai_event (tenant_id, session_id, event_time);

CREATE INDEX IF NOT EXISTS idx_ai_event_type_time
  ON ai_event (tenant_id, event_type, event_time);

CREATE INDEX IF NOT EXISTS idx_ai_event_hash
  ON ai_event (tenant_id, payload_hash);

CREATE INDEX IF NOT EXISTS idx_ai_record_user_type
  ON ai_record (tenant_id, user_id, memory_type, status, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_record_subject_predicate
  ON ai_record (tenant_id, space_id, subject, predicate, status);

CREATE INDEX IF NOT EXISTS idx_ai_record_validity
  ON ai_record (tenant_id, valid_from, valid_to, expires_at);

CREATE INDEX IF NOT EXISTS idx_ai_record_supersession
  ON ai_record (tenant_id, supersedes_memory_id, superseded_by_memory_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_record_source_uuid
  ON ai_record_source (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_record_source_event
  ON ai_record_source (tenant_id, event_id);

CREATE INDEX IF NOT EXISTS idx_ai_candidate_state
  ON ai_candidate (tenant_id, space_id, decision_state, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_candidate_target
  ON ai_candidate (tenant_id, target_memory_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_habit_uuid
  ON ai_habit (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_habit_stage
  ON ai_habit (tenant_id, space_id, stage, confidence, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_profile_created
  ON ai_retrieval_trace (tenant_id, retrieval_profile_id, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_trace_actor_created
  ON ai_retrieval_trace (tenant_id, actor_id, created_at);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_retrieval_hit_uuid
  ON ai_retrieval_hit (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_hit_memory
  ON ai_retrieval_hit (tenant_id, memory_id, status);

CREATE INDEX IF NOT EXISTS idx_ai_context_pack_trace
  ON ai_context_pack (tenant_id, retrieval_trace_id);

CREATE INDEX IF NOT EXISTS idx_ai_context_pack_actor_created
  ON ai_context_pack (tenant_id, actor_id, created_at);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_index_kind_space
  ON ai_index (tenant_id, space_id, index_kind, schema_version);

CREATE INDEX IF NOT EXISTS idx_ai_index_status
  ON ai_index (tenant_id, space_id, index_kind, status);

CREATE INDEX IF NOT EXISTS idx_ai_retrieval_profile_scope
  ON ai_retrieval_profile (tenant_id, space_id, status, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_implementation_profile_kind
  ON ai_implementation_profile (tenant_id, implementation_kind, status);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_provider_binding_code
  ON ai_provider_binding (tenant_id, provider_kind, provider_code);

CREATE INDEX IF NOT EXISTS idx_ai_provider_binding_health
  ON ai_provider_binding (tenant_id, provider_kind, health_state, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_eval_run_type_state
  ON ai_eval_run (tenant_id, eval_type, state, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_audit_actor_time
  ON ai_audit_log (tenant_id, actor_type, actor_id, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_audit_resource_time
  ON ai_audit_log (tenant_id, resource_type, resource_id, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_audit_action_time
  ON ai_audit_log (tenant_id, action, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_outbox_state
  ON ai_outbox_event (tenant_id, publish_state, created_at);

-- source: database/migrations/sqlite/0003_memory_tenant_preference.up.sql
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

-- source: database/migrations/sqlite/0004_memory_learning_job.up.sql
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

-- source: database/migrations/sqlite/0005_memory_record_fulltext_search.up.sql
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

-- source: database/migrations/sqlite/0006_memory_eval_run_extend.up.sql
-- Extend ai_eval_run to align with schema-registry 005-memory-governance.yaml.
-- Adds dataset_ref, profile_ref, result_json, started_at, finished_at columns
-- declared in the design contract but absent from 0001_memory_phase1.
--
-- Note: SQLite ALTER TABLE ADD COLUMN has no IF NOT EXISTS clause; the
-- migration runner tracks applied migrations so this runs at most once.

ALTER TABLE ai_eval_run ADD COLUMN dataset_ref TEXT;
ALTER TABLE ai_eval_run ADD COLUMN profile_ref TEXT;
ALTER TABLE ai_eval_run ADD COLUMN result_json TEXT;
ALTER TABLE ai_eval_run ADD COLUMN started_at TEXT;
ALTER TABLE ai_eval_run ADD COLUMN finished_at TEXT;

-- source: database/migrations/sqlite/0007_memory_commercial_management.up.sql
-- Commercial memory management tables for sqlite.
-- Activates planned tables (ai_entity, ai_edge, ai_policy) and adds the
-- commercial management layer (ai_subject, ai_memory_binding,
-- ai_capability_binding, ai_policy_assignment, ai_relation_rebuild_job,
-- ai_commercial_readiness_snapshot) per schema-registry 006-memory-commercial-management.yaml.
--
-- sqlite stores JSON as TEXT and timestamps as TEXT (ISO8601 UTC). Foreign
-- keys are declared but only enforced when PRAGMA foreign_keys = ON.

-- Activate ai_entity (previously planned in 001-memory-core.yaml).
CREATE TABLE IF NOT EXISTS ai_entity (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL REFERENCES ai_space(id),
  entity_type TEXT NOT NULL,
  canonical_name TEXT NOT NULL,
  aliases_json TEXT,
  attributes_json TEXT,
  sensitivity_level TEXT NOT NULL DEFAULT 'internal',
  status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_entity_uuid
  ON ai_entity (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_entity_name
  ON ai_entity (tenant_id, space_id, entity_type, canonical_name);

CREATE INDEX IF NOT EXISTS idx_ai_entity_type_status
  ON ai_entity (tenant_id, space_id, entity_type, status);

-- Activate ai_edge (previously planned in 001-memory-core.yaml).
CREATE TABLE IF NOT EXISTS ai_edge (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER NOT NULL REFERENCES ai_space(id),
  source_entity_id INTEGER NOT NULL REFERENCES ai_entity(id),
  target_entity_id INTEGER NOT NULL REFERENCES ai_entity(id),
  relation_type TEXT NOT NULL,
  weight REAL,
  source_memory_id INTEGER REFERENCES ai_record(id),
  valid_from TEXT,
  valid_to TEXT,
  status TEXT NOT NULL,
  metadata_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_edge_uuid
  ON ai_edge (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_edge_source
  ON ai_edge (tenant_id, space_id, source_entity_id, relation_type, status);

CREATE INDEX IF NOT EXISTS idx_ai_edge_target
  ON ai_edge (tenant_id, space_id, target_entity_id, relation_type, status);

CREATE INDEX IF NOT EXISTS idx_ai_edge_validity
  ON ai_edge (tenant_id, valid_from, valid_to);

-- Activate ai_policy (previously planned in 004-memory-provider.yaml).
CREATE TABLE IF NOT EXISTS ai_policy (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  policy_type TEXT NOT NULL,
  scope TEXT NOT NULL,
  scope_ref TEXT,
  status TEXT NOT NULL,
  policy_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_policy_uuid
  ON ai_policy (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_policy_type_scope
  ON ai_policy (tenant_id, policy_type, scope, status);

-- Commercial management: subject projections.
CREATE TABLE IF NOT EXISTS ai_subject (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  organization_id INTEGER NOT NULL DEFAULT 0,
  subject_type TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  display_name TEXT NOT NULL,
  default_space_id INTEGER REFERENCES ai_space(id),
  status TEXT NOT NULL,
  metadata_json TEXT,
  created_by INTEGER,
  updated_by INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_subject_uuid
  ON ai_subject (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_subject_ref
  ON ai_subject (tenant_id, subject_type, subject_ref);

CREATE INDEX IF NOT EXISTS idx_ai_subject_status
  ON ai_subject (tenant_id, subject_type, status, updated_at);

CREATE INDEX IF NOT EXISTS idx_ai_subject_space
  ON ai_subject (tenant_id, default_space_id, status);

-- Commercial management: auditable memory bindings.
CREATE TABLE IF NOT EXISTS ai_memory_binding (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  space_id INTEGER REFERENCES ai_space(id),
  binding_kind TEXT NOT NULL,
  source_subject_id INTEGER REFERENCES ai_subject(id),
  source_entity_id INTEGER REFERENCES ai_entity(id),
  source_memory_id INTEGER REFERENCES ai_record(id),
  source_external_ref_type TEXT,
  source_external_ref_id TEXT,
  source_external_ref_source TEXT,
  target_subject_id INTEGER REFERENCES ai_subject(id),
  target_entity_id INTEGER REFERENCES ai_entity(id),
  target_memory_id INTEGER REFERENCES ai_record(id),
  target_space_id INTEGER REFERENCES ai_space(id),
  target_external_ref_type TEXT,
  target_external_ref_id TEXT,
  target_external_ref_source TEXT,
  binding_role TEXT NOT NULL,
  capability_codes_json TEXT,
  retrieval_profile_id INTEGER REFERENCES ai_retrieval_profile(id),
  policy_assignment_id INTEGER,
  strength REAL,
  valid_from TEXT,
  valid_to TEXT,
  status TEXT NOT NULL,
  metadata_json TEXT,
  created_by INTEGER,
  updated_by INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_memory_binding_uuid
  ON ai_memory_binding (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_binding_source_subject
  ON ai_memory_binding (tenant_id, source_subject_id, binding_kind, status);

CREATE INDEX IF NOT EXISTS idx_ai_binding_source_entity
  ON ai_memory_binding (tenant_id, source_entity_id, binding_kind, status);

CREATE INDEX IF NOT EXISTS idx_ai_binding_target_memory
  ON ai_memory_binding (tenant_id, target_memory_id, binding_kind, status);

CREATE INDEX IF NOT EXISTS idx_ai_binding_target_space
  ON ai_memory_binding (tenant_id, target_space_id, binding_kind, status);

CREATE INDEX IF NOT EXISTS idx_ai_binding_external_source
  ON ai_memory_binding (tenant_id, source_external_ref_source, source_external_ref_type, source_external_ref_id);

CREATE INDEX IF NOT EXISTS idx_ai_binding_validity
  ON ai_memory_binding (tenant_id, valid_from, valid_to, status);

-- Commercial management: capability bindings.
CREATE TABLE IF NOT EXISTS ai_capability_binding (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  capability_code TEXT NOT NULL,
  target_type TEXT NOT NULL,
  target_id INTEGER NOT NULL,
  mode TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  retrieval_profile_id INTEGER REFERENCES ai_retrieval_profile(id),
  implementation_profile_id INTEGER REFERENCES ai_implementation_profile(id),
  policy_assignment_id INTEGER,
  status TEXT NOT NULL,
  valid_from TEXT,
  valid_to TEXT,
  metadata_json TEXT,
  created_by INTEGER,
  updated_by INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_capability_binding_uuid
  ON ai_capability_binding (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_capability_target
  ON ai_capability_binding (tenant_id, target_type, target_id, capability_code, status);

CREATE INDEX IF NOT EXISTS idx_ai_capability_priority
  ON ai_capability_binding (tenant_id, capability_code, mode, priority);

CREATE INDEX IF NOT EXISTS idx_ai_capability_validity
  ON ai_capability_binding (tenant_id, valid_from, valid_to, status);

-- Commercial management: policy assignments.
CREATE TABLE IF NOT EXISTS ai_policy_assignment (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  policy_id INTEGER NOT NULL REFERENCES ai_policy(id),
  target_type TEXT NOT NULL,
  target_id INTEGER NOT NULL,
  priority INTEGER NOT NULL DEFAULT 0,
  inheritance_mode TEXT NOT NULL,
  status TEXT NOT NULL,
  valid_from TEXT,
  valid_to TEXT,
  created_by INTEGER,
  updated_by INTEGER,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT,
  version INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_policy_assignment_uuid
  ON ai_policy_assignment (tenant_id, uuid);

CREATE INDEX IF NOT EXISTS idx_ai_policy_assignment_target
  ON ai_policy_assignment (tenant_id, target_type, target_id, status, priority);

CREATE INDEX IF NOT EXISTS idx_ai_policy_assignment_policy
  ON ai_policy_assignment (tenant_id, policy_id, status);

CREATE INDEX IF NOT EXISTS idx_ai_policy_assignment_validity
  ON ai_policy_assignment (tenant_id, valid_from, valid_to, status);

-- Commercial management: relation rebuild jobs.
CREATE TABLE IF NOT EXISTS ai_relation_rebuild_job (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  job_type TEXT NOT NULL,
  state TEXT NOT NULL,
  scope_type TEXT NOT NULL,
  scope_id TEXT,
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

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_relation_rebuild_job_uuid
  ON ai_relation_rebuild_job (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_relation_rebuild_job_idempotency
  ON ai_relation_rebuild_job (tenant_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_ai_relation_rebuild_job_state
  ON ai_relation_rebuild_job (tenant_id, state, created_at);

CREATE INDEX IF NOT EXISTS idx_ai_relation_rebuild_job_scope
  ON ai_relation_rebuild_job (tenant_id, scope_type, scope_id, state);

-- Commercial management: commercial readiness snapshot (read model).
CREATE TABLE IF NOT EXISTS ai_commercial_readiness_snapshot (
  id BIGINT NOT NULL PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  implementation_profile_id INTEGER REFERENCES ai_implementation_profile(id),
  score REAL NOT NULL,
  state TEXT NOT NULL,
  contract_coverage_json TEXT,
  management_coverage_json TEXT,
  runtime_conformance_json TEXT,
  privacy_coverage_json TEXT,
  audit_coverage_json TEXT,
  sdk_coverage_json TEXT,
  evaluation_coverage_json TEXT,
  observability_coverage_json TEXT,
  migration_coverage_json TEXT,
  blocking_findings_json TEXT,
  warning_findings_json TEXT,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uk_ai_commercial_readiness_uuid
  ON ai_commercial_readiness_snapshot (tenant_id, uuid);

CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_commercial_readiness_tenant
  ON ai_commercial_readiness_snapshot (tenant_id, implementation_profile_id);

-- source: database/migrations/sqlite/0008_memory_fts_predicate.up.sql
-- Rebuild SQLite FTS5 index to include predicate for parity with PostgreSQL tsvector.

DROP TABLE IF EXISTS ai_record_fts;

CREATE VIRTUAL TABLE ai_record_fts USING fts5(
  memory_uuid UNINDEXED,
  tenant_id UNINDEXED,
  space_id UNINDEXED,
  canonical_text,
  object_text,
  subject,
  predicate,
  tokenize = 'unicode61 remove_diacritics 1'
);

INSERT INTO ai_record_fts(
  rowid, memory_uuid, tenant_id, space_id, canonical_text, object_text, subject, predicate
)
SELECT id, uuid, tenant_id, space_id,
       coalesce(canonical_text, ''), coalesce(object_text, ''), coalesce(subject, ''),
       coalesce(predicate, '')
FROM ai_record
WHERE status <> 'deleted';

-- source: database/migrations/sqlite/0009_memory_outbox_delivery_lease.up.sql
ALTER TABLE ai_outbox_event ADD COLUMN lease_owner TEXT;
ALTER TABLE ai_outbox_event ADD COLUMN lease_token TEXT;
ALTER TABLE ai_outbox_event ADD COLUMN lease_expires_at TEXT;
ALTER TABLE ai_outbox_event ADD COLUMN next_attempt_at TEXT;

CREATE INDEX IF NOT EXISTS idx_ai_outbox_event_delivery_lease
  ON ai_outbox_event (publish_state, next_attempt_at, lease_expires_at, id);

-- source: database/migrations/sqlite/0010_memory_job_execution_lease.up.sql
ALTER TABLE ai_learning_job ADD COLUMN lease_owner TEXT;
ALTER TABLE ai_learning_job ADD COLUMN lease_token TEXT;
ALTER TABLE ai_learning_job ADD COLUMN lease_expires_at TEXT;

ALTER TABLE ai_eval_run ADD COLUMN lease_owner TEXT;
ALTER TABLE ai_eval_run ADD COLUMN lease_token TEXT;
ALTER TABLE ai_eval_run ADD COLUMN lease_expires_at TEXT;

CREATE INDEX IF NOT EXISTS idx_ai_learning_job_execution_lease
  ON ai_learning_job (state, lease_expires_at, priority, id);
CREATE INDEX IF NOT EXISTS idx_ai_eval_run_execution_lease
  ON ai_eval_run (state, lease_expires_at, id);
