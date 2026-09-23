import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const testDir = path.dirname(fileURLToPath(import.meta.url));
const sdksRoot = path.resolve(testDir, "..");
const workspaceRoot = path.resolve(sdksRoot, "..");

const families = [
  {
    root: "sdkwork-memory-sdk",
    owner: "sdkwork-memory",
    authority: "sdkwork-memory-open-api",
    input: "openapi/memory-open-api.openapi.json",
    manifest: "sdk-manifest.json",
    forbiddenPathPrefixes: ["/app/v3/api/", "/backend/v3/api/"],
  },
  {
    root: "sdkwork-memory-app-sdk",
    owner: "sdkwork-memory",
    authority: "sdkwork-memory.app",
    input: "openapi/memory-app-api.openapi.json",
    manifest: "sdk-manifest.json",
    forbiddenPathPrefixes: ["/backend/v3/api/", "/mem/v3/api/"],
  },
  {
    root: "sdkwork-memory-backend-sdk",
    owner: "sdkwork-memory",
    authority: "sdkwork-memory.backend",
    input: "openapi/memory-backend-api.openapi.json",
    manifest: "sdk-manifest.json",
    forbiddenPathPrefixes: ["/app/v3/api/", "/mem/v3/api/"],
  },
];

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(workspaceRoot, relativePath), "utf8"));
}

function operationEntries(openapi) {
  const entries = [];
  for (const [pathKey, pathItem] of Object.entries(openapi.paths || {})) {
    for (const [method, operation] of Object.entries(pathItem || {})) {
      if (!["get", "put", "post", "patch", "delete", "head", "options", "trace"].includes(method)) {
        continue;
      }
      entries.push({ pathKey, method, operation });
    }
  }
  return entries;
}

test("memory SDK manifests record owner and authority boundaries", () => {
  for (const family of families) {
    const manifest = readJson(path.join("sdks", family.root, family.manifest));
    assert.equal(manifest.sdkOwner, family.owner, `${family.root} manifest must declare sdkOwner`);
    assert.equal(manifest.apiAuthority, family.authority, `${family.root} manifest must declare apiAuthority`);
    assert.equal(
      manifest.generationInputSpec,
      family.input,
      `${family.root} manifest must point at owner-only OpenAPI input`,
    );
    assert.equal(manifest.standardProfile, "sdkwork-v3", `${family.root} manifest must declare standardProfile sdkwork-v3`);
    assert.equal(
      manifest.discoverySurface.apiPrefix,
      manifest.apiPrefix,
      `${family.root} manifest discovery prefix must match apiPrefix`,
    );
  }
});

test("memory generated OpenAPI inputs contain only sdkwork-memory owned operations", () => {
  for (const family of families) {
    const openapi = readJson(path.join("sdks", family.root, family.input));
    assert.equal(openapi["x-sdkwork-owner"], family.owner);
    assert.equal(openapi["x-sdkwork-api-authority"], family.authority);

    for (const { pathKey, method, operation } of operationEntries(openapi)) {
      assert.equal(
        operation["x-sdkwork-owner"],
        family.owner,
        `${family.root} ${method.toUpperCase()} ${pathKey} must be memory-owned`,
      );
      assert.equal(
        operation["x-sdkwork-api-authority"],
        family.authority,
        `${family.root} ${method.toUpperCase()} ${pathKey} must use ${family.authority}`,
      );
      assert.equal(
        operation["x-sdkwork-request-context"],
        "WebRequestContext",
        `${family.root} ${method.toUpperCase()} ${pathKey} must declare WebRequestContext`,
      );
      assert(
        !family.forbiddenPathPrefixes.some((prefix) => pathKey.startsWith(prefix)),
        `${family.root} must not copy dependency-owned route ${method.toUpperCase()} ${pathKey}`,
      );
    }
  }
});

test("backend capability resolution preserves its named page data contract", () => {
  const openapi = readJson(
    path.join(
      "sdks",
      "sdkwork-memory-backend-sdk",
      "openapi/memory-backend-api.openapi.json",
    ),
  );
  const operation = openapi.paths["/backend/v3/api/memory/capabilities/resolve"].post;
  const successSchema = operation.responses["200"].content["application/json"].schema;
  const pageSchema = openapi.components.schemas.MemoryResolvedCapabilityList;
  const responseSchema = openapi.components.schemas.MemoryResolvedCapabilityListResponse;

  assert.deepEqual(successSchema, {
    $ref: "#/components/schemas/MemoryResolvedCapabilityListResponse",
  });
  assert.deepEqual(pageSchema.required, ["items", "pageInfo"]);
  assert.equal(
    pageSchema.properties.items.items.$ref,
    "#/components/schemas/MemoryResolvedCapability",
  );
  assert.equal(pageSchema.properties.pageInfo.$ref, "#/components/schemas/PageInfo");
  assert.equal(
    responseSchema.allOf[1].properties.data.$ref,
    "#/components/schemas/MemoryResolvedCapabilityList",
  );
});

test("backend TypeScript capability resolution returns the named page type", () => {
  const generatedApi = readFileSync(
    path.join(
      workspaceRoot,
      "sdks",
      "sdkwork-memory-backend-sdk",
      "sdkwork-memory-backend-sdk-typescript",
      "generated/server-openapi/src/api/memory.ts",
    ),
    "utf8",
  );

  // Pins the contract this test is named for: the request body type and the
  // named page return type. Parameter arity is deliberately not pinned —
  // `params` becomes required as soon as the operation carries a required
  // parameter such as the Idempotency-Key header, and the generator appends a
  // `requestOptions` argument to every method.
  assert.match(
    generatedApi,
    /async resolve\(body: MemoryResolveCapabilitiesRequest,[^)]*\): Promise<MemoryResolvedCapabilityList> \{/,
  );
});

test("generated TypeScript SDK methods do not expose current tenant input", () => {
  const generatedApiFiles = [
    ["sdkwork-memory-sdk", "sdkwork-memory-sdk-typescript"],
    ["sdkwork-memory-app-sdk", "sdkwork-memory-app-sdk-typescript"],
    ["sdkwork-memory-backend-sdk", "sdkwork-memory-backend-sdk-typescript"],
  ];
  const generatedMethods = generatedApiFiles
    .map(([family, workspace]) =>
      readFileSync(
        path.join(
          workspaceRoot,
          "sdks",
          family,
          workspace,
          "generated/server-openapi/src/api/memory.ts",
        ),
        "utf8",
      ),
    )
    .join("\n");

  assert.doesNotMatch(generatedMethods, /\btenantId\b/);
});
