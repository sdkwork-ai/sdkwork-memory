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

/**
 * Asserts the shared cross-engine DDL invariants that every Memory dialect must satisfy: the
 * tables exist, ids are application generated rather than database allocated, and the physical
 * column profile stays portable.
 *
 * @param {string} engine engine label used in assertion messages
 * @param {string} baselinePath absolute path to the consolidated baseline SQL
 * @param {{ allowTextProfileOnly: boolean }} options dialect specific expectations
 */
function assertBaselineInvariants(engine, baselinePath, options) {
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

  assert.doesNotMatch(
    baseline,
    /\bbigserial\b|\bserial\b/u,
    `${engine} business IDs must be application generated, not database allocated`,
  );

  if (options.allowTextProfileOnly) {
    assert.doesNotMatch(
      baseline,
      /\b(?:numeric|decimal)\s*\(/u,
      "native SQL stored floating-point scores must use DOUBLE PRECISION",
    );
    assert.doesNotMatch(
      baseline,
      /\bjsonb\b|\btimestamptz\b/u,
      "PostgreSQL native-sql storage must retain the cross-engine TEXT physical profile used by SQLx Any",
    );
  } else {
    assert.doesNotMatch(
      baseline,
      /\bid\s+integer\s+primary\s+key\b/u,
      "SQLite business IDs must not use rowid allocation",
    );
  }
}

// Initialization state: `baselineStrategy` is `baseline-plus-migrations`, so the engine's
// committed consolidated baseline is the authoritative DDL snapshot
// (DATABASE_FRAMEWORK_SPEC section 584) and is what parity is asserted against. Only `postgres`
// is a module root here: this repository is an `authoritative-server` root and must claim exactly
// one engine (DATABASE_FRAMEWORK_SPEC sections 40, 344, 421).
const authoritativeBaseline = path.join(
  root,
  "database",
  "ddl",
  "baseline",
  "postgres",
  "0001_memory_baseline.sql",
);
assertBaselineInvariants("postgres", authoritativeBaseline, { allowTextProfileOnly: true });

// Post-baseline migrations may legitimately be empty until the contract evolves
// (DATABASE_FRAMEWORK_SPEC section 353). When a migration is present, its rollback strategy must
// stay consistent with whether it ships a down file (DATABASE_SPEC section 586).
const migrationRoot = path.join(root, "database", "migrations", "postgres");
assert.ok(
  fs.existsSync(migrationRoot),
  "postgres must own database/migrations/postgres/ as the post-baseline change venue",
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
    `postgres/${upFile} must declare "-- reversible: true|false"`,
  );
  const hasDown = fs.existsSync(path.join(migrationRoot, upFile.replace(/\.up\.sql$/u, ".down.sql")));
  if (reversible === "true") {
    assert.ok(
      hasDown,
      `postgres/${upFile} declares reversible: true and must ship its paired .down.sql`,
    );
  } else {
    assert.ok(
      !hasDown,
      `postgres/${upFile} declares reversible: false and must NOT ship a misleading .down.sql`,
    );
  }
}

// The SQLite dialect is owned by the composed `client-local` plugin and is exercised from the
// test fixture tree, never from this application root's `database/` directory. Parity means the
// fixture still describes the same logical tables under the shared invariants, and that no
// second DDL authority has crept back into `database/`.
const sqliteFixtureBaseline = path.join(
  root,
  "tests",
  "fixtures",
  "database",
  "sqlite",
  "ddl",
  "baseline",
  "0001_memory_baseline.sql",
);
assertBaselineInvariants("sqlite", sqliteFixtureBaseline, { allowTextProfileOnly: false });
assert.ok(
  !fs.existsSync(path.join(root, "database", "ddl", "baseline", "sqlite")),
  "an authoritative-server root must not claim a second engine baseline directory",
);
assert.ok(
  !fs.existsSync(path.join(root, "database", "migrations", "sqlite")),
  "an authoritative-server root must not claim a second engine migration directory",
);

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
