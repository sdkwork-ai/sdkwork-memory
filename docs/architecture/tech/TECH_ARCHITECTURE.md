# SDKWork Memory Technical Architecture

Status: active current-state Canon

Owner: SDKWork Memory maintainers

Updated: 2026-07-21

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
| `sdkwork-memory-plugin-native-sql` | PostgreSQL/SQLite persistence and store-level pagination |
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
- Generated transport packages are private implementation details; composed SDK packages are the consumer boundary.
- Lists return `data.items` and `data.pageInfo`. Cursors are opaque HMAC-protected tokens (`SDKWORK_MEMORY_CURSOR_SIGNING_KEY`); forged or stale tokens fail as invalid parameters. SQL-backed histories constrain tenant, scope/type, cursor, and `LIMIT` in the query, with matching composite indexes.
- Success and error serialization is delegated to SDKWork web-framework response helpers.

## PC Package Architecture

| Package family | Responsibility | Allowed HTTP SDK |
| --- | --- | --- |
| `memory-pc-core` | Host composition, runtime config, auth route registration | none directly |
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
- Outbox, learning, and evaluation workers use persisted owner/token/expiry leases. Heartbeats extend current leases, and stale completion is fenced at the SQL update. Stale requeues increment `attempt_count`; rows past the configured ceiling land in a terminal `dead` state instead of looping, and a panicking job isolates to its own task instead of aborting its batch.
- Forget, export, consolidation, retention, and migration jobs persist typed snapshots in `ai_audit_log`; App history also constrains the authenticated actor in SQL.
- Entities, edges, policies, subjects, bindings, capability bindings, assignments, feedback signals (`ai_feedback`), and readiness snapshots use dedicated `ai_` tables with bounded-enum CHECK constraints on state columns.
- Search indexes and provider projections are derived and rebuildable. Canonical relational data remains authoritative.
- Outbox writes are part of mutation boundaries where domain event delivery is required.
- PostgreSQL and SQLite share one logical storage model through `sqlx::Any`: application-generated Snowflake IDs, validated JSON/UTC instants stored as text, and floating algorithm scores stored as `DOUBLE PRECISION`/`REAL`.

## Security And Privacy

- App and Backend APIs require SDKWork IAM context; Open API uses its declared credential mode.
- Every store operation includes tenant and required space/actor predicates before materialization.
- Restricted sensitivity access fails closed. Provider calls receive only the authorized projection.
- Forget workflows physically remove targeted canonical and derived data according to scope and record an auditable result.
- Export applies sensitivity filtering and uses the approved Drive uploader for Drive targets.
- Export collection is keyset-paginated and byte-bounded. Inline export defaults to 4 MiB and Drive export to 64 MiB, with a 256 MiB absolute cap. The current Drive SPI is a bounded single-buffer upload, not streaming multipart.
- Outbound provider and Outbox HTTP clients validate every resolved address, reject non-public or mixed DNS answers, pin validated addresses, and disable redirects.
- `ProblemDetail` exposes numeric code and server trace id; the PC never displays raw response bodies, tokens, or headers.
- Production PC artifacts exclude source maps and repository-private runtime state.

## Deployment And Release

| Profile | Purpose |
| --- | --- |
| `standalone.development` | local source runtime and local dependencies |
| `standalone.production` | self-contained production deployment |
| `cloud.development` | explicit remote development services |
| `cloud.production` | platform-managed production deployment |

The server publishes profile-bound runtime artifacts. The PC publishes a cloud browser ZIP with deterministic file order/timestamps, SHA-256, SPDX SBOM, provenance, and CI OIDC attestation. Publishing, deployment, and rollback remain separate workflow phases.

Production-like HTTP surfaces require shared Redis stores for rate limiting, idempotency, and concurrent admission, plus IAM database readiness. The gateway applies a bounded request deadline, body limit, and local concurrency ceiling. This repository remains a release candidate until immutable OCI/browser artifacts, signatures, attestations, deployment smoke tests, load evidence, and rollback records pass the release gates.

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

Static standards and browser viewport checks are additional evidence, not replacements for these commands.
