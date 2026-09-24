#!/usr/bin/env node
// Release immutability gate for sibling-repository dependency pins.
//
// `etc/dependency-refs.json` is the committed record of the exact commit SHA
// every release commit is built against. The GitHub packaging workflow stays
// thin per GITHUB_WORKFLOW_SPEC and resolves `SDKWORK_*_REF` from workflow
// inputs or organization variables; this gate guarantees those variables have
// a committed source of truth and that every pin is a full 40-hex commit SHA,
// so a moving ref like `main` can never be the release basis.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const pinsPath = resolve(root, "etc/dependency-refs.json");
const pins = JSON.parse(readFileSync(pinsPath, "utf8"));
const failures = [];
const SHA_PATTERN = /^[0-9a-f]{40}$/;

for (const [key, value] of Object.entries(pins)) {
  if (key.startsWith("$")) continue;
  if (typeof value !== "string" || !SHA_PATTERN.test(value)) {
    failures.push(`${key} must be a full 40-hex commit SHA (got: ${JSON.stringify(value)})`);
  }
}

if (Object.keys(pins).filter((key) => !key.startsWith("$")).length < 12) {
  failures.push("dependency pin manifest must declare every SDKWORK_*_REF sibling dependency");
}

if (failures.length > 0) {
  console.error("dependency pin check failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}
console.log("dependency pin check passed:", pinsPath);
