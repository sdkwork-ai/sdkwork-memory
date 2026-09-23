#!/usr/bin/env node
/**
 * Manages the engine-specific DDL baseline for this database root.
 *
 * DATABASE_FRAMEWORK_SPEC.md section 984 defines the canonical DDL source per
 * `baselineStrategy`:
 *
 *   - `migrations-only`      -> the ordered `migrations/{engine}/*.up.sql` set,
 *                               with the baseline file derived by folding them.
 *   - `baseline-plus-migrations`
 *   - `baseline-only-dev`    -> the committed baseline file itself. Section 584
 *                               requires exactly one primary baseline at
 *                               `ddl/baseline/{engine}/0001_<moduleId>_baseline.sql`,
 *                               and migrations hold only post-baseline deltas.
 *
 * The direction matters. Folding migrations over a committed baseline in a
 * `baseline-plus-migrations` root replaces the authoritative schema DDL with the
 * handful of post-baseline deltas, so the primary baseline for Memory collapsed
 * from the full schema to a single ALTER-only file and a fresh install would
 * create no tables at all. This script therefore never writes in that mode; it
 * asserts the committed baseline exists and is non-empty.
 *
 * Migration reversibility follows DATABASE_SPEC.md section 586 and
 * DATABASE_FRAMEWORK_SPEC.md section 7.1: a `.down.sql` is optional and is
 * permitted only for a bounded, data-preserving reversal. A migration declaring
 * `reversible: false` MUST NOT ship one, so the check is bidirectional rather
 * than an unconditional presence requirement.
 */

import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { join, relative, resolve, sep } from "node:path";

const root = resolve(import.meta.dirname, "..");
const checkOnly = process.argv.includes("--check");

const BASELINE_STRATEGIES = new Set(["migrations-only", "baseline-plus-migrations", "baseline-only-dev"]);
const REVERSIBLE = "true";
const IRREVERSIBLE = "false";
const UP_MIGRATION = /^\d+_[a-z0-9_]+\.up\.sql$/u;
const HEADER_FIELD = /^-- ([a-z_]+): (.*)$/u;

/** Reads the leading `-- key: value` metadata block, ignoring `--` continuation prose. */
function migrationHeader(text) {
  const header = {};
  for (const line of text.split("\n")) {
    const match = HEADER_FIELD.exec(line);
    if (match) {
      if (!(match[1] in header)) header[match[1]] = match[2].trim();
      continue;
    }
    if (line.startsWith("--")) continue;
    break;
  }
  return header;
}

function assertDownMigrationContract({ engine, upFile, downFile, reversible, hasDown }) {
  if (reversible === REVERSIBLE && !hasDown) {
    throw new Error(
      `canonical migration ${engine}/${upFile} declares "-- reversible: true" but is missing ${downFile}; ` +
        "a declared reversible change requires a tested, data-preserving paired down migration",
    );
  }
  if (reversible === IRREVERSIBLE && hasDown) {
    throw new Error(
      `canonical migration ${engine}/${upFile} declares "-- reversible: false" but ships ${downFile}; ` +
        "a lossy change MUST use a forward-fix or restore-cutover strategy and MUST NOT ship a misleading down migration",
    );
  }
}

function listMigrations(engine, migrationRoot) {
  if (!existsSync(migrationRoot)) {
    return [];
  }
  const upFiles = readdirSync(migrationRoot)
    .filter((name) => UP_MIGRATION.test(name))
    .sort();
  for (const upFile of upFiles) {
    const downFile = upFile.replace(/\.up\.sql$/u, ".down.sql");
    const upText = readFileSync(join(migrationRoot, upFile), "utf8").replace(/\r\n/gu, "\n");
    assertDownMigrationContract({
      engine,
      upFile,
      downFile,
      reversible: migrationHeader(upText).reversible,
      hasDown: existsSync(join(migrationRoot, downFile)),
    });
  }
  return upFiles;
}

const manifestPath = join(root, "database", "database.manifest.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8").replace(/\r\n/gu, "\n"));
const baselineStrategy = manifest.baselineStrategy;
if (!BASELINE_STRATEGIES.has(baselineStrategy)) {
  throw new Error(
    `database/database.manifest.json baselineStrategy must be one of ${[...BASELINE_STRATEGIES].join(", ")} (found ${baselineStrategy ?? "missing"})`,
  );
}
const moduleId = manifest.moduleId;
if (typeof moduleId !== "string" || moduleId.length === 0) {
  throw new Error("database/database.manifest.json must declare a non-empty moduleId");
}

const engines =
  Array.isArray(manifest.engines) && manifest.engines.length > 0
    ? manifest.engines
    : [manifest.defaultEngine].filter(Boolean);
if (engines.length === 0) {
  throw new Error("database/database.manifest.json declares neither engines nor defaultEngine");
}

const migrated = [];

for (const engine of engines) {
  const migrationRoot = join(root, "database", "migrations", engine);
  const migrationFiles = listMigrations(engine, migrationRoot);

  if (baselineStrategy === "migrations-only") {
    if (migrationFiles.length === 0) {
      throw new Error(`no canonical ${engine} migrations found; baselineStrategy "migrations-only" derives the baseline from them`);
    }
    const generated = [
      "-- Generated from canonical application-root migrations.",
      "-- Do not edit this folded baseline directly; run `pnpm db:materialize:baseline`.",
      "",
      ...migrationFiles.flatMap((name) => {
        const path = join(migrationRoot, name);
        return [
          `-- source: ${portable(relative(root, path))}`,
          readFileSync(path, "utf8").replace(/\r\n/gu, "\n").trimEnd(),
          "",
        ];
      }),
    ].join("\n");
    const outputPath = join(root, "database", "ddl", "baseline", engine, `0001_${moduleId}_baseline.sql`);
    if (checkOnly) {
      const current = readFileSync(outputPath, "utf8").replace(/\r\n/gu, "\n");
      if (current !== generated) {
        throw new Error(`${portable(relative(root, outputPath))} is stale; run pnpm db:materialize:baseline`);
      }
    } else {
      writeFileSync(outputPath, generated, "utf8");
      migrated.push(portable(relative(root, outputPath)));
    }
    continue;
  }

  // `baseline-plus-migrations` and `baseline-only-dev`: the committed baseline is
  // the authoritative DDL source (DATABASE_FRAMEWORK_SPEC section 584) and must
  // never be regenerated from the post-baseline deltas.
  const baselinePath = join(root, "database", "ddl", "baseline", engine, `0001_${moduleId}_baseline.sql`);
  if (!existsSync(baselinePath)) {
    throw new Error(
      `baselineStrategy "${baselineStrategy}" requires a committed baseline at ${portable(relative(root, baselinePath))}`,
    );
  }
  if (statSync(baselinePath).size === 0) {
    throw new Error(`${portable(relative(root, baselinePath))} is empty; baselineStrategy "${baselineStrategy}" requires the committed schema DDL`);
  }
}

if (baselineStrategy === "migrations-only") {
  console.log(`Memory database baselines ${checkOnly ? "are current" : `materialized (${migrated.join(", ")})`}`);
} else {
  console.log(
    `Memory database baselines are committed and present (baselineStrategy: ${baselineStrategy}, ${engines.length} engine(s), ${checkOnly ? "checked" : "validated"})`,
  );
}

function portable(value) {
  return value.split(sep).join("/");
}
