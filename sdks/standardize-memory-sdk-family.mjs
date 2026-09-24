#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(scriptDir, "..");
const checkOnly = process.argv.includes("--check");

const owner = "sdkwork-memory";
const standardVersion = "2026-06-10";
const HTTP_METHODS = new Set(["get", "post", "put", "patch", "delete"]);
const QUERY_PARAM_PATTERN = /^[a-z][a-z0-9_]*$/;

const families = [
  {
    root: "sdks/sdkwork-memory-sdk",
    authority: "sdkwork-memory-open-api",
    authoritySpec: "apis/open-api/memory-open-api.openapi.json",
    input: "openapi/memory-open-api.openapi.json",
    packageName: "@sdkwork/memory-sdk",
    apiPrefix: "/mem/v3/api",
    clientName: "SdkworkMemoryOpenClient",
    forbiddenPathPrefixes: ["/app/v3/api/", "/backend/v3/api/"],
  },
  {
    root: "sdks/sdkwork-memory-app-sdk",
    authority: "sdkwork-memory.app",
    authoritySpec: "apis/app-api/memory-app-api.openapi.json",
    input: "openapi/memory-app-api.openapi.json",
    packageName: "@sdkwork/memory-app-sdk",
    apiPrefix: "/app/v3/api",
    clientName: "SdkworkMemoryAppClient",
    forbiddenPathPrefixes: ["/backend/v3/api/", "/mem/v3/api/"],
  },
  {
    root: "sdks/sdkwork-memory-backend-sdk",
    authority: "sdkwork-memory.backend",
    authoritySpec: "apis/backend-api/memory-backend-api.openapi.json",
    input: "openapi/memory-backend-api.openapi.json",
    packageName: "@sdkwork/memory-backend-sdk",
    apiPrefix: "/backend/v3/api",
    clientName: "SdkworkMemoryBackendClient",
    forbiddenPathPrefixes: ["/app/v3/api/", "/mem/v3/api/"],
  },
];

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(workspaceRoot, relativePath), "utf8"));
}

const failures = [];
const syncedInputs = [];

for (const family of families) {
  const manifest = readJson(path.join(family.root, "sdk-manifest.json"));
  const component = readJson(path.join(family.root, "specs/component.spec.json"));

  const authorityPath = path.join(workspaceRoot, family.authoritySpec);
  const inputPath = path.join(workspaceRoot, family.root, family.input);
  const authorityText = readFileSync(authorityPath, "utf8");
  if (authorityText !== readFileSync(inputPath, "utf8")) {
    if (checkOnly) {
      failures.push(
        `${family.root} generation input must be a byte-identical mirror of ${family.authoritySpec}`,
      );
    } else {
      writeFileSync(inputPath, authorityText, "utf8");
      syncedInputs.push(`${family.root}/${family.input}`);
    }
  }

  if (manifest.sdkOwner !== owner) {
    failures.push(`${family.root} manifest sdkOwner must be ${owner}`);
  }
  if (manifest.apiAuthority !== family.authority) {
    failures.push(`${family.root} apiAuthority mismatch`);
  }
  if (manifest.generationInputSpec !== family.input) {
    failures.push(`${family.root} generationInputSpec mismatch`);
  }
  if (
    manifest.apiPrefix !== family.apiPrefix
    || manifest.discoverySurface?.apiPrefix !== family.apiPrefix
  ) {
    failures.push(`${family.root} discovery surface mismatch`);
  }
  if (manifest.standardProfile !== "sdkwork-v3") {
    failures.push(`${family.root} must declare standardProfile sdkwork-v3`);
  }
  if (manifest.packageName !== family.packageName) {
    failures.push(`${family.root} packageName mismatch`);
  }
  if (!component.contracts.sdkClients.includes(family.clientName)) {
    failures.push(`${family.root} component spec must declare ${family.clientName}`);
  }

  const openapi = readJson(path.join(family.root, family.input));
  if (openapi["x-sdkwork-owner"] !== owner) {
    failures.push(`${family.root} OpenAPI x-sdkwork-owner mismatch`);
  }
  for (const [routePath, pathItem] of Object.entries(openapi.paths ?? {})) {
    for (const prefix of family.forbiddenPathPrefixes) {
      if (routePath.startsWith(prefix)) {
        failures.push(`${family.root} must not include dependency route ${routePath}`);
      }
    }
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (!HTTP_METHODS.has(method)) continue;
      for (const parameter of operation.parameters ?? []) {
        if (parameter.in !== "query") continue;
        if (!QUERY_PARAM_PATTERN.test(parameter.name)) {
          failures.push(
            `${family.root} ${operation.operationId ?? routePath} query parameter '${parameter.name}' must be lower_snake_case (PAGINATION_SPEC 14.1.1)`,
          );
        }
      }
    }
  }
}

if (failures.length > 0) {
  console.error(JSON.stringify({ ok: false, mode: checkOnly ? "check" : "validate", failures }, null, 2));
  process.exit(1);
}

console.log(
  JSON.stringify(
    {
      ok: true,
      mode: checkOnly ? "check" : "validate",
      owner,
      standardVersion,
      families: families.length,
      syncedInputs,
    },
    null,
    2,
  ),
);
