import assert from "node:assert/strict";
import fs from "node:fs";

// `database/` is an `authoritative-server` root, so it declares exactly `engines: ["postgres"]`
// and commits one consolidated baseline (DATABASE_FRAMEWORK_SPEC sections 344 and 584). The
// SQLite dialect schema the native-sql plugin bootstraps is deliberately NOT a second module
// root under database/, because that root must not claim both roles (section 40).
const baselinePaths = [
  "database/ddl/baseline/postgres/0001_memory_baseline.sql",
  "tests/fixtures/database/sqlite/ddl/baseline/0001_memory_baseline.sql",
];

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
  "ai_index",
  "ai_retrieval_profile",
  "ai_implementation_profile",
  "ai_provider_binding",
  "ai_eval_run",
  "ai_audit_log",
  "ai_outbox_event",
];

for (const baselinePath of baselinePaths) {
  assert.ok(fs.existsSync(baselinePath), `${baselinePath} must exist`);
  const sql = fs.readFileSync(baselinePath, "utf8").toLowerCase();

  for (const table of requiredTables) {
    assert.match(
      sql,
      new RegExp(`create\\s+table\\s+(if\\s+not\\s+exists\\s+)?${table}\\b`),
      `${baselinePath} must create ${table}`,
    );
  }

  assert.doesNotMatch(
    sql,
    /\b(vector|embedding|embeddings|pgvector)\b/,
    `${baselinePath} must not require vector or embedding storage in native_sql phase1`,
  );
}
