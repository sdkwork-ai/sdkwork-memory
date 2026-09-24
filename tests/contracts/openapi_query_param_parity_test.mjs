// Contract parity gate: OpenAPI-declared GET list query parameters MUST equal
// the deserializable field set of the contract query DTO bound to the handler.
//
// - declared but not on the DTO  => the server rejects it with 400
//   (deny_unknown_fields), so declaring it is a lie to SDK consumers;
// - on the DTO but not declared  => SDK consumers cannot reach a filter that
//   the server actually applies.
//
// DTO fields are parsed from `crates/sdkwork-memory-contract/src/*.rs` by
// matching `pub struct <Name>` blocks and their `pub <field>:` entries.
// Fields annotated `skip_deserializing` (tenant_id injected from the request
// context by the routes) are not wire parameters and are excluded.
//
// The operationId -> DTO mapping is an explicit, hardcoded enumeration: each
// GET `*.list` operation of the three surfaces must be covered.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(repoRoot, relativePath), 'utf8'));
}

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

// ---------------------------------------------------------------------------
// DTO field extraction
// ---------------------------------------------------------------------------

const CONTRACT_SOURCES = [
  'crates/sdkwork-memory-contract/src/dto.rs',
  'crates/sdkwork-memory-contract/src/commercial.rs',
  'crates/sdkwork-memory-contract/src/space.rs',
  'crates/sdkwork-memory-contract/src/admin_dto.rs',
];

function extractQueryDtoFields() {
  const dtos = new Map();
  for (const source of CONTRACT_SOURCES) {
    const text = readText(source);
    const structPattern = /pub struct (\w+)\s*\{/g;
    for (const match of text.matchAll(structPattern)) {
      const name = match[1];
      if (!name.startsWith('List') || !name.endsWith('Query')) {
        continue;
      }
      const bodyStart = match.index + match[0].length;
      const bodyEnd = text.indexOf('\n}', bodyStart);
      const body = text.slice(bodyStart, bodyEnd);
      const fields = [];
      // Fields appear as `pub <snake_case_name>:`; the serde attributes that
      // precede each `pub` live in the gap since the previous match. A
      // `skip_deserializing` annotation means the value is injected
      // server-side and is not a wire parameter.
      const fieldPattern = /pub (\w+):/g;
      let cursor = 0;
      for (const fieldMatch of body.matchAll(fieldPattern)) {
        const gap = body.slice(cursor, fieldMatch.index);
        cursor = fieldMatch.index + fieldMatch[0].length;
        if (!/skip_deserializing/.test(gap)) {
          fields.push(fieldMatch[1]);
        }
      }
      dtos.set(name, fields.sort());
    }
  }
  return dtos;
}

// ---------------------------------------------------------------------------
// operationId -> query DTO binding (explicit enumeration)
// ---------------------------------------------------------------------------

const SURFACES = [
  {
    openapiPath: 'apis/open-api/memory-open-api.openapi.json',
    label: 'open-api',
    operationDtos: {
      'memories.list': 'ListMemoriesQuery',
      'candidates.list': 'ListCandidatesQuery',
      'entities.list': 'ListEntitiesQuery',
      'edges.list': 'ListEdgesQuery',
    },
  },
  {
    openapiPath: 'apis/app-api/memory-app-api.openapi.json',
    label: 'app-api',
    operationDtos: {
      'spaces.list': 'ListSpacesQuery',
      'memories.list': 'ListMemoriesQuery',
      'memories.sources.list': 'ListMemorySourcesQuery',
      'forgetRequests.list': 'ListJobsQuery',
      'candidates.list': 'ListCandidatesQuery',
      'habits.list': 'ListHabitsQuery',
      'exportJobs.list': 'ListJobsQuery',
      'entities.list': 'ListEntitiesQuery',
      'policyAssignments.list': 'ListPolicyAssignmentsQuery',
    },
  },
  {
    openapiPath: 'apis/backend-api/memory-backend-api.openapi.json',
    label: 'backend-api',
    operationDtos: {
      'spaces.list': 'ListSpacesQuery',
      'memories.list': 'ListMemoriesQuery',
      'events.list': 'ListEventsQuery',
      'candidates.list': 'ListCandidatesQuery',
      'extractionJobs.list': 'ListJobsQuery',
      'consolidationJobs.list': 'ListJobsQuery',
      'retentionJobs.list': 'ListJobsQuery',
      'migrationJobs.list': 'ListJobsQuery',
      'indexes.list': 'ListAdminResourcesQuery',
      'retrievalProfiles.list': 'ListAdminResourcesQuery',
      'implementationProfiles.list': 'ListAdminResourcesQuery',
      'providerBindings.list': 'ListAdminResourcesQuery',
      'evalRuns.list': 'ListAdminResourcesQuery',
      'retrievalTraces.list': 'ListRetrievalTracesQuery',
      'auditLogs.list': 'ListAuditLogsQuery',
      'subjects.list': 'ListSubjectsQuery',
      'bindings.list': 'ListBindingsQuery',
      'capabilityBindings.list': 'ListCapabilityBindingsQuery',
      'entities.list': 'ListEntitiesQuery',
      'edges.list': 'ListEdgesQuery',
      'policies.list': 'ListPoliciesQuery',
      'policyAssignments.list': 'ListPolicyAssignmentsQuery',
    },
  },
];

// Shared-DTO fields that a specific operation accepts on the wire but that the
// service does not honor, so they stay undeclared. Each entry must cite the
// service behavior; removing the drift requires splitting the DTO (service-side
// change), not re-adding the parameter to the OpenAPI document.
const UNDECLARED_DTO_FIELDS = {
  // ListJobsQuery is shared; space_id filters only extractionJobs.list.
  'app-api forgetRequests.list': {
    space_id: 'service silently ignores space_id on governance job listings',
  },
  'app-api exportJobs.list': {
    space_id: 'service silently ignores space_id on governance job listings',
  },
  'backend-api consolidationJobs.list': {
    space_id: 'service rejects space_id with 400 INVALID_PARAMETER',
  },
  'backend-api retentionJobs.list': {
    space_id: 'service rejects space_id with 400 INVALID_PARAMETER',
  },
  'backend-api migrationJobs.list': {
    space_id: 'service rejects space_id with 400 INVALID_PARAMETER',
  },
  // ListAdminResourcesQuery is shared; space_id filters only indexes and
  // retrieval profiles.
  'backend-api implementationProfiles.list': {
    space_id: 'service silently ignores space_id on this collection',
  },
  'backend-api providerBindings.list': {
    space_id: 'service silently ignores space_id on this collection',
  },
  'backend-api evalRuns.list': {
    space_id: 'service silently ignores space_id on this collection',
  },
};

function declaredQueryParams(operation) {
  const names = new Set();
  for (const parameter of operation.parameters ?? []) {
    if (parameter.in === 'query') {
      names.add(parameter.name);
    }
  }
  return names;
}

const dtos = extractQueryDtoFields();
assert.ok(dtos.size >= 16, `contract query DTO extraction found ${dtos.size} DTOs`);

for (const surface of SURFACES) {
  test(`${surface.openapiPath} GET list query parameters match contract DTO fields`, () => {
    const openapi = readJson(surface.openapiPath);
    const seen = new Set();
    for (const operations of Object.values(openapi.paths ?? {})) {
      for (const [method, operation] of Object.entries(operations ?? {})) {
        if (method !== 'get') {
          continue;
        }
        const operationId = operation?.operationId;
        if (!operationId?.endsWith('.list')) {
          continue;
        }
        assert.ok(
          Object.hasOwn(surface.operationDtos, operationId),
          `${surface.label} ${operationId} is missing from the operationId->DTO map`,
        );
        seen.add(operationId);

        const dtoName = surface.operationDtos[operationId];
        const dtoFields = dtos.get(dtoName);
        assert.ok(dtoFields, `contract DTO ${dtoName} not found in sdkwork-memory-contract`);

        const key = `${surface.label} ${operationId}`;
        const exceptions = UNDECLARED_DTO_FIELDS[key] ?? {};
        for (const field of Object.keys(exceptions)) {
          assert.ok(
            dtoFields.includes(field),
            `${key}: exception field ${field} is not a field of ${dtoName}`,
          );
        }

        const declared = declaredQueryParams(operation);
        const expected = new Set(dtoFields.filter((field) => !Object.hasOwn(exceptions, field)));

        const undeclared = [...expected].filter((field) => !declared.has(field));
        assert.deepEqual(
          undeclared,
          [],
          `${key}: DTO ${dtoName} fields implemented by the handler but not declared in OpenAPI`,
        );

        const unimplemented = [...declared].filter((field) => !expected.has(field));
        assert.deepEqual(
          unimplemented,
          [],
          `${key}: OpenAPI declares query parameters the ${dtoName} DTO rejects (deny_unknown_fields 400)`,
        );
      }
    }
    assert.deepEqual(
      [...seen].sort(),
      Object.keys(surface.operationDtos).sort(),
      `${surface.label}: every GET *.list operation must be covered exactly once`,
    );
  });
}
