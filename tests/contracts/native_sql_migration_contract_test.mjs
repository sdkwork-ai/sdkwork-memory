import assert from "node:assert/strict";
import fs from "node:fs";

// Initialization state: `baselineStrategy` is `baseline-plus-migrations`, so each declared
// engine owns exactly one consolidated baseline and migrations/ holds only post-baseline
// deltas. The per-migration SQL files referenced here previously were folded into these
// baselines by reset-database-initialization-state.mjs.
const baselinePaths = [
  "database/ddl/baseline/postgres/0001_memory_baseline.sql",
  "database/ddl/baseline/sqlite/0001_memory_baseline.sql",
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
