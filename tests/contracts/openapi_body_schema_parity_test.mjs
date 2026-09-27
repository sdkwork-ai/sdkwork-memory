// OpenAPI ↔ contract-DTO body parity gate.
//
// The route-manifest and query-param parity gates prove inventory and GET
// query parity, but nothing proved that published request/response BODY
// schemas match the Rust DTOs the handlers actually (de)serialize. That blind
// spot let `learningSettings` publish a fully fictional schema. This gate
// closes it: for every OpenAPI component whose name matches a
// `#[serde(rename_all = "camelCase")]` struct in the contract crate, the
// property set and the required set must equal the DTO's serde field set.

import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const contractSrcDir = path.join(root, "crates/sdkwork-memory-contract/src");

function snakeToCamel(name) {
  return name.replace(/_([a-z0-9])/g, (_, c) => c.toUpperCase());
}

// Extract `#[serde(...)] pub struct X { ... }` blocks with balanced braces and
// reduce each to `{ field: { required } }` under serde rules: a field is
// required unless it is `Option<T>` or carries `#[serde(default ...)]`.
function parseContractStructs() {
  const structs = new Map();
  for (const entry of fs.readdirSync(contractSrcDir, { recursive: true })) {
    const file = path.join(contractSrcDir, entry);
    if (!file.endsWith(".rs")) continue;
    const source = fs.readFileSync(file, "utf8");
    const structRegex = /(#\[\s*serde\s*\(([^)]*)\)\s*]\s*)?pub struct (\w+)\s*\{/g;
    for (const match of source.matchAll(structRegex)) {
      const [, , serdeArgs, name] = match;
      const bodyStart = match.index + match[0].length;
      let depth = 1;
      let i = bodyStart;
      while (i < source.length && depth > 0) {
        if (source[i] === "{") depth += 1;
        else if (source[i] === "}") depth -= 1;
        i += 1;
      }
      const body = source.slice(bodyStart, i - 1);
      if (serdeArgs && /untagged|flatten/.test(serdeArgs)) continue;
      const fields = new Map();
      // Rust struct fields end with a comma (never a semicolon); the type
      // fragment only feeds the Option-detection below, so a generic type
      // with an internal comma may truncate harmlessly.
      const fieldRegex = /((?:#\[[^\]]*\]\s*)*)pub\s+(\w+)\s*:\s*([A-Za-z_][^;,]*)[,;]/g;
      for (const fieldMatch of body.matchAll(fieldRegex)) {
        const [, attrs, fieldName, fieldType] = fieldMatch;
        const attrText = attrs ?? "";
        const rename = attrText.match(/rename\s*=\s*"([^"]+)"/)?.[1];
        const wireName = rename ?? snakeToCamel(fieldName);
        const optional =
          /Option</.test(fieldType.split("//")[0]) ||
          /serde\s*\([^)]*\bdefault\b/.test(attrText);
        fields.set(wireName, { required: !optional });
      }
      if (fields.size > 0 && !structs.has(name)) {
        structs.set(name, fields);
      }
    }
  }
  return structs;
}

function loadOpenApiComponents(relativePath) {
  const document = JSON.parse(fs.readFileSync(path.join(root, relativePath), "utf8"));
  return document.components?.schemas ?? {};
}

const openApiFiles = [
  "apis/open-api/memory-open-api.openapi.json",
  "apis/app-api/memory-app-api.openapi.json",
  "apis/backend-api/memory-backend-api.openapi.json",
];

const ignoredComponents = new Set([
  // Envelope/standard components contributed by the shared specs library.
  "SdkWorkApiResponse",
  "SdkWorkListResponse",
  "SdkWorkCommandResponse",
  "SdkWorkCommandData",
  "SdkWorkListQuery",
  "PageInfo",
  "ProblemDetail",
  "FieldError",
]);

test("contract DTO structs are discovered", () => {
  const structs = parseContractStructs();
  assert.ok(structs.size > 40, `expected a real DTO inventory, found ${structs.size}`);
});

for (const openApiFile of openApiFiles) {
  test(`OpenAPI component schemas match contract DTOs: ${openApiFile}`, () => {
    const components = loadOpenApiComponents(openApiFile);
    const structs = parseContractStructs();
    const problems = [];

    for (const [schemaName, schema] of Object.entries(components)) {
      if (ignoredComponents.has(schemaName)) continue;
      const dto = structs.get(schemaName);
      if (!dto) continue;
      if (schema.type !== "object" || !schema.properties) continue;

      const schemaProps = new Set(Object.keys(schema.properties));
      const dtoProps = new Set(dto.keys());
      for (const prop of schemaProps) {
        if (!dtoProps.has(prop)) {
          problems.push(`${schemaName}: OpenAPI declares unknown property "${prop}"`);
        }
      }
      for (const prop of dtoProps) {
        if (!schemaProps.has(prop)) {
          problems.push(`${schemaName}: DTO field "${prop}" is missing from OpenAPI`);
        }
      }
      const required = new Set(schema.required ?? []);
      for (const [prop, { required: shouldBeRequired }] of dto) {
        if (!schemaProps.has(prop)) continue;
        if (shouldBeRequired && !required.has(prop)) {
          problems.push(`${schemaName}: "${prop}" is required in the DTO but optional in OpenAPI`);
        }
        if (!shouldBeRequired && required.has(prop)) {
          problems.push(`${schemaName}: "${prop}" is optional in the DTO but required in OpenAPI`);
        }
      }
    }

    assert.equal(
      problems.length,
      0,
      `body-schema drift between ${openApiFile} and contract DTOs:\n${problems.map((p) => `  - ${p}`).join("\n")}`,
    );
  });
}
