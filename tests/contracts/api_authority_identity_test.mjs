// API authority identity gate.
//
// API_SPEC requires api-authority names to be lower-kebab-case identifiers
// (`^sdkwork-[a-z0-9-]+$`). Earlier Memory materialization shipped dotted
// aliases (`sdkwork-memory.app`, `sdkwork-memory.backend`), which violate that
// grammar and split the authority namespace into two spellings for one
// surface. This gate scans every published authority carrier — materialized
// authority OpenAPI documents, SDK family manifests, and route manifests — and
// asserts no dotted (or otherwise malformed) authority value remains anywhere.
//
// It is inventory-complete on purpose: a new carrier file that forgets to
// rename, or a hand-maintained manifest that reintroduces the dotted spelling,
// turns this gate red instead of hiding behind the renamed materializer.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const AUTHORITY_PATTERN = /^sdkwork-[a-z0-9-]+$/u;

function listFiles(relativeDir, fileName) {
  const dir = path.join(root, relativeDir);
  if (!fs.existsSync(dir)) return [];
  const found = [];
  for (const entry of fs.readdirSync(dir, { recursive: true })) {
    if (path.basename(String(entry)) === fileName) {
      found.push(path.join(relativeDir, String(entry)).replaceAll("\\", "/"));
    }
  }
  return found.sort();
}

function listByExtension(relativeDir, extension) {
  const dir = path.join(root, relativeDir);
  if (!fs.existsSync(dir)) return [];
  const found = [];
  for (const entry of fs.readdirSync(dir, { recursive: true })) {
    if (String(entry).endsWith(extension)) {
      found.push(path.join(relativeDir, String(entry)).replaceAll("\\", "/"));
    }
  }
  return found.sort();
}

function readJson(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(root, relativePath), "utf8"));
}

function assertAuthorityValue(value, source, field) {
  assert.ok(
    typeof value === "string" && value.length > 0,
    `${source}: ${field} must be a non-empty string, got ${JSON.stringify(value)}`,
  );
  assert.match(
    value,
    AUTHORITY_PATTERN,
    `${source}: ${field} "${value}" violates the API_SPEC authority grammar (lower-kebab-case, no dots)`,
  );
}

function collectOpenApiAuthorities() {
  const authorities = [];
  for (const relativePath of listByExtension("apis", ".openapi.json")) {
    const document = readJson(relativePath);
    authorities.push({
      source: relativePath,
      field: "x-sdkwork-api-authority (root)",
      value: document["x-sdkwork-api-authority"],
    });
    for (const [pathKey, pathItem] of Object.entries(document.paths ?? {})) {
      for (const [method, operation] of Object.entries(pathItem ?? {})) {
        if (!["get", "post", "put", "patch", "delete"].includes(method)) continue;
        authorities.push({
          source: `${relativePath} ${method.toUpperCase()} ${pathKey}`,
          field: "x-sdkwork-api-authority (operation)",
          value: operation?.["x-sdkwork-api-authority"],
        });
      }
    }
  }
  return authorities;
}

function collectSdkManifestAuthorities() {
  return listFiles("sdks", "sdk-manifest.json").map((relativePath) => ({
    source: relativePath,
    field: "apiAuthority",
    value: readJson(relativePath).apiAuthority,
  }));
}

function collectRouteManifestAuthorities() {
  const authorities = [];
  for (const relativePath of listByExtension("sdks/_route-manifests", ".route-manifest.json")) {
    const manifest = readJson(relativePath);
    authorities.push({
      source: relativePath,
      field: "apiAuthority (root)",
      value: manifest.apiAuthority,
    });
    for (const route of manifest.routes ?? []) {
      authorities.push({
        source: `${relativePath} ${route.method} ${route.path}`,
        field: "ownership.apiAuthority",
        value: route?.ownership?.apiAuthority,
      });
    }
  }
  return authorities;
}

test("inventory finds authority carriers", () => {
  assert.ok(listByExtension("apis", ".openapi.json").length >= 3, "expected the three materialized authority OpenAPI documents");
  assert.ok(listFiles("sdks", "sdk-manifest.json").length >= 3, "expected the three SDK family manifests");
  assert.ok(
    listByExtension("sdks/_route-manifests", ".route-manifest.json").length >= 3,
    "expected the three route manifests",
  );
});

test("every published api-authority value matches the API_SPEC grammar", () => {
  const authorities = [
    ...collectOpenApiAuthorities(),
    ...collectSdkManifestAuthorities(),
    ...collectRouteManifestAuthorities(),
  ];
  assert.ok(authorities.length > 100, `expected a real authority inventory, found ${authorities.length}`);
  for (const { source, field, value } of authorities) {
    assertAuthorityValue(value, source, field);
  }
});

test("authority identity is consistent within each route manifest", () => {
  for (const relativePath of listByExtension("sdks/_route-manifests", ".route-manifest.json")) {
    const manifest = readJson(relativePath);
    const rootAuthority = manifest.apiAuthority;
    for (const route of manifest.routes ?? []) {
      assert.equal(
        route?.ownership?.apiAuthority,
        rootAuthority,
        `${relativePath}: route ${route.method} ${route.path} ownership authority diverges from the manifest root`,
      );
    }
  }
});
