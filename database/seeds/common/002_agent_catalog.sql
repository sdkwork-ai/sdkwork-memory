-- SDKWork official agent catalog: per-agent memory space identities (neutral base).

-- Catalog revision: 1.2.1.

BEGIN;

SELECT set_config('app.current_tenant_id', '100001', true);

SELECT set_config('app.current_organization_id', '0', true);

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000001, 'd9af12fd-b31e-45a4-878f-c0bf8d226a55', 100001, 0,
  'agent', 'agent.programmer.fullstack', 'agent_memory', 'agent.programmer.fullstack', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.fullstack","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000001, 'c0fe2e56-a14c-490e-817b-321271c1a912', 100001, 0,
  'agent', 'agent.programmer.fullstack', 'agent.programmer.fullstack', 534000000000000001, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.fullstack","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000001, '2dc20d39-d0d6-4379-8828-9ce8f8f2744e', 100001, 534000000000000001, 'agent_space',
  536000000000000001, 534000000000000001, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.fullstack","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000002, 'e9055d9d-48a8-4579-8ebf-5ddf3b4e7d02', 100001, 0,
  'agent', 'agent.programmer.frontend', 'agent_memory', 'agent.programmer.frontend', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.frontend","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000002, '87ac319f-96da-42a9-8d32-01bf8d060ada', 100001, 0,
  'agent', 'agent.programmer.frontend', 'agent.programmer.frontend', 534000000000000002, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.frontend","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000002, 'b234d083-4bc4-49fb-854f-49b98c430323', 100001, 534000000000000002, 'agent_space',
  536000000000000002, 534000000000000002, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.frontend","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000003, '0773421f-aeff-46a9-82b8-f39527706bbe', 100001, 0,
  'agent', 'agent.edu.k12.math', 'agent_memory', 'agent.edu.k12.math', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.math","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000003, '2d4acf19-840b-415a-87ca-70b3770593dd', 100001, 0,
  'agent', 'agent.edu.k12.math', 'agent.edu.k12.math', 534000000000000003, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.math","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000003, '36832d24-3e9e-4bab-837b-dd12bb461b80', 100001, 534000000000000003, 'agent_space',
  536000000000000003, 534000000000000003, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.math","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000004, '6344a7c8-4cea-4fee-8223-da3f30f2af0d', 100001, 0,
  'agent', 'agent.edu.uni.cs', 'agent_memory', 'agent.edu.uni.cs', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.cs","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000004, '9f75ce26-f5f3-47ac-8fef-dffa56cc2cf6', 100001, 0,
  'agent', 'agent.edu.uni.cs', 'agent.edu.uni.cs', 534000000000000004, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.cs","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000004, '4d56491d-9de7-4700-8342-6bcaace9dff7', 100001, 534000000000000004, 'agent_space',
  536000000000000004, 534000000000000004, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.cs","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000005, 'f59822d1-bfac-4fe8-826d-3cd4e139cfe0', 100001, 0,
  'agent', 'agent.doctor.general', 'agent_memory', 'agent.doctor.general', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.general","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000005, '81492dc0-fc45-4bb1-8d3e-e7805533bd6c', 100001, 0,
  'agent', 'agent.doctor.general', 'agent.doctor.general', 534000000000000005, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.doctor.general","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000005, 'e8f86cf8-efcf-448b-8baa-2f28ef93e4a1', 100001, 534000000000000005, 'agent_space',
  536000000000000005, 534000000000000005, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.general","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000006, 'd853a19f-1655-408c-8fa9-dc7b6564af9a', 100001, 0,
  'agent', 'agent.lawyer.civil', 'agent_memory', 'agent.lawyer.civil', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.civil","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000006, '4606f83c-fffd-481b-8ed4-997fe6820130', 100001, 0,
  'agent', 'agent.lawyer.civil', 'agent.lawyer.civil', 534000000000000006, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.civil","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000006, '499697d4-d5ab-4aca-8792-c6bc27a91410', 100001, 534000000000000006, 'agent_space',
  536000000000000006, 534000000000000006, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.civil","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000007, 'babcb59d-d6ec-4601-8219-47013c15bd48', 100001, 0,
  'agent', 'agent.travel.planner', 'agent_memory', 'agent.travel.planner', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.travel.planner","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000007, 'b86b4964-2549-40c0-8d5e-e795781983a5', 100001, 0,
  'agent', 'agent.travel.planner', 'agent.travel.planner', 534000000000000007, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.travel.planner","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000007, '5870ca99-c96b-47b4-81d8-59c242e2caa4', 100001, 534000000000000007, 'agent_space',
  536000000000000007, 534000000000000007, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.travel.planner","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000008, 'd7b4728e-c97f-4b30-87aa-a293c668124d', 100001, 0,
  'agent', 'agent.ecommerce.operations', 'agent_memory', 'agent.ecommerce.operations', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.operations","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000008, '844e2122-16cb-424c-8006-a4a2f827b746', 100001, 0,
  'agent', 'agent.ecommerce.operations', 'agent.ecommerce.operations', 534000000000000008, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.operations","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000008, 'bef11a80-c895-4220-87b8-feb1aee3d54b', 100001, 534000000000000008, 'agent_space',
  536000000000000008, 534000000000000008, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.operations","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000009, 'd712c6fa-429e-401f-8189-b0ac075b052c', 100001, 0,
  'agent', 'agent.realestate.buyer', 'agent_memory', 'agent.realestate.buyer', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.realestate.buyer","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000009, '81adbbf0-645a-4012-8430-679f845f3346', 100001, 0,
  'agent', 'agent.realestate.buyer', 'agent.realestate.buyer', 534000000000000009, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.realestate.buyer","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000009, '177cb0c2-906f-4d0b-88d7-323a7c14ca39', 100001, 534000000000000009, 'agent_space',
  536000000000000009, 534000000000000009, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.realestate.buyer","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000010, '25a1caed-0c25-42a2-8c29-4e8e8f593345', 100001, 0,
  'agent', 'agent.edu.k12.chinese', 'agent_memory', 'agent.edu.k12.chinese', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.chinese","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000010, '12f9ade3-f525-4725-8ad9-1fa5ab1ec8cc', 100001, 0,
  'agent', 'agent.edu.k12.chinese', 'agent.edu.k12.chinese', 534000000000000010, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.chinese","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000010, '936ebda7-674d-4316-8aaf-a0abca06ef17', 100001, 534000000000000010, 'agent_space',
  536000000000000010, 534000000000000010, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.chinese","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000011, 'cea95782-7f49-4133-80ea-2920ff38d837', 100001, 0,
  'agent', 'agent.edu.k12.english', 'agent_memory', 'agent.edu.k12.english', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.english","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000011, 'cf27cdca-d17c-4c58-87fb-8c208e5b0be5', 100001, 0,
  'agent', 'agent.edu.k12.english', 'agent.edu.k12.english', 534000000000000011, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.english","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000011, 'f0aa66b2-4670-4934-876d-c7d428f7ab49', 100001, 534000000000000011, 'agent_space',
  536000000000000011, 534000000000000011, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.english","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000012, '6a9001bf-8838-4fa2-8229-d17428d60884', 100001, 0,
  'agent', 'agent.edu.k12.physics', 'agent_memory', 'agent.edu.k12.physics', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.physics","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000012, 'f48aed5c-390d-450f-815b-1b12e37630f0', 100001, 0,
  'agent', 'agent.edu.k12.physics', 'agent.edu.k12.physics', 534000000000000012, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.physics","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000012, '7b1c991e-9eb8-4ccc-88c3-99e56d3ce77c', 100001, 534000000000000012, 'agent_space',
  536000000000000012, 534000000000000012, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.physics","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000013, '2f512f0d-31d3-4b12-8e14-ebb34cb39269', 100001, 0,
  'agent', 'agent.edu.k12.chemistry', 'agent_memory', 'agent.edu.k12.chemistry', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.chemistry","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000013, 'e20978df-1fad-4290-8f5c-988a2290b375', 100001, 0,
  'agent', 'agent.edu.k12.chemistry', 'agent.edu.k12.chemistry', 534000000000000013, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.chemistry","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000013, '852c7a1a-cc50-4b53-841b-60d01696b4be', 100001, 534000000000000013, 'agent_space',
  536000000000000013, 534000000000000013, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.chemistry","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000014, '1fa0a7ad-9684-4005-87c1-7804393b29d2', 100001, 0,
  'agent', 'agent.edu.k12.biology', 'agent_memory', 'agent.edu.k12.biology', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.biology","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000014, '8c8028fd-33e3-4d8c-8e19-fb2ed24a5bda', 100001, 0,
  'agent', 'agent.edu.k12.biology', 'agent.edu.k12.biology', 534000000000000014, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.biology","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000014, '4cf0e762-0d7b-4ac7-8a07-03fe45abb54a', 100001, 534000000000000014, 'agent_space',
  536000000000000014, 534000000000000014, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.biology","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000015, 'bb363e38-9529-4fdf-8183-778afee9cf83', 100001, 0,
  'agent', 'agent.edu.k12.history', 'agent_memory', 'agent.edu.k12.history', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.history","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000015, 'e2e3b508-86e1-4dbc-84da-592be872c342', 100001, 0,
  'agent', 'agent.edu.k12.history', 'agent.edu.k12.history', 534000000000000015, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.history","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000015, '39efed31-46b6-4f74-8725-aef13e11c9e4', 100001, 534000000000000015, 'agent_space',
  536000000000000015, 534000000000000015, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.history","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000016, '24651dea-4bcd-443d-8742-91ad51479d27', 100001, 0,
  'agent', 'agent.edu.k12.geography', 'agent_memory', 'agent.edu.k12.geography', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.geography","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000016, 'b0029548-7032-46fb-864c-b3ac4e73e9e9', 100001, 0,
  'agent', 'agent.edu.k12.geography', 'agent.edu.k12.geography', 534000000000000016, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.geography","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000016, '5dc7a7d7-dc4f-486e-8d81-d9b2fe70786b', 100001, 534000000000000016, 'agent_space',
  536000000000000016, 534000000000000016, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.geography","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000017, '0f116de3-34fb-43a3-8427-f48effd642ff', 100001, 0,
  'agent', 'agent.edu.k12.politics', 'agent_memory', 'agent.edu.k12.politics', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.politics","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000017, '60806b8a-c0fc-4fbc-8d6e-26bf26542f94', 100001, 0,
  'agent', 'agent.edu.k12.politics', 'agent.edu.k12.politics', 534000000000000017, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.politics","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000017, '454fe9e0-39e0-4aaf-8835-a6043ff0677a', 100001, 534000000000000017, 'agent_space',
  536000000000000017, 534000000000000017, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.k12.politics","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000018, 'c4a9bf9a-f8f3-4f9c-851f-edbf337f923e', 100001, 0,
  'agent', 'agent.programmer.backend', 'agent_memory', 'agent.programmer.backend', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.backend","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000018, '4c579b2b-6d19-473b-8e4c-b56d4a267df8', 100001, 0,
  'agent', 'agent.programmer.backend', 'agent.programmer.backend', 534000000000000018, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.backend","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000018, 'da81556d-2873-473e-8fa4-35ca4e0d2de0', 100001, 534000000000000018, 'agent_space',
  536000000000000018, 534000000000000018, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.backend","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000019, 'ab1e13ba-3f01-48ec-86d6-23345cf0c175', 100001, 0,
  'agent', 'agent.programmer.architect', 'agent_memory', 'agent.programmer.architect', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.architect","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000019, 'fc39388f-b89a-4de6-8505-7191251666d8', 100001, 0,
  'agent', 'agent.programmer.architect', 'agent.programmer.architect', 534000000000000019, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.architect","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000019, 'f0398cca-2db7-4d85-894b-5e0b93c1b547', 100001, 534000000000000019, 'agent_space',
  536000000000000019, 534000000000000019, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.architect","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000020, 'aafce814-b5fd-4867-803f-98ac3d45b034', 100001, 0,
  'agent', 'agent.programmer.devops', 'agent_memory', 'agent.programmer.devops', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.devops","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000020, '40b1e6f3-7e6e-41de-8fac-5113db3d2fe5', 100001, 0,
  'agent', 'agent.programmer.devops', 'agent.programmer.devops', 534000000000000020, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.devops","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000020, '0443e4a5-f80c-45c0-815b-10bf869662f0', 100001, 534000000000000020, 'agent_space',
  536000000000000020, 534000000000000020, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.devops","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000021, '579802e9-e52a-4115-87cf-5cc69e22cd7b', 100001, 0,
  'agent', 'agent.programmer.qa', 'agent_memory', 'agent.programmer.qa', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.qa","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000021, '371ec343-357c-4af3-8137-9c7fd36292b9', 100001, 0,
  'agent', 'agent.programmer.qa', 'agent.programmer.qa', 534000000000000021, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.qa","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000021, '4687967e-474f-4596-8b67-9ca9d8f25dfa', 100001, 534000000000000021, 'agent_space',
  536000000000000021, 534000000000000021, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.qa","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000022, '88d98c8b-82d6-43b2-8633-92a7f8255d9d', 100001, 0,
  'agent', 'agent.programmer.data.ai', 'agent_memory', 'agent.programmer.data.ai', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.data.ai","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000022, '85e8e747-3070-40e2-87fa-643687db4f97', 100001, 0,
  'agent', 'agent.programmer.data.ai', 'agent.programmer.data.ai', 534000000000000022, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.programmer.data.ai","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000022, 'a4274678-6a3e-4f5e-8d38-98df1405cdcb', 100001, 534000000000000022, 'agent_space',
  536000000000000022, 534000000000000022, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.programmer.data.ai","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000023, 'e0ee3857-6d5b-4cd5-852d-264c5ae8f334', 100001, 0,
  'agent', 'agent.edu.uni.math', 'agent_memory', 'agent.edu.uni.math', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.math","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000023, '26f42bcc-d49c-4f42-8044-099da9e6574a', 100001, 0,
  'agent', 'agent.edu.uni.math', 'agent.edu.uni.math', 534000000000000023, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.math","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000023, '61f68261-dad1-460c-8a65-00cdb03b6c84', 100001, 534000000000000023, 'agent_space',
  536000000000000023, 534000000000000023, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.math","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000024, '766e90c2-04a5-4f7d-89dc-f605b18c9c18', 100001, 0,
  'agent', 'agent.edu.uni.physics', 'agent_memory', 'agent.edu.uni.physics', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.physics","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000024, '48b94305-ff4f-4d1e-8342-0e5c6b156554', 100001, 0,
  'agent', 'agent.edu.uni.physics', 'agent.edu.uni.physics', 534000000000000024, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.physics","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000024, '5e7881fe-0368-4a26-8076-b1a398b1a291', 100001, 534000000000000024, 'agent_space',
  536000000000000024, 534000000000000024, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.physics","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000025, 'd65b8691-6f1a-48de-8fcb-b7ec7b989bae', 100001, 0,
  'agent', 'agent.edu.uni.economics', 'agent_memory', 'agent.edu.uni.economics', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.economics","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000025, '20375340-0638-4766-8c3d-30f00f0d1eee', 100001, 0,
  'agent', 'agent.edu.uni.economics', 'agent.edu.uni.economics', 534000000000000025, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.economics","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000025, '72ff3e05-56bb-4c2c-8d1b-18c0fcbd071c', 100001, 534000000000000025, 'agent_space',
  536000000000000025, 534000000000000025, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.economics","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000026, '09c21ff0-6b5a-4ef4-8a49-89824c32a691', 100001, 0,
  'agent', 'agent.edu.uni.law', 'agent_memory', 'agent.edu.uni.law', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.law","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000026, 'fbd26374-b12e-435e-834a-19a426bab4e2', 100001, 0,
  'agent', 'agent.edu.uni.law', 'agent.edu.uni.law', 534000000000000026, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.law","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000026, 'b333b147-fbee-43df-857f-97fbb8ceafff', 100001, 534000000000000026, 'agent_space',
  536000000000000026, 534000000000000026, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.law","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000027, '0a8713b4-88d8-4fe3-8098-dfb71ebe3b51', 100001, 0,
  'agent', 'agent.edu.uni.medicine', 'agent_memory', 'agent.edu.uni.medicine', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.medicine","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000027, '87760119-e31e-4c26-8a76-c4f74653d39b', 100001, 0,
  'agent', 'agent.edu.uni.medicine', 'agent.edu.uni.medicine', 534000000000000027, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.medicine","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000027, '718f917c-eee8-4921-866f-78df5930c3ce', 100001, 534000000000000027, 'agent_space',
  536000000000000027, 534000000000000027, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.medicine","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000028, '9a3c1f66-4086-43f2-8c40-40b689cdc68e', 100001, 0,
  'agent', 'agent.edu.uni.engineering', 'agent_memory', 'agent.edu.uni.engineering', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.engineering","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000028, 'a90451fb-cd9e-4fde-8437-1d7145a3bb25', 100001, 0,
  'agent', 'agent.edu.uni.engineering', 'agent.edu.uni.engineering', 534000000000000028, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.engineering","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000028, '48d18dc3-2e82-4ae8-893b-43d780ad0504', 100001, 534000000000000028, 'agent_space',
  536000000000000028, 534000000000000028, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.edu.uni.engineering","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000029, 'fd3e66ea-7b48-450d-8f06-7f5785c50968', 100001, 0,
  'agent', 'agent.doctor.pediatrics', 'agent_memory', 'agent.doctor.pediatrics', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.pediatrics","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000029, 'c9e7afc1-41f1-4ab0-8776-cc384a93def3', 100001, 0,
  'agent', 'agent.doctor.pediatrics', 'agent.doctor.pediatrics', 534000000000000029, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.doctor.pediatrics","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000029, '346efd11-04fe-4335-8747-842d1fdce949', 100001, 534000000000000029, 'agent_space',
  536000000000000029, 534000000000000029, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.pediatrics","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000030, 'd900a2a7-39f3-4ba9-8705-63775e4b749c', 100001, 0,
  'agent', 'agent.doctor.cardiology', 'agent_memory', 'agent.doctor.cardiology', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.cardiology","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000030, '6f2d74f0-cd58-4ba4-893f-5c0f79610c1f', 100001, 0,
  'agent', 'agent.doctor.cardiology', 'agent.doctor.cardiology', 534000000000000030, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.doctor.cardiology","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000030, 'aacda5f1-d48c-452f-8fac-bada6da41336', 100001, 534000000000000030, 'agent_space',
  536000000000000030, 534000000000000030, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.cardiology","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000031, '7ea228a4-6436-4de8-8176-ce739232453a', 100001, 0,
  'agent', 'agent.doctor.mental.health', 'agent_memory', 'agent.doctor.mental.health', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.mental.health","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000031, 'c1f6f801-6da0-4f03-80c0-13f5a4d9d9fa', 100001, 0,
  'agent', 'agent.doctor.mental.health', 'agent.doctor.mental.health', 534000000000000031, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.doctor.mental.health","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000031, '35a78765-151c-4545-884d-be32cb334140', 100001, 534000000000000031, 'agent_space',
  536000000000000031, 534000000000000031, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.doctor.mental.health","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000032, '6a6029d8-a591-4620-82be-6f64a0247831', 100001, 0,
  'agent', 'agent.lawyer.labor', 'agent_memory', 'agent.lawyer.labor', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.labor","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000032, 'dc6b40a5-9dad-4dab-8524-c1910df0cac5', 100001, 0,
  'agent', 'agent.lawyer.labor', 'agent.lawyer.labor', 534000000000000032, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.labor","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000032, 'd5113038-a110-4041-84c1-1b24db2dfd36', 100001, 534000000000000032, 'agent_space',
  536000000000000032, 534000000000000032, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.labor","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000033, 'fc8d65b0-0ca9-46eb-86ef-fbce98e5c9ae', 100001, 0,
  'agent', 'agent.lawyer.criminal', 'agent_memory', 'agent.lawyer.criminal', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.criminal","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000033, '1d0af98f-aafe-4dfa-81f4-4c4133113df1', 100001, 0,
  'agent', 'agent.lawyer.criminal', 'agent.lawyer.criminal', 534000000000000033, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.criminal","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000033, '701dd8bb-09bc-4beb-8ffc-977b3fc62b6d', 100001, 534000000000000033, 'agent_space',
  536000000000000033, 534000000000000033, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.criminal","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000034, '44b15135-3731-4b92-8bbe-4a20716be44b', 100001, 0,
  'agent', 'agent.lawyer.ip', 'agent_memory', 'agent.lawyer.ip', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.ip","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000034, '22f992c2-7f3a-493b-816b-48a095633747', 100001, 0,
  'agent', 'agent.lawyer.ip', 'agent.lawyer.ip', 534000000000000034, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.ip","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000034, '8b49eace-7741-4cb2-825b-2d8f4b9edbf3', 100001, 534000000000000034, 'agent_space',
  536000000000000034, 534000000000000034, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.ip","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000035, '1873d693-1895-4b51-831f-d5ff71129906', 100001, 0,
  'agent', 'agent.lawyer.family', 'agent_memory', 'agent.lawyer.family', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.family","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000035, 'c5e83ff0-f153-492f-83ad-0042a4ff763d', 100001, 0,
  'agent', 'agent.lawyer.family', 'agent.lawyer.family', 534000000000000035, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.family","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000035, '729e60c2-84eb-41b5-8a60-413c040e2749', 100001, 534000000000000035, 'agent_space',
  536000000000000035, 534000000000000035, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.lawyer.family","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000036, 'a39a0850-9ee1-41b2-8ced-1f53612668d6', 100001, 0,
  'agent', 'agent.travel.guide', 'agent_memory', 'agent.travel.guide', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.travel.guide","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000036, '80c9d0da-04d9-42a6-8cc9-36c748571721', 100001, 0,
  'agent', 'agent.travel.guide', 'agent.travel.guide', 534000000000000036, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.travel.guide","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000036, 'c8aa238a-9e16-4e7c-8a4e-a74221c459bf', 100001, 534000000000000036, 'agent_space',
  536000000000000036, 534000000000000036, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.travel.guide","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000037, 'f70f4700-1c2b-4e84-8163-94ca8258da36', 100001, 0,
  'agent', 'agent.travel.visa', 'agent_memory', 'agent.travel.visa', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.travel.visa","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000037, '14512fa6-8aa1-4afc-8738-28010a8847e6', 100001, 0,
  'agent', 'agent.travel.visa', 'agent.travel.visa', 534000000000000037, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.travel.visa","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000037, '08e66e7a-6b94-450b-8e49-e168bfee8629', 100001, 534000000000000037, 'agent_space',
  536000000000000037, 534000000000000037, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.travel.visa","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000038, '0361b9d0-e505-4e92-8f2d-b22bd6e194b2', 100001, 0,
  'agent', 'agent.ecommerce.product', 'agent_memory', 'agent.ecommerce.product', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.product","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000038, 'a2a9a6e2-769d-4d0c-8e5a-9217ebe60b8e', 100001, 0,
  'agent', 'agent.ecommerce.product', 'agent.ecommerce.product', 534000000000000038, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.product","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000038, '70c44577-18cd-40ce-8b46-0e07d953812b', 100001, 534000000000000038, 'agent_space',
  536000000000000038, 534000000000000038, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.product","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000039, 'b5679d67-2c72-462f-8a0a-495d8a80a429', 100001, 0,
  'agent', 'agent.ecommerce.customer.service', 'agent_memory', 'agent.ecommerce.customer.service', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.customer.service","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000039, '0a955aa4-f2f7-4307-819a-aae6c145f468', 100001, 0,
  'agent', 'agent.ecommerce.customer.service', 'agent.ecommerce.customer.service', 534000000000000039, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.customer.service","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000039, 'fb1dbfa9-6139-45cc-874a-9561853a4eab', 100001, 534000000000000039, 'agent_space',
  536000000000000039, 534000000000000039, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.customer.service","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000040, '0aab7946-6b8e-4bc0-80db-cfb4be6fbc9f', 100001, 0,
  'agent', 'agent.ecommerce.ads', 'agent_memory', 'agent.ecommerce.ads', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.ads","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000040, '888bd570-efda-4dbf-8f3d-f89e28841d69', 100001, 0,
  'agent', 'agent.ecommerce.ads', 'agent.ecommerce.ads', 534000000000000040, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.ads","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000040, '5078710f-1ea6-4d26-82e4-41e9be388beb', 100001, 534000000000000040, 'agent_space',
  536000000000000040, 534000000000000040, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.ecommerce.ads","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000041, '78ae8450-9bce-45d3-8660-39f403574ad4', 100001, 0,
  'agent', 'agent.realestate.rental', 'agent_memory', 'agent.realestate.rental', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.realestate.rental","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000041, 'eee64605-50b0-4101-8fc6-845ecd208b9e', 100001, 0,
  'agent', 'agent.realestate.rental', 'agent.realestate.rental', 534000000000000041, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.realestate.rental","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000041, 'a1246d93-a493-482e-8a3c-7a806d43f4de', 100001, 534000000000000041, 'agent_space',
  536000000000000041, 534000000000000041, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.realestate.rental","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000042, '0e6b1243-f453-4869-8bb5-4e136f174dab', 100001, 0,
  'agent', 'agent.realestate.valuation', 'agent_memory', 'agent.realestate.valuation', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.realestate.valuation","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000042, '8c53895b-ac89-437b-8a0b-f0ee87f26f6b', 100001, 0,
  'agent', 'agent.realestate.valuation', 'agent.realestate.valuation', 534000000000000042, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.realestate.valuation","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000042, '349c5ef1-d506-457f-8933-ae68c720f0c4', 100001, 534000000000000042, 'agent_space',
  536000000000000042, 534000000000000042, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.realestate.valuation","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000043, '85eb76e5-0dc0-4f7b-8dd7-e04b99190d5c', 100001, 0,
  'agent', 'agent.restaurant.operations', 'agent_memory', 'agent.restaurant.operations', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.operations","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000043, '128a6d2e-9173-4f93-826f-685201582f41', 100001, 0,
  'agent', 'agent.restaurant.operations', 'agent.restaurant.operations', 534000000000000043, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.operations","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000043, '24fe232f-c031-4c03-8d72-bf792874e1a2', 100001, 534000000000000043, 'agent_space',
  536000000000000043, 534000000000000043, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.operations","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000044, 'a0f2d0c3-6773-4c42-8009-5dfe512b3d41', 100001, 0,
  'agent', 'agent.restaurant.menu.development', 'agent_memory', 'agent.restaurant.menu.development', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.menu.development","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000044, '8ce0cb52-c908-404c-8880-c8b5610fcb62', 100001, 0,
  'agent', 'agent.restaurant.menu.development', 'agent.restaurant.menu.development', 534000000000000044, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.menu.development","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000044, '71feb37b-6fb6-4e13-8a37-19cd5a655e4d', 100001, 534000000000000044, 'agent_space',
  536000000000000044, 534000000000000044, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.menu.development","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000045, 'b1554463-2e9c-4fbe-885f-30ede2fb4be6', 100001, 0,
  'agent', 'agent.restaurant.food.safety', 'agent_memory', 'agent.restaurant.food.safety', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.food.safety","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000045, '9ac03b20-28ef-4280-846b-2eb99d8736e4', 100001, 0,
  'agent', 'agent.restaurant.food.safety', 'agent.restaurant.food.safety', 534000000000000045, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.food.safety","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000045, '23f1d14a-52d2-460a-829f-e25cc3bbdf15', 100001, 534000000000000045, 'agent_space',
  536000000000000045, 534000000000000045, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.food.safety","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000046, 'a261ef13-ef96-417e-8066-13f27158c252', 100001, 0,
  'agent', 'agent.restaurant.marketing', 'agent_memory', 'agent.restaurant.marketing', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.marketing","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000046, '016af061-18da-4037-8a5f-233101f431db', 100001, 0,
  'agent', 'agent.restaurant.marketing', 'agent.restaurant.marketing', 534000000000000046, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.marketing","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000046, '26806721-4f7c-4a03-855d-04ce9a0c423f', 100001, 534000000000000046, 'agent_space',
  536000000000000046, 534000000000000046, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.restaurant.marketing","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000047, '2f0cdc42-090d-445d-8189-20fdaeb47259', 100001, 0,
  'agent', 'agent.enterprise.consulting', 'agent_memory', 'agent.enterprise.consulting', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.consulting","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000047, 'cf46ab1e-13e7-4470-8abe-d8c57c155abf', 100001, 0,
  'agent', 'agent.enterprise.consulting', 'agent.enterprise.consulting', 534000000000000047, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.consulting","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000047, '093136f2-dbf5-47d3-86e7-9ed5e9422a5f', 100001, 534000000000000047, 'agent_space',
  536000000000000047, 534000000000000047, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.consulting","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000048, '5bbc58ef-8225-42c9-8f0b-dea903e33a2c', 100001, 0,
  'agent', 'agent.enterprise.hr', 'agent_memory', 'agent.enterprise.hr', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.hr","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000048, '0f7c8d0f-c5e2-4a1f-8015-c1d6ed6fd613', 100001, 0,
  'agent', 'agent.enterprise.hr', 'agent.enterprise.hr', 534000000000000048, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.hr","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000048, '0689d8bb-de98-4682-8431-d22b0ec46124', 100001, 534000000000000048, 'agent_space',
  536000000000000048, 534000000000000048, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.hr","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000049, 'a2b42c95-5dff-4637-8d38-22dfbb8947cc', 100001, 0,
  'agent', 'agent.enterprise.tax', 'agent_memory', 'agent.enterprise.tax', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.tax","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000049, '5727599d-01cf-4501-8ab5-7e371936c2eb', 100001, 0,
  'agent', 'agent.enterprise.tax', 'agent.enterprise.tax', 534000000000000049, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.tax","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000049, 'ebe9d25e-f397-4975-8dc0-65bf452d399a', 100001, 534000000000000049, 'agent_space',
  536000000000000049, 534000000000000049, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.tax","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000050, 'c0f2f688-a768-4ab7-802d-a25559ca1d59', 100001, 0,
  'agent', 'agent.enterprise.b2b.sales', 'agent_memory', 'agent.enterprise.b2b.sales', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.b2b.sales","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000050, '3066bbf0-760d-45a9-847c-778fbb711ac5', 100001, 0,
  'agent', 'agent.enterprise.b2b.sales', 'agent.enterprise.b2b.sales', 534000000000000050, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.b2b.sales","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000050, '6f8f6816-8e44-412d-8a63-90b944918eb7', 100001, 534000000000000050, 'agent_space',
  536000000000000050, 534000000000000050, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.enterprise.b2b.sales","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000051, 'e54d2d4b-893e-4d09-8f67-37b00160df9b', 100001, 0,
  'agent', 'agent.government.service.guide', 'agent_memory', 'agent.government.service.guide', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.service.guide","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000051, '0e4e4642-2ad7-4661-8b08-9da7cbb05b6f', 100001, 0,
  'agent', 'agent.government.service.guide', 'agent.government.service.guide', 534000000000000051, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.government.service.guide","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000051, '22570199-57fc-4ecc-8a9a-9b1159d09fff', 100001, 534000000000000051, 'agent_space',
  536000000000000051, 534000000000000051, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.service.guide","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000052, 'b7a905dc-fce2-4469-8c7e-6714bcea843f', 100001, 0,
  'agent', 'agent.government.policy.interpretation', 'agent_memory', 'agent.government.policy.interpretation', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.policy.interpretation","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000052, '9aab8bf6-e479-4ec0-8b9e-6b52165ac7e9', 100001, 0,
  'agent', 'agent.government.policy.interpretation', 'agent.government.policy.interpretation', 534000000000000052, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.government.policy.interpretation","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000052, 'c4b9fdc1-9b4d-4a84-89aa-49e7e57f89ec', 100001, 534000000000000052, 'agent_space',
  536000000000000052, 534000000000000052, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.policy.interpretation","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000053, 'bc8b703f-1a79-4320-82b3-6236219f9687', 100001, 0,
  'agent', 'agent.government.document', 'agent_memory', 'agent.government.document', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.document","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000053, 'f65bf464-b307-47d1-87f2-cafe8a89ab97', 100001, 0,
  'agent', 'agent.government.document', 'agent.government.document', 534000000000000053, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.government.document","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000053, 'd425aea6-4f0d-4516-8488-ab4a0c67f571', 100001, 534000000000000053, 'agent_space',
  536000000000000053, 534000000000000053, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.document","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000054, 'd29059eb-6032-426e-8e85-05bd40957fc7', 100001, 0,
  'agent', 'agent.government.digital', 'agent_memory', 'agent.government.digital', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.digital","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000054, '0548d306-2234-47e0-8828-977b97a98fcf', 100001, 0,
  'agent', 'agent.government.digital', 'agent.government.digital', 534000000000000054, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.government.digital","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000054, '26a2d3cf-41b3-45a0-8207-05b027f34228', 100001, 534000000000000054, 'agent_space',
  536000000000000054, 534000000000000054, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.government.digital","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000055, 'b6c08a90-cb27-4850-8e09-bbef8a95a665', 100001, 0,
  'agent', 'agent.hotel.operations', 'agent_memory', 'agent.hotel.operations', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.operations","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000055, 'bba870b3-b811-475b-827d-f257b7a92acf', 100001, 0,
  'agent', 'agent.hotel.operations', 'agent.hotel.operations', 534000000000000055, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.hotel.operations","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000055, 'd3e13437-e9ae-4d0e-84bd-a963c1e7b34f', 100001, 534000000000000055, 'agent_space',
  536000000000000055, 534000000000000055, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.operations","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000056, 'bfaf3089-d1ee-4b94-8138-e0fcb9228e49', 100001, 0,
  'agent', 'agent.hotel.revenue', 'agent_memory', 'agent.hotel.revenue', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.revenue","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000056, 'b441ec62-f04b-4593-8db3-a93503e170ae', 100001, 0,
  'agent', 'agent.hotel.revenue', 'agent.hotel.revenue', 534000000000000056, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.hotel.revenue","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000056, 'b6cf52bf-b6ec-410c-887f-32c9338a0e6d', 100001, 534000000000000056, 'agent_space',
  536000000000000056, 534000000000000056, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.revenue","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000057, 'bce1dbcf-b184-443e-8599-dcf1ca64976d', 100001, 0,
  'agent', 'agent.hotel.guest.service', 'agent_memory', 'agent.hotel.guest.service', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.guest.service","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000057, '60b6b531-31c3-46f8-8a1d-31d22c3b6552', 100001, 0,
  'agent', 'agent.hotel.guest.service', 'agent.hotel.guest.service', 534000000000000057, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.hotel.guest.service","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000057, 'fa5dba66-8374-4e9a-82d4-280b93bbe7fc', 100001, 534000000000000057, 'agent_space',
  536000000000000057, 534000000000000057, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.guest.service","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_space (
  id, uuid, tenant_id, organization_id, owner_subject_type, owner_subject_id, space_type,
  display_name, default_scope, lifecycle_status, metadata_json, policy_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  534000000000000058, 'ffee2a54-339e-481c-8bdb-cdb5a39bab9e', 100001, 0,
  'agent', 'agent.hotel.marketing', 'agent_memory', 'agent.hotel.marketing', 'user', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.marketing","catalogVersion":"1.2.1"}', '{"mode":"read_write","retentionDays":365}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (id) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  lifecycle_status = EXCLUDED.lifecycle_status,
  metadata_json = EXCLUDED.metadata_json,
  policy_json = EXCLUDED.policy_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_subject (
  id, uuid, tenant_id, organization_id, subject_type, subject_ref, display_name, default_space_id,
  status, metadata_json, created_by, updated_by, created_at, updated_at, version
) VALUES (
  536000000000000058, 'f9c6d1e4-881f-491c-8708-a7160b79f17a', 100001, 0,
  'agent', 'agent.hotel.marketing', 'agent.hotel.marketing', 534000000000000058, 'active', '{"catalog":"sdkwork-agents","agentId":"agent.hotel.marketing","catalogVersion":"1.2.1"}',
  1, 1, '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, subject_type, subject_ref) DO UPDATE SET
  display_name = EXCLUDED.display_name,
  default_space_id = EXCLUDED.default_space_id,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

INSERT INTO ai_memory_binding (
  id, uuid, tenant_id, space_id, binding_kind, source_subject_id, target_space_id,
  binding_role, capability_codes_json, status, metadata_json, created_by, updated_by,
  created_at, updated_at, version
) VALUES (
  537000000000000058, '0a694be9-c6af-41f7-8de9-872949aaaec7', 100001, 534000000000000058, 'agent_space',
  536000000000000058, 534000000000000058, 'retriever', '["memory.retrieve","memory.write"]', 'active',
  '{"catalog":"sdkwork-agents","agentId":"agent.hotel.marketing","catalogVersion":"1.2.1"}', 1, 1,
  '2026-11-03T00:00:00+08:00', '2026-11-03T00:00:00+08:00', 0
)
ON CONFLICT (tenant_id, uuid) DO UPDATE SET
  space_id = EXCLUDED.space_id,
  binding_kind = EXCLUDED.binding_kind,
  source_subject_id = EXCLUDED.source_subject_id,
  target_space_id = EXCLUDED.target_space_id,
  binding_role = EXCLUDED.binding_role,
  capability_codes_json = EXCLUDED.capability_codes_json,
  status = EXCLUDED.status,
  metadata_json = EXCLUDED.metadata_json,
  updated_at = EXCLUDED.updated_at
;

COMMIT;
