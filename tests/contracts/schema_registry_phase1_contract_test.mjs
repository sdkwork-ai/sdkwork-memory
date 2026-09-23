import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";

const schemaRegistryDir = "docs/schema-registry/tables";
const phase1Tables = new Set([
  "ai_space",
  "ai_event",
  "ai_record",
  "ai_record_source",
  "ai_candidate",
  "ai_habit",
  "ai_retrieval_trace",
  "ai_retrieval_hit",
  "ai_context_pack",
  "ai_index",
  "ai_retrieval_profile",
  "ai_implementation_profile",
  "ai_provider_binding",
  "ai_eval_run",
  "ai_audit_log",
  "ai_outbox_event",
]);

// Initialization state: each declared engine owns exactly one consolidated baseline, so the
// schema-registry indexes and tables must be materialized by that baseline rather than by the
// per-version SQL files that reset-database-initialization-state.mjs folded into it.
const migrationPaths = [
  "database/ddl/baseline/postgres/0001_memory_baseline.sql",
  "database/ddl/baseline/sqlite/0001_memory_baseline.sql",
];

function loadSchemaRegistryIndexes() {
  const indexes = [];
  for (const file of fs.readdirSync(schemaRegistryDir)) {
    if (!file.endsWith(".yaml")) {
      continue;
    }
    const text = fs.readFileSync(path.join(schemaRegistryDir, file), "utf8");
    let currentTable = null;
    let inIndexesSection = false;
    for (const line of text.split("\n")) {
      const tableMatch = line.match(/^\s+- table:\s+(\w+)/);
      if (tableMatch) {
        currentTable = tableMatch[1];
        inIndexesSection = false;
        continue;
      }
      if (/^\s+indexes:/.test(line)) {
        inIndexesSection = true;
        continue;
      }
      if (/^\s+columns:/.test(line)) {
        inIndexesSection = false;
        continue;
      }
      if (/^\s+- table:/.test(line) || /^\s+serialization:/.test(line)) {
        inIndexesSection = false;
      }
      const indexMatch = line.match(/^\s+- \{ name: (\w+),/);
      if (
        indexMatch
        && currentTable
        && phase1Tables.has(currentTable)
        && inIndexesSection
      ) {
        indexes.push(indexMatch[1]);
      }
    }
  }
  return [...new Set(indexes)].sort();
}

const requiredIndexes = loadSchemaRegistryIndexes();
assert.ok(requiredIndexes.length > 0, "schema registry must declare phase1 indexes");

const migrationGroups = migrationPaths.map((migrationPath) => [migrationPath]);

for (const group of migrationGroups) {
  const combinedSql = group
    .map((migrationPath) => {
      assert.ok(fs.existsSync(migrationPath), `${migrationPath} must exist`);
      return fs.readFileSync(migrationPath, "utf8");
    })
    .join("\n")
    .toLowerCase();

  for (const indexName of requiredIndexes) {
    assert.match(
      combinedSql,
      new RegExp(`\\b${indexName.toLowerCase()}\\b`),
      `${group.join(" + ")} must materialize schema-registry index ${indexName}`,
    );
  }
}

const requiredTables = [
  "ai_space",
  "ai_event",
  "ai_record",
  "ai_record_source",
  "ai_candidate",
  "ai_habit",
  "ai_retrieval_trace",
  "ai_retrieval_hit",
  "ai_context_pack",
];

for (const migrationPath of migrationPaths) {
  const sql = fs.readFileSync(migrationPath, "utf8").toLowerCase();
  for (const table of requiredTables) {
    assert.match(
      sql,
      new RegExp(`create\\s+table\\s+(if\\s+not\\s+exists\\s+)?${table}\\b`),
      `${migrationPath} missing ${table}`,
    );
  }
  assert.doesNotMatch(
    sql,
    // Word-anchored: the native-sql profile legitimately uses PostgreSQL full-text search
    // (`TSVECTOR`, `to_tsvector`), which an unanchored /vector/ substring match misreads as
    // vector storage.
    /\b(vector|embedding|embeddings|pgvector)\b/,
    `${migrationPath} must not require vector or embedding storage in Phase 1`,
  );
}

const storeSource = fs.readFileSync(
  "plugins/sdkwork-memory-plugin-native-sql/src/store.rs",
  "utf8",
);
assert.ok(
  storeSource.includes("database/ddl/baseline/postgres/0001_memory_baseline.sql") &&
    storeSource.includes("database/ddl/baseline/sqlite/0001_memory_baseline.sql"),
  "native-sql store must consume the application-root consolidated baseline for both engines",
);

console.log(
  `Schema registry phase1 contract test passed (${requiredIndexes.length} indexes)`,
);
