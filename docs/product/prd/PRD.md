# SDKWork Memory Product Requirements

Status: active current-state Canon

Owner: SDKWork Memory maintainers

Updated: 2026-09-30

Specs: `REQUIREMENTS_SPEC.md`, `DOCUMENTATION_SPEC.md`

## Product Intent

SDKWork Memory is the tenant-isolated memory capability for SDKWork applications. It stores evidence, canonical memories, learned candidates and habits, retrieval traces, knowledge graph data, policies, and governance history without requiring an embedding provider. Provider integrations remain replaceable through SDKWork SPI contracts.

The repository also owns the Memory PC application:

- Console serves customers, tenant owners, and end users managing memory within their authenticated tenant and subject scope.
- Admin serves authorized internal operations, support, security, audit, and platform-management roles.

## Audiences And Outcomes

| Audience | Required outcome |
| --- | --- |
| Application developer | Stable Open/App SDKs, typed errors, idempotent commands, and no provider lock-in |
| End user | Inspect, create, update, retrieve, export, and forget authorized memory data |
| Tenant owner | Govern spaces, learning behavior, knowledge entities, and policy assignments |
| Internal operator | Operate jobs, providers, indexes, evaluation, IAM bindings, audit, retention, and migration |
| SRE/security | Deploy both standard profiles, observe health, investigate traces, rotate credentials, and roll back safely |

## Current Capability Contract

| Capability | Current implementation authority |
| --- | --- |
| Spaces, events, records, sources | `ai_space`, `ai_event`, `ai_record`, `ai_record_source` plus contract/service/route crates |
| Learning | Candidate, habit, extraction, consolidation, and SQL-backed learning job contracts |
| Retrieval | Keyword and provider-backed retrieval, profiles, context packs, feedback, and traces |
| Knowledge | Tenant-scoped entities and edges with App/Backend SDK operations |
| Governance | Policies, assignments, audit logs, retention, migration, export, and forget workflows |
| Commercial control plane | Subjects, bindings, capability bindings, resolution, tenant quota policies, usage facts, and readiness snapshots |
| PC Console | `/console/*`; consumes only `@sdkwork/memory-app-sdk` |
| PC Admin | `/admin/*`; consumes only `@sdkwork/memory-backend-sdk` |
| Deployment | `standalone.development`, `standalone.production`, `cloud.development`, `cloud.production` |

Generated route manifests and authority OpenAPI files are the operation inventory; this document intentionally does not copy operation counts.

## Product Rules

- Authentication context is the tenant authority. Business request bodies and list query parameters never select the current tenant.
- Cross-tenant isolation is evidenced by an adversarial two-tenant matrix test in `crates/sdkwork-memory-integration-tests` (including foreign cursor-token replay), and purged-cursor replay is covered fail-closed at the store contract level; zero cross-tenant access is a gate, not an aspiration.
- Console cannot import or call Backend SDK operations. Admin cannot substitute raw HTTP for the composed Backend SDK.
- Interactive lists use server pagination. High-volume histories use store-level keyset windows and never download all rows for client slicing.
- SDKWork-owned HTTP success responses use the standard response envelope; errors use `application/problem+json` with numeric `code` and `traceId`.
- Memory mutations preserve evidence and audit semantics. Destructive commands require typed reasons where the domain requires a reason; ordinary `DELETE` operations use explicit confirmation and no fictitious request body. Bulk `delete_all` returns a count-only receipt — the deleted id list is never returned.
- Embeddings are optional. Native SQL retrieval remains operational when external providers are absent or degraded.
- Restricted and sensitive data access fails closed and is constrained before the store query or provider call.
- Exports use approved Drive integration when a Drive target is requested. Credentials and provider secrets are references, never repository data.
- Export memory is bounded: inline defaults to 4 MiB and never exceeds a 32 MiB absolute cap; Drive defaults to 64 MiB with the 256 MiB absolute cap, and neither path may exceed its cap until streaming multipart is implemented and verified.
- Cluster workers use database-fenced leases; an expired Outbox, learning, or evaluation worker cannot acknowledge or complete work after takeover. Learning execution errors requeue with attempt-aware backoff, and exhausted attempt budgets dead-letter with logged, metric-exported transitions. The retention sweep and provider-health probe are lease-admitted so exactly one replica runs each pass.
- A scheduled retention worker hard-deletes terminal outbox events, terminal learning/evaluation jobs, retrieval traces with their hits and context packs, and audit logs past per-table configurable windows, so high-churn tables stay bounded under production traffic.

## Quality And Operations Targets

| Signal | Target |
| --- | --- |
| Monthly API availability | 99.9% |
| p99 normal read latency | below 200 ms at the documented page bound |
| p99 normal write latency | below 500 ms excluding explicitly asynchronous work |
| Cross-tenant access | zero |
| API/SDK contract drift | zero unreviewed drift |
| Interactive list page size | default 20, maximum 200 |
| Privacy request traceability | every accepted request has tenant, actor, audit record, state, and timestamps |

## Release State

The application is an internal release candidate, not an active production release. Server/container and PC packaging definitions exist, and local contract evidence is generated, but publication remains blocked until the gateway has a validated process-shared database pool contract and bounded combined driver budget, CI produces immutable artifacts for the release commit, detached signatures/OIDC attestations, byte-bound SBOM and provenance, PostgreSQL and container smoke evidence, load/soak evidence, and a recorded rollback exercise. Candidate packages must not be represented as deployed or production-ready.

## Explicit Non-Goals

- Building a proprietary vector database.
- Replacing SDKWork IAM, Drive, secret management, or deployment frameworks.
- Runtime loading of unreviewed plugin binaries.
- Claiming multi-region active-active behavior without a separately approved architecture and operational evidence.
- Maintaining pre-launch legacy request fields, raw HTTP fallbacks, numeric cursor aliases, or duplicate API authorities.

## Acceptance Evidence

```powershell
pnpm check
pnpm verify
pnpm --dir apps/sdkwork-memory-pc check
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
```

Production publication additionally requires the release workflow, artifact attestation, immutable upload, deployment smoke test, and rollback record described in `docs/releases/README.md`.
