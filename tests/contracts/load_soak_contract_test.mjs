// Contract gate for the load/soak evidence toolchain (PRD release blocker:
// operational evidence). Verifies the driver's structural integrity, its CLI
// contract (--help exits 0; missing BASE_URL fails closed with a nonzero exit),
// the honest "no archived runs yet" statement in the evidence protocol README,
// and the package.json wiring. Zero dependencies beyond Node built-ins.
//
// HTTP behavior is exercised by importing the driver's exported scenario engine
// (executeScenarios) and running it against a same-process stub server: some CI
// sandboxes block cross-process loopback connections, so networked gate coverage
// must not rely on a spawned child connecting back to this test's stub. CLI
// usage/exit-code behavior (no network involved) is still verified via child
// processes with a sanitized environment.

import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import http from "node:http";
import { spawnSync } from "node:child_process";
import {
  buildConfig,
  executeScenarios,
  timestampedEvidencePath,
  validateConfig,
  writeEvidenceJson,
} from "../../scripts/load-soak.mjs";

const repoRoot = process.cwd();
const driverPath = path.join(repoRoot, "scripts", "load-soak.mjs");
const evidenceReadmePath = path.join(repoRoot, "docs", "engineering", "evidence", "README.md");
const packageJsonPath = path.join(repoRoot, "package.json");

const ENV_KEYS = [
  "BASE_URL",
  "DURATION_SECS",
  "CONCURRENCY",
  "SDKWORK_LOAD_OPEN_API_KEY",
  "SDKWORK_LOAD_MEM0_API_KEY",
  "SDKWORK_LOAD_APP_ACCESS_TOKEN",
  "SDKWORK_LOAD_APP_BEARER_TOKEN",
  "SDKWORK_LOAD_SCENARIOS",
  "SDKWORK_LOAD_P99_BUDGET_MS",
  "SDKWORK_LOAD_DURATION_SECS",
  "SDKWORK_LOAD_CONCURRENCY",
];

// Env copy with every load/soak input stripped, so spawn results cannot depend
// on the developer shell that happened to run the gate.
function sanitizedEnv(extra = {}) {
  const env = { ...process.env };
  for (const key of ENV_KEYS) delete env[key];
  return { ...env, ...extra };
}

test("load/soak driver and evidence protocol files exist", () => {
  assert.ok(fs.existsSync(driverPath), "scripts/load-soak.mjs must exist");
  assert.ok(fs.existsSync(evidenceReadmePath), "docs/engineering/evidence/README.md must exist");
});

test("driver is zero-dependency (node: built-ins only)", () => {
  const source = fs.readFileSync(driverPath, "utf8");
  const imports = [...source.matchAll(/from\s+"([^"]+)"/g)].map((match) => match[1]);
  assert.ok(imports.length > 0, "driver must declare its imports");
  for (const specifier of imports) {
    assert.ok(
      specifier.startsWith("node:"),
      `driver must only import node: built-ins, found ${specifier}`,
    );
  }
});

test("driver declares the three audit-aligned scenarios statically", () => {
  const source = fs.readFileSync(driverPath, "utf8");
  assert.match(source, /SCENARIO_NAMES\s*=\s*\["retrieval",\s*"list-cursor",\s*"mem0-batch"\]/);
  assert.match(source, /async function runRetrievalScenario\(/, "retrieval scenario function missing");
  assert.match(source, /async function runListCursorScenario\(/, "list-cursor scenario function missing");
  assert.match(source, /async function runMem0BatchScenario\(/, "mem0-batch scenario function missing");
  // Surfaces exactly as pinned by the authority OpenAPI documents.
  assert.match(source, /\/mem\/v3\/api\/memory\/retrievals/);
  assert.match(source, /\/app\/v3\/api\/memory\/memories/);
  assert.match(source, /\/v1\/batch\//);
  assert.match(source, /page_size=200/, "cursor walk must use the max page_size=200");
});

test("driver pins budgets, batch size, and auth header forms", () => {
  const source = fs.readFileSync(driverPath, "utf8");
  assert.match(source, /DEFAULT_P99_BUDGET_MS\s*=\s*200/, "p99 budget default must be 200ms");
  assert.match(source, /DEFAULT_BATCH_SIZE\s*=\s*50/, "mem0 batch size default must be 50");
  assert.match(source, /DEFAULT_DURATION_SECS\s*=\s*60/);
  assert.match(source, /DEFAULT_CONCURRENCY\s*=\s*8/);
  // Auth header forms verified against the authority OpenAPI securitySchemes.
  assert.match(source, /"x-api-key"/, "open/mem0 faces must send X-API-Key");
  assert.match(source, /"access-token"/, "app face must support the Access-Token header");
  assert.match(source, /`Bearer \$\{config\.appBearerToken\}`/, "app face must support Authorization: Bearer");
  // Percentile math and the evidence JSON sink must be present.
  assert.match(source, /function percentile\(/);
  assert.match(source, /load-soak-/, "evidence file naming must carry the load-soak prefix");
  assert.match(source, /toISOString\(\)/, "evidence file naming must be UTC-timestamped");
  assert.match(source, /docs\/engineering\/evidence|EVIDENCE_DIR/);
});

test("--help exits 0 and documents env contract", () => {
  const result = spawnSync(process.execPath, [driverPath, "--help"], {
    cwd: repoRoot,
    env: sanitizedEnv(),
    encoding: "utf8",
  });
  assert.equal(result.status, 0, `--help must exit 0, got ${result.status}: ${result.stderr}`);
  assert.match(result.stdout, /SDKWORK_LOAD_P99_BUDGET_MS/);
  assert.match(result.stdout, /SDKWORK_LOAD_OPEN_API_KEY/);
  assert.match(result.stdout, /SDKWORK_LOAD_APP_ACCESS_TOKEN/);
  assert.match(result.stdout, /SDKWORK_LOAD_MEM0_API_KEY/);
  assert.match(result.stdout, /X-API-Key/);
  assert.match(result.stdout, /Access-Token/);
  assert.match(result.stdout, /\/v1\/batch\//);
});

test("driver fails closed with nonzero exit when BASE_URL is missing", () => {
  const result = spawnSync(process.execPath, [driverPath], {
    cwd: repoRoot,
    env: sanitizedEnv(),
    encoding: "utf8",
  });
  assert.equal(result.status, 2, `missing BASE_URL must exit 2, got ${result.status}`);
  assert.match(result.stderr, /BASE_URL/);
});

test("driver fails closed with nonzero exit when a scenario credential is missing", () => {
  const result = spawnSync(process.execPath, [driverPath, "--base-url", "http://127.0.0.1:9", "--scenarios", "retrieval"], {
    cwd: repoRoot,
    env: sanitizedEnv(),
    encoding: "utf8",
  });
  assert.equal(result.status, 2, `missing credential must exit 2, got ${result.status}`);
  assert.match(result.stderr, /SDKWORK_LOAD_OPEN_API_KEY/);
});

async function startStubServer(handler) {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return server;
}

test("retrieval soak passes against a same-process 2xx stub and honors the p99 gate", async () => {
  const server = await startStubServer((request, response) => {
    void request;
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ code: 0, data: {}, traceId: "00000000-0000-0000-0000-000000000000" }));
  });
  const { port } = server.address();
  const jsonPath = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "load-soak-gate-")), "result.json");
  try {
    const config = buildConfig(
      {
        baseUrl: `http://127.0.0.1:${port}`,
        scenarios: "retrieval",
        durationSecs: "1",
        concurrency: "2",
        p99BudgetMs: "60000",
        json: true,
        jsonPath,
      },
      sanitizedEnv({ SDKWORK_LOAD_OPEN_API_KEY: "gate-key" }),
    );
    validateConfig(config);
    const report = await executeScenarios(config);
    assert.equal(report.schema, "sdkwork.load-soak.v1");
    assert.equal(report.passed, true);
    assert.equal(report.scenarios.retrieval.enabled, true);
    assert.ok(report.scenarios.retrieval.total > 0, "soak must issue requests");
    assert.equal(report.scenarios.retrieval.err5xx, 0);
    assert.ok(report.scenarios.retrieval.p99Ms !== null);
    assert.ok(report.scenarios.retrieval.p99Ms <= 60000, "p99 must be measured against the budget");
    // The evidence JSON must not embed credentials.
    writeEvidenceJson(report, jsonPath);
    assert.doesNotMatch(fs.readFileSync(jsonPath, "utf8"), /gate-key/);
  } finally {
    server.close();
  }
});

test("scenario engine fails the run when the server answers 5xx", async () => {
  const server = await startStubServer((request, response) => {
    void request;
    response.writeHead(500, { "content-type": "application/json" });
    response.end("{}");
  });
  const { port } = server.address();
  try {
    const config = buildConfig(
      { baseUrl: `http://127.0.0.1:${port}`, scenarios: "retrieval", durationSecs: "1", concurrency: "2" },
      sanitizedEnv({ SDKWORK_LOAD_OPEN_API_KEY: "gate-key" }),
    );
    const report = await executeScenarios(config);
    assert.equal(report.passed, false);
    assert.ok(report.scenarios.retrieval.err5xx > 0, "5xx responses must be counted and fail the run");
  } finally {
    server.close();
  }
});

test("cursor walk reaches exhaustion on a terminating cursor and passes", async () => {
  let page = 0;
  const server = await startStubServer((request, response) => {
    void request;
    page += 1;
    const nextCursor = page < 3 ? `cursor-${page}` : null;
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({
      code: 0,
      data: { items: [], pageInfo: { mode: "cursor", nextCursor } },
      traceId: "00000000-0000-0000-0000-000000000000",
    }));
  });
  const { port } = server.address();
  try {
    const config = buildConfig(
      { baseUrl: `http://127.0.0.1:${port}`, scenarios: "list-cursor", durationSecs: "1" },
      sanitizedEnv({ SDKWORK_LOAD_APP_ACCESS_TOKEN: "gate-app-token" }),
    );
    const report = await executeScenarios(config);
    assert.equal(report.passed, true);
    assert.equal(report.scenarios["list-cursor"].exhausted, true);
    assert.equal(report.scenarios["list-cursor"].pages, 3);
  } finally {
    server.close();
  }
});

test("cursor walk dead-loop guard fails the run on a never-ending cursor", async () => {
  const server = await startStubServer((request, response) => {
    void request;
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({
      code: 0,
      data: { items: [], pageInfo: { mode: "cursor", nextCursor: "cursor-forever" } },
      traceId: "00000000-0000-0000-0000-000000000000",
    }));
  });
  const { port } = server.address();
  try {
    const config = buildConfig(
      { baseUrl: `http://127.0.0.1:${port}`, scenarios: "list-cursor", durationSecs: "1" },
      sanitizedEnv({ SDKWORK_LOAD_APP_ACCESS_TOKEN: "gate-app-token", SDKWORK_LOAD_MAX_PAGES: "5" }),
    );
    const report = await executeScenarios(config);
    assert.equal(report.passed, false);
    assert.match(report.scenarios["list-cursor"].reasons.join("; "), /dead-loop guard tripped/);
  } finally {
    server.close();
  }
});

test("CLI tuning flags take precedence over env and defaults", () => {
  const config = buildConfig(
    { baseUrl: "http://example.test", scenarios: "retrieval", durationSecs: "3", concurrency: "2", p99BudgetMs: "150" },
    sanitizedEnv({ DURATION_SECS: "999", CONCURRENCY: "999", SDKWORK_LOAD_P99_BUDGET_MS: "999" }),
  );
  assert.equal(config.durationSecs, 3);
  assert.equal(config.concurrency, 2);
  assert.equal(config.p99BudgetMs, 150);
  const fromEnv = buildConfig({}, sanitizedEnv({ DURATION_SECS: "7" }));
  assert.equal(fromEnv.durationSecs, 7);
  const defaults = buildConfig({}, sanitizedEnv());
  assert.equal(defaults.durationSecs, 60);
  assert.equal(defaults.concurrency, 8);
  assert.equal(defaults.p99BudgetMs, 200);
});

test("evidence sink names are UTC-timestamped with the load-soak prefix", () => {
  const evidencePath = timestampedEvidencePath(new Date("2026-09-30T12:34:56Z"));
  assert.match(
    evidencePath.replace(/\\/g, "/"),
    /docs\/engineering\/evidence\/load-soak-\d{8}T\d{6}Z\.json/,
    "evidence files must land in docs/engineering/evidence with a UTC load-soak stamp",
  );
  assert.match(
    evidencePath.replace(/\\/g, "/"),
    /load-soak-20260930T123456Z\.json/,
    "the stamp must be derived from the UTC instant, not local time",
  );
});

test("CLI maps the gate verdict onto exit codes (0 pass, 1 SLO violation)", () => {
  const source = fs.readFileSync(driverPath, "utf8");
  assert.match(source, /process\.exitCode = report\.passed \? 0 : 1/);
  assert.match(source, /process\.exitCode = 2/, "usage/configuration errors must exit 2");
});

test("evidence README states the honest no-archived-runs status and the gates", () => {
  const readme = fs.readFileSync(evidenceReadmePath, "utf8");
  // Honest status: the protocol must not pretend a run has been archived.
  assert.match(readme, /尚无[^。\n]*已归档/, "README must state that no load/soak run is archived yet");
  // Result file naming with a UTC timestamp.
  assert.match(readme, /load-soak-<YYYYMMDDTHHMMSSZ>\.json/);
  // Admission gates: 5xx=0, p99 budget, cursor walk terminates, >=1 2xx.
  assert.match(readme, /5xx\s*=\s*0/);
  assert.match(readme, /SDKWORK_LOAD_P99_BUDGET_MS/);
  assert.match(readme, /nextCursor/);
  // A recorded run = one archived JSON + environment description.
  assert.match(readme, /环境描述/);
  assert.match(readme, /replica|副本/i);
  assert.match(readme, /pool capacity|池容量/i);
});

test("package.json wires perf:soak to the driver and gates it in the check chain", () => {
  const pkg = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  // `perf` is the standard tool namespace for soak/load evidence
  // (PNPM_SCRIPT_SPEC.md section 4); `load` is not a public namespace.
  assert.equal(pkg.scripts["perf:soak"], "node scripts/load-soak.mjs");
  assert.match(
    pkg.scripts["_sdkwork:check"],
    /node --test tests\/contracts\/load_soak_contract_test\.mjs/,
    "_sdkwork:check must run this contract gate at its tail",
  );
});
