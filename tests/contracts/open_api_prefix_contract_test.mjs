import assert from "node:assert/strict";
import fs from "node:fs";

const openApiPrefix = "/mem/v3/api";
const legacyOpenApiPrefix = "/memory/v3/api";
const legacyOpenApiSchemaUrl = "/memory/v3/openapi.json";

const readJson = (path) => JSON.parse(fs.readFileSync(path, "utf8"));

const collectMarkdownFiles = (rootDir) => {
  const ignoredDirs = new Set([".git", "target"]);
  const files = [];
  for (const entry of fs.readdirSync(rootDir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      if (!ignoredDirs.has(entry.name)) {
        files.push(...collectMarkdownFiles(`${rootDir}/${entry.name}`));
      }
      continue;
    }
    if (entry.isFile() && entry.name.endsWith(".md")) {
      files.push(`${rootDir}/${entry.name}`);
    }
  }
  return files;
};

const rootSpec = readJson("specs/component.spec.json");
const openAuthority = rootSpec.contracts.apiAuthorities.find(
  (authority) => authority.name === "sdkwork-memory-open-api",
);
assert.ok(openAuthority, "Root component spec must declare sdkwork-memory-open-api");
assert.equal(
  openAuthority.prefix,
  openApiPrefix,
  "Memory public open-api prefix must use /mem/v3/api to avoid /memory/.../memory URL duplication",
);

const sdkManifest = readJson("sdks/sdkwork-memory-sdk/sdk-manifest.json");
assert.equal(
  sdkManifest.discoverySurface.apiPrefix,
  openApiPrefix,
  "Memory open SDK manifest must use the /mem/v3/api public prefix",
);
// schemaUrl was removed pre-launch: no /mem/v3/openapi.json discovery endpoint
// is implemented, and the manifest must not promise one.
assert.equal(
  sdkManifest.discoverySurface.schemaUrl,
  undefined,
  "Memory open SDK manifest must not declare an unimplemented schema URL",
);
assert.equal(
  sdkManifest.apiPrefix,
  openApiPrefix,
  "Memory open SDK manifest must use the /mem/v3/api public prefix",
);

const sdkComponent = readJson("sdks/sdkwork-memory-sdk/specs/component.spec.json");
assert.equal(
  sdkComponent.contracts.apiAuthority.prefix,
  openApiPrefix,
  "Memory open SDK component spec must use the /mem/v3/api public prefix",
);

const openApi = readJson("sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json");
assert.equal(
  openApi["x-sdkwork-api-prefix"],
  openApiPrefix,
  "Memory open OpenAPI authority must advertise the /mem/v3/api public prefix",
);

// Vendor compatibility wire operations (sdkwork-specs/API_SPEC.md section 4.5.2)
// mirror an upstream third-party protocol under that protocol's own path
// prefixes (`/v1/...`, `/v3/...`) and are identified by the operation-level
// marker pair below. They are exempt from the SDKWork-owned `/mem/v3/api/memory`
// prefix rule; SDKWork-owned operations in the same document are not.
const isExternalProtocolOperation = (operation) =>
  operation?.["x-sdkwork-wire-protocol"] === "external" &&
  typeof operation?.["x-sdkwork-external-protocol-id"] === "string" &&
  operation["x-sdkwork-external-protocol-id"].trim().length > 0;

let sdkWorkOwnedPathCount = 0;
for (const [path, pathItem] of Object.entries(openApi.paths)) {
  const ownedOperations = Object.values(pathItem ?? {}).filter(
    (operation) =>
      operation && typeof operation === "object" && operation.operationId !== undefined,
  ).filter((operation) => !isExternalProtocolOperation(operation));
  if (ownedOperations.length === 0) {
    assert.ok(
      Object.values(pathItem ?? {}).some(isExternalProtocolOperation),
      `Memory open OpenAPI path must be either SDKWork-owned or an external-protocol operation: ${path}`,
    );
    continue;
  }
  sdkWorkOwnedPathCount += 1;
  assert.ok(
    path.startsWith(`${openApiPrefix}/memory`),
    `Memory open OpenAPI path must start with ${openApiPrefix}/memory: ${path}`,
  );
  assert.ok(
    !path.startsWith(legacyOpenApiPrefix),
    `Memory open OpenAPI path must not keep legacy duplicated prefix: ${path}`,
  );
}

// Guard against the exemption silently swallowing the whole document: the
// SDKWork-owned surface must still be present and prefix-correct.
assert.ok(
  sdkWorkOwnedPathCount > 0,
  "Memory open OpenAPI must still expose SDKWork-owned /mem/v3/api/memory operations",
);

for (const markdownPath of collectMarkdownFiles(".")) {
  const markdown = fs.readFileSync(markdownPath, "utf8");
  assert.ok(
    !markdown.includes(legacyOpenApiPrefix),
    `${markdownPath} must not document the legacy Memory open-api prefix`,
  );
  assert.ok(
    !markdown.includes(legacyOpenApiSchemaUrl),
    `${markdownPath} must not document the legacy Memory open-api schema URL`,
  );
}
