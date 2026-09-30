// mem0 upstream request-schema reconciliation gate.
//
// The mem0 compatibility wire mirrors a moving upstream contract
// (`external/mem0/docs/openapi.json`). When upstream adds a request field,
// this surface must make an explicit decision — declare it, or refuse it by
// name — before the drift becomes an undeclared wire behavior. This gate
// enforces that decision point: every request attribute the upstream document
// declares for the mirrored write/read/erase operations must be accounted for
// by the local authority OpenAPI as either
//
//   - a declared field of the local request DTO (accepted, possibly with
//     "Refused with 501" semantics spelled in its description), or
//   - a declared operation query parameter (the local spelling of an
//     upstream body field, e.g. `page`/`page_size`), or
//   - a member of REFUSED_PROPERTIES below, which must stay in lockstep with
//     the named refusals the mem0 compatibility handlers enforce at runtime.
//
// A new upstream field satisfies none of the three → this gate goes red, and
// the field must be triaged instead of silently accepted or silently dropped.
// `metadata` is listed as refused for the erase/list paths per the named
// refusal set even where some local DTOs declare a `metadata` property; the
// union is intentional (declared ∪ refused), so tightening a refusal never
// requires editing this gate first.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const UPSTREAM_OPENAPI = "external/mem0/docs/openapi.json";
const LOCAL_OPENAPI = "apis/open-api/memory-open-api.openapi.json";

// Mirrors the named refusals enforced by the mem0 compatibility handlers
// (crates/sdkwork-routes-memory-open-api/src/mem0/). Keep in lockstep with
// that set: a refusal added there must be listed here, and vice versa.
const REFUSED_PROPERTIES = new Set([
  "immutable",
  "includes",
  "excludes",
  "enable_graph",
  "output_format",
  "prompt_profile_id",
  "temporal_reasoning",
  "timezone",
  "observation_datetime",
  "observation_date",
  "fields",
  "keywords",
  "metadata",
]);

// Upstream operations this surface refuses **as a whole** — registered in
// `paths::MEM0_REFUSED_PATHS`, answered `501` by name, and declared with no 2xx
// at all. This is the operation-level sibling of REFUSED_PROPERTIES (which
// refuses individual request attributes): upstream declaring one of these is
// expected and must map onto a local refusal operation, not silently 404.
//
// Each entry asserts three things, all upstream-authoritative:
//   1. the upstream document still declares the operation (else upstream moved
//      and this gate must be re-triaged),
//   2. the local authority declares the same path+method carrying the external
//      wire-protocol marker,
//   3. the local responses declare 501 and no success status — publishing a 2xx
//      would promise a response the surface can never produce.
const REFUSED_OPERATIONS = [
  { upstreamPath: "/v1/entities/{entity_type}/{entity_id}/", upstreamMethod: "delete", localOperationId: "mem0.entity.remove" },
  { upstreamPath: "/v1/exports/", upstreamMethod: "post", localOperationId: "mem0.export.create" },
  { upstreamPath: "/v1/exports/get/", upstreamMethod: "post", localOperationId: "mem0.export.retrieve" },
  { upstreamPath: "/v1/summary/", upstreamMethod: "post", localOperationId: "mem0.summary.create" },
  { upstreamPath: "/v2/entities/{entity_type}/{entity_id}/", upstreamMethod: "delete", localOperationId: "mem0.entity.delete" },
  { upstreamPath: "/v2/entities/{entity_type}/{entity_id}/profile/", upstreamMethod: "get", localOperationId: "mem0.entity.profile" },
  { upstreamPath: "/v2/profiles/jobs/", upstreamMethod: "post", localOperationId: "mem0.profile.job.create" },
  { upstreamPath: "/v2/profiles/jobs/{job_id}/", upstreamMethod: "get", localOperationId: "mem0.profile.job.retrieve" },
  { upstreamPath: "/v2/profiles/settings/", upstreamMethod: "get", localOperationId: "mem0.profile.settings.retrieve" },
  { upstreamPath: "/v2/profiles/settings/", upstreamMethod: "post", localOperationId: "mem0.profile.settings.update" },
  { upstreamPath: "/api/v1/webhooks/projects/{project_id}/", upstreamMethod: "get", localOperationId: "mem0.webhook.list" },
  { upstreamPath: "/api/v1/webhooks/projects/{project_id}/", upstreamMethod: "post", localOperationId: "mem0.webhook.create" },
  { upstreamPath: "/api/v1/webhooks/{webhook_id}/", upstreamMethod: "put", localOperationId: "mem0.webhook.update" },
  { upstreamPath: "/api/v1/webhooks/{webhook_id}/", upstreamMethod: "delete", localOperationId: "mem0.webhook.remove" },
  { upstreamPath: "/api/v1/orgs/organizations/{org_id}/projects/{project_id}/", upstreamMethod: "get", localOperationId: "mem0.project.retrieve" },
  { upstreamPath: "/api/v1/orgs/organizations/{org_id}/projects/{project_id}/", upstreamMethod: "patch", localOperationId: "mem0.project.update" },
];

// `/api/v1/` must be a declared mem0 prefix (`paths::MEM0_PATH_PREFIXES`): the
// official clients build their webhook and org/project calls under it, and a
// prefix left undeclared is answered by the framework's surface classifier as a
// `401` problem document the clients cannot parse — indistinguishable from an
// authentication failure for a correctly credentialed caller.
const REQUIRED_MEM0_PREFIXES = ["/v1/", "/v2/", "/v3/", "/api/v1/"];

const PATHS_RS = "crates/sdkwork-routes-memory-open-api/src/paths.rs";

function readDeclaredMem0Prefixes() {
  const text = fs.readFileSync(path.join(root, PATHS_RS), "utf8");
  const match = text.match(/pub const MEM0_PATH_PREFIXES: \[&str; \d+\] = \[([^\]]*)\];/);
  assert.ok(match, `${PATHS_RS} must declare MEM0_PATH_PREFIXES`);
  const prefixes = [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  assert.ok(prefixes.length > 0, "MEM0_PATH_PREFIXES must not be empty");
  return prefixes;
}

// Upstream operation -> mirrored local operation. `attributeSource` selects
// where the upstream request attributes live: `body` (JSON request schema
// properties) or `query` (DELETE has no request body upstream).
const MIRRORED_OPERATIONS = [
  {
    localOperationId: "mem0.memory.add",
    upstream: { path: "/v3/memories/add/", method: "post" },
    attributeSource: "body",
  },
  {
    localOperationId: "mem0.memory.list",
    upstream: { path: "/v3/memories/", method: "post" },
    attributeSource: "body",
  },
  {
    localOperationId: "mem0.memory.removeAll",
    upstream: { path: "/v1/memories/", method: "delete" },
    attributeSource: "query",
  },
];

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(root, relativePath), "utf8"));
}

function resolveRef(document, schema) {
  if (schema && typeof schema.$ref === "string") {
    const name = schema.$ref.split("/").pop();
    return { name, schema: document.components?.schemas?.[name] ?? null };
  }
  return { name: null, schema };
}

function findLocalOperation(document, operationId) {
  for (const pathItem of Object.values(document.paths ?? {})) {
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (["get", "post", "put", "patch", "delete"].includes(method) && operation?.operationId === operationId) {
        return { method, operation };
      }
    }
  }
  return null;
}

function localRequestBodySchema(document, operation) {
  const mediaType = operation.requestBody?.content?.["application/json"];
  if (!mediaType) return null;
  return resolveRef(document, mediaType.schema);
}

function localQueryParameterNames(operation) {
  const names = new Set();
  for (const parameter of operation.parameters ?? []) {
    if (parameter?.in === "query" && typeof parameter.name === "string") {
      names.add(parameter.name);
    }
  }
  return names;
}

function upstreamRequestAttributes(document, { path: routePath, method }) {
  const operation = document.paths?.[routePath]?.[method];
  if (!operation) {
    throw new Error(`upstream document is missing ${method.toUpperCase()} ${routePath}: the upstream contract moved and this gate must be re-triaged`);
  }
  if (operation.requestBody?.content?.["application/json"]?.schema) {
    const { schema } = resolveRef(document, operation.requestBody.content["application/json"].schema);
    if (schema?.properties) {
      return { attributes: Object.keys(schema.properties), attributeSource: "body" };
    }
  }
  const attributes = (operation.parameters ?? [])
    .filter((parameter) => parameter?.in === "query" && typeof parameter.name === "string")
    .map((parameter) => parameter.name);
  if (attributes.length === 0) {
    throw new Error(`upstream ${method.toUpperCase()} ${routePath} declares neither a body schema nor query parameters`);
  }
  return { attributes, attributeSource: "query" };
}

function allowedLocalNames(localDocument, localOperation) {
  const allowed = new Set(REFUSED_PROPERTIES);
  const body = localRequestBodySchema(localDocument, localOperation);
  if (body?.schema?.properties) {
    for (const name of Object.keys(body.schema.properties)) allowed.add(name);
  }
  for (const name of localQueryParameterNames(localOperation)) allowed.add(name);
  return allowed;
}

for (const mirrored of MIRRORED_OPERATIONS) {
  test(`upstream request attributes are reconciled with the local authority: ${mirrored.localOperationId}`, () => {
    const upstream = readJson(UPSTREAM_OPENAPI);
    const local = readJson(LOCAL_OPENAPI);

    const { operation } = findLocalOperation(local, mirrored.localOperationId) ?? {};
    assert.ok(operation, `local authority must declare ${mirrored.localOperationId}`);

    const { attributes, attributeSource } = upstreamRequestAttributes(upstream, mirrored.upstream);
    assert.equal(
      attributeSource,
      mirrored.attributeSource,
      `${mirrored.upstream.method.toUpperCase()} ${mirrored.upstream.path}: upstream attribute surface changed (expected ${mirrored.attributeSource}, found ${attributeSource}); re-triage this gate`,
    );

    const allowed = allowedLocalNames(local, operation);
    const unaccounted = attributes.filter((name) => !allowed.has(name));
    assert.deepEqual(
      unaccounted,
      [],
      `upstream declares request attributes the local authority neither declares nor refuses for ${mirrored.localOperationId} ` +
        `(allowed = local declared fields ∪ declared query parameters ∪ REFUSED_PROPERTIES):\n` +
        unaccounted.map((name) => `  - ${name}`).join("\n"),
    );
  });
}

const OPERATION_METHODS = ["get", "post", "put", "patch", "delete"];

for (const refused of REFUSED_OPERATIONS) {
  test(`upstream operation is a declared named refusal: ${refused.upstreamMethod.toUpperCase()} ${refused.upstreamPath}`, () => {
    const upstream = readJson(UPSTREAM_OPENAPI);
    const local = readJson(LOCAL_OPENAPI);

    const upstreamOperation = upstream.paths?.[refused.upstreamPath]?.[refused.upstreamMethod];
    assert.ok(
      upstreamOperation,
      `upstream document no longer declares ${refused.upstreamMethod.toUpperCase()} ${refused.upstreamPath}: the upstream contract moved and this refusal must be re-triaged`,
    );

    // Locate the local refusal by upstream path shape, not only by operationId,
    // so a renamed local operation cannot hide a broken path mapping.
    const localEntry = Object.entries(local.paths ?? {}).find(
      ([pathKey]) => pathKey === refused.upstreamPath,
    );
    assert.ok(
      localEntry,
      `local authority must declare the refused path ${refused.upstreamPath}`,
    );
    const localOperation = localEntry[1]?.[refused.upstreamMethod];
    assert.ok(
      localOperation,
      `local authority must declare ${refused.upstreamMethod.toUpperCase()} ${refused.upstreamPath}`,
    );
    assert.equal(
      localOperation.operationId,
      refused.localOperationId,
      `local refusal operationId drifted for ${refused.upstreamMethod.toUpperCase()} ${refused.upstreamPath}`,
    );
    assert.equal(
      localOperation["x-sdkwork-wire-protocol"],
      "external",
      `a refused mem0 operation must carry the external wire marker: ${refused.localOperationId}`,
    );

    const statuses = Object.keys(localOperation.responses ?? {});
    assert.ok(
      statuses.includes("501"),
      `${refused.localOperationId} must declare 501 (the named refusal), declared: ${statuses.join(", ")}`,
    );
    const successStatuses = statuses.filter((status) => /^[23]\d\d$/.test(status));
    assert.deepEqual(
      successStatuses,
      [],
      `${refused.localOperationId} is a refusal and must publish no success response, declared: ${statuses.join(", ")}`,
    );
  });
}

test("the mem0 prefix set declares every prefix the official clients build calls under", () => {
  const prefixes = readDeclaredMem0Prefixes();
  for (const required of REQUIRED_MEM0_PREFIXES) {
    assert.ok(
      prefixes.includes(required),
      `${PATHS_RS} must declare ${required} as a mem0 prefix; undeclared, calls under it are ` +
        `answered by the framework's surface classifier as a ` +
        `problem-document 401 the mem0 clients cannot parse`,
    );
  }
  assert.deepEqual(
    prefixes.filter((prefix, index) => prefixes.indexOf(prefix) !== index),
    [],
    "MEM0_PATH_PREFIXES holds duplicate entries",
  );
});
