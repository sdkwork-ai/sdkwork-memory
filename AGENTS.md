# Repository Guidelines

<!-- SDKWORK-AGENTS-GENERATED: v2 -->

## SDKWORK Soul

Read `../sdkwork-specs/SOUL.md` before executing tasks in this root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Canonical SDKWORK specs path from this root:

- `../sdkwork-specs/README.md`
- `../sdkwork-specs/SOUL.md`
- `../sdkwork-specs/AGENTS_SPEC.md`
- `../sdkwork-specs/PNPM_SCRIPT_SPEC.md`
- `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`
- `../sdkwork-specs/CODE_STYLE_SPEC.md`
- `../sdkwork-specs/NAMING_SPEC.md`
- `../sdkwork-specs/SOURCE_CONFIG_SPEC.md`

Do not copy root standard text into this repository. If these relative paths do not resolve, stop and report the broken workspace layout.

## Documentation Canon

- [Memory product PRD](docs/product/prd/PRD.md)
- [Memory technical architecture](docs/architecture/tech/TECH_ARCHITECTURE.md)

## Application Identity

Read `sdkwork.app.config.json` only when the task touches Memory application behavior, runtime config, SDK wiring, release metadata, app-owned capabilities, packaging, or deployment. For unrelated documentation or tooling work, do not expand into the full app manifest unless evidence requires it.

## Local Dictionary Structure

- `AGENTS.md`: repository agent entrypoint and relative SDKWork spec index.
- `CLAUDE.md`, `GEMINI.md`, `CODEX.md`: compatibility shims that point to `AGENTS.md` and must not duplicate rules.
- `sdkwork.app.config.json`: Memory application identity, runtime, release, and capability metadata.
- `sdkwork.workflow.json`: GitHub packaging/release workflow manifest governed by `GITHUB_WORKFLOW_SPEC.md`.
- `.github/workflows/package.yml`: thin reusable workflow call only.
- `.sdkwork/`: repository/application AI workspace metadata, local skills, local plugins, and manifests.
- `specs/`: local application/component contracts and narrowing rules.
- `apis/`: Memory-owned API contract sources and materialized OpenAPI inputs.
- `apps/`: independently governed client application roots, including the Memory PC Console and Admin application.
- `crates/`: reusable Rust service, repository, route, and API server crates.
- `sdks/`: SDK families, SDK generation manifests, route manifests, and generated SDK artifacts.
- `database/`: database contract, baseline DDL, migrations, seeds, and drift policy.
- `etc/`, `deployments/`, `scripts/`, `tools/`, `docs/`, `tests/`: source-owned config templates, deployment descriptors, thin command entrypoints, validators, documentation, and verification assets.
- `package.json`, `Cargo.toml`: language/build manifests.

## Spec Resolution Order

Use dynamic progressive loading:

1. Read this `AGENTS.md` and any nearer component-level `AGENTS.md`.
2. Read `sdkwork.app.config.json` only when app behavior, runtime config, SDK wiring, release, packaging, or app-owned capabilities are touched.
3. Read local `specs/README.md` and `specs/component.spec.json` only when the task touches that local contract.
4. Read local `.sdkwork/README.md`, `.sdkwork/skills/`, and `.sdkwork/plugins/` only when local agent extensions are relevant.
5. Read `../sdkwork-specs/README.md`, then only the task-specific root specs.
6. Inspect implementation files after the dictionary and relevant specs are clear.

Do not load the whole repository or every root spec before identifying the task surface.

## Required Specs By Task Type

- Agent/workflow changes: `../sdkwork-specs/SOUL.md`, `../sdkwork-specs/AGENTS_SPEC.md`, `../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`, `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Package script changes: `../sdkwork-specs/PNPM_SCRIPT_SPEC.md`, `../sdkwork-specs/APP_RUNTIME_TOPOLOGY_SPEC.md`, `../sdkwork-specs/CONFIG_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Any code change: `../sdkwork-specs/CODE_STYLE_SPEC.md`, `../sdkwork-specs/NAMING_SPEC.md`, plus only the touched language/framework spec.
- Rust code: `../sdkwork-specs/RUST_CODE_SPEC.md`; add `../sdkwork-specs/RUST_RPC_SPEC.md` when RPC is touched.
- API/SDK changes: `../sdkwork-specs/API_SPEC.md`, `../sdkwork-specs/WEB_FRAMEWORK_SPEC.md`, `../sdkwork-specs/WEB_BACKEND_SPEC.md`, `../sdkwork-specs/SDK_SPEC.md`, `../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Database changes: `../sdkwork-specs/DATABASE_SPEC.md`, `../sdkwork-specs/DATABASE_FRAMEWORK_SPEC.md`, `../sdkwork-specs/PRIVACY_SPEC.md`, and `../sdkwork-specs/TEST_SPEC.md`.
- Runtime/deployment/release changes: `../sdkwork-specs/SOURCE_CONFIG_SPEC.md`, `../sdkwork-specs/CONFIG_SPEC.md`, `../sdkwork-specs/ENVIRONMENT_SPEC.md`, `../sdkwork-specs/DEPLOYMENT_SPEC.md`, `../sdkwork-specs/RELEASE_SPEC.md`, and `../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`.
- Provider/integration changes: `../sdkwork-specs/INTEGRATION_SPEC.md`, `../sdkwork-specs/SECURITY_SPEC.md`, and `../sdkwork-specs/PRIVACY_SPEC.md`.

Language-specific specs are on-demand; do not load Rust, Java, TypeScript, and frontend specs for unrelated tasks.

## Code Style Rules

Read `../sdkwork-specs/CODE_STYLE_SPEC.md` and `../sdkwork-specs/NAMING_SPEC.md` before code changes. Generated SDK output under `generated/server-openapi` must not be hand-edited. Fix OpenAPI, route manifests, generator input, or approved composed facades, then regenerate. Use `sdkwork-utils-rust` and `sdkwork-id-core` for shared helpers instead of duplicating utility logic locally.

## Build, Test, and Verification

Use canonical root package scripts from `PNPM_SCRIPT_SPEC.md`:

```powershell
pnpm verify
pnpm check
pnpm topology:validate
pnpm db:validate
```

## Agent Execution Rules

Do not rely on memory when a relevant SDKWork spec exists. Do not replace generated SDK calls with raw HTTP. Stop when the relative specs path, app identity, component spec, API authority, SDK family, table prefix, or provider ownership is ambiguous.

## Task-Specific Standards

API work loads `../sdkwork-specs/API_SPEC.md` and its validators. List/search work loads `../sdkwork-specs/PAGINATION_SPEC.md` and `check-pagination.mjs`. SDK consumer work loads `../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md` and `../sdkwork-specs/SDK_SPEC.md` for the touched surface. Source configuration work loads `../sdkwork-specs/SOURCE_CONFIG_SPEC.md` and `check-source-config-standard.mjs`. Link these authorities instead of copying their normative bodies into `AGENTS.md`.

## HTTP API Response Envelope

All L2+ SDKWork-owned custom HTTP contracts, including `app-api`, `backend-api`, and SDKWork-owned business `open-api`, `MUST` follow `API_SPEC.md` section 4.5, section 14, and section 15:

- **Default classification:** omitted `x-sdkwork-wire-protocol` means SDKWork-owned custom API (`sdkwork-v3`); only operation-level `x-sdkwork-wire-protocol: external` plus `x-sdkwork-external-protocol-id` identifies a third-party compatibility `open-api` operation.
- **Input:** typed request bodies, section 14.1 list/search/command input, `SdkWorkListQuery`, and `q` for free-text search.
- **Success output:** `SdkWorkApiResponse` with `{ "code": 0, "data": <payload>, "traceId": "<server-uuid>" }`.
- **Error output:** HTTP 4xx/5xx `application/problem+json` (`ProblemDetail`) with numeric `code` and `traceId`; SDKWork-owned errors may include `i18nKey` and `locale` presentation metadata.
- Success `code` is numeric `int32`; HTTP 2xx JSON bodies `MUST` use `0` only. REST semantics remain on HTTP status (`201`, `202`, etc.).
- Platform error codes are numeric non-zero values per section 15.3 (`40001`, `40101`, `40401`, …).
- Single resource: `data.item`
- Lists: `data.items` + `data.pageInfo` (`PageInfo.mode` is `offset` or `cursor`)
- Commands: `data.accepted` plus optional `resourceId` / `status`
- Async accept (`202`): `data.operationId`, `data.status`, optional `pollUrl`
- Operation patterns: retrieve/list/search/create/update/delete/command/async/bulk semantics follow `API_SPEC.md` section 15.4; create uses `201`, delete uses `204` with no JSON body, and `PUT`/`PATCH` use SDK action `update`.

Vendor compatibility `open-api` routes that mirror upstream tool or provider wire (for example OpenAI `/v1/*`, Anthropic/Claude `/anthropic/v1/*`, Google/Gemini `/google/v1beta/*`, Claude Code, or Codex) `MAY` opt out only when every exempt operation declares operation-level `x-sdkwork-wire-protocol: external` and `x-sdkwork-external-protocol-id` per `API_SPEC.md` section 4.5.2. SDKWork-owned business `open-api` operations `MUST NOT` opt out. Mixed OpenAPI documents are validated per operation; one external operation never exempts SDKWork-owned operations in the same document.

Errors `MUST` use HTTP 4xx/5xx with `application/problem+json` (`ProblemDetail`) including required numeric `code` and `traceId`. Optional `i18nKey` and `locale` are display metadata only. Business failures `MUST NOT` use HTTP 2xx with non-zero `code`, string wire codes, `success`, or human `message`.

Forbidden legacy envelopes and fields: `PlusApiResult`, `AppbaseApiResult`, `StoreApiResult`, `SdkWorkResponse`, per-domain `*ApiResult`, wire field `requestId`, bare domain DTOs at the HTTP root, and top-level `{ items, pageInfo, traceId }` without `data`.

Handlers `MUST` serialize success and map errors through `sdkwork-web-framework` response mapping. Generated HTTP SDKs (`--standard-profile sdkwork-v3`) unwrap `data` by default and expose typed numeric `ProblemDetail.code` / `traceId` and returned localization metadata on errors; use `.raw` when the full envelope is required.

Before completing API contract, SDK generation, or frontend service work, run:

```bash
node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace <workspace-root>
node <sdkwork-specs>/tools/check-api-response-envelope.mjs --workspace <workspace-root>
```

Authority: `sdkwork-specs/API_SPEC.md` section 4.5 and sections 14–16, `SDK_SPEC.md` section 4.2, `FRONTEND_SPEC.md`, `MIGRATION_SPEC.md` section 4.2.

## Human Review Rules

Human review is required for breaking public API changes, schema migrations, privacy/security exceptions, generated SDK ownership changes, provider lock-in decisions, and destructive filesystem or data operations.
