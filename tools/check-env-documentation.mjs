#!/usr/bin/env node
//
// check-env-documentation.mjs
//
// Keeps `.env.example` honest against the environment variables the Rust
// runtime actually reads.
//
// Why this gate exists
// -------------------
// The fourth audit (REVIEW-20260930-fourth-audit-remediation.md, finding G4)
// found 81 environment variable names referenced by `crates/` and `plugins/`
// while `.env.example` documented a fraction of them. Undocumented variables
// are operator traps: a deployment that needs a bound (for example the
// production cursor signing key) has no checked-in hint that it exists.
//
// Rules
// -----
// R1  Every string literal in `crates/**.rs` and `plugins/**.rs` that names a
//     variable with an `SDKWORK_` or `OTEL_` prefix is a "code reference".
//     This deliberately over-approximates: reads go through
//     `std::env::var`, `std::env::var_os`, `read_env_u64`/`read_env_usize`
//     helpers, name-bearing `const` definitions, and alias arrays — the one
//     shape they all share is the quoted variable name next to the read site.
// R2  Every code reference must either be documented in `.env.example` (active
//     or commented `NAME=` line) or appear in the explicit `ALLOWLIST` below
//     with a reviewed reason (test-only fixtures, inline probes, and legacy
//     platform-family aliases are not deployment configuration).
// R3  Every allowlist entry must still be found by the scan (no stale
//     exemptions) and every documented variable that the scan no longer sees
//     is reported as informational, never a failure — `.env.example` also
//     documents framework-level keys read outside this repository.
// R4  Fail closed: scanning zero Rust files, finding zero variable names, or a
//     missing `.env.example` is a failure, not a pass.
//
// Usage:
//   node tools/check-env-documentation.mjs
//   node tools/check-env-documentation.mjs --root <path>
//   node tools/check-env-documentation.mjs --json
//   node tools/check-env-documentation.mjs --self-test
//   node tools/check-env-documentation.mjs --help
//
// Exit codes: 0 = clean (or self-test passed), 1 = undocumented variables,
// stale/broken allowlist, empty scan, or a failed self-test.

import { readdirSync, readFileSync, existsSync } from 'node:fs';
import { join, resolve, dirname, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);

if (args.includes('--help')) {
  process.stdout.write(`check-env-documentation.mjs — .env.example drift gate

Scans every .rs file under crates/ and plugins/ for SDKWORK_*/OTEL_* variable
name literals and diffs them against the keys documented in .env.example.

Options:
  --root <path>   repository root (default: the repo containing this script)
  --json          machine-readable findings
  --self-test     run the built-in parser assertions and exit
  --help          this text

Exit codes: 0 = green, 1 = drift, empty scan, or failed self-test.
`);
  process.exit(0);
}

const SELF_TEST = args.includes('--self-test');
const AS_JSON = args.includes('--json');
const getArg = (name, fallback = null) => {
  const index = args.findIndex((a) => a === `--${name}` || a.startsWith(`--${name}=`));
  if (index < 0) return fallback;
  const hit = args[index];
  if (hit.includes('=')) return hit.slice(hit.indexOf('=') + 1);
  const next = args[index + 1];
  return next && !next.startsWith('--') ? next : fallback;
};

const repoRoot = getArg('root', resolve(dirname(fileURLToPath(import.meta.url)), '..'));

const ENV_EXAMPLE_RELATIVE = '.env.example';
const SCAN_ROOTS = ['crates', 'plugins'];
const SKIP_DIR_NAMES = new Set(['target', 'node_modules', '.git', 'dist', 'build']);
const VARIABLE_NAME_PATTERN = /^(?:SDKWORK|OTEL)_[A-Z0-9_]+$/;

/**
 * Names that appear in Rust sources but are deliberately not `.env.example`
 * material. Every entry needs a reviewed reason; the gate fails when an entry
 * becomes stale (R3) so this table cannot silently rot.
 */
const ALLOWLIST = new Map([
  [
    'SDKWORK_CLOUDROUTER_ENVIRONMENT',
    'legacy cloudrouter-family alias of SDKWORK_MEMORY_ENVIRONMENT read by memory_id_fallback_is_forbidden (crates/sdkwork-intelligence-memory-service/src/platform.rs); not a Memory-owned key',
  ],
  [
    'SDKWORK_CLOUDROUTER_DEPLOYMENT_PROFILE',
    'legacy cloudrouter-family alias of SDKWORK_MEMORY_DEPLOYMENT_PROFILE read by memory_id_fallback_is_forbidden (crates/sdkwork-intelligence-memory-service/src/platform.rs); not a Memory-owned key',
  ],
  [
    'SDKWORK_CLOUDROUTER_RUNTIME_TARGET',
    'legacy cloudrouter-family alias of SDKWORK_MEMORY_RUNTIME_TARGET read by memory_id_fallback_is_forbidden (crates/sdkwork-intelligence-memory-service/src/platform.rs); not a Memory-owned key',
  ],
  [
    'SDKWORK_DATABASE_TEST_POSTGRES_URL',
    'contract-test fixture only (plugins/sdkwork-memory-plugin-native-sql/tests/postgres_store_contract.rs); never read by the runtime',
  ],
  [
    'SDKWORK_MEMORY_POSTGRES_LIFECYCLE_TEST_URL',
    'lifecycle test fixture only (crates/sdkwork-memory-database-host/tests/postgres_lifecycle.rs); never read by the runtime',
  ],
  [
    'SDKWORK_MEMORY_CONTRACT_ENV_SCOPE_PROBE',
    'inline #[cfg(test)] environment-scope probe in crates/sdkwork-memory-contract/src/runtime_env.rs; not a deployment variable',
  ],
  [
    'SDKWORK_ENV',
    'local launcher key for the mem0 example server and test support (crates/sdkwork-routes-memory-open-api/examples, crates/sdkwork-memory-test-support); not a deployment variable',
  ],
]);

/** Extract variable-name string literals from one Rust source string. */
export function extractVariableNames(content) {
  const names = new Set();
  const literal = /"([A-Za-z_][A-Za-z0-9_]*)"/g;
  let match;
  while ((match = literal.exec(content)) !== null) {
    if (VARIABLE_NAME_PATTERN.test(match[1])) names.add(match[1]);
  }
  return names;
}

/** Parse documented keys out of an .env.example body: active or commented `NAME=`. */
export function parseEnvExampleKeys(content) {
  const keys = new Set();
  for (const line of content.split(/\r?\n/)) {
    const hit = /^\s*#?\s*([A-Z][A-Z0-9_]*)\s*=/.exec(line);
    if (hit) keys.add(hit[1]);
  }
  return keys;
}

function walkRustFiles(dir, out) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIR_NAMES.has(entry.name)) continue;
      walkRustFiles(path, out);
    } else if (entry.isFile() && entry.name.endsWith('.rs')) {
      out.push(path);
    }
  }
}

function runScan(root) {
  const files = [];
  for (const scanRoot of SCAN_ROOTS) {
    walkRustFiles(join(root, scanRoot), files);
  }
  /** @type {Map<string, string[]>} name -> relative file locations */
  const references = new Map();
  for (const file of files) {
    const relative = file.slice(root.length + 1).split(sep).join('/');
    for (const name of extractVariableNames(readFileSync(file, 'utf8'))) {
      const bucket = references.get(name) ?? [];
      if (bucket.length < 5) bucket.push(relative);
      references.set(name, bucket);
    }
  }
  return { files, references };
}

function runGate(root) {
  const envExamplePath = join(root, ENV_EXAMPLE_RELATIVE);
  const failures = [];
  if (!existsSync(envExamplePath)) {
    return { ok: false, failures: [`${ENV_EXAMPLE_RELATIVE} is missing`], scannedFiles: 0, references: new Map() };
  }
  const { files, references } = runScan(root);
  if (files.length === 0) {
    failures.push('scan examined zero .rs files under crates/ and plugins/ (fail closed)');
  }
  if (references.size === 0) {
    failures.push('scan found zero SDKWORK_*/OTEL_* variable references (fail closed)');
  }

  const documented = parseEnvExampleKeys(readFileSync(envExamplePath, 'utf8'));
  const undocumented = [];
  for (const [name, locations] of [...references.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    if (documented.has(name)) continue;
    if (ALLOWLIST.has(name)) continue;
    undocumented.push({ name, locations });
  }
  for (const { name, locations } of undocumented) {
    failures.push(`undocumented environment variable ${name} (read in ${locations.join(', ')}); add it to ${ENV_EXAMPLE_RELATIVE} or allowlist it with a reviewed reason in tools/check-env-documentation.mjs`);
  }

  for (const [name] of ALLOWLIST) {
    if (!references.has(name)) {
      failures.push(`stale allowlist entry ${name}: no longer found in crates/ or plugins/ sources; remove it from tools/check-env-documentation.mjs`);
    }
  }

  const undocumentedPlatformKeys = [...documented]
    .filter((name) => VARIABLE_NAME_PATTERN.test(name) && !references.has(name) && !ALLOWLIST.has(name))
    .sort((a, b) => a.localeCompare(b));

  return { ok: failures.length === 0, failures, scannedFiles: files.length, references, undocumentedPlatformKeys };
}

/** Minimal assertion helper for --self-test. */
function selfTest() {
  const assert = (condition, message) => {
    if (!condition) {
      process.stderr.write(`self-test FAILED: ${message}\n`);
      process.exit(1);
    }
  };

  // Extraction catches plain reads, helper reads split across lines, const
  // definitions, alias arrays, and set_var calls alike (R1).
  const names = extractVariableNames([
    'let a = std::env::var("SDKWORK_MEMORY_MAX_BODY_BYTES").ok();',
    'platform::read_env_u64(',
    '    "SDKWORK_MEMORY_JOB_MAX_ATTEMPTS", 5);',
    'const ENV_API_KEY: &str = "SDKWORK_MEMORY_OPENAI_API_KEY";',
    'let aliases = ["SDKWORK_MEMORY_WEB_REDIS_URL", "SDKWORK_REDIS_URL"];',
    'std::env::set_var("SDKWORK_TRACING_SAMPLE_RATIO", "0.5");',
    'let not_env = "sdkwork_memory_not_an_env";',
    'let doc = "SDKWORK-INDEX-0001";',
  ].join('\n'));
  assert(names.size === 6, `expected 6 extracted names, got ${names.size}: ${[...names].join(', ')}`);
  assert(names.has('SDKWORK_MEMORY_MAX_BODY_BYTES'), 'plain std::env::var read missed');
  assert(names.has('SDKWORK_MEMORY_JOB_MAX_ATTEMPTS'), 'multiline helper read missed');
  assert(names.has('SDKWORK_MEMORY_OPENAI_API_KEY'), 'const definition missed');
  assert(names.has('SDKWORK_REDIS_URL'), 'alias array entry missed');
  assert(!names.has('sdkwork_memory_not_an_env'), 'non-env literal extracted');
  assert(!names.has('SDKWORK-INDEX-0001'), 'hyphenated literal extracted');

  // Parsing covers active and commented entries and ignores prose (R2).
  const keys = parseEnvExampleKeys([
    '# === Outbox delivery ===',
    'SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE=log',
    '# SDKWORK_MEMORY_OUTBOX_DELIVERY_URL=',
    '  # SDKWORK_MEMORY_OUTBOX_MAX_RETRIES = 5',
    '# see SDKWORK_MEMORY_RETENTION_TRACES_DAYS for details',
    'OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318',
  ].join('\n'));
  assert(keys.size === 4, `expected 4 documented keys, got ${keys.size}`);
  assert(keys.has('SDKWORK_MEMORY_OUTBOX_DELIVERY_MODE'), 'active key missed');
  assert(keys.has('SDKWORK_MEMORY_OUTBOX_MAX_RETRIES'), 'commented key with spaces missed');
  assert(!keys.has('SDKWORK_MEMORY_RETENTION_TRACES_DAYS'), 'prose mention parsed as key');

  // Drift diff reports code names absent from the example file.
  const missing = [...extractVariableNames('std::env::var("SDKWORK_MEMORY_MISSING_VAR")').values()]
    .flat()
    .filter((name) => !parseEnvExampleKeys('SDKWORK_MEMORY_PRESENT_VAR=1').has(name));
  assert(missing.length === 1 && missing[0] === 'SDKWORK_MEMORY_MISSING_VAR', 'drift diff wrong');

  process.stdout.write('self-test passed: extraction, .env parsing, and drift diff behave as specified\n');
  process.exit(0);
}

if (SELF_TEST) selfTest();

const result = runGate(repoRoot);

if (AS_JSON) {
  process.stdout.write(`${JSON.stringify({
    ok: result.ok,
    scannedFiles: result.scannedFiles,
    referencedVariables: result.references.size,
    failures: result.failures,
    informational: {
      documentedWithoutRustReference: result.undocumentedPlatformKeys ?? [],
    },
  }, null, 2)}\n`);
} else {
  process.stdout.write(`check-env-documentation: scanned ${result.scannedFiles} .rs files, found ${result.references.size} distinct SDKWORK_*/OTEL_* variables\n`);
  if ((result.undocumentedPlatformKeys ?? []).length > 0) {
    process.stdout.write(`note: ${result.undocumentedPlatformKeys.length} documented keys are read outside crates//plugins/ (framework-level, informational only)\n`);
  }
  if (result.failures.length > 0) {
    for (const failure of result.failures) process.stderr.write(`FAIL ${failure}\n`);
    process.stderr.write(`check-env-documentation: ${result.failures.length} finding(s)\n`);
    process.exit(1);
  }
  process.stdout.write('check-env-documentation: OK — every referenced environment variable is documented or explicitly allowlisted\n');
}
