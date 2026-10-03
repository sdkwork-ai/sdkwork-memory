# SDKWork Memory PC Application

<!-- SDKWORK-AGENTS-GENERATED: v2 -->

## SDKWORK Soul

Read `../../../sdkwork-specs/SOUL.md` before executing tasks in this root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Canonical SDKWork standards from this application root:

- `../../../sdkwork-specs/README.md`
- `../../../sdkwork-specs/SOUL.md`
- `../../../sdkwork-specs/AGENTS_SPEC.md`
- `../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md`
- `../../../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`
- `../../../sdkwork-specs/CODE_STYLE_SPEC.md`
- `../../../sdkwork-specs/NAMING_SPEC.md`
- `../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md`

Do not copy global spec bodies into this application root. If these relative paths do not resolve, stop and report the broken workspace layout.

## Application Identity

Read `sdkwork.app.config.json` when work touches the PC application's behavior, SDK wiring, release metadata, app-owned capabilities, packaging, or deployment. Read `etc/` for concrete browser runtime and deployment configuration; the application manifest is not runtime configuration authority.

## Local Dictionary Structure

- `AGENTS.md`: application agent entrypoint.
- `sdkwork.app.config.json`: PC application identity, release, and capability metadata.
- `etc/`: deployable-root source configuration and browser runtime templates.
- `specs/`: PC application contract and narrowing rules.
- `packages/`: infrastructure, shell, Console capability, and Admin capability packages.
- `src/`: composition root, IAM boundary, and lazy surface entrypoints.
- `tests/`: runtime, SDK-boundary, pagination, and visual verification assets.
- `.sdkwork/`: application-local AI workspace metadata.

## Spec Resolution Order

Use dynamic progressive loading: read this file and the local dictionary first, then `sdkwork.app.config.json` or local component specs only when the task touches them, then task-specific files from `../../../sdkwork-specs/README.md`, and only then implementation files. Language-specific specs are on-demand; do not load unrelated Rust, Java, native, or mobile standards for PC React work.

## Required Specs By Task Type

- PC architecture and package changes: `../../../sdkwork-specs/APP_PC_ARCHITECTURE_SPEC.md`, `../../../sdkwork-specs/APP_PC_REACT_UI_SPEC.md`, and `../../../sdkwork-specs/MODULE_SPEC.md`.
- Console SDK changes: `../../../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md` and `../../../sdkwork-specs/SDK_SPEC.md`.
- Admin SDK changes: `../../../sdkwork-specs/SDK_SPEC.md`, `../../../sdkwork-specs/BACKEND_UI_SPEC.md`, and the backend SDK integration skill.
- Frontend code: `../../../sdkwork-specs/FRONTEND_CODE_SPEC.md`, `../../../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md`, `../../../sdkwork-specs/I18N_SPEC.md`, and `../../../sdkwork-specs/TEST_SPEC.md`.
- Source config and deployment: `../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md`, `../../../sdkwork-specs/CONFIG_SPEC.md`, `../../../sdkwork-specs/ENVIRONMENT_SPEC.md`, and `../../../sdkwork-specs/DEPLOYMENT_SPEC.md`.
- Package scripts and workflows: `../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md`, `../../../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md`, and `../../../sdkwork-specs/TEST_SPEC.md`.

## Code Style Rules

Read `../../../sdkwork-specs/CODE_STYLE_SPEC.md` and `../../../sdkwork-specs/NAMING_SPEC.md` before code changes. Generated SDK output must not be hand-edited. Console packages consume only `@sdkwork/memory-app-sdk` through Console core; Admin packages consume only `@sdkwork/memory-backend-sdk` through Admin core. Do not add raw HTTP, manual auth headers, local SDK forks, or cross-surface business imports.

## Build, Test, and Verification

Use the application package scripts and root SDKWork validators:

```powershell
pnpm --dir apps/sdkwork-memory-pc check
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
node ../sdkwork-specs/tools/check-source-config-standard.mjs --root apps/sdkwork-memory-pc
```

## Agent Execution Rules

Fail closed when runtime configuration, IAM state, SDK authority, route permission hints, or surface ownership is ambiguous. The server remains the authorization authority; frontend permission hints are navigation and visibility aids only. Preserve lazy loading so the Console entry path does not load Backend SDK code.

## Task-Specific Standards

API work loads `../../../sdkwork-specs/API_SPEC.md`; list/search work loads `../../../sdkwork-specs/PAGINATION_SPEC.md`; SDK consumer work loads `../../../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md` and `../../../sdkwork-specs/SDK_SPEC.md`; source configuration work loads `../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md`. Link the authority and validator instead of copying normative bodies here.

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

Human review is required for breaking public API changes, privacy/security exceptions, generated SDK ownership changes, production IAM policy changes, and destructive filesystem or data operations.
