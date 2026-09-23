#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const requiredTables = ["ai_space", "ai_event", "ai_record", "ai_tenant_preference", "ai_subject"];

/** Reads the leading `-- key: value` metadata block, ignoring `--` continuation prose. */
function migrationHeader(text) {
  const header = {};
  for (const line of text.split("\n")) {
    const match = /^-- ([a-z_]+): (.*)$/u.exec(line);
    if (match) {
      if (!(match[1] in header)) header[match[1]] = match[2].trim();
      continue;
    }
    if (line.startsWith("--")) continue;
    break;
  }
  return header;
}

for (const engine of ["postgres", "sqlite"]) {
  // Initialization state: `baselineStrategy` is `baseline-plus-migrations`, so each engine's
  // committed consolidated baseline is the authoritative DDL snapshot
  // (DATABASE_FRAMEWORK_SPEC section 584) and is what parity is asserted against.
  const baselinePath = path.join(
    root,
    "database",
    "ddl",
    "baseline",
    engine,
    "0001_memory_baseline.sql",
  );
  assert.ok(
    fs.existsSync(baselinePath),
    `${engine} must commit its consolidated baseline at ${path.relative(root, baselinePath)}`,
  );
  const baseline = fs.readFileSync(baselinePath, "utf8").toLowerCase();

  for (const table of requiredTables) {
    assert.match(
      baseline,
      new RegExp(`create\\s+table\\s+(if\\s+not\\s+exists\\s+)?${table}\\b`),
      `${engine} baseline must create ${table}`,
    );
  }

  if (engine === "postgres") {
    assert.doesNotMatch(baseline, /\bbigserial\b|\bserial\b/u, "PostgreSQL IDs must be application generated");
    assert.doesNotMatch(
      baseline,
      /\b(?:numeric|decimal)\s*\(/u,
      "native SQL Any profile floating-point scores must use DOUBLE PRECISION",
    );
    assert.doesNotMatch(
      baseline,
      /\bjsonb\b|\btimestamptz\b/u,
      "PostgreSQL native-sql storage must retain the cross-engine TEXT physical profile used by SQLx Any",
    );
  } else {
    assert.doesNotMatch(baseline, /\bid\s+integer\s+primary\s+key\b/u, "SQLite business IDs must not use rowid allocation");
  }

  // Post-baseline migrations may legitimately be empty until the contract evolves
  // (DATABASE_FRAMEWORK_SPEC section 353). When a migration is present, its rollback strategy
  // must stay consistent with whether it ships a down file (DATABASE_SPEC section 586).
  const migrationRoot = path.join(root, "database", "migrations", engine);
  assert.ok(
    fs.existsSync(migrationRoot),
    `${engine} must own database/migrations/${engine}/ as the post-baseline change venue`,
  );
  const upFiles = fs
    .readdirSync(migrationRoot)
    .filter((name) => name.endsWith(".up.sql"))
    .sort();
  for (const upFile of upFiles) {
    const upText = fs.readFileSync(path.join(migrationRoot, upFile), "utf8").replace(/\r\n/gu, "\n");
    const reversible = migrationHeader(upText).reversible;
    assert.ok(
      reversible === "true" || reversible === "false",
      `${engine}/${upFile} must declare "-- reversible: true|false"`,
    );
    const hasDown = fs.existsSync(
      path.join(migrationRoot, upFile.replace(/\.up\.sql$/u, ".down.sql")),
    );
    if (reversible === "true") {
      assert.ok(
        hasDown,
        `${engine}/${upFile} declares reversible: true and must ship its paired .down.sql`,
      );
    } else {
      assert.ok(
        !hasDown,
        `${engine}/${upFile} declares reversible: false and must NOT ship a misleading .down.sql`,
      );
    }
  }
}

// The plugin's legacy migration tree is retired: the application root owns migrations now. The
// directory is retained as a marked shell so SQL is not re-added here by accident, and only the
// engine directories that actually existed are asserted — a never-created engine directory is
// not a gap, whereas a live `.sql` file here would be a second migration authority.
const pluginMigrationsRoot = path.join(
  root,
  "plugins",
  "sdkwork-memory-plugin-native-sql",
  "migrations",
);
assert.ok(
  fs.existsSync(pluginMigrationsRoot),
  "the plugin must keep a deprecated migrations shell rather than a live migration tree",
);
const legacyEngineDirs = fs
  .readdirSync(pluginMigrationsRoot)
  .filter((name) => fs.statSync(path.join(pluginMigrationsRoot, name)).isDirectory())
  .sort();
assert.deepEqual(
  legacyEngineDirs,
  ["postgres"],
  "the plugin must not grow a second migration authority for another engine",
);
for (const engineDir of legacyEngineDirs) {
  const enginePath = path.join(pluginMigrationsRoot, engineDir);
  const legacySql = fs.readdirSync(enginePath).filter((name) => name.endsWith(".sql"));
  assert.deepEqual(legacySql, [], `${enginePath} must not remain a second migration authority`);
  assert.ok(
    fs.existsSync(path.join(enginePath, "DEPRECATED.md")),
    `${enginePath} must record its retirement with DEPRECATED.md`,
  );
}

const nativeSqlRoot = path.join(root, "plugins", "sdkwork-memory-plugin-native-sql");
for (const sourceDirectory of ["src", "tests"]) {
  const sourceRoot = path.join(nativeSqlRoot, sourceDirectory);
  for (const sourceName of fs.readdirSync(sourceRoot).filter((name) => name.endsWith(".rs"))) {
    const source = fs.readFileSync(path.join(sourceRoot, sourceName), "utf8");
    for (const match of source.matchAll(/insert\s+into\s+(ai_[a-z_]+)\s*\(([^)]+)\)/giu)) {
      const table = match[1].toLowerCase();
      if (table.endsWith("_fts") || table === "ai_schema_migration") continue;
      const columns = match[2].split(",").map((column) => column.trim().toLowerCase());
      assert.ok(
        columns.includes("id"),
        `${sourceDirectory}/${sourceName} insert into ${table} must bind an approved application-generated id`,
      );
    }
  }
}
