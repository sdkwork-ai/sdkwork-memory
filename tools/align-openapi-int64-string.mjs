#!/usr/bin/env node
/**
 * Aligns the Memory OpenAPI authorities with API_SPEC.md section 13.6:
 * every int64 wire field MUST be `type: string` + `format: int64` + a decimal
 * `pattern` + the `x-sdkwork-int64-string: true` marker.
 *
 * `type: integer, format: int64` is a contract violation: generated TypeScript
 * SDKs then emit `number` and browsers silently round ids past 2^53.
 *
 * Structural (not regex) rewriting: the authorities are byte-exact
 * `JSON.stringify(doc, null, 2)` with LF endings, so a parse -> normalize ->
 * serialize round trip is lossless. A no-op round-trip guard aborts if that
 * assumption ever stops holding, and a post-serialization conformance walk
 * refuses to persist a document that still violates section 13.6.
 *
 * Usage:
 *   node tools/align-openapi-int64-string.mjs          # normalize in place
 *   node tools/align-openapi-int64-string.mjs --check  # fail if drift exists
 */
import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const checkOnly = process.argv.includes("--check");

const AUTHORITIES = [
  "apis/open-api/memory-open-api.openapi.json",
  "apis/app-api/memory-app-api.openapi.json",
  "apis/backend-api/memory-backend-api.openapi.json",
];

const INT64_MARKER_KEY = "x-sdkwork-int64-string";
const INT64_FORMAT = "int64";
const INT64_PATTERN = "^[0-9]+$";

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function isPlainObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

/**
 * Rebuilds a marked schema node so that `format: int64` sits immediately after
 * `type`, preserving every other key and its original relative order.
 */
function normalizeMarkedNode(node) {
  const rebuilt = {};
  for (const [key, value] of Object.entries(node)) {
    if (key === "format") continue; // re-emitted right after `type`
    rebuilt[key] = value;
    if (key === "type") rebuilt.format = INT64_FORMAT;
  }
  if (!("format" in rebuilt)) rebuilt.format = INT64_FORMAT;
  rebuilt[INT64_MARKER_KEY] = true;
  return rebuilt;
}

/** Walks the document, rewriting every int64-string schema. Returns change count. */
function normalizeDocument(node) {
  let changed = 0;
  const visit = (value) => {
    if (Array.isArray(value)) {
      for (let i = 0; i < value.length; i += 1) value[i] = visit(value[i]);
      return value;
    }
    if (!isPlainObject(value)) return value;

    const isMarked = value[INT64_MARKER_KEY] === true;
    const isIntegerInt64 = value.type === "integer" && value.format === INT64_FORMAT;

    for (const key of Object.keys(value)) {
      if (key === INT64_MARKER_KEY) continue;
      value[key] = visit(value[key]);
    }

    if (!isMarked) return value;

    const needsWork =
      value.type !== "string" ||
      value.format !== INT64_FORMAT ||
      value.pattern !== INT64_PATTERN ||
      Object.keys(value).indexOf("format") !== Object.keys(value).indexOf("type") + 1;
    if (needsWork) {
      const rebuilt = normalizeMarkedNode(value);
      if (!isIntegerInt64 && JSON.stringify(rebuilt) !== JSON.stringify(value)) changed += 1;
      return rebuilt;
    }
    return value;
  };
  const result = visit(node);
  return { document: result, changed };
}

/** Structural conformance walk used both as a pre-check and as a post-write guard. */
function conformanceFailures(document, relative) {
  const failures = [];
  const visit = (node, trail) => {
    if (Array.isArray(node)) {
      node.forEach((item, index) => visit(item, `${trail}[${index}]`));
      return;
    }
    if (!isPlainObject(node)) return;

    if (node[INT64_MARKER_KEY] === true) {
      if (node.type !== "string") {
        failures.push(`${relative} ${trail}: marked int64 schema must use type "string" (found ${JSON.stringify(node.type)})`);
      }
      if (node.format !== INT64_FORMAT) {
        failures.push(`${relative} ${trail}: marked int64 schema must declare format "int64"`);
      }
      if (node.pattern !== INT64_PATTERN) {
        failures.push(`${relative} ${trail}: marked int64 schema must declare pattern ${JSON.stringify(INT64_PATTERN)}`);
      }
    }

    if (node.type === "integer" && node.format === INT64_FORMAT) {
      failures.push(`${relative} ${trail}: type "integer" + format "int64" is an API_SPEC 13.6 violation; use a string type with the marker`);
    }

    for (const [key, value] of Object.entries(node)) visit(value, `${trail}/${key}`);
  };
  visit(document, "");
  return failures;
}

function serialize(document) {
  return `${JSON.stringify(document, null, 2)}\n`;
}

const failures = [];
const aligned = [];
const alreadyClean = [];

for (const relative of AUTHORITIES) {
  const filePath = path.join(repoRoot, relative);
  // `.gitattributes` pins `* text=auto eol=lf` and the canonical materializer
  // (tools/materialize_phase1_contracts.mjs) writes LF, so the tool owns LF as
  // the worktree EOL. Normalizing CRLF here keeps a stray Windows checkout from
  // tripping the canonicality guard or producing an EOL-flip diff.
  const original = fs.readFileSync(filePath, "utf8").replace(/\r\n/g, "\n");

  let document;
  try {
    document = JSON.parse(original);
  } catch (error) {
    failures.push(`${relative}: cannot parse authority JSON (${error.message})`);
    continue;
  }

  // Guard: the structural rewrite is only lossless if the file is canonical
  // pretty-printed JSON. Verify a no-op round trip before touching anything.
  if (serialize(clone(document)) !== original) {
    failures.push(
      `${relative}: authority is not canonical JSON.stringify(doc, null, 2) + LF; refusing structural rewrite`,
    );
    continue;
  }

  const preExisting = conformanceFailures(document, relative);

  if (checkOnly) {
    if (preExisting.length > 0) failures.push(...preExisting.slice(0, 10));
    else alreadyClean.push(relative);
    continue;
  }

  const { document: normalized, changed } = normalizeDocument(document);

  const postFailures = conformanceFailures(normalized, relative);
  if (postFailures.length > 0) {
    failures.push(...postFailures.slice(0, 10));
    continue;
  }

  const next = serialize(normalized);
  if (next === original) {
    alreadyClean.push(relative);
    continue;
  }

  try {
    JSON.parse(next);
  } catch (error) {
    failures.push(`${relative}: normalization produced invalid JSON (${error.message}); file left untouched`);
    continue;
  }

  fs.writeFileSync(filePath, next, "utf8");
  aligned.push(`${relative} (${changed} schema(s))`);
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`[openapi-int64] ${failure}`);
  console.error(`[openapi-int64] failed with ${failures.length} problem(s)`);
  process.exit(1);
}

if (checkOnly) {
  console.log(`[openapi-int64] ok: ${alreadyClean.length} authority file(s) conform to API_SPEC 13.6`);
} else if (aligned.length === 0) {
  console.log("[openapi-int64] unchanged: all authorities already conform to API_SPEC 13.6");
} else {
  console.log(`[openapi-int64] aligned: ${aligned.join(", ")}`);
}
