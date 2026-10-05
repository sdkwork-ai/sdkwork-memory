# Operational Evidence (Load / Soak)

Protocol for archiving Memory load/soak evidence. This answers the commercial-readiness
audit blocker "压测 / soak / 容量证据 — 完全缺失"
([REVIEW-20260923-memory-commercial-readiness-audit.md](../reviews/REVIEW-20260923-memory-commercial-readiness-audit.md))
and exercises the surfaces named in the fourth-audit remediation record
([REVIEW-20260930-fourth-audit-remediation.md](../reviews/REVIEW-20260930-fourth-audit-remediation.md)):
G5 (latency p99 budget), B11 (cursor window semantics), C8 (mem0 batch write path).

The driver is `scripts/load-soak.mjs` (zero runtime dependencies, Node built-ins only),
invoked through `pnpm perf:soak` (the `perf` tool namespace; script file
`scripts/load-soak.mjs`). It is a driver, not a result: a run only becomes
evidence when its JSON plus environment description are archived here.

## Current status

**首个已归档条目（2026-10-04，开发线级 harness，非部署级证据）：**
`load-soak-20261004T152436Z.json` + `load-soak-20261004T152436Z.environment.md`。
对 `mem0_platform_server` 线级 harness（进程内 SQLite fixture、单进程、48 条种子记录）
跑了 retrieval 与 mem0-batch 两个场景各 60 秒（8 并发）：retrieval 12,994 请求
0 错误 p99 50.7ms；mem0-batch 3,732 请求 0 错误 p99 182.6ms —— 均在 200ms 预算内。
list-cursor 场景未跑（该 harness 不提供 app 面凭据）。

**部署级证据仍然缺失：** 上述结果证明的是"协议面与驱动在真实 HTTP 上按预算工作"，
不是生产容量。99.9% 可用性与部署级 p99 目标的证据，仍需一次对真实部署形态
（PostgreSQL、多副本、共享 Redis、生产规模数据）的有记录压测——按
`docs/releases/README.md` 的发布门禁归档。

## How to run

Prerequisites: a reachable Memory deployment with seeded data, and credentials for the
faces being exercised (header forms are pinned by the authority OpenAPI — see the file
header of `scripts/load-soak.mjs`):

- open face `/mem/v3/api/*`: `X-API-Key` header → env `SDKWORK_LOAD_OPEN_API_KEY`
- mem0 compat face `/v1/batch/`: `X-API-Key` header → env `SDKWORK_LOAD_MEM0_API_KEY`
  (the mem0 SDK's native `Authorization: Token` form is bridged to `X-Api-Key` by the gateway)
- app face `/app/v3/api/*`: `Access-Token` header and/or `Authorization: Bearer` → env
  `SDKWORK_LOAD_APP_ACCESS_TOKEN` / `SDKWORK_LOAD_APP_BEARER_TOKEN`

Local run (example, all three scenarios, 120s each, 8 workers):

```bash
export BASE_URL="https://memory.example.test"
export SDKWORK_LOAD_OPEN_API_KEY="..."
export SDKWORK_LOAD_MEM0_API_KEY="..."
export SDKWORK_LOAD_APP_ACCESS_TOKEN="..."
pnpm perf:soak --scenarios retrieval,list-cursor,mem0-batch \
  --duration-secs 120 --concurrency 8 --json
```

CI: the same command, with credentials injected from the secret store; never print or
commit tokens. The driver is fail-closed — missing `BASE_URL` or a missing per-face
credential exits with code 2 and runs nothing.

Scenario notes:

- `retrieval` — POST `/mem/v3/api/memory/retrievals` with random queries from a seeded
  word pool (seed via `SDKWORK_LOAD_SEED` for a reproducible mix).
- `list-cursor` — GET `/app/v3/api/memory/memories?page_size=200`, following
  `data.pageInfo.nextCursor` to exhaustion; a dead-loop guard caps pages
  (`SDKWORK_LOAD_MAX_PAGES`) and rejects a repeated cursor.
- `mem0-batch` — PUT `/v1/batch/` with batches of 50 (`SDKWORK_LOAD_BATCH_SIZE`).
  By default it synthesizes placeholder `memory_id`s to measure the batched write
  path; point `SDKWORK_LOAD_MEM0_ID_POOL` at real ids to measure update semantics
  against seeded memories.

## Result file naming

One run = one JSON file, written by `--json`:

```
docs/engineering/evidence/load-soak-<YYYYMMDDTHHMMSSZ>.json
```

The timestamp is UTC (`generatedAtUtc` inside the file matches the file name).
Do not edit an archived JSON after the fact; correct data requires a new run.

## Admission gates

A run counts as evidence only if the driver exits 0, which enforces:

1. `5xx = 0` for every enabled scenario (any server error fails the run).
2. p99 latency ≤ budget for every enabled soak scenario (budget
   `SDKWORK_LOAD_P99_BUDGET_MS`, default 200 ms).
3. Cursor walk terminates: the walk reaches exhaustion (2xx page with null
   `nextCursor`) without hitting the page cap or a repeated-cursor dead loop.
4. Every enabled scenario recorded at least one 2xx (a run against a dead or
   mis-authenticated target is not evidence).

Exit code 1 means a gate failed; exit code 2 means usage/configuration error.

## What counts as a recorded load test

有记录的压测 = 一次归档 JSON + 环境描述。Each archived JSON must be accompanied by an
environment description (committed next to it, or embedded in the PR description that
adds it) stating at minimum:

- deployment shape: replica count, instance size, gateway/topology profile
  (standalone/cloud), database engine and size;
- connection pool capacity of the service under test;
- data scale: memory/space counts seeded before the run;
- driver parameters: `durationSecs`, `concurrency`, `p99BudgetMs`, scenario list, seed;
- what was NOT covered (e.g. placeholder mem0 ids, no upstream LLM/embedding provider
  traffic), so readers do not over-read the numbers.

Suggested companion file name: `load-soak-<same timestamp>.environment.md`.
