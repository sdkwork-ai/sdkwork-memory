#!/usr/bin/env node
/**
 * Cross-dialect DDL parity gate.
 *
 * PostgreSQL (authoritative baseline + deltas) and SQLite (fixture migrations,
 * which the runtime embeds verbatim) must describe the same logical schema:
 * same tables, same columns, same CHECK bounds, same index coverage. The two
 * engines previously drifted silently — the SQLite projection lost three
 * migrations and CHECK constraints, and `ai_learning_job.state` carried a
 * divergent value set — because nothing compared them. This gate closes that
 * hole with an explicit divergence allowlist: undeclared drift fails.
 *
 * The comparison is normalized, not textual: VARCHAR(n)/TEXT, DOUBLE
 * PRECISION/REAL and expression whitespace/case differences are structural
 * dialect translation, not drift. Declared divergences live in ALLOWED.
 */

import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");

// Divergences that are deliberate dialect translation, keyed as
// `"kind|table|name"`. Anything not listed here must match across engines.
const ALLOWED = new Set([
  // Full-text search: PostgreSQL keeps the document on the row and mirrors it
  // through tsvector expressions/triggers; SQLite uses a virtual FTS5 table.
  "column|ai_record|search_document",
  "table|ai_record_fts|sqlite-only",
  "index|ai_record_fts|sqlite-only",
  // Tenant-level preference uniqueness: PostgreSQL 15+ uses NULLS NOT
  // DISTINCT over the nullable user_id; SQLite cannot express it and instead
  // normalizes "no user" to a -1 sentinel at the store layer.
  "index|ai_tenant_preference|uk_ai_tenant_preference_scope",
]);

function sqlFiles(dir, suffix) {
  return readdirSync(dir)
    .filter((name) => name.endsWith(suffix))
    .sort()
    .map((name) => readFileSync(join(dir, name), "utf8").replace(/\r\n/gu, "\n"));
}

function effectiveSchema(files) {
  return files.join("\n;\n");
}

function stripComments(sql) {
  return sql
    .split("\n")
    .filter((line) => !line.trimStart().startsWith("--"))
    .join("\n");
}

function blockAfter(sql, keyword, startIndex) {
  const open = sql.indexOf("(", startIndex);
  if (open === -1) return null;
  let depth = 0;
  for (let i = open; i < sql.length; i += 1) {
    if (sql[i] === "(") depth += 1;
    else if (sql[i] === ")") {
      depth -= 1;
      if (depth === 0) return { open, close: i, end: i + 1 };
    }
  }
  return null;
}

function normalizeType(type) {
  const flattened = type.replaceAll(/\s+/gu, " ").trim().toUpperCase();
  if (/^VARCHAR\s*\(\s*\d+\s*\)$/.test(flattened)) return "TEXT";
  if (flattened === "DOUBLE PRECISION") return "REAL";
  if (/^TIMESTAMP(\s*\(\d+\))?$/.test(flattened)) return "TEXT";
  if (flattened === "INTEGER" || flattened === "INT" || flattened === "BOOLEAN") return "BIGINT";
  return flattened;
}

/** Splits `text` on `separator` at parenthesis depth zero. */
function splitTopLevel(text, separator) {
  const parts = [];
  let depth = 0;
  let current = "";
  for (const char of text) {
    if (char === "(") depth += 1;
    if (char === ")") depth -= 1;
    if (char === separator && depth === 0) {
      parts.push(current);
      current = "";
    } else {
      current += char;
    }
  }
  if (current.trim()) parts.push(current);
  return parts;
}

function normalizeExpression(expression) {
  return expression
    .replaceAll(/\s+/gu, " ")
    .replaceAll(/,\s*/gu, ",")
    .trim()
    .toUpperCase();
}

// Words that end the type and begin the column-constraint clause.
const CONSTRAINT_KEYWORDS = new Set([
  "PRIMARY", "NOT", "NULL", "UNIQUE", "CHECK", "DEFAULT", "REFERENCES",
  "GENERATED", "COLLATE", "CONSTRAINT",
]);

/** Splits `VARCHAR(64) NOT NULL DEFAULT 0` into { type, constraints }. */
function splitColumnDefinition(rest) {
  const tokens = rest.replaceAll(/\s+/gu, " ").trim().split(" ");
  let index = 0;
  while (index < tokens.length) {
    const token = tokens[index].toUpperCase();
    // Type words: bare words, bracketed lengths, or words carrying a length
    // (`VARCHAR(64)`); the only multi-word type in this schema is
    // DOUBLE PRECISION.
    if (/^[A-Z]+\(\d+\)$/.test(token) || /^\(\d+\)$/.test(token)) {
      index += 1;
      continue;
    }
    if (!/^[A-Z]+$/.test(token) || CONSTRAINT_KEYWORDS.has(token)) break;
    if (token === "DOUBLE" && tokens[index + 1]?.toUpperCase() === "PRECISION") {
      index += 2;
      continue;
    }
    index += 1;
  }
  return {
    type: tokens.slice(0, index).join(" "),
    constraints: tokens.slice(index).join(" "),
  };
}

function normalizeConstraints(constraints) {
  let normalized = normalizeExpression(constraints);
  // SQLite's INTEGER PRIMARY KEY implies NOT NULL; PostgreSQL spells it out
  // on the PK column. Both describe the same single-column key.
  normalized = normalized.replaceAll("NOT NULL PRIMARY KEY", "PRIMARY KEY");
  // Boolean literals: SQLite fixtures spell defaults numerically, PostgreSQL
  // uses TRUE/FALSE; the stored values are identical.
  normalized = normalized.replaceAll("DEFAULT FALSE", "DEFAULT 0");
  normalized = normalized.replaceAll("DEFAULT TRUE", "DEFAULT 1");
  return normalized;
}

/** Parses CREATE TABLE / CREATE INDEX / ALTER TABLE ADD COLUMN statements. */
function parseSchema(sql) {
  const tables = new Map();
  const indexes = new Map();

  const createTable = /CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(/giu;
  for (const match of sql.matchAll(createTable)) {
    const block = blockAfter(sql, "(", match.index);
    if (!block) continue;
    const body = sql.slice(block.open + 1, block.close);
    const tableName = match[1].toLowerCase();
    const table = { columns: new Map(), checks: new Set(), primaryKey: null };

    // Split top-level commas only: column definitions contain parens (CHECK
    // value lists, REFERENCES target columns).
    const definitions = splitTopLevel(body, ",");

    for (const definition of definitions) {
      const text = definition.trim().replaceAll(/\s+/gu, " ");
      if (/^(CONSTRAINT|CHECK)\b/iu.test(text)) {
        table.checks.add(normalizeExpression(text));
        continue;
      }
      if (/^PRIMARY\s+KEY\b/iu.test(text) || /^UNIQUE\b/iu.test(text)) {
        table.checks.add(normalizeExpression(text));
        continue;
      }
      // SQLite declares foreign keys as table-level clauses while PostgreSQL
      // puts REFERENCES inline on the column — both enforce identically, so
      // merge the table-level form into the column's constraint record.
      const tableLevelFk = text.match(/^FOREIGN\s+KEY\s*\((\w+)\)\s*REFERENCES\s+([\s\S]+)$/iu);
      if (tableLevelFk) {
        const referencedColumn = table.columns.get(tableLevelFk[1].toLowerCase());
        if (referencedColumn) {
          referencedColumn.normalized = normalizeConstraints(
            `${referencedColumn.normalized} REFERENCES ${tableLevelFk[2]}`,
          );
        }
        continue;
      }
      const column = text.match(/^"(\w+)"\s+(.+)$/u) ?? text.match(/^(\w+)\s+(.+)$/u);
      if (!column) continue;
      const columnName = column[1].toLowerCase();
      const { type, constraints } = splitColumnDefinition(column[2]);
      table.columns.set(columnName, {
        type: normalizeType(type),
        normalized: normalizeConstraints(constraints),
      });
    }
    tables.set(tableName, table);
  }

  const createIndex =
    /CREATE\s+(UNIQUE\s+)?INDEX\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s+ON\s+(\w+)\s*\(/giu;
  for (const match of sql.matchAll(createIndex)) {
    const block = blockAfter(sql, "(", match.index);
    if (!block) continue;
    const columns = normalizeExpression(sql.slice(block.open + 1, block.close));
    indexes.set(match[2].toLowerCase(), {
      table: match[3].toLowerCase(),
      unique: Boolean(match[1]),
      columns,
    });
  }

  const addColumn = /ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(\w+)\s+ADD\s+COLUMN\s+([^;]+);/giu;
  for (const match of sql.matchAll(addColumn)) {
    const table = tables.get(match[1].toLowerCase());
    if (!table) continue;
    // PostgreSQL folds several columns into one ALTER statement separated by
    // commas, each segment prefixed `ADD COLUMN [IF NOT EXISTS]`.
    const segments = splitTopLevel(match[2], ",");
    for (const segment of segments) {
      const stripped = segment
        .trim()
        .replace(/^ADD\s+COLUMN\s+/iu, "")
        .replace(/^IF\s+NOT\s+EXISTS\s+/iu, "")
        .trim();
      const column = stripped.match(/^(\w+)\s+([\s\S]+)$/u);
      if (!column) continue;
      const { type, constraints } = splitColumnDefinition(column[2]);
      table.columns.set(column[1].toLowerCase(), {
        type: normalizeType(type),
        normalized: normalizeConstraints(constraints),
      });
    }
  }

  // The learning-job state CHECK was narrowed by migration 0002
  // (DROP CONSTRAINT + ADD CONSTRAINT). Model the replacement: an ADD
  // CONSTRAINT whose CHECK expression mentions a parsed column replaces that
  // column's recorded check.
  const addConstraint =
    /ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(\w+)\s+ADD\s+CONSTRAINT\s+(\w+)\s+CHECK\s*\(/giu;
  for (const match of sql.matchAll(addConstraint)) {
    const table = tables.get(match[1].toLowerCase());
    if (!table) continue;
    const block = blockAfter(sql, "(", match.index);
    if (!block) continue;
    const expression = normalizeExpression(sql.slice(block.open + 1, block.close));
    for (const [columnName, column] of table.columns) {
      if (new RegExp(`\\b${columnName}\\b`, "iu").test(expression)) {
        // Swap the CHECK clause in place, preserving any other column
        // constraints (NOT NULL, DEFAULT) recorded before it.
        const original = column.normalized;
        column.normalized = original.includes("CHECK (")
          ? original.replace(/CHECK \(.*\)$/u, `CHECK (${expression})`)
          : `${original} CHECK (${expression})`.trim();
        break;
      }
    }
  }

  return { tables, indexes };
}

const postgres = parseSchema(
  effectiveSchema([
    ...sqlFiles(join(root, "database", "ddl", "baseline", "postgres"), ".sql"),
    ...sqlFiles(join(root, "database", "migrations", "postgres"), ".up.sql"),
  ]).split(";").map(stripComments).join(";"),
);
const sqlite = parseSchema(
  effectiveSchema(sqlFiles(join(root, "tests", "fixtures", "database", "sqlite", "migrations"), ".up.sql"))
    .split(";")
    .map(stripComments)
    .join(";"),
);

const problems = [];

for (const [table, pgTable] of postgres.tables) {
  const sqliteTable = sqlite.tables.get(table);
  if (!sqliteTable) {
    if (!ALLOWED.has(`table|${table}|sqlite-only`)) {
      problems.push(`table ${table} exists in PostgreSQL but not in SQLite`);
    }
    continue;
  }
  for (const [column, pgColumn] of pgTable.columns) {
    const sqliteColumn = sqliteTable.columns.get(column);
    if (!sqliteColumn) {
      if (!ALLOWED.has(`column|${table}|${column}`)) {
        problems.push(`column ${table}.${column} exists in PostgreSQL but not in SQLite`);
      }
      continue;
    }
    if (pgColumn.type !== sqliteColumn.type) {
      problems.push(
        `column ${table}.${column} type diverges: postgres ${pgColumn.type} vs sqlite ${sqliteColumn.type}`,
      );
    }
    if (pgColumn.normalized !== sqliteColumn.normalized) {
      problems.push(
        `column ${table}.${column} constraints diverge:\n    postgres: ${pgColumn.normalized}\n    sqlite:   ${sqliteColumn.normalized}`,
      );
    }
  }
  for (const column of sqliteTable.columns.keys()) {
    if (!pgTable.columns.has(column) && !ALLOWED.has(`column|${table}|${column}`)) {
      problems.push(`column ${table}.${column} exists in SQLite but not in PostgreSQL`);
    }
  }
}

for (const table of sqlite.tables.keys()) {
  if (!postgres.tables.has(table) && !ALLOWED.has(`table|${table}|sqlite-only`)) {
    problems.push(`table ${table} exists in SQLite but not in PostgreSQL`);
  }
}

for (const [name, index] of postgres.indexes) {
  const sqliteIndex = sqlite.indexes.get(name);
  if (!sqliteIndex) {
    problems.push(`index ${name} (${index.table}) exists in PostgreSQL but not in SQLite`);
    continue;
  }
  if (index.columns !== sqliteIndex.columns) {
    problems.push(
      `index ${name} columns diverge: postgres (${index.columns}) vs sqlite (${sqliteIndex.columns})`,
    );
  }
}
for (const [name, index] of sqlite.indexes) {
  if (!postgres.indexes.has(name)) {
    problems.push(`index ${name} (${index.table}) exists in SQLite but not in PostgreSQL`);
  }
}

if (problems.length > 0) {
  console.error(
    `cross-dialect DDL parity failed with ${problems.length} undeclared divergence(s):\n` +
      problems.map((problem) => `  - ${problem}`).join("\n"),
  );
  process.exit(1);
}

console.log(
  `cross-dialect DDL parity passed: ${postgres.tables.size} postgres tables, ${sqlite.tables.size} sqlite tables, ${postgres.indexes.size} postgres indexes, ${sqlite.indexes.size} sqlite indexes`,
);
