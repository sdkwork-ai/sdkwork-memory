# SDKWork Memory Technical Architecture

Status: active current-state Canon

Owner: SDKWork Memory maintainers

Updated: 2026-09-30

Specs: `ARCHITECTURE_DECISION_SPEC.md`, `API_SPEC.md`, `SDK_SPEC.md`, `DATABASE_SPEC.md`, `SECURITY_SPEC.md`, `DEPLOYMENT_SPEC.md`

## Authority

This document describes the implemented architecture. Machine-readable authority remains in `specs/component.spec.json`, authority OpenAPI files, route manifests, database contracts, `sdkwork.app.config.json`, and `sdkwork.workflow.json`. Dated `TECH-*` documents are archived design records unless this file links one as an active ADR.

## Historical Design Shards

These records preserve earlier designs and implementation evidence. They are reference material, not current capability or release authority:

- [AI Memory architecture design](TECH-2026-06-10-ai-memory-architecture-design.md)
- [Commercial memory management design](TECH-2026-06-10-commercial-memory-management-design.md)
- [Memory implementation family baseline](TECH-2026-06-10-memory-implementation-family-baseline.md)
- [Memory Open API and no-embedding MVP](TECH-2026-06-10-memory-open-api-and-no-embedding-mvp.md)
- [Memory SPI plugin architecture](TECH-2026-06-10-memory-spi-plugin-architecture-design.md)
- [Memory SPI plugin runtime plan](TECH-2026-06-10-memory-spi-plugin-runtime-implementation-plan.md)
- [Commercial retrieval hardening](TECH-2026-07-20-memory-commercial-retrieval-hardening.md)
- [Topology standard redirect](TECH-topology-standard.md)

## Runtime Shape

SDKWork Memory is a Rust service and React PC application sharing one product authority:

```text
Console -> Memory App SDK -----> App API -----+
                                              |
Admin   -> Memory Backend SDK -> Backend API --+-> service use cases -> SPI/store ports -> SQL/provider adapters
                                              |
External integrations --------> Open API -----+
```

The public deployment axis contains only `standalone` and `cloud`. Development and production configuration are selected from `etc/`; internal process layout is not a public profile.

## Ownership Boundaries

| Layer | Ownership |
| --- | --- |
| `sdkwork-memory-contract` | DTOs, App/Backend/Open service ports, typed errors, pagination data |
| `sdkwork-intelligence-memory-service` | Authorization-aware use cases, learning/retrieval/governance orchestration |
| `sdkwork-memory-spi` | Provider-neutral ports, plugin manifests, registry contracts |
| `sdkwork-memory-plugin-native-sql` | PostgreSQL/SQLite persistence and store-level pagination; SQLite fixtures and the PostgreSQL baseline are cross-checked by the dialect parity gate |
| `sdkwork-memory-retrieval` | Retrieval and context composition algorithms |
| route crates | Thin axum adapters and SDKWork response mapping for each authority surface |
| standalone gateway | SDKWork web bootstrap, context injection, route assembly, health and metrics |
| `apps/sdkwork-memory-pc` | Browser host, Console/Admin shells, feature packages, IAM session integration |
| generated SDK families | Materialized clients from authority OpenAPI; never hand-edited |

## API And SDK Boundaries

| Surface | Prefix | Consumer | Tenant authority |
| --- | --- | --- | --- |
| Open | `/mem/v3/api` | `@sdkwork/memory-sdk` and approved integrators | API-key/request context |
| App | `/app/v3/api/memory` | Console and application features through `@sdkwork/memory-app-sdk` | authenticated App context |
| Backend | `/backend/v3/api/memory` | Admin only through `@sdkwork/memory-backend-sdk` | authenticated operator context |

Rules:

- Request schemas do not expose client-writable `tenantId` for current-tenant operations.
- Routes inject tenant and actor/operator identity from `Memory*RequestContext`.
- Every operation declares exactly one kebab-case `x-sdkwork-api-authority` identity (`sdkwork-memory-open-api`, `sdkwork-memory-app-api`, `sdkwork-memory-backend-api`); dotted aliases are gone and the grammar is machine-gated (`tests/contracts/api_authority_identity_test.mjs`).
- Generated transport packages are private implementation details; composed SDK packages are the consumer boundary.
- Lists return `data.items` and `data.pageInfo`. Cursors are opaque HMAC-protected tokens (`SDKWORK_MEMORY_CURSOR_SIGNING_KEY`; production startup fails closed when it is unset); forged or stale tokens fail as invalid parameters. A cursor whose backing keyset row was purged — for example by retention — resolves to a fail-closed empty page on every numeric-keyset surface, never a replayed window (text-cursor lists such as audit history replay from the head instead — an ordering shift, never a data leak). SQL-backed histories constrain tenant, scope/type, cursor, and `LIMIT` in the query, with matching composite indexes.
- Success serialization is delegated to SDKWork web-framework response helpers; every failure path — including malformed JSON bodies, malformed path ids, handler panics, and 5xx — renders `application/problem+json` with a numeric `code` and `traceId`.
- OpenAPI body schemas are machine-gated against the contract DTOs (`tests/contracts/openapi_body_schema_parity_test.mjs`), and the two engine DDLs are machine-gated for parity (`tools/check-database-dialect-parity.mjs`).
- `POST /memories/delete_all` returns a count-only receipt; the deleted id list never travels back through the API.

## PC Package Architecture

| Package family | Responsibility | Allowed HTTP SDK |
| --- | --- | --- |
| `memory-pc-core` | Host composition, runtime config, auth route registration, console SDK client construction (the one composition-root `@sdkwork/memory-app-sdk` import) | App SDK only (composition root) |
| `memory-pc-commons` | Shared page shell, pagination, typed action controls, safe error rendering, i18n | none |
| `memory-pc-console-core` | Console App SDK provider and resource registry | App SDK only |
| `memory-pc-console-*` | Customer/user modules and route metadata | App SDK through Console core |
| `memory-pc-console-shell` | Console navigation and permission hints | none directly |
| `memory-pc-admin-core` | Admin Backend SDK provider and resource registry | Backend SDK only |
| `memory-pc-admin-*` | Internal operation modules and route metadata | Backend SDK through Admin core |
| `memory-pc-admin-shell` | Admin navigation and permission hints | none directly |

Both surfaces reuse visual primitives but do not share SDK clients, session context, or permission declarations. Lazy Admin loading prevents Backend SDK code from entering the initial Console execution path.

## Data And Job Model

- Canonical evidence is stored in `ai_event`, `ai_record`, and `ai_record_source`.
- Learning jobs use `ai_learning_job`; extraction history is keyset-paginated by tenant, type, optional space, and stable row id.
- Extraction retries are idempotent: extracted candidates link back to their originating job through `ai_candidate.learning_job_uuid` (SQLite fixture migration 0017, PostgreSQL delta 0004), so a requeued run cannot duplicate candidates, and a run whose model refuses every input completes as success with an explicit zero-candidate note instead of retrying forever.
- Outbox, learning, and evaluation workers use persisted owner/token/expiry leases. Heartbeats extend current leases, and stale completion is fenced at the SQL update. Stale requeues increment `attempt_count`; rows past the configured ceiling land in a terminal `dead` state instead of looping, a panicking job isolates to its own task instead of aborting its batch, and dead transitions are logged and exported as `memory_*_dead_total` metrics. Learning execution errors requeue with attempt-aware backoff (`next_attempt_at`) instead of failing terminally; the outbox stale sweep charges the same retry budget as delivery failures and dead-letters past the ceiling, and the SPI `mark_outbox_failed` port takes the same `max_retries` ceiling so first-failure and delivery-failure paths dead-letter identically. Every embedded worker task holds a keepalive clone of the shutdown sender, so dropping the host's handle parks the workers on their receive arm instead of busy-spinning on a closed channel.
- Eval runs merge their stored configuration into the persisted result on completion (the result never overwrites it) and carry an optimistic `ai_eval_run.version` that every state transition increments.
- Tenant quotas resolve per tenant: environment defaults are overridden key-by-key by the active tenant-scoped `memory.quota` policy (`ai_policy.policy_json`), resolved through a bounded process cache whose key includes the environment defaults, so a retuned environment never serves a stale composition; invalid policy JSON fails closed. Every journaled canonical mutation, accepted event ingest, and stored retrieval trace charges a per-tenant, per-UTC-day usage fact in `ai_usage_daily` (SQLite 0018 mirrors PostgreSQL delta 0005), and the Backend surface exposes the facts through the keyset-paginated `usage.list` operation.
- An accepted event commits its event row and its audit line in one transaction, so evidence can never outlive its audit record.
- Provider health treats a binding with no endpoint as `unknown` — counted separately, never as healthy — aggregates at most `SDKWORK_MEMORY_PROVIDER_HEALTH_MAX_BINDINGS` bindings (default 500), answers past that cap with a `bindingsTruncated` degraded summary instead of an error, and reports per-state totals in `stateCounts`.
- A retention worker hard-deletes terminal and derived high-churn rows (terminal outbox events, terminal learning/eval jobs, retrieval traces with their context packs, audit logs) in bounded keyset batches on per-table configurable windows (`SDKWORK_MEMORY_RETENTION_*`). Canonical records and events stay under product semantics (space retention jobs, forget), not mechanical cleanup. The sweep is lease-admitted (`pg_try_advisory_lock` on a dedicated connection): exactly one replica sweeps per window, and the rest skip their tick.
- Outbox acknowledge/fail port methods carry the lease triple and fence at the SQL UPDATE; the unfenced mutation surface was removed.
- Forget, export, consolidation, retention, and migration jobs persist typed snapshots in `ai_audit_log`; App history also constrains the authenticated actor in SQL. A forget run persists its running audit row (`resource_type="forget_job_progress"`, `action="forget.request.start"`, uuid `{job_id}:start`) before scanning, so an interrupted run always leaves discoverable governance evidence, and a scope with zero hits completes as `succeeded`. Space- and user-scoped forget sweeps delete events in bounded 500-row batches (evidence sources before their events, per batch) and reject forget candidates in the same 500-row batches with keyset-enumerated matching queries, so a large scope never pins its rows for one unbounded statement.
- Entities, edges, policies, subjects, bindings, capability bindings, assignments, feedback signals (`ai_feedback`), and readiness snapshots use dedicated `ai_` tables with bounded-enum CHECK constraints on state columns. Entity-link reads return at most 1,000 rows, commercial list paths clamp page size, and metadata filters bound condition nodes (256) and per-set values (1,024); a non-numeric ordering bound fails closed identically on both SQL dialects.
- Search indexes and provider projections are derived and rebuildable. Canonical relational data remains authoritative.
- Outbox writes are part of mutation boundaries where domain event delivery is required.
- PostgreSQL and SQLite share one logical storage model through `sqlx::Any`: application-generated Snowflake IDs, validated JSON/UTC instants stored as text, and floating algorithm scores stored as `DOUBLE PRECISION`/`REAL`. Both bootstrap paths assert the deployment floor before touching the schema: the data plane and the root database lifecycle each reject a PostgreSQL server older than 15 (`NULLS NOT DISTINCT` baseline requirement) with a named diagnostic. The plugin's embedded bootstrap remains a test/local-tool path only, and it replays the PostgreSQL baseline plus every post-baseline delta (0001-0005) so a locally bootstrapped PostgreSQL carries the same schema as the application-root lifecycle.
- The canonical `create_record` SPI path performs its FTS projection inside the record's own transaction, so a crash cannot leave the search index diverging from the row.

## Security And Privacy

- App and Backend APIs require SDKWork IAM context; Open API uses its declared credential mode.
- Every store operation includes tenant and required space/actor predicates before materialization.
- Restricted sensitivity access fails closed. Provider calls receive only the authorized projection.
- Forget workflows physically remove targeted canonical and derived data according to scope and record an auditable result.
- Export applies sensitivity filtering and uses the approved Drive uploader for Drive targets.
- Export collection is keyset-paginated and byte-bounded, and the payload serializes exactly once through a size-guarded writer that aborts at the cap. Inline export defaults to 4 MiB under a 32 MiB absolute cap, and Drive export defaults to 64 MiB under the 256 MiB absolute cap. The current Drive SPI is a bounded single-buffer upload, not streaming multipart.
- Outbound provider and Outbox HTTP clients validate every resolved address — including IPv6 transition forms (NAT64, 6to4, Teredo) that embed an IPv4 endpoint — reject non-public or mixed DNS answers, pin validated addresses, disable redirects, and buffer at most 16 MiB of any response body.
- Retrieval traces persist best-effort: a trace write failure never fails the retrieval that already produced results.
- `threshold` gates the semantic score before fusion under the additive strategy, and the normalized fused score after fusion under RRF strategies; under the additive strategy an unavailable embedder (not bound or bind failed) drops the semantic threshold to 0 and degrades to pure lexical ranking with a `degraded` marker and a `semantic_threshold_relaxed` degradation code, never an empty result.
- Capability bindings enforce `subject` targets at runtime; `binding` and `memory` targets have no defined runtime semantics and are rejected by name when a binding is created, so no stored binding can silently fall outside authorization.
- CJK keyword matching tokenizes runs into adjacent character pairs (bigrams), so shared single characters alone do not earn relevance.
- Unknown retrieval profile ids fail as not-found instead of silently ranking with deployment defaults.
- `ProblemDetail` exposes numeric code and server trace id; a caller-supplied `X-Request-Id` is never echoed as the trace id, and the PC never displays raw response bodies, tokens, or headers.
- Cursor tokens are verified with a constant-time MAC comparison, and production startup rejects a `SDKWORK_MEMORY_CURSOR_SIGNING_KEY` shorter than 32 bytes as well as an unset one.
- Production rate limiting additionally charges a shared pre-auth aggregate bucket per path+tier (`pre_auth_aggregate_multiplier = 20`) so rotating credential kinds cannot multiply the pre-auth budget; legitimate keys resolve to their tenant bucket after auth and are unaffected.
- Extraction runs bound their prompt: input events are covered in request order until `SDKWORK_MEMORY_EXTRACTION_MAX_INPUT_BYTES` (default 2 MiB) or `SDKWORK_MEMORY_EXTRACTION_MAX_EVENTS` (default 1,000) is exhausted, and the result reports the remainder as `skippedEventCount` instead of silently truncating.
- Retrieval requests bound their inputs: a query longer than 8,192 characters is rejected as invalid parameters, `contextBudgetTokens` is capped at 100,000, and each batched embeddings request carries at most `SDKWORK_MEMORY_EMBED_BATCH_MAX_BYTES` (default 2 MiB, clamped to 64 KiB-16 MiB) of input text, with a single oversized text truncated rather than rejected.
- Governance reads and reviews surface stored evidence: a single retrieval-trace read returns its real stored `created_at` and `space_id` (never read-time placeholders), candidate approve/reject persist the typed review reason, the audit log list exposes `metadata`, `traceId`, and `reason`, App-side policy assignments validate their target before storage (a space target requires write access; a subject target must be the caller), and App space listings include spaces shared through memory bindings.
- Exports admit through a process-level semaphore (`SDKWORK_MEMORY_EXPORT_MAX_CONCURRENCY`, default 2) so per-run byte caps cannot multiply across concurrent requests, and an unsupported export format is rejected before any payload is collected.
- Retrieval rehydrates its candidate pool with one bulk store read per space instead of one point query per candidate, so a full candidate pool cannot pin the database pool with hundreds of concurrent round trips.
- The mem0 compatibility surface refuses rather than ignores upstream-only behavior: the upstream request fields it cannot honor (extraction switches, immutability, graph/output toggles, per-request model profile, and the third `metadata` filter spelling on `delete_all`) are answered with a named 501, a memory history longer than the 200-entry bound is answered with a named 501 instead of a silently truncated prefix, `removeAll` sits in the `authCritical` rate-limit tier, and the batch surface authorizes the space once, pre-checks every target, and applies updates in bounded chunks that preserve per-record semantics. `top_k` and `threshold` carry authoritative schema bounds (1-100 and 0-1).
- Production PC artifacts exclude source maps and repository-private runtime state.

## Deployment And Release

| Profile | Purpose |
| --- | --- |
| `standalone.development` | local source runtime and local dependencies |
| `standalone.production` | self-contained production deployment |
| `cloud.development` | explicit remote development services |
| `cloud.production` | platform-managed production deployment |

The server publishes profile-bound runtime artifacts. The PC publishes a cloud browser ZIP with deterministic file order/timestamps, SHA-256, SPDX SBOM, provenance, and CI OIDC attestation. Publishing, deployment, and rollback remain separate workflow phases.

Production-like HTTP surfaces require shared Redis stores for rate limiting, idempotency, and concurrent admission, plus IAM database readiness. The gateway applies a bounded request deadline, body limit, and local concurrency ceiling; the infrastructure probes (`/healthz`, `/livez`, `/readyz`, `/metrics`) sit outside that ceiling so a saturated admission limit cannot hide a healthy process from the orchestrator, and `/readyz` fails closed when the Snowflake id-generator lease is unhealthy. `/metrics` exports `memory_write_request_duration_seconds` per write method and the `memory_rate_limit_rejections_total` counter alongside the shared runtime series. OTel head sampling follows `SDKWORK_TRACING_SAMPLE_RATIO` (default 1.0, clamped to 0.0-1.0). This repository remains a release candidate until immutable OCI/browser artifacts, signatures, attestations, deployment smoke tests, load evidence, and rollback records pass the release gates.

## Verification

```powershell
node tools/materialize_phase1_contracts.mjs
pnpm sdk:generate
cargo check --workspace
node scripts/cargo-test-workspace.mjs
pnpm --dir apps/sdkwork-memory-pc check
pnpm check
pnpm verify
```

Static standards and browser viewport checks are additional evidence, not replacements for these commands. `pnpm check` also runs `node tools/check-env-documentation.mjs`, which fails when an environment variable read under `crates/` or `plugins/` is missing from `.env.example` (or from the gate's reviewed allowlist of test-only variables).
