import assert from "node:assert/strict";
import fs from "node:fs";

const open = JSON.parse(
  fs.readFileSync(
    "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json",
    "utf8",
  ),
);

const operations = Object.values(open.paths).flatMap((pathItem) =>
  ["get", "post", "put", "patch", "delete"]
    .filter((method) => pathItem[method])
    .map((method) => pathItem[method]),
);

const operationIds = new Set(operations.map((operation) => operation.operationId));
assert.equal(operationIds.size, operations.length);

const requiredPhase1OperationIds = [
  "capabilities.retrieve",
  "events.create",
  "events.retrieve",
  "memories.create",
  "memories.list",
  "memories.retrieve",
  "memories.update",
  "memories.delete",
  "retrievals.create",
  "retrievals.retrieve",
  "contextPacks.create",
  "contextPacks.retrieve",
  "feedback.create",
  "extractions.create",
  "candidates.list",
  "candidates.retrieve",
  "providerHealth.retrieve",
  "entities.list",
  "entities.create",
  "entities.retrieve",
  "entities.update",
  "edges.list",
  "edges.create",
  "edges.retrieve",
  "edges.update",
  "edges.delete",
];
for (const operationId of requiredPhase1OperationIds) {
  assert.ok(operationIds.has(operationId), `missing Phase 1 operation: ${operationId}`);
}

assert.ok(open.components.securitySchemes.ApiKey);
assert.ok(!open.components.securitySchemes.AuthToken);
assert.ok(!open.components.securitySchemes.AccessToken);
for (const operation of operations) {
  assert.deepEqual(operation.security, [{ ApiKey: [] }]);
  assert.equal(operation["x-sdkwork-auth-mode"], "api-key");
  assert.equal(operation["x-sdkwork-api-authority"], "sdkwork-memory-open-api");
}

for (const [pathKey, pathItem] of Object.entries(open.paths)) {
  for (const method of ["post", "put", "patch", "delete"]) {
    const operation = pathItem[method];
    if (!operation) {
      continue;
    }
    // The auth-critical tier is reserved for the destructive/privacy-critical
    // set the materializer assigns via AUTH_CRITICAL_OPERATION_IDS; on this
    // surface that is the canonical bulk delete plus the mem0 compatibility
    // wire's whole-space erase, which is the same destructive class.
    const authCritical =
      operation.operationId === "memories.delete" ||
      operation.operationId === "mem0.memory.removeAll";
    const expectedTier = authCritical ? "authCritical" : "openApiDefault";
    assert.equal(
      operation["x-sdkwork-rate-limit-tier"],
      expectedTier,
      `${method.toUpperCase()} ${pathKey} must declare ${expectedTier} rate limit tier`,
    );
  }
}

const deleteOp = operations.find((operation) => operation.operationId === "memories.delete");
assert.ok(deleteOp);
assert.equal(deleteOp["x-sdkwork-rate-limit-tier"], "authCritical");

const mem0RemoveAllOp = operations.find(
  (operation) => operation.operationId === "mem0.memory.removeAll",
);
assert.ok(mem0RemoveAllOp, "mem0 compatibility wire must declare mem0.memory.removeAll");
assert.equal(
  mem0RemoveAllOp["x-sdkwork-rate-limit-tier"],
  "authCritical",
  "the mem0 whole-space erase rides the auth-critical tier like memories.delete",
);

console.log("OpenAPI phase1 contract test passed");
