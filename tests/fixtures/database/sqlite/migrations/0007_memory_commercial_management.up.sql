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
