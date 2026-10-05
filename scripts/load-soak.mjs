// SDKWORK Memory — minimal load/soak evidence driver (zero runtime dependencies).
//
// Evidence protocol: docs/engineering/evidence/README.md
// Why this exists (release blocker): docs/engineering/reviews/REVIEW-20260923-memory-commercial-readiness-audit.md
// lists "压测 / soak / 容量证据" as 完全缺失 / commercial-launch blocking, and
// docs/engineering/reviews/REVIEW-20260930-fourth-audit-remediation.md items
// G5 (write p99 budget), B11 (cursor window semantics), C8 (mem0 batch write path)
// define the exercised surfaces.
//
// Auth header evidence (grep-verified against the authority OpenAPI, not guessed):
// - Open face `/mem/v3/api/*` uses header `X-API-Key`
//   (apis/open-api/memory-open-api.openapi.json components.securitySchemes.ApiKey,
//   per-operation `x-sdkwork-auth-mode: "api-key"`).
// - mem0 compat face `/v1/batch/` is materialized on the same open face with the
//   ApiKey profile, so it also takes `X-API-Key`. The mem0 SDK's native
//   `Authorization: Token <k>` header is bridged to `X-Api-Key` by the gateway
//   (docs/engineering/reviews/REVIEW-20260928-mem0-wire-compatibility-plan.md).
// - App face `/app/v3/api/*` accepts EITHER `Authorization: Bearer <jwt>` (scheme
//   AuthToken) OR the `Access-Token` header (scheme AccessToken); the list
//   operation declares security `[{"AuthToken":[]},{"AccessToken":[]}]`
//   (apis/app-api/memory-app-api.openapi.json components.securitySchemes).
//
// Exit codes: 0 = all gates passed; 1 = SLO/gate violation (5xx, p99 budget,
// cursor dead-loop guard, or an enabled scenario produced zero 2xx);
// 2 = usage/configuration error (fail-closed).
//
// The scenario engine (executeScenarios and friends) is exported so the contract
// gate (tests/contracts/load_soak_contract_test.mjs) can drive it in-process
// against a same-process stub server. Some CI sandboxes block cross-process
// loopback connections, so networked gate coverage must not depend on spawning
// a child process that connects back to the test's stub server; CLI usage and
// exit-code behavior (no network needed) are still verified via child processes.

import { performance } from "node:perf_hooks";
import fs from "node:fs";
import path from "node:path";
import { createHash, randomUUID } from "node:crypto";
import { pathToFileURL } from "node:url";

const SCENARIO_NAMES = ["retrieval", "list-cursor", "mem0-batch"];

// Surfaces (verified against apis/open-api/memory-open-api.openapi.json and
// apis/app-api/memory-app-api.openapi.json; those are generated authority copies).
const RETRIEVALS_PATH = "/mem/v3/api/memory/retrievals";
const MEMORIES_LIST_PATH = "/app/v3/api/memory/memories";
const LIST_FIRST_PAGE_QUERY = "?page_size=200";
const MEM0_BATCH_PATH = "/v1/batch/";

// Budgets and defaults (gate thresholds; overridable via env/flags below).
const DEFAULT_P99_BUDGET_MS = 200;
const DEFAULT_BATCH_SIZE = 50;
const DEFAULT_DURATION_SECS = 60;
const DEFAULT_CONCURRENCY = 8;
const DEFAULT_MAX_PAGES = 10000;
const DEFAULT_REQUEST_TIMEOUT_MS = 30000;
const DEFAULT_SEED = 20260930;
const EVIDENCE_DIR = "docs/engineering/evidence";

const QUERY_WORD_POOL = [
  "meeting", "notes", "decision", "release", "incident", "runbook",
  "onboarding", "customer", "feedback", "invoice", "refund", "policy",
  "roadmap", "sprint", "retro", "deployment", "outage", "postmortem",
  "contract", "renewal", "escalation", "handoff", "design", "review",
];

const USAGE_TEXT = `sdkwork-memory load/soak evidence driver (zero-dependency)

Usage:
  node scripts/load-soak.mjs --base-url <BASE_URL> [flags]
  BASE_URL=<...> node scripts/load-soak.mjs [flags]
  pnpm perf:soak --scenarios retrieval --duration-secs 120 --json

Scenarios (aligned with the fourth-round audit plan):
  retrieval     POST    {BASE}/mem/v3/api/memory/retrievals    closed-loop, DURATION_SECS, CONCURRENCY workers
                Random queries drawn from a word pool; body follows MemoryRetrievalRequest
                (query, spaceIds, topK, contextBudgetTokens). Asserts 2xx and records p99
                against SDKWORK_LOAD_P99_BUDGET_MS.
  list-cursor   GET     {BASE}/app/v3/api/memory/memories?page_size=200
                Follows data.pageInfo.nextCursor until exhaustion (single sequential walk,
                not duration-based). Dead-loop guards: SDKWORK_LOAD_MAX_PAGES cap and
                repeated-cursor detection. Requires APP face auth.
  mem0-batch    PUT     {BASE}/v1/batch/    closed-loop, batches of SDKWORK_LOAD_BATCH_SIZE (default 50)
                Body {"memories":[{memory_id,text}]}. Measures the batched write path.

Auth (env, per face; header forms are pinned by the authority OpenAPI, see file header):
  SDKWORK_LOAD_OPEN_API_KEY      open face X-API-Key header (retrieval scenario)
  SDKWORK_LOAD_MEM0_API_KEY      mem0 compat face X-API-Key header (/v1/batch/)
  SDKWORK_LOAD_APP_ACCESS_TOKEN  app face Access-Token header (list-cursor scenario), or
  SDKWORK_LOAD_APP_BEARER_TOKEN  app face "Authorization: Bearer <jwt>" (either one suffices)

Options:
  --base-url <url>          Target base URL (env BASE_URL). Required unless --help.
  --scenarios <list>        Comma list from: retrieval,list-cursor,mem0-batch
                            (env SDKWORK_LOAD_SCENARIOS; default: all three).
  --duration-secs <n>       Closed-loop duration per soak scenario
                            (env DURATION_SECS or SDKWORK_LOAD_DURATION_SECS; default ${DEFAULT_DURATION_SECS}).
  --concurrency <n>         Concurrent workers per soak scenario
                            (env CONCURRENCY or SDKWORK_LOAD_CONCURRENCY; default ${DEFAULT_CONCURRENCY}).
  --p99-budget-ms <n>       p99 latency budget gate
                            (env SDKWORK_LOAD_P99_BUDGET_MS; default ${DEFAULT_P99_BUDGET_MS}).
  --json[=file]             Write the full result JSON to docs/engineering/evidence/
                            load-soak-<UTC timestamp>.json (or the explicit file path).
                            An archived load/soak run is one JSON file plus a written
                            environment description (see docs/engineering/evidence/README.md).
  --help                    Show this help and exit 0.

Tuning (env):
  SDKWORK_LOAD_SPACE_IDS        comma list of spaceIds sent with retrieval bodies (default [])
  SDKWORK_LOAD_MEM0_ID_POOL     comma list of real memory_id values for batch updates;
                                default synthesizes placeholder ids (see README caveat)
  SDKWORK_LOAD_BATCH_SIZE       mem0 batch entries per request (default ${DEFAULT_BATCH_SIZE})
  SDKWORK_LOAD_MAX_PAGES        cursor-walk page cap / dead-loop guard (default ${DEFAULT_MAX_PAGES})
  SDKWORK_LOAD_REQUEST_TIMEOUT_MS  per-request fetch timeout (default ${DEFAULT_REQUEST_TIMEOUT_MS})
  SDKWORK_LOAD_SEED             PRNG seed for reproducible query pools (default ${DEFAULT_SEED})

Exit codes: 0 gates passed; 1 SLO/gate violation; 2 usage/configuration error.
`;

class UsageError extends Error {}

function failUsage(message) {
  throw new UsageError(message);
}

function parseFlags(argv) {
  const flags = { json: false, jsonPath: null, help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    const eq = arg.indexOf("=");
    const name = eq === -1 ? arg : arg.slice(0, eq);
    const inlineValue = eq === -1 ? null : arg.slice(eq + 1);
    const takeValue = () => {
      if (inlineValue !== null) return inlineValue;
      i += 1;
      if (i >= argv.length) failUsage(`flag ${name} requires a value`);
      return argv[i];
    };
    switch (name) {
      case "--help":
      case "-h":
        flags.help = true;
        break;
      case "--base-url":
        flags.baseUrl = takeValue();
        break;
      case "--scenarios":
        flags.scenarios = takeValue();
        break;
      case "--duration-secs":
        flags.durationSecs = takeValue();
        break;
      case "--concurrency":
        flags.concurrency = takeValue();
        break;
      case "--p99-budget-ms":
        flags.p99BudgetMs = takeValue();
        break;
      case "--json":
        if (inlineValue !== null) {
          flags.json = true;
          flags.jsonPath = inlineValue;
        } else if (argv[i + 1] && !argv[i + 1].startsWith("-")) {
          flags.json = true;
          flags.jsonPath = argv[i + 1];
          i += 1;
        } else {
          flags.json = true;
        }
        break;
      default:
        failUsage(`unknown flag ${name}`);
    }
  }
  return flags;
}

function readNumberValue(raw, label) {
  const value = Number(raw);
  if (!Number.isFinite(value) || value <= 0) {
    failUsage(`${label} must be a positive number, got ${JSON.stringify(raw)}`);
  }
  return value;
}

// Precedence: explicit flag > env keys > default. CLI flags are documented in
// --help and the evidence README, so they must actually win over env defaults.
function readNumberEnv(env, keys, fallback, flagValue, flagName) {
  if (flagValue !== undefined) return readNumberValue(flagValue, flagName);
  for (const key of keys) {
    const raw = env[key];
    if (raw !== undefined && raw !== "") {
      return readNumberValue(raw, `env ${key}`);
    }
  }
  return fallback;
}

function buildConfig(flags, env) {
  const scenarios = (flags.scenarios ?? env.SDKWORK_LOAD_SCENARIOS ?? SCENARIO_NAMES.join(","))
    .split(",")
    .map((name) => name.trim())
    .filter((name) => name.length > 0);
  for (const name of scenarios) {
    if (!SCENARIO_NAMES.includes(name)) {
      failUsage(`unknown scenario ${JSON.stringify(name)}; expected any of ${SCENARIO_NAMES.join(",")}`);
    }
  }
  return {
    baseUrl: (flags.baseUrl ?? env.BASE_URL ?? "").replace(/\/+$/, ""),
    scenarios,
    durationSecs: readNumberEnv(env, ["SDKWORK_LOAD_DURATION_SECS", "DURATION_SECS"], DEFAULT_DURATION_SECS, flags.durationSecs, "--duration-secs"),
    concurrency: readNumberEnv(env, ["SDKWORK_LOAD_CONCURRENCY", "CONCURRENCY"], DEFAULT_CONCURRENCY, flags.concurrency, "--concurrency"),
    p99BudgetMs: readNumberEnv(env, ["SDKWORK_LOAD_P99_BUDGET_MS"], DEFAULT_P99_BUDGET_MS, flags.p99BudgetMs, "--p99-budget-ms"),
    maxPages: readNumberEnv(env, ["SDKWORK_LOAD_MAX_PAGES"], DEFAULT_MAX_PAGES),
    batchSize: readNumberEnv(env, ["SDKWORK_LOAD_BATCH_SIZE"], DEFAULT_BATCH_SIZE),
    requestTimeoutMs: readNumberEnv(env, ["SDKWORK_LOAD_REQUEST_TIMEOUT_MS"], DEFAULT_REQUEST_TIMEOUT_MS),
    seed: readNumberEnv(env, ["SDKWORK_LOAD_SEED"], DEFAULT_SEED),
    spaceIds: (env.SDKWORK_LOAD_SPACE_IDS ?? "")
      .split(",")
      .map((id) => id.trim())
      .filter((id) => id.length > 0),
    mem0IdPool: (env.SDKWORK_LOAD_MEM0_ID_POOL ?? "")
      .split(",")
      .map((id) => id.trim())
      .filter((id) => id.length > 0),
    openApiKey: env.SDKWORK_LOAD_OPEN_API_KEY ?? "",
    mem0ApiKey: env.SDKWORK_LOAD_MEM0_API_KEY ?? "",
    appAccessToken: env.SDKWORK_LOAD_APP_ACCESS_TOKEN ?? "",
    appBearerToken: env.SDKWORK_LOAD_APP_BEARER_TOKEN ?? "",
    json: flags.json,
    jsonPath: flags.jsonPath,
  };
}

function validateConfig(config) {
  if (!config.baseUrl) {
    failUsage("BASE_URL (or --base-url) is required");
  }
  try {
    new URL(config.baseUrl);
  } catch {
    failUsage(`BASE_URL is not an absolute URL: ${config.baseUrl}`);
  }
  const missing = [];
  if (config.scenarios.includes("retrieval") && !config.openApiKey) {
    missing.push("retrieval requires SDKWORK_LOAD_OPEN_API_KEY (open face X-API-Key)");
  }
  if (config.scenarios.includes("list-cursor") && !config.appAccessToken && !config.appBearerToken) {
    missing.push(
      "list-cursor requires SDKWORK_LOAD_APP_ACCESS_TOKEN (Access-Token header) or " +
        "SDKWORK_LOAD_APP_BEARER_TOKEN (Authorization: Bearer), per the app-face OpenAPI security",
    );
  }
  if (config.scenarios.includes("mem0-batch") && !config.mem0ApiKey) {
    missing.push("mem0-batch requires SDKWORK_LOAD_MEM0_API_KEY (mem0 compat face X-API-Key)");
  }
  if (missing.length > 0) {
    failUsage(`missing credentials\n  - ${missing.join("\n  - ")}`);
  }
}

// Deterministic PRNG (mulberry32) so archived evidence can state its query mix.
function createRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function percentile(sortedSamples, p) {
  if (sortedSamples.length === 0) return null;
  const index = Math.min(sortedSamples.length - 1, Math.max(0, Math.ceil((p / 100) * sortedSamples.length) - 1));
  return sortedSamples[index];
}

function summarizeLatencies(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  return {
    p50Ms: percentile(sorted, 50),
    p95Ms: percentile(sorted, 95),
    p99Ms: percentile(sorted, 99),
    maxMs: sorted.length > 0 ? sorted[sorted.length - 1] : null,
  };
}

function emptyStats() {
  return { total: 0, ok2xx: 0, err4xx: 0, err5xx: 0, networkErrors: 0, latenciesMs: [], extra: {} };
}

async function requestOnce(url, options, timeoutMs) {
  const startedAt = performance.now();
  let response;
  try {
    response = await fetch(url, { ...options, signal: AbortSignal.timeout(timeoutMs) });
  } catch {
    return { kind: "network", elapsedMs: performance.now() - startedAt };
  }
  let bodyText = null;
  try {
    bodyText = await response.text(); // drain body so sockets close promptly under load
  } catch {
    bodyText = null;
  }
  return { kind: "http", status: response.status, elapsedMs: performance.now() - startedAt, bodyText };
}

function recordOutcome(stats, outcome) {
  stats.total += 1;
  if (outcome.kind === "network") {
    stats.networkErrors += 1;
    return null;
  }
  stats.latenciesMs.push(outcome.elapsedMs);
  if (outcome.status >= 200 && outcome.status < 300) stats.ok2xx += 1;
  else if (outcome.status >= 400 && outcome.status < 500) stats.err4xx += 1;
  else stats.err5xx += 1;
  return outcome;
}

// Closed-loop soak: CONCURRENCY workers issue back-to-back requests until the deadline.
async function runClosedLoop(config, issueRequest, stats) {
  const deadline = performance.now() + config.durationSecs * 1000;
  const worker = async () => {
    while (performance.now() < deadline) {
      const outcome = await issueRequest();
      recordOutcome(stats, outcome);
    }
  };
  const startedAt = performance.now();
  await Promise.all(Array.from({ length: config.concurrency }, () => worker()));
  return (performance.now() - startedAt) / 1000;
}

function finalizeScenario(name, enabled, stats, elapsedSecs, config, reasons) {
  const latencies = summarizeLatencies(stats.latenciesMs);
  const passed =
    enabled &&
    reasons.length === 0 &&
    stats.err5xx === 0 &&
    stats.ok2xx > 0 &&
    latencies.p99Ms !== null &&
    latencies.p99Ms <= config.p99BudgetMs;
  return {
    enabled,
    total: stats.total,
    ok2xx: stats.ok2xx,
    err4xx: stats.err4xx,
    err5xx: stats.err5xx,
    networkErrors: stats.networkErrors,
    ...latencies,
    rps: elapsedSecs > 0 ? Number((stats.latenciesMs.length / elapsedSecs).toFixed(3)) : 0,
    elapsedSecs: Number(elapsedSecs.toFixed(3)),
    ...(Object.keys(stats.extra).length > 0 ? { ...stats.extra } : {}),
    reasons,
    passed,
  };
}

// Scenario (a): retrieval closed-loop against the open face. Asserts 2xx and the p99 budget.
async function runRetrievalScenario(config) {
  const stats = emptyStats();
  const reasons = [];
  const random = createRandom(config.seed);
  const randomQuery = () => {
    const words = Array.from(
      { length: 2 + Math.floor(random() * 3) },
      () => QUERY_WORD_POOL[Math.floor(random() * QUERY_WORD_POOL.length)],
    );
    return `${words.join(" ")} #${Math.floor(random() * 1_000_000)}`;
  };
  const url = `${config.baseUrl}${RETRIEVALS_PATH}`;
  const elapsedSecs = await runClosedLoop(config, async () => {
    const body = JSON.stringify({
      query: randomQuery(),
      spaceIds: config.spaceIds,
      topK: 8,
      contextBudgetTokens: 4096,
    });
    // `retrievals.create` is an idempotent SDKWork command: each request
    // carries its own key plus the body fingerprint the gateway verifies.
    const headers = {
      "content-type": "application/json",
      "x-api-key": config.openApiKey,
      "idempotency-key": randomUUID(),
      "x-content-sha256": createHash("sha256").update(body).digest("hex"),
    };
    return requestOnce(url, { method: "POST", headers, body }, config.requestTimeoutMs);
  }, stats);
  return finalizeScenario("retrieval", true, stats, elapsedSecs, config, reasons);
}

// Scenario (b): app-face list walk. Follows data.pageInfo.nextCursor to exhaustion with
// dead-loop guards (page cap + repeated-cursor detection). An empty page is a fail-closed
// window end, so exhaustion = a null/absent nextCursor on a 2xx page.
async function runListCursorScenario(config) {
  const stats = emptyStats();
  const reasons = [];
  const headers = {};
  if (config.appAccessToken) headers["access-token"] = config.appAccessToken;
  if (config.appBearerToken) headers.authorization = `Bearer ${config.appBearerToken}`;
  const seenCursors = new Set();
  let cursor = null;
  let exhausted = false;
  const startedAt = performance.now();
  while (stats.total < config.maxPages) {
    const url = `${config.baseUrl}${MEMORIES_LIST_PATH}${LIST_FIRST_PAGE_QUERY}${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ""}`;
    const outcome = recordOutcome(stats, await requestOnce(url, { method: "GET", headers }, config.requestTimeoutMs));
    if (outcome === null) {
      reasons.push(`network error on page ${stats.total}; walk aborted`);
      break;
    }
    if (outcome.status >= 300) {
      reasons.push(`walk aborted at page ${stats.total} with HTTP ${outcome.status}`);
      break;
    }
    let body;
    try {
      body = JSON.parse(outcome.bodyText ?? "");
    } catch {
      reasons.push(`page ${stats.total} returned a non-JSON body; walk aborted`);
      break;
    }
    const nextCursor = body?.data?.pageInfo?.nextCursor ?? null;
    if (!nextCursor) {
      exhausted = true;
      break;
    }
    if (seenCursors.has(nextCursor)) {
      reasons.push(`server repeated cursor ${JSON.stringify(nextCursor)}; dead-loop guard tripped`);
      break;
    }
    seenCursors.add(nextCursor);
    cursor = nextCursor;
  }
  const elapsedSecs = (performance.now() - startedAt) / 1000;
  if (stats.total >= config.maxPages && !exhausted) {
    reasons.push(`page cap ${config.maxPages} reached before exhaustion; dead-loop guard tripped`);
  }
  if (!exhausted && reasons.length === 0) {
    reasons.push("cursor walk ended without exhaustion");
  }
  const result = finalizeScenario("list-cursor", true, stats, elapsedSecs, config, reasons);
  result.exhausted = exhausted;
  result.pages = stats.total;
  return result;
}

// Scenario (c): mem0 compat face batch update. C8 target: the batched write path.
async function runMem0BatchScenario(config) {
  const stats = emptyStats();
  const reasons = [];
  const random = createRandom(config.seed);
  const idPool = config.mem0IdPool.length > 0
    ? config.mem0IdPool
    : Array.from({ length: 500 }, (_, i) => `soak-placeholder-${(i + 1).toString().padStart(6, "0")}`);
  const url = `${config.baseUrl}${MEM0_BATCH_PATH}`;
  const headers = { "content-type": "application/json", "x-api-key": config.mem0ApiKey };
  const elapsedSecs = await runClosedLoop(config, async () => {
    const memories = Array.from({ length: config.batchSize }, (_, i) => ({
      memory_id: idPool[Math.floor(random() * idPool.length)],
      text: `soak batch update ${Math.floor(random() * 1_000_000)} entry ${i}`,
    }));
    return requestOnce(url, { method: "PUT", headers, body: JSON.stringify({ memories }) }, config.requestTimeoutMs);
  }, stats);
  stats.extra.batchSize = config.batchSize;
  return finalizeScenario("mem0-batch", true, stats, elapsedSecs, config, reasons);
}

function printScenarioSummary(name, result) {
  if (!result.enabled) {
    process.stdout.write(`  ${name.padEnd(12)} skipped\n`);
    return;
  }
  const fmt = (value) => (value === null ? "-" : String(Math.round(value * 1000) / 1000));
  process.stdout.write(
    `  ${name.padEnd(12)} total=${result.total} 2xx=${result.ok2xx} 4xx=${result.err4xx} ` +
      `5xx=${result.err5xx} net=${result.networkErrors} ` +
      `p50=${fmt(result.p50Ms)}ms p95=${fmt(result.p95Ms)}ms p99=${fmt(result.p99Ms)}ms ` +
      `max=${fmt(result.maxMs)}ms rps=${fmt(result.rps)} ${result.passed ? "PASS" : "FAIL"}` +
      `${result.reasons.length > 0 ? ` reasons: ${result.reasons.join("; ")}` : ""}\n`,
  );
}

function timestampedEvidencePath(now = new Date()) {
  const stamp = now.toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
  return path.join(EVIDENCE_DIR, `load-soak-${stamp}.json`);
}

// Runs every selected scenario sequentially and folds the results into the
// sdkwork.load-soak.v1 report. Pure with respect to process state (no printing,
// no file writes): the CLI decorates via hooks and the evidence sink separately,
// and the contract gate drives this function in-process.
// hooks: { onScenarioStart?(name), onScenarioResult?(name, result) }
// Additional named exports for the contract gate (see the header note): config
// construction/validation, the CLI parser, usage errors, and the evidence sink.
export {
  SCENARIO_NAMES,
  UsageError,
  parseFlags,
  buildConfig,
  validateConfig,
  timestampedEvidencePath,
};

export async function executeScenarios(config, hooks = {}) {
  const scenarioResults = {};
  for (const name of SCENARIO_NAMES) {
    if (!config.scenarios.includes(name)) {
      scenarioResults[name] = { enabled: false, passed: false, reasons: ["not selected"] };
      hooks.onScenarioResult?.(name, scenarioResults[name]);
      continue;
    }
    hooks.onScenarioStart?.(name);
    if (name === "retrieval") scenarioResults[name] = await runRetrievalScenario(config);
    else if (name === "list-cursor") scenarioResults[name] = await runListCursorScenario(config);
    else scenarioResults[name] = await runMem0BatchScenario(config);
    hooks.onScenarioResult?.(name, scenarioResults[name]);
  }
  const passed = config.scenarios.every((name) => scenarioResults[name]?.passed === true);
  return {
    schema: "sdkwork.load-soak.v1",
    generatedAtUtc: new Date().toISOString(),
    baseUrl: config.baseUrl,
    durationSecs: config.durationSecs,
    concurrency: config.concurrency,
    p99BudgetMs: config.p99BudgetMs,
    scenarios: scenarioResults,
    passed,
    gate: {
      err5xxMustBeZero: true,
      p99BudgetMs: config.p99BudgetMs,
      cursorWalkMustExhaust: true,
      everyEnabledScenarioNeeds2xx: true,
    },
  };
}

export function writeEvidenceJson(report, outPath) {
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, `${JSON.stringify(report, null, 2)}\n`);
  return outPath;
}

async function main() {
  try {
    const flags = parseFlags(process.argv.slice(2));
    if (flags.help) {
      process.stdout.write(USAGE_TEXT);
      process.exitCode = 0;
      return;
    }
    const config = buildConfig(flags, process.env);
    validateConfig(config);

    process.stdout.write(
      `load/soak against ${config.baseUrl} (duration=${config.durationSecs}s, concurrency=${config.concurrency}, p99 budget=${config.p99BudgetMs}ms)\n`,
    );
    const report = await executeScenarios(config, {
      onScenarioStart: (name) => process.stdout.write(`  running ${name}...\n`),
      onScenarioResult: (name, result) => printScenarioSummary(name, result),
    });

    if (config.json) {
      const outPath = config.jsonPath ?? timestampedEvidencePath();
      writeEvidenceJson(report, outPath);
      process.stdout.write(`evidence JSON written: ${outPath}\n`);
      process.stdout.write("remember to archive the matching environment description (replicas, pool capacity, data scale)\n");
    }

    process.exitCode = report.passed ? 0 : 1;
  } catch (error) {
    if (error instanceof UsageError) {
      process.stderr.write(`usage error: ${error.message}\n(run with --help for usage)\n`);
      process.exitCode = 2;
      return;
    }
    throw error;
  }
}

const isMainModule = Boolean(process.argv[1]) && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMainModule) {
  await main();
}
