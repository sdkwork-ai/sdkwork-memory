import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..");
const checkOnly = process.argv.includes("--check");

const authorities = [
  "apis/open-api/memory-open-api.openapi.json",
  "apis/app-api/memory-app-api.openapi.json",
  "apis/backend-api/memory-backend-api.openapi.json",
];

const DEFAULT_PAGE_SIZE = 20;
const MAX_PAGE_SIZE = 200;
const PAGE_SIZE_PARAM = '"name": "page_size"';

function findObjectEnd(text, openIndex) {
  let depth = 0;
  let inString = false;
  for (let i = openIndex; i < text.length; i += 1) {
    const ch = text[i];
    if (inString) {
      if (ch === "\\") {
        i += 1;
      } else if (ch === '"') {
        inString = false;
      }
      continue;
    }
    if (ch === '"') {
      inString = true;
    } else if (ch === "{") {
      depth += 1;
    } else if (ch === "}") {
      depth -= 1;
      if (depth === 0) return i;
    }
  }
  return -1;
}

function pageSizeSchemaSlices(text) {
  const slices = [];
  let cursor = 0;
  for (;;) {
    const paramIndex = text.indexOf(PAGE_SIZE_PARAM, cursor);
    if (paramIndex === -1) break;
    cursor = paramIndex + PAGE_SIZE_PARAM.length;
    const schemaKey = text.indexOf('"schema":', cursor);
    if (schemaKey === -1) continue;
    const open = text.indexOf("{", schemaKey);
    if (open === -1) continue;
    const close = findObjectEnd(text, open);
    if (close === -1 || close < paramIndex) continue;
    slices.push({ start: open, end: close, body: text.slice(open, close + 1) });
  }
  return slices;
}

const failures = [];
const updatedFiles = [];

for (const relative of authorities) {
  const filePath = path.join(repoRoot, relative);
  const original = fs.readFileSync(filePath, "utf8");
  const slices = pageSizeSchemaSlices(original);

  if (slices.length === 0) {
    failures.push(`${relative}: no page_size query parameter found`);
    continue;
  }

  for (const slice of slices) {
    const maximum = /"maximum"\s*:\s*(\d+)/.exec(slice.body);
    if (maximum === null || Number(maximum[1]) !== MAX_PAGE_SIZE) {
      failures.push(`${relative}: page_size schema must declare maximum ${MAX_PAGE_SIZE}`);
    }
  }

  const missing = slices.filter((slice) => !/"default"\s*:/.test(slice.body));
  if (missing.length === 0) continue;

  if (checkOnly) {
    failures.push(
      `${relative}: ${missing.length} page_size query parameter(s) must declare default ${DEFAULT_PAGE_SIZE} (PAGINATION_SPEC 3.1)`,
    );
    continue;
  }

  let updated = original;
  for (const slice of [...missing].reverse()) {
    const segment = updated.slice(slice.start, slice.end + 1);
    const closeIndex = segment.lastIndexOf("}");
    const insertAt = segment.lastIndexOf("\n", closeIndex);
    if (insertAt === -1) {
      failures.push(`${relative}: page_size schema is single-line; format it before aligning`);
      continue;
    }
    const indentMatch = /\n(\s+)\S/.exec(segment);
    const indent = indentMatch ? indentMatch[1] : "  ";
    const patched = `${segment.slice(0, insertAt)},\n${indent}"default": ${DEFAULT_PAGE_SIZE}${segment.slice(insertAt)}`;
    updated = `${updated.slice(0, slice.start)}${patched}${updated.slice(slice.end + 1)}`;
  }

  if (updated === original) {
    failures.push(`${relative}: alignment produced no change despite ${missing.length} missing default(s)`);
    continue;
  }
  fs.writeFileSync(filePath, updated, "utf8");
  updatedFiles.push(`${relative} (${missing.length})`);
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`[page-size-default] ${failure}`);
  console.error(`[page-size-default] failed (${failures.length} problem(s))`);
  process.exit(1);
}

if (checkOnly) {
  console.log(
    `[page-size-default] ok: all page_size parameters declare default ${DEFAULT_PAGE_SIZE} and maximum ${MAX_PAGE_SIZE}`,
  );
} else {
  console.log(
    updatedFiles.length === 0
      ? "[page-size-default] unchanged: all page_size parameters already aligned"
      : `[page-size-default] updated: ${updatedFiles.join(", ")}`,
  );
}
