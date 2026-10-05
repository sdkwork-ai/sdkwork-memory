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

## Standards Routing For API Envelope Work

HTTP API response envelope rules are owned by the shared specs, not this
repository. For any API contract, SDK generation, or frontend service work,
follow `../sdkwork-specs/API_SPEC.md` section 4.5 (wire classification and
vendor `open-api` exemptions) and sections 14-16 (list/search/command input,
`SdkWorkApiResponse`, `ProblemDetail`, operation patterns), plus
`../sdkwork-specs/SDK_SPEC.md` section 4.2 for generated SDK unwrapping.
Before completing that work run:

```bash
node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace <workspace-root>
node <sdkwork-specs>/tools/check-api-response-envelope.mjs --workspace <workspace-root>
```

## Human Review Rules

Human review is required for breaking public API changes, privacy/security exceptions, generated SDK ownership changes, production IAM policy changes, and destructive filesystem or data operations.
