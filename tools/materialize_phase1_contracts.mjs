import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { migrateOpenApiDocument } from "../../sdkwork-specs/tools/lib/migrate-openapi-legacy-envelope.mjs";

const root = process.cwd();
const owner = "sdkwork-memory";
const domain = "intelligence";
const capability = "memory";
const version = "0.1.0";
const standardVersion = "2026-06-10";
const memoryOpenApiPrefix = "/mem/v3/api";
const canonicalDesignDoc =
  "docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md";
const canonicalSpiDesignDoc =
  "docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md";
const legacyDesignStub =
  "docs/superpowers/specs/2026-06-10-ai-memory-architecture-design.md";
const legacySpiDesignStub =
  "docs/superpowers/specs/2026-06-10-memory-spi-plugin-architecture-design.md";

const AUTH_CRITICAL_OPERATION_IDS = new Set([
  "forgetRequests.create",
  "exportJobs.create",
  "memories.delete",
  "retentionJobs.create",
  "migrationJobs.create",
  "indexes.rebuild",
]);

function resolveRateLimitTier({ rateLimitTier, authMode, method, operationId }) {
  if (rateLimitTier) {
    return rateLimitTier;
  }
  if (AUTH_CRITICAL_OPERATION_IDS.has(operationId)) {
    return "authCritical";
  }
  if (authMode === "api-key" && ["post", "patch", "delete"].includes(method)) {
    return "openApiDefault";
  }
  return null;
}

function rateLimitTierRustSuffix(tier) {
  if (tier === "authCritical") {
    return ".with_rate_limit_tier(RateLimitTier::AuthCritical)";
  }
  if (tier === "openApiDefault") {
    return ".with_rate_limit_tier(RateLimitTier::OpenApiDefault)";
  }
  return "";
}

function writeText(relativePath, content) {
  const target = path.join(root, relativePath);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, content.replace(/\r\n/g, "\n"), "utf8");
  console.log(`wrote ${relativePath}`);
}

function writeJson(relativePath, value) {
  writeText(relativePath, `${JSON.stringify(value, null, 2)}\n`);
}

// Align the shared envelope ProblemDetail with the wire truth emitted by
// `sdkwork-utils-rust` `SdkWorkProblemDetail` (crates never send `errors[]` or
// `locale`, and `traceId` is the X-SdkWork-Trace-Id response-header value, not
// a UUID-typed field). The shared specs envelope library still injects the
// canonical schema, so the alignment runs after `migrateOpenApiDocument`.
function alignProblemDetailWithImplementation(document) {
  const schemas = document?.components?.schemas;
  const problem = schemas?.ProblemDetail;
  if (problem?.properties) {
    delete problem.properties.locale;
    delete problem.properties.errors;
    problem.properties.traceId = {
      type: "string",
      description:
        "Server-generated request correlation id. The same value is returned in the X-SdkWork-Trace-Id response header."
    };
  }
  const fieldErrorReferenced = JSON.stringify(schemas ?? {}).includes(
    '"#/components/schemas/FieldError"'
  );
  if (!fieldErrorReferenced) {
    delete schemas?.FieldError;
  }
}

function writeAlignedOpenApi(relativePath, document) {
  const aligned = migrateOpenApiDocument(document);
  alignProblemDetailWithImplementation(aligned);
  writeJson(relativePath, aligned);
}

function readJsonIfExists(relativePath) {
  const target = path.join(root, relativePath);
  if (!fs.existsSync(target)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(target, "utf8"));
}

function isPlaceholderChecksum(checksum) {
  if (checksum == null) {
    return true;
  }
  if (typeof checksum !== "string" || checksum.length < 16) {
    return true;
  }
  const sample = checksum.slice(0, 8);
  return (
    checksum === sample.repeat(Math.ceil(checksum.length / sample.length)).slice(0, checksum.length)
  );
}

function resolvePreservedReleaseChecksum(existingManifest) {
  const packages = existingManifest?.artifacts?.installConfig?.packages ?? [];
  for (const pkg of packages) {
    if (
      [
        "container-x64-server-docker-image",
        "container-x64-dual-container-docker-image",
        "container-x64-standalone-container-docker-image",
        "container-x64-cloud-container-docker-image",
      ].includes(pkg.id)
      && pkg.enabled !== false
      && pkg.metadata?.releaseEvidenceState === "verified-ci"
      && typeof pkg.url === "string"
      && pkg.url.startsWith("oci://")
      && !isPlaceholderChecksum(pkg.checksum)
    ) {
      return pkg.checksum;
    }
  }

  return null;
}

const packageByLanguage = {
  appbaseApp: {
    typescript: "@sdkwork/iam-app-sdk",
    rust: "sdkwork-iam-app-sdk",
    java: "com.sdkwork:sdkwork-iam-app-sdk",
    python: "sdkwork-iam-app-sdk",
    go: "github.com/sdkwork/sdkwork-iam-app-sdk"
  },
  appbaseBackend: {
    typescript: "@sdkwork/iam-backend-sdk",
    rust: "sdkwork-iam-backend-sdk",
    java: "com.sdkwork:sdkwork-iam-backend-sdk",
    python: "sdkwork-iam-backend-sdk",
    go: "github.com/sdkwork/sdkwork-iam-backend-sdk"
  },
  driveApp: {
    typescript: "@sdkwork/drive-app-sdk",
    rust: "sdkwork-drive-app-sdk",
    java: "com.sdkwork:sdkwork-drive-app-sdk",
    python: "sdkwork-drive-app-sdk",
    go: "github.com/sdkwork/sdkwork-drive-app-sdk"
  },
  driveBackend: {
    typescript: "@sdkwork/drive-backend-sdk",
    rust: "sdkwork-drive-backend-sdk",
    java: "com.sdkwork:sdkwork-drive-backend-sdk",
    python: "sdkwork-drive-backend-sdk",
    go: "github.com/sdkwork/sdkwork-drive-backend-sdk"
  },
  knowledgebaseApp: {
    typescript: "@sdkwork/knowledgebase-app-sdk",
    rust: "sdkwork-knowledgebase-app-sdk",
    java: "com.sdkwork:sdkwork-knowledgebase-app-sdk",
    python: "sdkwork-knowledgebase-app-sdk",
    go: "github.com/sdkwork/sdkwork-knowledgebase-app-sdk"
  },
  knowledgebaseBackend: {
    typescript: "@sdkwork/knowledgebase-backend-sdk",
    rust: "sdkwork-knowledgebase-backend-sdk",
    java: "com.sdkwork:sdkwork-knowledgebase-backend-sdk",
    python: "sdkwork-knowledgebase-backend-sdk",
    go: "github.com/sdkwork/sdkwork-knowledgebase-backend-sdk"
  }
};

const appSdkDependencies = [
  {
    workspace: "sdkwork-iam-app-sdk",
    role: "appbase-identity-and-session-capability",
    required: true,
    dependencyMode: "consumer-sdk",
    apiPrefix: "/app/v3/api",
    apiAuthority: "sdkwork-iam-app-api",
    generatedTransportImportPolicy: "forbidden",
    packageByLanguage: packageByLanguage.appbaseApp
  },
  {
    workspace: "sdkwork-drive-app-sdk",
    role: "drive-memory-export-import-capability",
    required: true,
    dependencyMode: "consumer-sdk",
    apiPrefix: "/app/v3/api",
    apiAuthority: "sdkwork-drive.app",
    generatedTransportImportPolicy: "forbidden",
    packageByLanguage: packageByLanguage.driveApp
  },
  {
    workspace: "sdkwork-knowledgebase-app-sdk",
    role: "knowledgebase-context-composition-capability",
    required: false,
    dependencyMode: "consumer-sdk",
    apiPrefix: "/app/v3/api",
    apiAuthority: "sdkwork-knowledgebase.app",
    generatedTransportImportPolicy: "forbidden",
    packageByLanguage: packageByLanguage.knowledgebaseApp
  }
];

const backendSdkDependencies = [
  {
    workspace: "sdkwork-iam-backend-sdk",
    role: "appbase-backend-management-capability",
    required: true,
    dependencyMode: "consumer-sdk",
    apiPrefix: "/backend/v3/api",
    apiAuthority: "sdkwork-iam-backend-api",
    generatedTransportImportPolicy: "forbidden",
    packageByLanguage: packageByLanguage.appbaseBackend
  },
  {
    workspace: "sdkwork-drive-backend-sdk",
    role: "drive-backend-memory-export-and-retention-capability",
    required: true,
    dependencyMode: "consumer-sdk",
    apiPrefix: "/backend/v3/api",
    apiAuthority: "sdkwork-drive.backend",
    generatedTransportImportPolicy: "forbidden",
    packageByLanguage: packageByLanguage.driveBackend
  },
  {
    workspace: "sdkwork-knowledgebase-backend-sdk",
    role: "knowledgebase-backend-index-and-eval-capability",
    required: false,
    dependencyMode: "consumer-sdk",
    apiPrefix: "/backend/v3/api",
    apiAuthority: "sdkwork-knowledgebase.backend",
    generatedTransportImportPolicy: "forbidden",
    packageByLanguage: packageByLanguage.knowledgebaseBackend
  }
];

const platformWorkspaceDependencies = [
  {
    workspace: "sdkwork-web-framework",
    role: "http-web-framework-runtime",
    required: true,
    dependencyMode: "platform-framework",
    generatedTransportImportPolicy: "forbidden"
  },
  {
    workspace: "sdkwork-database",
    role: "database-runtime",
    required: true,
    dependencyMode: "platform-framework",
    generatedTransportImportPolicy: "forbidden"
  },
  {
    workspace: "sdkwork-appbase",
    role: "appbase-platform-runtime",
    required: true,
    dependencyMode: "platform-framework",
    generatedTransportImportPolicy: "forbidden"
  },
  {
    workspace: "sdkwork-utils",
    role: "cross-language-utility-runtime",
    required: true,
    dependencyMode: "platform-framework",
    generatedTransportImportPolicy: "forbidden"
  },
  {
    workspace: "sdkwork-id",
    role: "id-generation-runtime",
    required: true,
    dependencyMode: "platform-framework",
    generatedTransportImportPolicy: "forbidden"
  },
  {
    workspace: "sdkwork-sdk-generator",
    role: "sdk-generation-tooling",
    required: true,
    dependencyMode: "platform-tooling",
    generatedTransportImportPolicy: "forbidden"
  }
];

const rootSdkDependencies = [
  ...platformWorkspaceDependencies,
  ...appSdkDependencies,
  ...backendSdkDependencies,
];

const rootCanonicalSpecs = [
  ["SOUL.md", "../sdkwork-specs/SOUL.md", "SDKWork execution soul."],
  ["COMPONENT_SPEC.md", "../sdkwork-specs/COMPONENT_SPEC.md", "Component-local contract and discovery rules."],
  ["NAMING_SPEC.md", "../sdkwork-specs/NAMING_SPEC.md", "Canonical naming for domains, APIs, SDKs, tables, and events."],
  ["DOMAIN_SPEC.md", "../sdkwork-specs/DOMAIN_SPEC.md", "Canonical domain and capability boundaries."],
  ["API_SPEC.md", "../sdkwork-specs/API_SPEC.md", "HTTP/OpenAPI and generated SDK contract rules."],
  ["SDK_SPEC.md", "../sdkwork-specs/SDK_SPEC.md", "SDK generation, SDK dependency, and SDK integration rules."],
  ["SDK_WORKSPACE_GENERATION_SPEC.md", "../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md", "SDK workspace and OpenAPI authority placement rules."],
  ["WEB_FRAMEWORK_SPEC.md", "../sdkwork-specs/WEB_FRAMEWORK_SPEC.md", "Mandatory sdkwork-web-framework integration for HTTP *-api surfaces."],
  ["WEB_BACKEND_SPEC.md", "../sdkwork-specs/WEB_BACKEND_SPEC.md", "Backend route, service, repository, provider, and request-context boundaries."],
  ["DEPLOYMENT_SPEC.md", "../sdkwork-specs/DEPLOYMENT_SPEC.md", "Standalone/cloud deployment profile parity and runtime packaging."],
  ["DATABASE_SPEC.md", "../sdkwork-specs/DATABASE_SPEC.md", "Schema registry, table contract, migration, and storage rules."],
  ["EVENT_SPEC.md", "../sdkwork-specs/EVENT_SPEC.md", "Domain event and outbox contract rules."],
  ["PRIVACY_SPEC.md", "../sdkwork-specs/PRIVACY_SPEC.md", "Memory privacy, sensitive data, retention, export, and deletion rules."],
  ["DRIVE_SPEC.md", "../sdkwork-specs/DRIVE_SPEC.md", "Drive-backed export upload and object store integration rules."],
  ["OBSERVABILITY_SPEC.md", "../sdkwork-specs/OBSERVABILITY_SPEC.md", "Request, retrieval, provider, job, audit, and evaluation observability rules."],
  ["TEST_SPEC.md", "../sdkwork-specs/TEST_SPEC.md", "Verification, contract testing, and evidence-before-completion rules."]
].map(([file, specPath, purpose]) => ({ file, path: specPath, purpose }));

const sdkCanonicalSpecs = [
  ["API_SPEC.md", "../sdkwork-specs/API_SPEC.md", "HTTP/OpenAPI and generated SDK contract rules."],
  ["SDK_SPEC.md", "../sdkwork-specs/SDK_SPEC.md", "SDK generation, SDK dependency, and SDK integration rules."],
  ["SDK_WORKSPACE_GENERATION_SPEC.md", "../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md", "SDK workspace, SDK family naming, API authority naming, and OpenAPI generation rules."],
  ["COMPONENT_SPEC.md", "../sdkwork-specs/COMPONENT_SPEC.md", "SDK family component spec and discovery rules."]
].map(([file, specPath, purpose]) => ({ file, path: specPath, purpose }));

function writeAgentEntrypoints() {
  writeText("AGENTS.md", `# Repository Guidelines

<!-- SDKWORK-AGENTS-GENERATED: v2 -->

## SDKWORK Soul

Read \`../sdkwork-specs/SOUL.md\` before executing tasks in this root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Canonical SDKWORK specs path from this root:

- \`../sdkwork-specs/README.md\`
- \`../sdkwork-specs/SOUL.md\`
- \`../sdkwork-specs/AGENTS_SPEC.md\`
- \`../sdkwork-specs/PNPM_SCRIPT_SPEC.md\`
- \`../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md\`
- \`../sdkwork-specs/CODE_STYLE_SPEC.md\`
- \`../sdkwork-specs/NAMING_SPEC.md\`
- \`../sdkwork-specs/SOURCE_CONFIG_SPEC.md\`

Do not copy root standard text into this repository. If these relative paths do not resolve, stop and report the broken workspace layout.

## Documentation Canon

- [Memory product PRD](docs/product/prd/PRD.md)
- [Memory technical architecture](docs/architecture/tech/TECH_ARCHITECTURE.md)

## Application Identity

Read \`sdkwork.app.config.json\` only when the task touches Memory application behavior, runtime config, SDK wiring, release metadata, app-owned capabilities, packaging, or deployment. For unrelated documentation or tooling work, do not expand into the full app manifest unless evidence requires it.

## Local Dictionary Structure

- \`AGENTS.md\`: repository agent entrypoint and relative SDKWork spec index.
- \`CLAUDE.md\`, \`GEMINI.md\`, \`CODEX.md\`: compatibility shims that point to \`AGENTS.md\` and must not duplicate rules.
- \`sdkwork.app.config.json\`: Memory application identity, runtime, release, and capability metadata.
- \`sdkwork.workflow.json\`: GitHub packaging/release workflow manifest governed by \`GITHUB_WORKFLOW_SPEC.md\`.
- \`.github/workflows/package.yml\`: thin reusable workflow call only.
- \`.sdkwork/\`: repository/application AI workspace metadata, local skills, local plugins, and manifests.
- \`specs/\`: local application/component contracts and narrowing rules.
- \`apis/\`: Memory-owned API contract sources and materialized OpenAPI inputs.
- \`apps/\`: independently governed client application roots, including the Memory PC Console and Admin application.
- \`crates/\`: reusable Rust service, repository, route, and API server crates.
- \`sdks/\`: SDK families, SDK generation manifests, route manifests, and generated SDK artifacts.
- \`database/\`: database contract, baseline DDL, migrations, seeds, and drift policy.
- \`etc/\`, \`deployments/\`, \`scripts/\`, \`tools/\`, \`docs/\`, \`tests/\`: source-owned config templates, deployment descriptors, thin command entrypoints, validators, documentation, and verification assets.
- \`package.json\`, \`Cargo.toml\`: language/build manifests.

## Spec Resolution Order

Use dynamic progressive loading:

1. Read this \`AGENTS.md\` and any nearer component-level \`AGENTS.md\`.
2. Read \`sdkwork.app.config.json\` only when app behavior, runtime config, SDK wiring, release, packaging, or app-owned capabilities are touched.
3. Read local \`specs/README.md\` and \`specs/component.spec.json\` only when the task touches that local contract.
4. Read local \`.sdkwork/README.md\`, \`.sdkwork/skills/\`, and \`.sdkwork/plugins/\` only when local agent extensions are relevant.
5. Read \`../sdkwork-specs/README.md\`, then only the task-specific root specs.
6. Inspect implementation files after the dictionary and relevant specs are clear.

Do not load the whole repository or every root spec before identifying the task surface.

## Required Specs By Task Type

- Agent/workflow changes: \`../sdkwork-specs/SOUL.md\`, \`../sdkwork-specs/AGENTS_SPEC.md\`, \`../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md\`, \`../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md\`, and \`../sdkwork-specs/TEST_SPEC.md\`.
- Package script changes: \`../sdkwork-specs/PNPM_SCRIPT_SPEC.md\`, \`../sdkwork-specs/APP_RUNTIME_TOPOLOGY_SPEC.md\`, \`../sdkwork-specs/CONFIG_SPEC.md\`, and \`../sdkwork-specs/TEST_SPEC.md\`.
- Any code change: \`../sdkwork-specs/CODE_STYLE_SPEC.md\`, \`../sdkwork-specs/NAMING_SPEC.md\`, plus only the touched language/framework spec.
- Rust code: \`../sdkwork-specs/RUST_CODE_SPEC.md\`; add \`../sdkwork-specs/RUST_RPC_SPEC.md\` when RPC is touched.
- API/SDK changes: \`../sdkwork-specs/API_SPEC.md\`, \`../sdkwork-specs/WEB_FRAMEWORK_SPEC.md\`, \`../sdkwork-specs/WEB_BACKEND_SPEC.md\`, \`../sdkwork-specs/SDK_SPEC.md\`, \`../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md\`, and \`../sdkwork-specs/TEST_SPEC.md\`.
- Database changes: \`../sdkwork-specs/DATABASE_SPEC.md\`, \`../sdkwork-specs/DATABASE_FRAMEWORK_SPEC.md\`, \`../sdkwork-specs/PRIVACY_SPEC.md\`, and \`../sdkwork-specs/TEST_SPEC.md\`.
- Runtime/deployment/release changes: \`../sdkwork-specs/SOURCE_CONFIG_SPEC.md\`, \`../sdkwork-specs/CONFIG_SPEC.md\`, \`../sdkwork-specs/ENVIRONMENT_SPEC.md\`, \`../sdkwork-specs/DEPLOYMENT_SPEC.md\`, \`../sdkwork-specs/RELEASE_SPEC.md\`, and \`../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md\`.
- Provider/integration changes: \`../sdkwork-specs/INTEGRATION_SPEC.md\`, \`../sdkwork-specs/SECURITY_SPEC.md\`, and \`../sdkwork-specs/PRIVACY_SPEC.md\`.

Language-specific specs are on-demand; do not load Rust, Java, TypeScript, and frontend specs for unrelated tasks.

## Code Style Rules

Read \`../sdkwork-specs/CODE_STYLE_SPEC.md\` and \`../sdkwork-specs/NAMING_SPEC.md\` before code changes. Generated SDK output under \`generated/server-openapi\` must not be hand-edited. Fix OpenAPI, route manifests, generator input, or approved composed facades, then regenerate. Use \`sdkwork-utils-rust\` and \`sdkwork-id-core\` for shared helpers instead of duplicating utility logic locally.

## Build, Test, and Verification

Use canonical root package scripts from \`PNPM_SCRIPT_SPEC.md\`:

\`\`\`powershell
pnpm verify
pnpm check
pnpm topology:validate
pnpm db:validate
\`\`\`

## Agent Execution Rules

Do not rely on memory when a relevant SDKWork spec exists. Do not replace generated SDK calls with raw HTTP. Stop when the relative specs path, app identity, component spec, API authority, SDK family, table prefix, or provider ownership is ambiguous.

## Task-Specific Standards

API work loads \`../sdkwork-specs/API_SPEC.md\` and its validators. List/search work loads \`../sdkwork-specs/PAGINATION_SPEC.md\` and \`check-pagination.mjs\`. SDK consumer work loads \`../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md\` and \`../sdkwork-specs/SDK_SPEC.md\` for the touched surface. Source configuration work loads \`../sdkwork-specs/SOURCE_CONFIG_SPEC.md\` and \`check-source-config-standard.mjs\`. Link these authorities instead of copying their normative bodies into \`AGENTS.md\`.

## Human Review Rules

Human review is required for breaking public API changes, schema migrations, privacy/security exceptions, generated SDK ownership changes, provider lock-in decisions, and destructive filesystem or data operations.
`);

  const shim = (toolName) => `# ${toolName} Entry

This file is a compatibility shim for ${toolName}. The authoritative SDKWork agent entrypoint is \`AGENTS.md\`.

Read in this order:

1. \`AGENTS.md\`
2. \`../sdkwork-specs/SOUL.md\`
3. \`../sdkwork-specs/AGENTS_SPEC.md\`
4. Task-specific files from \`../sdkwork-specs/README.md\`

Do not duplicate or override SDKWork rules here.
`;
  writeText("CODEX.md", shim("Codex"));
  writeText("CLAUDE.md", shim("Claude Code"));
  writeText("GEMINI.md", shim("Gemini CLI"));

  writeText(".sdkwork/README.md", `# SDKWork Memory Workspace Metadata

This directory is the source-controlled SDKWork workspace metadata root for \`sdkwork-memory\`.

It is distinct from generated SDK output \`.sdkwork/\` directories under \`generated/server-openapi\`.
Generated output must not store repository skills, plugins, runtime files, databases, logs, caches, or secrets.

Canonical standards:

- \`../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md\`
- \`../sdkwork-specs/AGENTS_SPEC.md\`
- \`../sdkwork-specs/COMPONENT_SPEC.md\`
`);
  writeText(".sdkwork/skills/README.md", `# SDKWork Memory Local Skills

No repository-local skills are defined yet.

Add local skills only when they encode repeatable Memory-specific workflows that should live with this repository. Do not copy root SDKWork standards into skills.
`);
  writeText(".sdkwork/plugins/README.md", `# SDKWork Memory Local Plugins

No repository-local plugins are defined yet.

Add local plugins only when this repository needs checked-in plugin metadata. Do not place generated SDK control-plane files here.
`);
}

function writeAppManifest() {
  const existingManifest = readJsonIfExists("sdkwork.app.config.json");
  // Materialization must never promote a candidate based only on a previously
  // observed checksum. Production publication is an explicit release action
  // after every gate in docs/releases/README.md has passed.
  const preservedChecksum = undefined;
  const preservedPublishedAt = existingManifest?.release?.notes?.find(
    (note) => note?.version === version && note?.metadata?.contractState === "production-ready",
  )?.publishedAt;
  const packageIds = [
    "container-x64-standalone-container-oci",
    "container-x64-cloud-container-oci",
  ];
  const defaultPackageId = packageIds[1];
  const manifest = {
    schemaVersion: 3,
    kind: "sdkwork.app",
    app: {
      key: owner,
      name: "SDKWork Memory",
      displayName: "SDKWork Memory",
      description: "SDKWork Memory service and SDK families for embedding-optional AI memory, self-learning, habit memory, and provider-switchable retrieval.",
      vendor: "SDKWork",
      officialWebsiteUrl: "https://sdkwork.com/apps/sdkwork-memory",
      supportUrl: "https://sdkwork.com/support",
      privacyPolicyUrl: "https://sdkwork.com/privacy",
      termsOfServiceUrl: "https://sdkwork.com/terms",
      iconUrl: "https://cdn.sdkwork.com/apps/sdkwork-memory/assets/icon-1024.png",
      appType: "SDK",
      versionSource: "manifest",
      identifiers: {
        packageName: null,
        bundleId: null,
        desktopAppId: null,
        containerImage: "registry.sdkwork.com/apps/sdkwork-memory"
      }
    },
    backend: {
      profileKey: "backend-root-admin",
      ownerMode: "tenant",
      grantMode: "current",
      platform: "API",
      // IAM bootstrap access token generation resolves the application
      // identity from this field; it must match app.key.
      appId: "sdkwork-memory",
      tenantId: "100001",
      organizationId: "0"
    },
    runtime: {
      family: "server",
      framework: "rust-service",
      runtimes: ["API"],
      deliveryModes: ["CONTAINER_IMAGE"],
      defaultPlatform: "API",
      defaultArchitecture: "x64",
      supportedDeploymentProfiles: ["standalone", "cloud"],
      defaultDeploymentProfile: "cloud"
    },
    media: {
      icons: {
        primary: {
          id: "sdkwork-memory-primary-icon",
          type: "ICON",
          purpose: "PRIMARY",
          url: "https://cdn.sdkwork.com/apps/sdkwork-memory/assets/icon-1024.png",
          platform: "API",
          locale: "en-US",
          width: 1024,
          height: 1024,
          format: "PNG",
          fileSizeBytes: 524288,
          alphaChannel: false,
          sortOrder: 0,
          enabled: true,
          metadata: {
            hostedOn: "sdkwork-cdn",
            assetVersion: version
          }
        },
        platform: [],
        metadata: {
          generatedBy: "tools/materialize_phase1_contracts.mjs",
          hostedOn: "sdkwork-cdn"
        }
      },
      screenshots: [],
      previews: [
        {
          id: "sdkwork-memory-catalog-preview",
          type: "PREVIEW_IMAGE",
          purpose: "CATALOG_PREVIEW",
          url: "https://cdn.sdkwork.com/apps/sdkwork-memory/media/preview-cover.png",
          platform: "API",
          locale: "en-US",
          width: 1600,
          height: 900,
          format: "PNG",
          fileSizeBytes: 786432,
          alphaChannel: false,
          caption: "SDKWork Memory service preview.",
          sortOrder: 0,
          enabled: true,
          metadata: {
            hostedOn: "sdkwork-cdn",
            assetVersion: version,
            altText: "SDKWork Memory service preview cover."
          }
        }
      ],
      metadata: {
        assetVersion: version,
        defaultLocale: "en-US"
      }
    },
    publish: {
      status: preservedChecksum ? "ACTIVE" : "INTERNAL",
      installSkill: {
        name: "sdkwork-skills-app"
      },
      platforms: ["API"],
      installPlatforms: ["API"],
      defaultPackageId,
      storeUrl: "https://sdkwork.com/apps/sdkwork-memory",
      stores: [],
      config: {
        workspaceRoot: "sdkwork-memory",
        framework: "rust-service",
        managedBy: "tools/materialize_phase1_contracts.mjs"
      }
    },
    artifacts: {
      installConfig: {
        defaultPackageId,
        installCommand: "sdkwork install sdkwork-memory",
        launchCommand: "sdkwork open sdkwork-memory",
        uninstallCommand: "sdkwork uninstall sdkwork-memory",
        packages: ["standalone", "cloud"].map((deploymentProfile, index) => (
          {
            id: packageIds[index],
            name: `SDKWork Memory ${deploymentProfile} Server Container`,
            sourceType: "CONTAINER_IMAGE",
            packageFormat: "DOCKER_IMAGE",
            platform: "API",
            ...(preservedChecksum ? {
              url: `oci://registry.sdkwork.com/apps/sdkwork-memory@sha256:${preservedChecksum}`,
              enabled: true
            } : {
              url: "oci://registry.sdkwork.com/apps/sdkwork-memory:0.1.0-rc.1",
              enabled: false
            }),
            architecture: "x64",
            profileBinding: "fixed",
            deploymentProfile,
            runtimeTarget: "container",
            metadata: {
              image: preservedChecksum
                ? "registry.sdkwork.com/apps/sdkwork-memory:0.1.0"
                : "registry.sdkwork.com/apps/sdkwork-memory:0.1.0-rc.1",
              digestRequiredBeforeRelease: true,
              releaseEvidenceState: preservedChecksum ? "verified-ci" : "pending-ci"
            },
            ...(preservedChecksum ? {
              checksumAlgorithm: "SHA-256",
              checksum: preservedChecksum
            } : {})
          }
        )),
        metadata: {
          workspaceRoot: "sdkwork-memory",
          framework: "rust-service",
          packageManager: "cargo",
          contractState: preservedChecksum ? "production-ready" : "release-candidate"
        }
      }
    },
    release: {
      currentVersion: version,
      defaultChannel: "DEV",
      latest: {
        DEV: version
      },
      notes: [
        {
          version,
          releaseChannel: "DEV",
          title: preservedChecksum
            ? `SDKWork Memory ${version} Production Ready`
            : `SDKWork Memory ${version} Internal Release Candidate`,
          summary: preservedChecksum
            ? "Production-ready Memory service and PC operations foundation with verified release supply-chain evidence."
            : "Internal validation candidate for the Memory service and PC operations surfaces; production publication remains blocked until immutable container, migration, PostgreSQL, load, recovery, and security evidence is complete.",
          content: preservedChecksum
            ? "SDKWork Memory delivers embedding-optional retrieval, tenant-scoped governance, isolated Console and Admin SDK boundaries, and verified release evidence."
            : "SDKWork Memory currently provides an embedding-optional native SQL baseline, tenant-scoped governance, isolated Console and Admin SDK boundaries, and draft Kubernetes assets. It is not approved for production publication.",
          highlights: [
            "Space-isolated memory access with fail-closed production auth",
            preservedChecksum
              ? "Privacy export and durable outbox flows"
              : "Privacy export and outbox flows under production hardening",
            preservedChecksum
              ? "Kubernetes migration, autoscaling, disruption, and monitoring descriptors"
              : "Draft Kubernetes migration, autoscaling, disruption, and monitoring descriptors",
            preservedChecksum
              ? "Verified SBOM, signature, and immutable OCI digest evidence"
              : "Release evidence pipeline pending immutable OCI publication and signing"
          ],
          packageIds,
          ...(preservedChecksum && preservedPublishedAt ? { publishedAt: preservedPublishedAt } : {}),
          current: true,
          forceUpdate: false,
          minSupportedVersion: version,
          metadata: {
            draft: !preservedChecksum,
            contractState: preservedChecksum ? "production-ready" : "release-candidate"
          }
        }
      ]
    },
    security: {
      checksumRequired: true,
      signatureRequired: true,
      sbomRequired: true
    },
    devApp: {
      build: {
        targets: []
      },
      sourceRoot: "sdkwork-memory"
    },
    metadata: {
      standardOwner: "sdkwork-platform",
      initializedAt: "2026-06-10T00:00:00Z",
      managedBy: "tools/materialize_phase1_contracts.mjs",
      deploymentConfig: "etc/sdkwork.deployment.config.json"
    }
  };

  writeJson("sdkwork.app.config.json", manifest);
}

function writeRootSpecs() {
  writeText("specs/README.md", `# SDKWork Memory Component Specs

This directory is the local component contract entrypoint for \`sdkwork-memory\`.

Authoritative root standards remain in \`../sdkwork-specs/\`. Local specs may narrow or instantiate those standards for Memory, but they must not redefine them.

Primary standards for this component:

- \`../sdkwork-specs/SOUL.md\`
- \`../sdkwork-specs/COMPONENT_SPEC.md\`
- \`../sdkwork-specs/NAMING_SPEC.md\`
- \`../sdkwork-specs/DOMAIN_SPEC.md\`
- \`../sdkwork-specs/API_SPEC.md\`
- \`../sdkwork-specs/SDK_SPEC.md\`
- \`../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md\`
- \`../sdkwork-specs/WEB_BACKEND_SPEC.md\`
- \`../sdkwork-specs/DATABASE_SPEC.md\`
- \`../sdkwork-specs/EVENT_SPEC.md\`
- \`../sdkwork-specs/PRIVACY_SPEC.md\`
- \`../sdkwork-specs/OBSERVABILITY_SPEC.md\`

Local design authority:

- \`${canonicalDesignDoc}\`
- \`${canonicalSpiDesignDoc}\`

Legacy redirect stubs (do not edit content here):

- \`${legacyDesignStub}\`
- \`${legacySpiDesignStub}\`

Canonical contract artifacts:

- \`docs/schema-registry/tables/*.yaml\`
- \`sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json\`
- \`sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json\`
- \`sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json\`
- \`sdks/sdkwork-memory-sdk/sdk-manifest.json\`
- \`sdks/sdkwork-memory-app-sdk/sdk-manifest.json\`
- \`sdks/sdkwork-memory-backend-sdk/sdk-manifest.json\`

Contract verification:

\`\`\`powershell
node tools/materialize_phase1_contracts.mjs
powershell -ExecutionPolicy Bypass -File tools/verify_phase1.ps1
\`\`\`
`);

  writeJson("specs/component.spec.json", {
    schemaVersion: 1,
    kind: "sdkwork.component.spec",
    component: {
      name: owner,
      displayName: "SDKWork Memory",
      version,
      type: "web-backend-service",
      root: "sdkwork-memory",
      domain,
      capability,
      surface: "backend-service",
      languages: ["rust", "typescript"],
      generated: false,
      status: "active",
      manifests: [
        "sdkwork.app.config.json",
        "AGENTS.md",
        "specs/component.spec.json",
        "sdks/sdkwork-memory-sdk/sdk-manifest.json",
        "sdks/sdkwork-memory-app-sdk/sdk-manifest.json",
        "sdks/sdkwork-memory-backend-sdk/sdk-manifest.json"
      ]
    },
    canonicalSpecs: rootCanonicalSpecs,
    contracts: {
      publicExports: [],
      runtimeEntrypoints: [],
      routeManifest: null,
      apiAuthorities: [
        {
          name: "sdkwork-memory-open-api",
          prefix: memoryOpenApiPrefix,
          authorityOpenApi: "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json",
          sdkFamily: "sdkwork-memory-sdk"
        },
        {
          name: "sdkwork-memory.app",
          prefix: "/app/v3/api",
          authorityOpenApi: "sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json",
          sdkFamily: "sdkwork-memory-app-sdk"
        },
        {
          name: "sdkwork-memory.backend",
          prefix: "/backend/v3/api",
          authorityOpenApi: "sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json",
          sdkFamily: "sdkwork-memory-backend-sdk"
        }
      ],
      sdkClients: [
        "SdkworkMemoryOpenClient",
        "SdkworkMemoryAppClient",
        "SdkworkMemoryBackendClient"
      ],
      sdkDependencies: rootSdkDependencies,
      dependencyApiExports: [],
      dependencyApiSurfaces: [],
      events: [
        "memory.space.created",
        "memory.event.appended",
        "memory.record.created",
        "memory.record.updated",
        "memory.record.deleted",
        "memory.record.superseded",
        "memory.candidate.created",
        "memory.candidate.approved",
        "memory.candidate.rejected",
        "memory.habit.promoted",
        "memory.habit.decayed",
        "memory.index.rebuild_requested",
        "memory.index.rebuild_completed",
        "memory.context_pack.created",
        "memory.retention.deleted",
        "memory.provider.health_changed"
      ],
      configKeys: []
    },
    integration: {
      authority: "Root SDKWork specs remain authoritative. Local specs may extend but must not contradict them.",
      memoryPolicy: "Canonical ai_record and ai_event tables are the source of truth; all indexes are derived and rebuildable.",
      implementationPolicy: "Memory implementation profiles select native_sql, event_sourced, graph_temporal, search_first, local_embedded, external_provider_bridge, or hybrid_platform without changing app/backend API contracts. Required ports are resolved per plugin binding; reference profiles are evaluation-only until their production deployment qualification is proven.",
      embeddingPolicy: "Embedding is an optional retriever/index provider, not a required storage dependency.",
      sdkPolicy: "Generated SDK clients are consumed through generated SDKs or approved composed wrappers; no raw HTTP fallback."
    },
    verification: {
      commands: [
        "node tools/materialize_phase1_contracts.mjs",
        "powershell -ExecutionPolicy Bypass -File tools/verify_phase1.ps1"
      ]
    },
    metadata: {
      standardVersion,
      status: "active",
      design: canonicalDesignDoc,
      managedBy: "tools/materialize_phase1_contracts.mjs"
    }
  });
}

const sdkSurfaceProfiles = {
  open: {
    family: "sdkwork-memory-sdk",
    packageName: "@sdkwork/memory-sdk",
    sdkTarget: "open-api",
    componentSurface: "open-api"
  },
  app: {
    family: "sdkwork-memory-app-sdk",
    packageName: "@sdkwork/memory-app-sdk",
    sdkTarget: "app",
    componentSurface: "app"
  },
  backend: {
    family: "sdkwork-memory-backend-sdk",
    packageName: "@sdkwork/memory-backend-sdk",
    sdkTarget: "backend",
    componentSurface: "backend-admin"
  }
};

function sdkSurfaceProfile(surface) {
  const profile = sdkSurfaceProfiles[surface];
  if (!profile) {
    throw new Error(`Unsupported SDK surface: ${surface}`);
  }
  return profile;
}

function sdkComponentSpec({ surface, prefix, title, authority, openapiFile, client, dependencies }) {
  const profile = sdkSurfaceProfile(surface);
  const family = profile.family;
  return {
    schemaVersion: 1,
    kind: "sdkwork.component.spec",
    component: {
      name: family,
      displayName: title,
      version,
      type: "sdk-family",
      root: `sdkwork-memory/sdks/${family}`,
      domain,
      capability,
      surface: profile.componentSurface,
      status: "active",
      languages: ["typescript"],
      generated: true,
      private: false,
      manifests: ["sdk-manifest.json"]
    },
    canonicalSpecs: sdkCanonicalSpecs,
    contracts: {
      apiAuthority: {
        name: authority,
        prefix,
        authorityOpenApi: `openapi/${openapiFile}`,
        derivedOpenApi: [`openapi/${openapiFile}`],
        owner,
        standard: "../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md"
      },
      publicExports: [],
      runtimeEntrypoints: ["sdk-manifest.json"],
      sdkDependencies: dependencies,
      dependencyApiExports: [],
      dependencyApiSurfaces: [],
      sdkClients: [client],
      events: [],
      configKeys: ["sdk-manifest.json"]
    },
    integration: {
      authority: "Root SDKWork specs remain authoritative. Local specs may extend but must not contradict them.",
      dependencyPolicy: "Dependency capabilities are consumed through declared SDK dependencies and are not copied into generated Memory transports.",
      sdkPolicy: "Generated SDK clients are injected through service/runtime boundaries; consumers must not create raw HTTP clients or manual auth headers.",
      languagePolicy: "TypeScript is the first declared generated package. Additional languages must use the same owner-only OpenAPI input and sdkDependencies."
    },
    verification: {
      commands: [
        "powershell -ExecutionPolicy Bypass -File tools/verify_phase1.ps1"
      ]
    },
    metadata: {
      managedBy: "tools/materialize_phase1_contracts.mjs",
      standardVersion
    }
  };
}

function sdkManifest({ surface, prefix, title, authority, openapiFile, client, dependencies }) {
  const profile = sdkSurfaceProfile(surface);
  const family = profile.family;
  const languageWorkspace = `${family}-typescript`;
  const transportPackageName = `${family}-generated-typescript`;
  return {
    schemaVersion: 1,
    sdkName: family,
    packageName: profile.packageName,
    sdkOwner: owner,
    apiAuthority: authority,
    sdkFamily: family,
    sdkType: surface,
    sdkSurface: surface,
    language: "typescript",
    apiPrefix: prefix,
    generationInputSpec: `openapi/${openapiFile}`,
    generatedOutput: `${languageWorkspace}/generated/server-openapi`,
    standardProfile: "sdkwork-v3",
    sdkDependencies: dependencies,
    dependencyApiExports: [],
    dependencyApiSurfaces: [],
    ownerOnlyOperationCount: null,
    standardVersion,
    managedBy: "tools/materialize_phase1_contracts.mjs",
    transportPackageName,
    typescript: {
      composedRoot: languageWorkspace,
      composedEntry: `${languageWorkspace}/src/index.ts`,
      transportRoot: `${languageWorkspace}/generated/server-openapi`,
      transportEntry: `${languageWorkspace}/generated/server-openapi/src/index.ts`
    },
    workspace: family,
    title,
    apiVersion: version,
    openapiVersion: "3.1.2",
    authoritySpec: `openapi/${openapiFile}`,
    derivedSpecs: {
      default: `openapi/${openapiFile}`
    },
    discoverySurface: {
      sdkTarget: profile.sdkTarget,
      apiPrefix: prefix,
      generatedProtocols: ["http-openapi"],
      manualTransports: []
    },
    metadata: {
      standardVersion,
      ownerOnlyOperationCount: null,
      materializationState: "authority-active",
      managedBy: "tools/materialize_phase1_contracts.mjs"
    },
    languages: [
      {
        language: "typescript",
        workspace: languageWorkspace,
        generationState: "declared",
        releaseState: "not_published",
        generatedPath: `${languageWorkspace}/generated/server-openapi`,
        manifestPath: `${languageWorkspace}/generated/server-openapi/package.json`,
        version,
        description: `Generator-owned TypeScript transport SDK for ${authority}.`,
        consumerSurface: {
          primaryClient: client,
          apiPrefix: prefix
        },
        consumerPackageName: profile.packageName,
        transportPackageName
      }
    ]
  };
}

function writeSdkgenConfig({ surface, prefix, authority, openapiFile }) {
  const profile = sdkSurfaceProfile(surface);
  const family = profile.family;
  const sdkgenFile = openapiFile.replace(".openapi.json", ".sdkgen.yaml");
  const apiSurface =
    surface === "open" ? "open-api" : surface === "app" ? "app-api" : "backend-api";
  writeText(
    `sdks/${family}/openapi/${sdkgenFile}`,
    `schemaVersion: 1
kind: sdkwork.sdkgen.config
input: ${openapiFile}
output: ../${family}-typescript/generated/server-openapi
sdkOwner: ${owner}
apiAuthority: ${authority}
sdkFamily: ${family}
standardProfile: sdkwork-v3
languageTargets:
  - typescript
ownerOnly: true
domain: ${domain}
capability: ${capability}
prefix: ${prefix}
surface: ${apiSurface}
`
  );
}

function writeSdkFamily({ surface, prefix, title, authority, openapiFile, client, dependencies }) {
  const profile = sdkSurfaceProfile(surface);
  const family = profile.family;
  writeText(`sdks/${family}/README.md`, `# ${title}

This is the SDK family root for the \`${authority}\` OpenAPI authority.

- SDK family: \`${family}\`
- API authority: \`${authority}\`
- API prefix: \`${prefix}\`
- Owner: \`${owner}\`
- Standard profile: \`sdkwork-v3\`
- Authority OpenAPI: \`openapi/${openapiFile}\`
- Declared generated client: \`${client}\`

Generated transport output, when materialized, belongs under:

- \`${family}-typescript/generated/server-openapi\`

Generated files must not be hand-edited. Fix OpenAPI, route manifests, generator input, or approved composed facades, then regenerate with the canonical SDKWork generator.

Credential mode:

- Open API SDKs use an API key credential provider for protected operations.
- App and backend SDKs use SDKWork dual-token credential injection.
`);
  writeText(`sdks/${family}/specs/README.md`, `# ${title} Component Specs

This directory is the local component contract entrypoint for \`${family}\`.

Root standards remain authoritative:

- \`../sdkwork-specs/API_SPEC.md\`
- \`../sdkwork-specs/SDK_SPEC.md\`
- \`../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md\`
- \`../sdkwork-specs/COMPONENT_SPEC.md\`

Local authority:

- \`../sdk-manifest.json\`
- \`../openapi/${openapiFile}\`
`);
  writeJson(`sdks/${family}/specs/component.spec.json`, sdkComponentSpec({ surface, prefix, title, authority, openapiFile, client, dependencies }));
  writeJson(`sdks/${family}/sdk-manifest.json`, sdkManifest({ surface, prefix, title, authority, openapiFile, client, dependencies }));
  writeSdkgenConfig({ surface, prefix, authority, openapiFile });
}

function writeSdkFamilies() {
  writeText("sdks/README.md", `# SDKWork Memory SDK Workspace

This directory owns SDKWork Memory SDK families and authority OpenAPI documents.

SDK families:

- \`sdkwork-memory-sdk\` for \`sdkwork-memory-open-api\` and \`${memoryOpenApiPrefix}\`
- \`sdkwork-memory-app-sdk\` for \`sdkwork-memory.app\` and \`/app/v3/api\`
- \`sdkwork-memory-backend-sdk\` for \`sdkwork-memory.backend\` and \`/backend/v3/api\`

Protected Open API clients use \`X-API-Key\` through generated SDK credential providers. They must not join app/backend token-manager client lists.

RPC SDK families are deferred until high-throughput backend/native RPC integration is needed.
`);
  writeSdkFamily({
    surface: "open",
    prefix: memoryOpenApiPrefix,
    title: "SDKWork Memory Open API SDK",
    authority: "sdkwork-memory-open-api",
    openapiFile: "memory-open-api.openapi.json",
    client: "SdkworkMemoryOpenClient",
    dependencies: []
  });
  writeSdkFamily({
    surface: "app",
    prefix: "/app/v3/api",
    title: "SDKWork Memory App API SDK",
    authority: "sdkwork-memory.app",
    openapiFile: "memory-app-api.openapi.json",
    client: "SdkworkMemoryAppClient",
    dependencies: appSdkDependencies
  });
  writeSdkFamily({
    surface: "backend",
    prefix: "/backend/v3/api",
    title: "SDKWork Memory Backend API SDK",
    authority: "sdkwork-memory.backend",
    openapiFile: "memory-backend-api.openapi.json",
    client: "SdkworkMemoryBackendClient",
    dependencies: backendSdkDependencies
  });
}

function writeSchemaRegistry() {
  writeText("docs/schema-registry/README.md", `# SDKWork Memory Schema Registry

Memory database contracts are defined here before migrations or ORM entities are created.

Rules:

- Physical table names use the \`ai_\` prefix.
- \`ai_record\` and \`ai_event\` are canonical source-of-truth tables.
- Index, retrieval, vector, graph, grep/file, and provider states are derived or governed tables and must be rebuildable from canonical records and events when possible.
- PostgreSQL is the production/server target; SQLite is allowed for local/private/test parity where feasible.
- 64-bit identifiers are serialized as strings in API/SDK contracts.
`);

  writeText("docs/schema-registry/tables/001-memory-core.yaml", `module: memory
owner: sdkwork-memory
domain: intelligence
bounded_context: memory-core
description: Canonical memory spaces, events, records, sources, entities, and edges. These tables are the durable source of truth for embedding-optional memory.
tables:
  - table: ai_space
    domain: intelligence
    profile: tenant_entity
    compliance_level: L3
    system_of_record: true
    description: Tenant-scoped memory namespace owned by a user, organization, app, project, agent, or external subject.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: organization_id, type: bigint, nullable: true }
      - { name: owner_subject_type, type: varchar(32), constraints: ["enum: [user, organization, app, project, agent, session, external]"] }
      - { name: owner_subject_id, type: varchar(128), constraints: [required] }
      - { name: space_type, type: varchar(32), constraints: ["enum: [personal, agent, team, app, project, session, imported, external_shadow]"] }
      - { name: display_name, type: varchar(200), constraints: [required] }
      - { name: default_scope, type: varchar(32), constraints: ["enum: [user, organization, app, agent, session, global]"] }
      - { name: lifecycle_status, type: varchar(32), constraints: ["enum: [active, archived, deleted]"] }
      - { name: metadata_json, type: json, nullable: true }
      - { name: policy_json, type: json, nullable: true }
      - { name: created_by, type: bigint, nullable: true }
      - { name: updated_by, type: bigint, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: deleted_at, type: timestamp, nullable: true }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_space_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_space_owner_type, unique: true, columns: [tenant_id, owner_subject_type, owner_subject_id, space_type] }
      - { name: idx_ai_space_tenant_status, columns: [tenant_id, lifecycle_status, updated_at] }
  - table: ai_event
    domain: intelligence
    profile: event_log
    compliance_level: L3
    system_of_record: true
    description: Append-first memory evidence event, including conversation facts, tool events, feedback, external imports, and deletion requests.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, constraints: [required], references: ai_space.id }
      - { name: user_id, type: bigint, nullable: true }
      - { name: actor_type, type: varchar(32), constraints: ["enum: [user, agent, backend, system, import, external_provider]"] }
      - { name: actor_id, type: varchar(128), nullable: true }
      - { name: session_id, type: varchar(128), nullable: true }
      - { name: trace_id, type: varchar(128), nullable: true }
      - { name: request_id, type: varchar(64), nullable: true }
      - { name: idempotency_key, type: varchar(128), nullable: true }
      - { name: event_type, type: varchar(64), constraints: [required] }
      - { name: source_type, type: varchar(64), constraints: ["enum: [conversation, tool, feedback, import, api, file, system, external_provider]"] }
      - { name: source_ref, type: varchar(256), nullable: true }
      - { name: event_time, type: timestamp, constraints: [required] }
      - { name: payload_json, type: json, constraints: [required] }
      - { name: payload_hash, type: varchar(128), constraints: [required] }
      - { name: sensitivity_level, type: varchar(32), constraints: ["enum: [public, internal, private, sensitive, restricted]"] }
      - { name: ingestion_status, type: varchar(32), constraints: ["enum: [received, processed, rejected, redacted, deleted]"] }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_event_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_event_idempotency, unique: true, columns: [tenant_id, idempotency_key], predicate: "idempotency_key IS NOT NULL" }
      - { name: idx_ai_event_space_time, columns: [tenant_id, space_id, event_time, id] }
      - { name: idx_ai_event_session_time, columns: [tenant_id, session_id, event_time] }
      - { name: idx_ai_event_type_time, columns: [tenant_id, event_type, event_time] }
      - { name: idx_ai_event_hash, columns: [tenant_id, payload_hash] }
  - table: ai_record
    domain: intelligence
    profile: core_entity
    compliance_level: L3
    system_of_record: true
    description: Canonical durable memory fact, preference, procedure, habit reference, relationship, or episode. All retrieval indexes derive from this table.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, constraints: [required], references: ai_space.id }
      - { name: user_id, type: bigint, nullable: true }
      - { name: scope, type: varchar(32), constraints: ["enum: [user, organization, app, agent, session, global]"] }
      - { name: memory_type, type: varchar(32), constraints: ["enum: [working, session, semantic, episodic, procedural, habit, relationship, domain_knowledge]"] }
      - { name: subject, type: varchar(256), nullable: true }
      - { name: predicate, type: varchar(128), nullable: true }
      - { name: object_text, type: text, constraints: [required] }
      - { name: canonical_text, type: text, constraints: [required] }
      - { name: summary_text, type: text, nullable: true }
      - { name: language, type: varchar(16), nullable: true }
      - { name: confidence, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: evidence_count, type: int32, constraints: [required, "min: 0"] }
      - { name: contradiction_count, type: int32, constraints: [required, "min: 0"] }
      - { name: importance_score, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: recency_score, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: habit_strength, type: decimal(5,4), nullable: true }
      - { name: valid_from, type: timestamp, nullable: true }
      - { name: valid_to, type: timestamp, nullable: true }
      - { name: expires_at, type: timestamp, nullable: true }
      - { name: status, type: varchar(32), constraints: ["enum: [candidate, active, inactive, superseded, deleted, rejected]"] }
      - { name: sensitivity_level, type: varchar(32), constraints: ["enum: [public, internal, private, sensitive, restricted]"] }
      - { name: metadata_json, type: json, nullable: true }
      - { name: tags_json, type: json, nullable: true }
      - { name: supersedes_memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: superseded_by_memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: created_by, type: bigint, nullable: true }
      - { name: updated_by, type: bigint, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: deleted_at, type: timestamp, nullable: true }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_record_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_record_scope_type_status, columns: [tenant_id, space_id, scope, memory_type, status, updated_at] }
      - { name: idx_ai_record_user_type, columns: [tenant_id, user_id, memory_type, status, updated_at] }
      - { name: idx_ai_record_subject_predicate, columns: [tenant_id, space_id, subject, predicate, status] }
      - { name: idx_ai_record_validity, columns: [tenant_id, valid_from, valid_to, expires_at] }
      - { name: idx_ai_record_supersession, columns: [tenant_id, supersedes_memory_id, superseded_by_memory_id] }
  - table: ai_record_source
    domain: intelligence
    profile: relation_entity
    compliance_level: L3
    system_of_record: true
    description: Evidence link from a memory record to one or more source events.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: memory_id, type: bigint, constraints: [required], references: ai_record.id }
      - { name: event_id, type: bigint, constraints: [required], references: ai_event.id }
      - { name: source_role, type: varchar(32), constraints: ["enum: [supporting, contradicting, originating, deletion, correction]"] }
      - { name: confidence_delta, type: decimal(5,4), nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_record_source_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_record_source_pair, unique: true, columns: [tenant_id, memory_id, event_id, source_role] }
      - { name: idx_ai_record_source_event, columns: [tenant_id, event_id] }
  - table: ai_entity
    domain: intelligence
    profile: dictionary_entity
    compliance_level: L2
    system_of_record: true
    description: Entity dictionary used by graph, dictionary, and deterministic retrieval without requiring embeddings.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, constraints: [required], references: ai_space.id }
      - { name: entity_type, type: varchar(64), constraints: [required] }
      - { name: canonical_name, type: varchar(256), constraints: [required] }
      - { name: aliases_json, type: json, nullable: true }
      - { name: attributes_json, type: json, nullable: true }
      - { name: status, type: varchar(32), constraints: ["enum: [active, merged, deleted]"] }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_entity_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_entity_name, unique: true, columns: [tenant_id, space_id, entity_type, canonical_name] }
      - { name: idx_ai_entity_type_status, columns: [tenant_id, space_id, entity_type, status] }
  - table: ai_edge
    domain: intelligence
    profile: relation_entity
    compliance_level: L2
    system_of_record: true
    description: Relationship edge between entities or memories for graph-temporal retrieval.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, constraints: [required], references: ai_space.id }
      - { name: source_entity_id, type: bigint, constraints: [required], references: ai_entity.id }
      - { name: target_entity_id, type: bigint, constraints: [required], references: ai_entity.id }
      - { name: relation_type, type: varchar(64), constraints: [required] }
      - { name: weight, type: decimal(8,4), nullable: true }
      - { name: source_memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: valid_from, type: timestamp, nullable: true }
      - { name: valid_to, type: timestamp, nullable: true }
      - { name: status, type: varchar(32), constraints: ["enum: [active, inactive, deleted]"] }
      - { name: metadata_json, type: json, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_edge_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_edge_source, columns: [tenant_id, space_id, source_entity_id, relation_type, status] }
      - { name: idx_ai_edge_target, columns: [tenant_id, space_id, target_entity_id, relation_type, status] }
      - { name: idx_ai_edge_validity, columns: [tenant_id, valid_from, valid_to] }
serialization:
  int64: string
  decimal: string
  time: iso8601_utc
`);

  writeText("docs/schema-registry/tables/002-memory-learning.yaml", `module: memory
owner: sdkwork-memory
domain: intelligence
bounded_context: memory-learning
description: Self-learning candidates, habit lifecycle, evidence signals, and learning jobs.
tables:
  - table: ai_candidate
    profile: core_entity
    compliance_level: L3
    system_of_record: true
    description: Reviewable memory candidate created by deterministic rules, LLM extraction, feedback, consolidation, or external provider import.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, constraints: [required], references: ai_space.id }
      - { name: user_id, type: bigint, nullable: true }
      - { name: candidate_type, type: varchar(32), constraints: ["enum: [create, update, delete, supersede, promote_habit, decay_habit]"] }
      - { name: memory_type, type: varchar(32), constraints: ["enum: [semantic, episodic, procedural, habit, relationship, domain_knowledge]"] }
      - { name: proposed_text, type: text, constraints: [required] }
      - { name: proposed_payload_json, type: json, nullable: true }
      - { name: target_memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: evidence_json, type: json, nullable: true }
      - { name: confidence, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: novelty_score, type: decimal(5,4), nullable: true }
      - { name: risk_score, type: decimal(5,4), nullable: true }
      - { name: decision_state, type: varchar(32), constraints: ["enum: [pending, auto_approved, approved, rejected, expired, superseded]"] }
      - { name: decision_reason, type: varchar(256), nullable: true }
      - { name: decided_by, type: bigint, nullable: true }
      - { name: decided_at, type: timestamp, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_candidate_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_candidate_state, columns: [tenant_id, space_id, decision_state, updated_at] }
      - { name: idx_ai_candidate_target, columns: [tenant_id, target_memory_id] }
  - table: ai_habit
    profile: core_entity
    compliance_level: L3
    system_of_record: true
    description: Habit-forming memory state promoted from repeated behavior signals and optional user confirmations.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, constraints: [required], references: ai_space.id }
      - { name: user_id, type: bigint, constraints: [required] }
      - { name: habit_key, type: varchar(160), constraints: [required] }
      - { name: habit_type, type: varchar(64), constraints: [required] }
      - { name: description, type: text, constraints: [required] }
      - { name: stage, type: varchar(32), constraints: ["enum: [observing, emerging, confirmed, decaying, inactive, rejected]"] }
      - { name: strength, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: confidence, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: support_count, type: int32, constraints: [required, "min: 0"] }
      - { name: last_signal_at, type: timestamp, nullable: true }
      - { name: promoted_memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: decay_after, type: timestamp, nullable: true }
      - { name: metadata_json, type: json, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_habit_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_habit_key, unique: true, columns: [tenant_id, space_id, user_id, habit_key] }
      - { name: idx_ai_habit_stage, columns: [tenant_id, space_id, stage, confidence, updated_at] }
  - table: ai_habit_signal
    profile: event_log
    compliance_level: L2
    system_of_record: true
    description: Evidence signal attached to habit scoring and promotion/decay decisions.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: habit_id, type: bigint, constraints: [required], references: ai_habit.id }
      - { name: event_id, type: bigint, nullable: true, references: ai_event.id }
      - { name: signal_type, type: varchar(64), constraints: [required] }
      - { name: signal_strength, type: decimal(5,4), constraints: ["range: 0..1"] }
      - { name: observed_at, type: timestamp, constraints: [required] }
      - { name: payload_json, type: json, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_habit_signal_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_habit_signal_habit, columns: [tenant_id, habit_id, observed_at] }
      - { name: idx_ai_habit_signal_event, columns: [tenant_id, event_id] }
  - table: ai_learning_job
    profile: event_log
    compliance_level: L2
    system_of_record: true
    description: Asynchronous extraction, consolidation, habit scoring, and review materialization job.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, nullable: true, references: ai_space.id }
      - { name: job_type, type: varchar(64), constraints: ["enum: [extract, consolidate, habit_score, index_sync, retention, migration]"] }
      - { name: state, type: varchar(32), constraints: ["enum: [queued, running, succeeded, failed, cancelled]"] }
      - { name: priority, type: int32, constraints: [required] }
      - { name: idempotency_key, type: varchar(128), nullable: true }
      - { name: input_json, type: json, nullable: true }
      - { name: result_json, type: json, nullable: true }
      - { name: error_json, type: json, nullable: true }
      - { name: started_at, type: timestamp, nullable: true }
      - { name: finished_at, type: timestamp, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_learning_job_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_learning_job_idempotency, unique: true, columns: [tenant_id, job_type, idempotency_key], predicate: "idempotency_key IS NOT NULL" }
      - { name: idx_ai_learning_job_state, columns: [tenant_id, job_type, state, priority, created_at] }
serialization: { int64: string, decimal: string, time: iso8601_utc }
`);

  writeText("docs/schema-registry/tables/003-memory-retrieval.yaml", `module: memory
owner: sdkwork-memory
domain: intelligence
bounded_context: memory-retrieval
description: Derived indexes, retrieval profiles, traces, hits, and context packs. Embedding/vector state is optional and represented as one index kind among many.
tables:
  - table: ai_index
    profile: projection
    compliance_level: L2
    system_of_record: false
    description: Provider-neutral index definition for sql, keyword, dictionary, time, event, vector, graph, grep_file, or custom retrievers.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, nullable: true, references: ai_space.id }
      - { name: index_kind, type: varchar(32), constraints: ["enum: [sql, keyword, dictionary, time, event, vector, graph, grep_file, custom]"] }
      - { name: implementation_profile_id, type: bigint, nullable: true }
      - { name: provider_binding_id, type: bigint, nullable: true }
      - { name: schema_version, type: varchar(32), constraints: [required] }
      - { name: status, type: varchar(32), constraints: ["enum: [active, rebuilding, degraded, disabled, deleted]"] }
      - { name: rebuild_cursor, type: varchar(256), nullable: true }
      - { name: config_json, type: json, nullable: true }
      - { name: last_rebuilt_at, type: timestamp, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_index_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_index_kind_space, unique: true, columns: [tenant_id, space_id, index_kind, schema_version] }
      - { name: idx_ai_index_status, columns: [tenant_id, space_id, index_kind, status] }
  - table: ai_index_entry
    profile: projection
    compliance_level: L2
    system_of_record: false
    description: Provider-neutral pointer to derived index state; vector payloads may be externalized while canonical memory stays in ai_record.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: index_id, type: bigint, constraints: [required], references: ai_index.id }
      - { name: memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: event_id, type: bigint, nullable: true, references: ai_event.id }
      - { name: entry_kind, type: varchar(32), constraints: ["enum: [memory, event, entity, edge, file_line, external_ref]"] }
      - { name: entry_hash, type: varchar(128), constraints: [required] }
      - { name: external_ref, type: varchar(512), nullable: true }
      - { name: payload_json, type: json, nullable: true }
      - { name: indexed_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_index_entry_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_index_entry_memory, unique: true, columns: [tenant_id, index_id, memory_id, entry_kind], predicate: "memory_id IS NOT NULL" }
      - { name: idx_ai_index_entry_hash, columns: [tenant_id, index_id, entry_hash] }
  - table: ai_retrieval_profile
    profile: dictionary_entity
    compliance_level: L2
    system_of_record: true
    description: Runtime retrieval policy describing retriever selection, fusion, rerank, context budget, and degraded-mode behavior.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, nullable: true, references: ai_space.id }
      - { name: name, type: varchar(160), constraints: [required] }
      - { name: strategy, type: varchar(64), constraints: ["enum: [deterministic, semantic, graph, file, hybrid, custom]"] }
      - { name: retrievers_json, type: json, constraints: [required] }
      - { name: fusion_policy_json, type: json, nullable: true }
      - { name: rerank_policy_json, type: json, nullable: true }
      - { name: top_k, type: int32, constraints: [required] }
      - { name: context_budget_tokens, type: int32, constraints: [required] }
      - { name: status, type: varchar(32), constraints: ["enum: [active, disabled, deleted]"] }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_retrieval_profile_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_retrieval_profile_scope, columns: [tenant_id, space_id, status, updated_at] }
  - table: ai_retrieval_trace
    profile: event_log
    compliance_level: L2
    system_of_record: true
    description: Trace for each retrieval orchestration, including selected retrievers, degraded mode, and scoring metadata.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: space_id, type: bigint, nullable: true, references: ai_space.id }
      - { name: retrieval_profile_id, type: bigint, nullable: true, references: ai_retrieval_profile.id }
      - { name: actor_id, type: varchar(128), nullable: true }
      - { name: query_text, type: text, nullable: true }
      - { name: query_hash, type: varchar(128), constraints: [required] }
      - { name: retrievers_json, type: json, nullable: true }
      - { name: latency_ms, type: int32, nullable: true }
      - { name: result_count, type: int32, constraints: [required] }
      - { name: degraded, type: boolean, constraints: [required] }
      - { name: metadata_json, type: json, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_retrieval_trace_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_retrieval_trace_profile_created, columns: [tenant_id, retrieval_profile_id, created_at] }
      - { name: idx_ai_retrieval_trace_actor_created, columns: [tenant_id, actor_id, created_at] }
  - table: ai_retrieval_hit
    profile: projection
    compliance_level: L2
    system_of_record: true
    description: Ranked hit materialized from a retrieval trace for explainability, evaluation, and feedback.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: retrieval_trace_id, type: bigint, constraints: [required], references: ai_retrieval_trace.id }
      - { name: memory_id, type: bigint, nullable: true, references: ai_record.id }
      - { name: retriever_name, type: varchar(64), constraints: [required] }
      - { name: result_rank, type: int32, constraints: [required] }
      - { name: raw_score, type: decimal(10,6), nullable: true }
      - { name: fused_score, type: decimal(10,6), nullable: true }
      - { name: explanation_json, type: json, nullable: true }
      - { name: status, type: varchar(32), constraints: ["enum: [included, filtered, suppressed]"] }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_retrieval_hit_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_retrieval_hit_trace_rank, columns: [tenant_id, retrieval_trace_id, result_rank] }
      - { name: idx_ai_retrieval_hit_memory, columns: [tenant_id, memory_id, status] }
  - table: ai_context_pack
    profile: snapshot
    compliance_level: L2
    system_of_record: true
    description: LLM-ready context assembled from one retrieval trace with citations and token budget metadata.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: retrieval_trace_id, type: bigint, nullable: true, references: ai_retrieval_trace.id }
      - { name: actor_id, type: varchar(128), nullable: true }
      - { name: query_text, type: text, nullable: true }
      - { name: pack_json, type: json, constraints: [required] }
      - { name: estimated_tokens, type: int32, constraints: [required] }
      - { name: truncated, type: boolean, constraints: [required] }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_context_pack_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_context_pack_trace, columns: [tenant_id, retrieval_trace_id] }
      - { name: idx_ai_context_pack_actor_created, columns: [tenant_id, actor_id, created_at] }
serialization: { int64: string, decimal: string, time: iso8601_utc }
`);

  writeText("docs/schema-registry/tables/004-memory-provider.yaml", `module: memory
owner: sdkwork-memory
domain: intelligence
bounded_context: memory-provider-policy
description: Implementation profiles, provider bindings, and policy definitions for provider-switchable memory.
tables:
  - table: ai_implementation_profile
    profile: dictionary_entity
    compliance_level: L3
    system_of_record: true
    description: Selects the concrete Memory runtime implementation behind stable app/backend APIs.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: name, type: varchar(160), constraints: [required] }
      - { name: implementation_kind, type: varchar(64), constraints: ["enum: [native_sql, event_sourced, graph_temporal, search_first, local_embedded, external_provider_bridge, hybrid_platform]"] }
      - { name: role, type: varchar(32), constraints: ["enum: [primary, shadow, migration_source, migration_target, eval_only]"] }
      - { name: status, type: varchar(32), constraints: ["enum: [draft, active, paused, deprecated, deleted]"] }
      - { name: capability_json, type: json, constraints: [required] }
      - { name: config_json, type: json, nullable: true }
      - { name: rollout_json, type: json, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_implementation_profile_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_implementation_profile_kind, columns: [tenant_id, implementation_kind, status] }
  - table: ai_provider_binding
    profile: dictionary_entity
    compliance_level: L3
    system_of_record: true
    description: Abstract binding for LLM, embedding, rerank, graph, search, file, and external memory providers. Secrets are referenced, never stored here.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: provider_kind, type: varchar(32), constraints: ["enum: [llm, embedding, rerank, vector, graph, search, file, memory, external]"] }
      - { name: provider_code, type: varchar(128), constraints: [required] }
      - { name: display_name, type: varchar(160), constraints: [required] }
      - { name: endpoint_ref, type: varchar(256), nullable: true }
      - { name: secret_ref, type: varchar(256), nullable: true }
      - { name: model_ref, type: varchar(256), nullable: true }
      - { name: capabilities_json, type: json, constraints: [required] }
      - { name: config_json, type: json, nullable: true }
      - { name: health_state, type: varchar(32), constraints: ["enum: [unknown, healthy, degraded, unhealthy, disabled]"] }
      - { name: last_health_at, type: timestamp, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_provider_binding_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: uk_ai_provider_binding_code, unique: true, columns: [tenant_id, provider_kind, provider_code] }
      - { name: idx_ai_provider_binding_health, columns: [tenant_id, provider_kind, health_state, updated_at] }
  - table: ai_policy
    profile: dictionary_entity
    compliance_level: L3
    system_of_record: true
    description: Retention, privacy, review, learning, retrieval, and provider governance policy.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: policy_type, type: varchar(64), constraints: ["enum: [retention, privacy, review, learning, retrieval, provider, export, deletion]"] }
      - { name: scope, type: varchar(32), constraints: ["enum: [tenant, organization, user, space, app, agent, global]"] }
      - { name: scope_ref, type: varchar(128), nullable: true }
      - { name: status, type: varchar(32), constraints: ["enum: [active, disabled, deleted]"] }
      - { name: policy_json, type: json, constraints: [required] }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
      - { name: version, type: bigint, constraints: [required, "min: 0"] }
    indexes:
      - { name: uk_ai_policy_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_policy_type_scope, columns: [tenant_id, policy_type, scope, status] }
serialization: { int64: string, decimal: string, time: iso8601_utc }
`);

  writeText("docs/schema-registry/tables/005-memory-governance.yaml", `module: memory
owner: sdkwork-memory
domain: intelligence
bounded_context: memory-governance
description: Audit, evaluation, and outbox contracts for governed memory operations.
tables:
  - table: ai_audit_log
    profile: audit_log
    compliance_level: L3
    system_of_record: true
    description: Immutable audit log for memory read, write, provider, retention, export, deletion, and admin operations.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: actor_type, type: varchar(32), constraints: [required] }
      - { name: actor_id, type: varchar(128), nullable: true }
      - { name: action, type: varchar(128), constraints: [required] }
      - { name: resource_type, type: varchar(64), constraints: [required] }
      - { name: resource_id, type: varchar(128), nullable: true }
      - { name: request_id, type: varchar(64), nullable: true }
      - { name: trace_id, type: varchar(128), nullable: true }
      - { name: result, type: varchar(32), constraints: ["enum: [success, denied, failed, partial]"] }
      - { name: reason, type: varchar(256), nullable: true }
      - { name: metadata_json, type: json, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_audit_log_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_audit_actor_time, columns: [tenant_id, actor_type, actor_id, created_at] }
      - { name: idx_ai_audit_resource_time, columns: [tenant_id, resource_type, resource_id, created_at] }
      - { name: idx_ai_audit_action_time, columns: [tenant_id, action, created_at] }
  - table: ai_eval_run
    profile: event_log
    compliance_level: L2
    system_of_record: true
    description: Offline or online evaluation run for write quality, retrieval quality, habit quality, and provider switching.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: eval_type, type: varchar(64), constraints: ["enum: [write_quality, retrieval_quality, habit_quality, provider_switch, regression]"] }
      - { name: state, type: varchar(32), constraints: ["enum: [queued, running, succeeded, failed, cancelled]"] }
      - { name: dataset_ref, type: varchar(256), nullable: true }
      - { name: profile_ref, type: varchar(256), nullable: true }
      - { name: metrics_json, type: json, nullable: true }
      - { name: result_json, type: json, nullable: true }
      - { name: started_at, type: timestamp, nullable: true }
      - { name: finished_at, type: timestamp, nullable: true }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_eval_run_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_eval_run_type_state, columns: [tenant_id, eval_type, state, created_at] }
  - table: ai_outbox_event
    profile: outbox_event
    compliance_level: L3
    system_of_record: true
    description: Transactional outbox for memory.* domain events.
    columns:
      - { name: id, type: bigint, constraints: [primary_key, snowflake] }
      - { name: uuid, type: varchar(64), constraints: [required, public_id] }
      - { name: tenant_id, type: bigint, constraints: [required] }
      - { name: aggregate_type, type: varchar(64), constraints: [required] }
      - { name: aggregate_id, type: varchar(128), constraints: [required] }
      - { name: event_type, type: varchar(128), constraints: [required] }
      - { name: event_version, type: varchar(32), constraints: [required] }
      - { name: payload_json, type: json, constraints: [required] }
      - { name: publish_state, type: varchar(32), constraints: ["enum: [pending, published, failed, skipped]"] }
      - { name: published_at, type: timestamp, nullable: true }
      - { name: retry_count, type: int32, constraints: [required, "min: 0"] }
      - { name: created_at, type: timestamp, constraints: [required] }
      - { name: updated_at, type: timestamp, constraints: [required] }
    indexes:
      - { name: uk_ai_outbox_event_uuid, unique: true, columns: [tenant_id, uuid] }
      - { name: idx_ai_outbox_state, columns: [tenant_id, publish_state, created_at] }
serialization: { int64: string, decimal: string, time: iso8601_utc }
`);
}

// API_SPEC section 13.6: int64 wire fields are strings carrying the int64 format
// plus the marker, so generated TypeScript SDKs keep them as `string` and never
// round ids past Number.MAX_SAFE_INTEGER.
const idSchema = {
  type: "string",
  format: "int64",
  pattern: "^[0-9]+$",
  "x-sdkwork-int64-string": true
};

// Request-content bounds. Each mirrors the Rust constant that enforces it, so the
// published contract and the runtime ceiling describe the same limit instead of
// the runtime silently accepting payloads the contract appears to allow.
// check_sdkwork_memory_architecture_alignment.mjs asserts both sides stay equal.
const MAX_SCOPE_SPACE_IDS = 32; // platform::MAX_SCOPE_SPACE_IDS
const MAX_FORGET_MEMORY_IDS = 200; // platform::MAX_FORGET_MEMORY_IDS (MAX_LIST_PAGE_SIZE)
const nullableIdSchema = { anyOf: [idSchema, { type: "null" }] };
const nullableString = { anyOf: [{ type: "string" }, { type: "null" }] };
const jsonObject = { type: "object", additionalProperties: true };
const nullableJsonObject = { anyOf: [jsonObject, { type: "null" }] };
const instant = { type: "string", format: "date-time" };
const nullableInstant = { anyOf: [instant, { type: "null" }] };

function schemaRef(name) {
  return { $ref: `#/components/schemas/${name}` };
}

function pageSchema(itemSchemaName) {
  return {
    type: "object",
    required: ["items", "pageInfo"],
    properties: {
      items: { type: "array", items: schemaRef(itemSchemaName) },
      pageInfo: schemaRef("PageInfo")
    }
  };
}

function errorResponses() {
  const problem = {
    description: "Problem detail",
    content: {
      "application/problem+json": {
        schema: schemaRef("ProblemDetail")
      }
    }
  };
  return {
    "400": problem,
    "401": problem,
    "403": problem,
    "404": problem,
    "409": problem,
    "429": problem
  };
}

function successResponse(status, schemaName, description = status === "201" ? "Created" : "OK") {
  if (status === "204") {
    return { description: "No content" };
  }
  return {
    description,
    content: {
      "application/json": {
        schema: schemaRef(schemaName)
      }
    }
  };
}

function pathParam(name) {
  return {
    name,
    in: "path",
    required: true,
    schema: idSchema
  };
}

function requiredSpaceIdQueryParam() {
  return { name: "space_id", in: "query", required: true, schema: idSchema };
}

// PAGINATION_SPEC section 3.1 mandates a single page_size contract for every
// list surface: default 20, max 200. Both list helpers share the factory so the
// default can never drift between offset and cursor paging again.
const DEFAULT_PAGE_SIZE = 20;
const MAX_PAGE_SIZE = 200;

function pageSizeSchema() {
  return {
    type: "integer",
    format: "int32",
    minimum: 1,
    maximum: MAX_PAGE_SIZE,
    default: DEFAULT_PAGE_SIZE
  };
}

function listParams(extra = []) {
  return [
    { name: "q", in: "query", required: false, schema: { type: "string" } },
    { name: "cursor", in: "query", required: false, schema: { type: "string" } },
    { name: "page_size", in: "query", required: false, schema: pageSizeSchema() },
    ...extra
  ];
}

function cursorListParams(extra = []) {
  return [
    { name: "cursor", in: "query", required: false, schema: { type: "string" } },
    { name: "page_size", in: "query", required: false, schema: pageSizeSchema() },
    ...extra
  ];
}

function idempotencyParam() {
  return {
    name: "Idempotency-Key",
    in: "header",
    // Required, not optional: the header is only ever attached to operations
    // already marked x-sdkwork-idempotent: true, and API_SPEC's operation-pattern
    // gate rejects an optional Idempotency-Key (idempotency-header-required).
    // A retry-safe mutation cannot let a client silently omit the key, or the
    // dedupe contract degrades to best-effort.
    required: true,
    schema: { type: "string", minLength: 1, maxLength: 128 },
    description: "Client retry idempotency key scoped by tenant, principal, method, and path."
  };
}

function resolveApiSurface({ authority, authMode, apiSurface }) {
  if (apiSurface) {
    return apiSurface;
  }
  if (authMode === "api-key") {
    return "open-api";
  }
  if (authority === "sdkwork-memory.backend") {
    return "backend-api";
  }
  return "app-api";
}

function operation({
  method,
  operationId,
  tag = "memory",
  authority,
  permission,
  auditEvent,
  pathParams = [],
  queryParams = [],
  requestSchema,
  responseSchema,
  status,
  resource,
  idempotent = false,
  authMode = "dual-token",
  apiSurface,
  rateLimitTier = null,
}) {
  const resolvedApiSurface = resolveApiSurface({ authority, authMode, apiSurface });
  const resolvedRateLimitTier = resolveRateLimitTier({
    rateLimitTier,
    authMode,
    method,
    operationId,
  });
  const responses = {
    [status ?? (method === "post" ? "201" : method === "delete" ? "204" : "200")]: successResponse(status ?? (method === "post" ? "201" : method === "delete" ? "204" : "200"), responseSchema),
    ...errorResponses()
  };
  const parameters = [...pathParams, ...queryParams];
  if (idempotent) {
    parameters.push(idempotencyParam());
  }
  const op = {
    operationId,
    tags: [tag],
    parameters,
    responses,
    security: authMode === "api-key" ? [{ ApiKey: [] }] : [{ AuthToken: [], AccessToken: [] }],
    "x-sdkwork-owner": owner,
    "x-sdkwork-api-authority": authority,
    "x-sdkwork-api-surface": resolvedApiSurface,
    "x-sdkwork-request-context": "WebRequestContext",
    "x-sdkwork-domain": domain,
    "x-sdkwork-resource": resource ?? operationId.split(".")[0],
    "x-sdkwork-permission": permission,
    "x-sdkwork-tenant-scope": "tenant",
    "x-sdkwork-data-scope": "owner",
    "x-sdkwork-auth-mode": authMode,
    "x-sdkwork-audit-event": auditEvent,
    "x-sdkwork-idempotent": idempotent
  };
  if (resolvedRateLimitTier) {
    op["x-sdkwork-rate-limit-tier"] = resolvedRateLimitTier;
  }
  if (requestSchema) {
    op.requestBody = {
      required: true,
      content: {
        "application/json": {
          schema: schemaRef(requestSchema)
        }
      }
    };
  }
  return op;
}

function addPath(paths, pathKey, method, op) {
  paths[pathKey] ??= {};
  paths[pathKey][method] = op;
}

function baseSchemas() {
  const enumSchema = (values) => ({ type: "string", enum: values });
  const memoryType = enumSchema(["working", "session", "semantic", "episodic", "procedural", "habit", "relationship", "domain_knowledge"]);
  const memoryStatus = enumSchema(["candidate", "active", "inactive", "superseded", "deleted", "rejected"]);
  const candidateState = enumSchema(["pending", "auto_approved", "approved", "rejected", "expired", "superseded"]);
  const habitStage = enumSchema(["observing", "emerging", "confirmed", "decaying", "inactive", "rejected"]);
  return {
    MemorySpace: {
      type: "object",
      required: ["spaceId", "tenantId", "ownerSubjectType", "ownerSubjectId", "spaceType", "displayName", "lifecycleStatus", "createdAt", "updatedAt", "version"],
      properties: {
        spaceId: idSchema,
        uuid: { type: "string" },
        tenantId: idSchema,
        organizationId: nullableIdSchema,
        ownerSubjectType: { type: "string" },
        ownerSubjectId: { type: "string" },
        spaceType: { type: "string" },
        displayName: { type: "string" },
        defaultScope: { type: "string" },
        lifecycleStatus: { type: "string" },
        metadata: nullableJsonObject,
        policy: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemorySpaceRequest: {
      type: "object",
      required: ["ownerSubjectType", "ownerSubjectId", "spaceType", "displayName"],
      properties: {
        organizationId: nullableIdSchema,
        ownerSubjectType: { type: "string" },
        ownerSubjectId: { type: "string" },
        spaceType: { type: "string" },
        displayName: { type: "string" },
        defaultScope: { type: "string" },
        metadata: nullableJsonObject,
        policy: nullableJsonObject,
        version: nullableIdSchema
      }
    },
    MemorySpaceList: pageSchema("MemorySpace"),
    MemoryEvent: {
      type: "object",
      required: ["eventId", "spaceId", "eventType", "sourceType", "eventTime", "payloadHash", "ingestionStatus", "createdAt"],
      properties: {
        eventId: idSchema,
        uuid: { type: "string" },
        spaceId: idSchema,
        userId: nullableIdSchema,
        actorType: { type: "string" },
        actorId: nullableString,
        sessionId: nullableString,
        traceId: nullableString,
        eventType: { type: "string" },
        sourceType: { type: "string" },
        sourceRef: nullableString,
        eventTime: instant,
        payload: jsonObject,
        payloadHash: { type: "string" },
        sensitivityLevel: { type: "string" },
        ingestionStatus: { type: "string" },
        createdAt: instant
      }
    },
    MemoryEventRequest: {
      type: "object",
      required: ["spaceId", "eventType", "sourceType", "eventTime", "payload"],
      properties: {
        spaceId: idSchema,
        userId: nullableIdSchema,
        actorType: { type: "string" },
        actorId: nullableString,
        sessionId: nullableString,
        traceId: nullableString,
        eventType: { type: "string" },
        sourceType: { type: "string" },
        sourceRef: nullableString,
        eventTime: instant,
        payload: jsonObject,
        sensitivityLevel: { type: "string" }
      }
    },
    MemoryEventList: pageSchema("MemoryEvent"),
    MemoryRecord: {
      type: "object",
      required: ["memoryId", "spaceId", "scope", "memoryType", "canonicalText", "confidence", "status", "createdAt", "updatedAt", "version"],
      properties: {
        memoryId: idSchema,
        uuid: { type: "string" },
        spaceId: idSchema,
        userId: nullableIdSchema,
        scope: { type: "string" },
        memoryType,
        subject: nullableString,
        predicate: nullableString,
        objectText: { type: "string" },
        canonicalText: { type: "string" },
        summaryText: nullableString,
        language: nullableString,
        confidence: { type: "number", format: "double" },
        evidenceCount: { type: "integer", format: "int32" },
        contradictionCount: { type: "integer", format: "int32" },
        importanceScore: { type: "number", format: "double" },
        recencyScore: { type: "number", format: "double" },
        habitStrength: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        expiresAt: nullableInstant,
        status: memoryStatus,
        sensitivityLevel: { type: "string" },
        metadata: nullableJsonObject,
        tags: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        supersedesMemoryId: nullableIdSchema,
        supersededByMemoryId: nullableIdSchema,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryRecordRequest: {
      type: "object",
      required: ["spaceId", "scope", "memoryType", "canonicalText"],
      properties: {
        spaceId: idSchema,
        userId: nullableIdSchema,
        scope: { type: "string" },
        memoryType,
        subject: nullableString,
        predicate: nullableString,
        objectText: nullableString,
        canonicalText: { type: "string" },
        summaryText: nullableString,
        confidence: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        expiresAt: nullableInstant,
        sensitivityLevel: { type: "string" },
        metadata: nullableJsonObject,
        tags: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        version: nullableIdSchema
      }
    },
    MemoryRecordList: pageSchema("MemoryRecord"),
    MemoryRecordSource: {
      type: "object",
      required: ["sourceId", "memoryId", "eventId", "sourceRole", "createdAt"],
      properties: {
        sourceId: idSchema,
        memoryId: idSchema,
        eventId: idSchema,
        sourceRole: { type: "string" },
        confidenceDelta: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        createdAt: instant
      }
    },
    MemoryRecordSourceList: pageSchema("MemoryRecordSource"),
    MemoryCandidate: {
      type: "object",
      required: ["candidateId", "spaceId", "candidateType", "memoryType", "proposedText", "confidence", "decisionState", "createdAt", "updatedAt"],
      properties: {
        candidateId: idSchema,
        spaceId: idSchema,
        userId: nullableIdSchema,
        candidateType: { type: "string" },
        memoryType,
        proposedText: { type: "string" },
        proposedPayload: nullableJsonObject,
        targetMemoryId: nullableIdSchema,
        evidence: nullableJsonObject,
        confidence: { type: "number", format: "double" },
        noveltyScore: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        riskScore: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        decisionState: candidateState,
        decisionReason: nullableString,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryCandidateList: pageSchema("MemoryCandidate"),
    MemoryReviewRequest: {
      type: "object",
      properties: {
        reason: nullableString,
        reviewerNote: nullableString,
        metadata: nullableJsonObject
      }
    },
    MemoryHabit: {
      type: "object",
      required: ["habitId", "spaceId", "userId", "habitKey", "habitType", "description", "stage", "strength", "confidence", "supportCount", "createdAt", "updatedAt", "version"],
      properties: {
        habitId: idSchema,
        spaceId: idSchema,
        userId: idSchema,
        habitKey: { type: "string" },
        habitType: { type: "string" },
        description: { type: "string" },
        stage: habitStage,
        strength: { type: "number", format: "double" },
        confidence: { type: "number", format: "double" },
        supportCount: { type: "integer", format: "int32" },
        lastSignalAt: nullableInstant,
        promotedMemoryId: nullableIdSchema,
        decayAfter: nullableInstant,
        metadata: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryHabitRequest: {
      type: "object",
      properties: {
        description: nullableString,
        stage: habitStage,
        metadata: nullableJsonObject,
        version: nullableIdSchema
      }
    },
    MemoryHabitList: pageSchema("MemoryHabit"),
    MemoryExtractionRequest: {
      type: "object",
      required: ["spaceId", "inputEvents"],
      properties: {
        spaceId: idSchema,
        inputEvents: { type: "array", items: idSchema },
        extractionMode: { type: "string", enum: ["deterministic", "llm_assisted", "hybrid"] },
        customInstructions: nullableString,
        observationDate: nullableString,
        reviewRequired: { type: "boolean" },
        metadata: nullableJsonObject
      }
    },
    MemoryLearningJob: {
      type: "object",
      required: ["jobId", "jobType", "state", "priority", "createdAt", "updatedAt"],
      properties: {
        jobId: idSchema,
        spaceId: nullableIdSchema,
        jobType: { type: "string" },
        state: { type: "string" },
        priority: { type: "integer", format: "int32" },
        result: nullableJsonObject,
        error: nullableJsonObject,
        startedAt: nullableInstant,
        finishedAt: nullableInstant,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryLearningJobList: pageSchema("MemoryLearningJob"),
    MemoryRetrievalRequest: {
      type: "object",
      required: ["query", "spaceIds", "topK", "contextBudgetTokens"],
      properties: {
        query: { type: "string" },
        spaceIds: { type: "array", items: idSchema, maxItems: MAX_SCOPE_SPACE_IDS },
        actorId: nullableString,
        retrievalProfileId: nullableIdSchema,
        memoryTypes: { anyOf: [{ type: "array", items: memoryType }, { type: "null" }] },
        filters: nullableJsonObject,
        topK: { type: "integer", format: "int32", minimum: 1, maximum: 100 },
        contextBudgetTokens: { type: "integer", format: "int32", minimum: 1 },
        showExpired: { type: "boolean" },
        threshold: { anyOf: [{ type: "number" }, { type: "null" }] },
        explain: { type: "boolean" },
        includeTrace: { type: "boolean" }
      }
    },
    MemoryRetrievalResult: {
      type: "object",
      required: ["retrievalId", "hits", "degraded"],
      properties: {
        retrievalId: idSchema,
        trace: { anyOf: [schemaRef("MemoryRetrievalTrace"), { type: "null" }] },
        hits: { type: "array", items: schemaRef("MemoryRetrievalHit") },
        degraded: { type: "boolean" }
      }
    },
    MemoryRetrievalTrace: {
      type: "object",
      required: ["traceId", "queryHash", "resultCount", "degraded", "createdAt"],
      properties: {
        traceId: idSchema,
        spaceId: nullableIdSchema,
        retrievalProfileId: nullableIdSchema,
        actorId: nullableString,
        queryText: nullableString,
        queryHash: { type: "string" },
        retrievers: nullableJsonObject,
        latencyMs: { anyOf: [{ type: "integer", format: "int32" }, { type: "null" }] },
        resultCount: { type: "integer", format: "int32" },
        degraded: { type: "boolean" },
        metadata: nullableJsonObject,
        createdAt: instant
      }
    },
    MemoryRetrievalTraceList: pageSchema("MemoryRetrievalTrace"),
    DeleteAllMemoriesRequest: {
      type: "object",
      required: ["spaceId"],
      properties: {
        spaceId: idSchema,
        userId: nullableIdSchema
      }
    },
    DeleteAllMemoriesResult: {
      type: "object",
      required: ["deletedCount", "deletedMemoryIds"],
      properties: {
        deletedCount: {
          type: "string",
          format: "int64",
          "x-sdkwork-int64-string": true
        },
        deletedMemoryIds: { type: "array", items: { type: "string" } }
      }
    },
    MemoryRetrievalHit: {
      type: "object",
      required: ["hitId", "retrieverName", "resultRank", "status"],
      properties: {
        hitId: idSchema,
        memory: { anyOf: [schemaRef("MemoryRecord"), { type: "null" }] },
        memoryId: nullableIdSchema,
        retrieverName: { type: "string" },
        resultRank: { type: "integer", format: "int32" },
        rawScore: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        fusedScore: { anyOf: [{ type: "number", format: "double" }, { type: "null" }] },
        explanation: nullableJsonObject,
        status: { type: "string" }
      }
    },
    MemoryContextPackRequest: {
      type: "object",
      required: ["query", "spaceIds", "contextBudgetTokens"],
      properties: {
        query: { type: "string" },
        spaceIds: { type: "array", items: idSchema },
        actorId: nullableString,
        retrievalProfileId: nullableIdSchema,
        contextBudgetTokens: { type: "integer", format: "int32" },
        includeCitations: { type: "boolean" },
        filters: nullableJsonObject
      }
    },
    MemoryContextPack: {
      type: "object",
      required: ["contextPackId", "pack", "estimatedTokens", "truncated", "createdAt"],
      properties: {
        contextPackId: idSchema,
        retrievalId: nullableIdSchema,
        query: nullableString,
        pack: jsonObject,
        estimatedTokens: { type: "integer", format: "int32" },
        truncated: { type: "boolean" },
        createdAt: instant
      }
    },
    MemoryFeedbackRequest: {
      type: "object",
      required: ["targetType", "targetId", "feedbackType"],
      properties: {
        targetType: { type: "string", enum: ["retrieval", "hit", "memory", "candidate", "habit"] },
        targetId: idSchema,
        feedbackType: { type: "string" },
        rating: { anyOf: [{ type: "integer", format: "int32" }, { type: "null" }] },
        comment: nullableString,
        metadata: nullableJsonObject
      }
    },
    MemoryFeedback: {
      type: "object",
      required: ["feedbackId", "targetType", "targetId", "feedbackType", "createdAt"],
      properties: {
        feedbackId: idSchema,
        targetType: { type: "string" },
        targetId: idSchema,
        feedbackType: { type: "string" },
        rating: { anyOf: [{ type: "integer", format: "int32" }, { type: "null" }] },
        comment: nullableString,
        createdAt: instant
      }
    },
    MemoryForgetRequest: {
      type: "object",
      required: ["scope", "reason"],
      properties: {
        // Per-scope event semantics: `space` and `user` partition an entire
        // footprint and therefore purge the ai_event rows that fed it, so a
        // caller sees a real purgedEvents count. `memory` targets named records
        // and `query` targets records matching text; ai_event references
        // ai_space only, never ai_record, so neither can purge events without
        // destroying inputs shared with records the caller did not name.
        scope: {
          type: "string",
          enum: ["memory", "space", "user", "query"],
          description:
            "Forget scope. space and user purge the ai_event inputs of the whole footprint; memory and query delete only the named or matched records and report purgedEvents 0 by construction.",
        },
        memoryIds: {
          anyOf: [
            { type: "array", items: idSchema, maxItems: MAX_FORGET_MEMORY_IDS },
            { type: "null" },
          ],
        },
        spaceId: nullableIdSchema,
        query: nullableString,
        reason: { type: "string" },
        metadata: nullableJsonObject,
      },
    },
    MemoryForgetJob: {
      type: "object",
      required: ["forgetRequestId", "state", "createdAt", "updatedAt"],
      properties: {
        forgetRequestId: idSchema,
        state: { type: "string", enum: ["queued", "running", "succeeded", "failed", "cancelled"] },
        result: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant
      }
    },
    MemoryForgetJobList: pageSchema("MemoryForgetJob"),
    MemoryExportRequest: {
      type: "object",
      required: ["spaceIds", "format"],
      properties: {
        spaceIds: { type: "array", items: idSchema, maxItems: MAX_SCOPE_SPACE_IDS },
        format: { type: "string", enum: ["json", "jsonl", "markdown"] },
        includeEvents: { type: "boolean" },
        driveTargetRef: nullableString,
        metadata: nullableJsonObject
      }
    },
    MemoryExportJob: {
      type: "object",
      required: ["exportJobId", "state", "format", "createdAt", "updatedAt"],
      properties: {
        exportJobId: idSchema,
        state: { type: "string" },
        format: { type: "string" },
        driveObjectRef: nullableString,
        result: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant
      }
    },
    MemoryExportJobList: pageSchema("MemoryExportJob"),
    MemoryLearningSettings: {
      type: "object",
      required: ["autoExtractEnabled", "autoApproveThreshold", "reviewRequiredBelowThreshold", "habitPromotionThreshold", "updatedAt", "version"],
      properties: {
        autoExtractEnabled: { type: "boolean" },
        autoApproveThreshold: { type: "number", format: "double" },
        reviewRequiredBelowThreshold: { type: "boolean" },
        habitPromotionThreshold: { type: "number", format: "double" },
        retentionPolicyRef: nullableString,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryLearningSettingsRequest: {
      type: "object",
      properties: {
        autoExtractEnabled: { type: "boolean" },
        autoApproveThreshold: { type: "number", format: "double" },
        reviewRequiredBelowThreshold: { type: "boolean" },
        habitPromotionThreshold: { type: "number", format: "double" },
        retentionPolicyRef: nullableString,
        version: nullableIdSchema
      }
    },
    MemoryIndex: {
      type: "object",
      required: ["indexId", "indexKind", "schemaVersion", "status", "createdAt", "updatedAt", "version"],
      properties: {
        indexId: idSchema,
        spaceId: nullableIdSchema,
        indexKind: { type: "string", enum: ["sql", "keyword", "dictionary", "time", "event", "vector", "graph", "grep_file", "custom"] },
        implementationProfileId: nullableIdSchema,
        providerBindingId: nullableIdSchema,
        schemaVersion: { type: "string" },
        status: { type: "string" },
        config: nullableJsonObject,
        lastRebuiltAt: nullableInstant,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryIndexRequest: {
      type: "object",
      required: ["indexKind", "schemaVersion"],
      properties: {
        spaceId: nullableIdSchema,
        indexKind: { type: "string" },
        implementationProfileId: nullableIdSchema,
        providerBindingId: nullableIdSchema,
        schemaVersion: { type: "string" },
        config: nullableJsonObject,
        status: { type: "string" },
        version: nullableIdSchema
      }
    },
    MemoryIndexList: pageSchema("MemoryIndex"),
    MemoryRetrievalProfile: {
      type: "object",
      required: ["retrievalProfileId", "name", "strategy", "retrievers", "topK", "contextBudgetTokens", "status", "createdAt", "updatedAt", "version"],
      properties: {
        retrievalProfileId: idSchema,
        spaceId: nullableIdSchema,
        name: { type: "string" },
        strategy: { type: "string" },
        retrievers: jsonObject,
        fusionPolicy: nullableJsonObject,
        rerankPolicy: nullableJsonObject,
        topK: { type: "integer", format: "int32" },
        contextBudgetTokens: { type: "integer", format: "int32" },
        status: { type: "string" },
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryRetrievalProfileRequest: {
      type: "object",
      required: ["name", "strategy", "retrievers", "topK", "contextBudgetTokens"],
      properties: {
        spaceId: nullableIdSchema,
        name: { type: "string" },
        strategy: { type: "string" },
        retrievers: jsonObject,
        fusionPolicy: nullableJsonObject,
        rerankPolicy: nullableJsonObject,
        topK: { type: "integer", format: "int32" },
        contextBudgetTokens: { type: "integer", format: "int32" },
        status: { type: "string" },
        version: nullableIdSchema
      }
    },
    MemoryRetrievalProfileList: pageSchema("MemoryRetrievalProfile"),
    MemoryImplementationProfile: {
      type: "object",
      required: ["implementationProfileId", "name", "implementationKind", "role", "status", "capabilities", "createdAt", "updatedAt", "version"],
      properties: {
        implementationProfileId: idSchema,
        name: { type: "string" },
        implementationKind: { type: "string", enum: ["native_sql", "event_sourced", "graph_temporal", "search_first", "local_embedded", "external_provider_bridge", "hybrid_platform"] },
        role: { type: "string" },
        status: { type: "string" },
        capabilities: jsonObject,
        config: nullableJsonObject,
        rollout: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryImplementationProfileRequest: {
      type: "object",
      required: ["name", "implementationKind", "role", "capabilities"],
      properties: {
        name: { type: "string" },
        implementationKind: { type: "string" },
        role: { type: "string" },
        status: { type: "string" },
        capabilities: jsonObject,
        config: nullableJsonObject,
        rollout: nullableJsonObject,
        version: nullableIdSchema
      }
    },
    MemoryImplementationProfileList: pageSchema("MemoryImplementationProfile"),
    MemoryProviderBinding: {
      type: "object",
      required: ["providerBindingId", "providerKind", "providerCode", "displayName", "capabilities", "healthState", "createdAt", "updatedAt", "version"],
      properties: {
        providerBindingId: idSchema,
        providerKind: { type: "string" },
        providerCode: { type: "string" },
        displayName: { type: "string" },
        endpointRef: nullableString,
        secretRef: nullableString,
        modelRef: nullableString,
        capabilities: jsonObject,
        config: nullableJsonObject,
        healthState: { type: "string" },
        lastHealthAt: nullableInstant,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryProviderBindingRequest: {
      type: "object",
      required: ["providerKind", "providerCode", "displayName", "capabilities"],
      properties: {
        providerKind: { type: "string" },
        providerCode: { type: "string" },
        displayName: { type: "string" },
        endpointRef: nullableString,
        secretRef: nullableString,
        modelRef: nullableString,
        capabilities: jsonObject,
        config: nullableJsonObject,
        healthState: { type: "string" },
        version: nullableIdSchema
      }
    },
    MemoryProviderBindingList: pageSchema("MemoryProviderBinding"),
    MemoryProviderHealth: {
      type: "object",
      required: ["status", "checkedAt", "providers"],
      properties: {
        status: { type: "string", enum: ["healthy", "degraded", "unhealthy", "unknown"] },
        checkedAt: instant,
        providers: { type: "array", items: schemaRef("MemoryProviderBinding") }
      }
    },
    MemoryCapabilities: {
      type: "object",
      required: ["embeddingOptional", "retrievers", "providerInterfaces", "implementationKinds", "openApiPrefix", "sdkFamily", "checkedAt"],
      properties: {
        embeddingOptional: { type: "boolean" },
        retrievers: {
          type: "array",
          items: {
            type: "string",
            enum: ["sql", "keyword", "dictionary", "time", "event", "vector", "graph", "grep_file", "custom"]
          }
        },
        providerInterfaces: {
          type: "array",
          items: {
            type: "string",
            enum: ["llm", "embedding", "rerank", "tokenizer", "graph", "search", "file", "memory"]
          }
        },
        implementationKinds: {
          type: "array",
          items: {
            type: "string",
            enum: ["native_sql", "event_sourced", "graph_temporal", "search_first", "local_embedded", "external_provider_bridge", "hybrid_platform"]
          }
        },
        openApiPrefix: { type: "string" },
        sdkFamily: { type: "string" },
        checkedAt: instant,
        metadata: nullableJsonObject
      }
    },
    MemoryEvalRun: {
      type: "object",
      required: ["evalRunId", "evalType", "state", "createdAt", "updatedAt"],
      properties: {
        evalRunId: idSchema,
        evalType: { type: "string" },
        state: { type: "string" },
        datasetRef: nullableString,
        profileRef: nullableString,
        metrics: nullableJsonObject,
        result: nullableJsonObject,
        startedAt: nullableInstant,
        finishedAt: nullableInstant,
        createdAt: instant,
        updatedAt: instant
      }
    },
    MemoryEvalRunRequest: {
      type: "object",
      required: ["evalType"],
      properties: {
        evalType: {
          type: "string",
          enum: ["retrieval_quality"],
          description: "Executable evaluation engine. Additional evaluation types remain unavailable until a production worker implementation exists."
        },
        datasetRef: nullableString,
        profileRef: nullableString,
        config: nullableJsonObject
      }
    },
    MemoryEvalRunList: pageSchema("MemoryEvalRun"),
    MemoryAuditLog: {
      type: "object",
      required: ["auditLogId", "actorType", "action", "resourceType", "result", "createdAt"],
      properties: {
        auditLogId: idSchema,
        actorType: { type: "string" },
        actorId: nullableString,
        action: { type: "string" },
        resourceType: { type: "string" },
        resourceId: nullableString,
        requestId: nullableString,
        traceId: nullableString,
        result: { type: "string" },
        reason: nullableString,
        metadata: nullableJsonObject,
        createdAt: instant
      }
    },
    MemoryAuditLogList: pageSchema("MemoryAuditLog"),
    MemoryRetentionJobRequest: {
      type: "object",
      required: ["scope", "reason"],
      properties: {
        scope: { type: "string" },
        reason: { type: "string", minLength: 1, maxLength: 500 },
        spaceId: nullableIdSchema,
        dryRun: { type: "boolean" },
        policyRef: nullableString,
        metadata: nullableJsonObject
      }
    },
    MemoryMigrationJobRequest: {
      type: "object",
      required: ["sourceImplementationProfileId", "targetImplementationProfileId", "mode", "reason"],
      properties: {
        sourceImplementationProfileId: idSchema,
        targetImplementationProfileId: idSchema,
        mode: { type: "string", enum: ["shadow", "dual_write", "backfill", "cutover", "rollback"] },
        reason: { type: "string", minLength: 1, maxLength: 500 },
        spaceIds: { anyOf: [{ type: "array", items: idSchema }, { type: "null" }] },
        dryRun: { type: "boolean" },
        metadata: nullableJsonObject
      }
    },
    MemorySubject: {
      type: "object",
      required: ["subjectId", "tenantId", "subjectType", "subjectRef", "displayName", "status", "createdAt", "updatedAt", "version"],
      properties: {
        subjectId: idSchema,
        tenantId: idSchema,
        organizationId: nullableIdSchema,
        subjectType: { type: "string", enum: ["tenant", "organization", "user", "application", "service"] },
        subjectRef: { type: "string" },
        displayName: { type: "string" },
        defaultSpaceId: nullableIdSchema,
        status: { type: "string" },
        metadata: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemorySubjectRequest: {
      type: "object",
      required: ["subjectType", "subjectRef", "displayName"],
      properties: {
        organizationId: nullableIdSchema,
        subjectType: { type: "string", enum: ["tenant", "organization", "user", "application", "service"] },
        subjectRef: { type: "string" },
        displayName: { type: "string" },
        defaultSpaceId: nullableIdSchema,
        metadata: nullableJsonObject
      }
    },
    MemorySubjectPatch: {
      type: "object",
      properties: {
        displayName: { type: "string" },
        defaultSpaceId: nullableIdSchema,
        status: { type: "string" },
        metadata: nullableJsonObject
      }
    },
    MemorySubjectList: pageSchema("MemorySubject"),
    MemoryBinding: {
      type: "object",
      required: ["bindingId", "tenantId", "bindingKind", "bindingRole", "status", "createdAt", "updatedAt", "version"],
      properties: {
        bindingId: idSchema,
        tenantId: idSchema,
        spaceId: nullableIdSchema,
        bindingKind: { type: "string", enum: ["ownership", "access", "share", "reference", "provision"] },
        bindingRole: { type: "string" },
        sourceSubjectId: nullableIdSchema,
        targetSubjectId: nullableIdSchema,
        targetSpaceId: nullableIdSchema,
        capabilityCodes: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        status: { type: "string" },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryBindingRequest: {
      type: "object",
      required: ["bindingKind", "bindingRole"],
      properties: {
        spaceId: nullableIdSchema,
        bindingKind: { type: "string", enum: ["ownership", "access", "share", "reference", "provision"] },
        bindingRole: { type: "string" },
        sourceSubjectId: nullableIdSchema,
        targetSubjectId: nullableIdSchema,
        targetSpaceId: nullableIdSchema,
        capabilityCodes: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject
      }
    },
    MemoryBindingList: pageSchema("MemoryBinding"),
    MemoryCapabilityBinding: {
      type: "object",
      required: ["capabilityBindingId", "tenantId", "capabilityCode", "targetType", "targetId", "mode", "priority", "status", "createdAt", "updatedAt", "version"],
      properties: {
        capabilityBindingId: idSchema,
        tenantId: idSchema,
        capabilityCode: { type: "string" },
        targetType: { type: "string", enum: ["subject", "space", "binding", "memory"] },
        targetId: idSchema,
        mode: { type: "string", enum: ["allow", "deny", "conditional"] },
        priority: { type: "integer", format: "int32" },
        status: { type: "string" },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryCapabilityBindingRequest: {
      type: "object",
      required: ["capabilityCode", "targetType", "targetId", "mode"],
      properties: {
        capabilityCode: { type: "string" },
        targetType: { type: "string", enum: ["subject", "space", "binding", "memory"] },
        targetId: idSchema,
        mode: { type: "string", enum: ["allow", "deny", "conditional"] },
        priority: { type: "integer", format: "int32" },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject
      }
    },
    MemoryCapabilityBindingList: pageSchema("MemoryCapabilityBinding"),
    MemoryResolvedCapability: {
      type: "object",
      required: ["capabilityCode", "mode", "priority", "source"],
      properties: {
        capabilityCode: { type: "string" },
        mode: { type: "string", enum: ["allow", "deny", "conditional"] },
        priority: { type: "integer", format: "int32" },
        source: { type: "string" }
      }
    },
    MemoryResolvedCapabilityList: pageSchema("MemoryResolvedCapability"),
    MemoryResolvedCapabilityListResponse: {
      allOf: [
        schemaRef("SdkWorkApiResponse"),
        {
          type: "object",
          required: ["data"],
          properties: {
            data: schemaRef("MemoryResolvedCapabilityList")
          }
        }
      ]
    },
    MemoryResolveCapabilitiesRequest: {
      type: "object",
      required: ["targetType", "targetId"],
      properties: {
        targetType: { type: "string", enum: ["subject", "space", "binding", "memory"] },
        targetId: idSchema
      }
    },
    MemoryEntity: {
      type: "object",
      required: ["entityId", "spaceId", "entityType", "canonicalName", "sensitivityLevel", "status", "createdAt", "updatedAt", "version"],
      properties: {
        entityId: idSchema,
        spaceId: idSchema,
        entityType: { type: "string" },
        canonicalName: { type: "string" },
        aliases: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        attributes: nullableJsonObject,
        sensitivityLevel: { type: "string" },
        status: { type: "string" },
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryEntityRequest: {
      type: "object",
      required: ["spaceId", "entityType", "canonicalName"],
      properties: {
        spaceId: idSchema,
        entityType: { type: "string" },
        canonicalName: { type: "string" },
        aliases: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        attributes: nullableJsonObject,
        sensitivityLevel: { type: "string" }
      }
    },
    MemoryEntityPatch: {
      type: "object",
      properties: {
        canonicalName: { type: "string" },
        aliases: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        attributes: nullableJsonObject,
        sensitivityLevel: { type: "string" },
        status: { type: "string" }
      }
    },
    MemoryEntityList: pageSchema("MemoryEntity"),
    MemoryEdge: {
      type: "object",
      required: ["edgeId", "spaceId", "sourceEntityId", "targetEntityId", "relationType", "status", "createdAt", "updatedAt", "version"],
      properties: {
        edgeId: idSchema,
        spaceId: idSchema,
        sourceEntityId: idSchema,
        targetEntityId: idSchema,
        relationType: { type: "string" },
        sourceMemoryId: nullableString,
        weight: { anyOf: [{ type: "number" }, { type: "null" }] },
        status: { type: "string" },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryEdgeRequest: {
      type: "object",
      required: ["spaceId", "sourceEntityId", "targetEntityId", "relationType"],
      properties: {
        spaceId: idSchema,
        sourceEntityId: idSchema,
        targetEntityId: idSchema,
        relationType: { type: "string" },
        sourceMemoryId: nullableString,
        weight: { anyOf: [{ type: "number" }, { type: "null" }] },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject
      }
    },
    MemoryEdgePatch: {
      type: "object",
      properties: {
        relationType: { type: "string" },
        weight: { anyOf: [{ type: "number" }, { type: "null" }] },
        status: { type: "string" },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        metadata: nullableJsonObject
      }
    },
    MemoryEdgeList: pageSchema("MemoryEdge"),
    MemoryPolicy: {
      type: "object",
      required: ["policyId", "tenantId", "policyType", "scope", "status", "policy", "createdAt", "updatedAt", "version"],
      properties: {
        policyId: idSchema,
        tenantId: idSchema,
        policyType: { type: "string" },
        scope: { type: "string" },
        scopeRef: nullableString,
        status: { type: "string" },
        policy: nullableJsonObject,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryPolicyRequest: {
      type: "object",
      required: ["policyType", "scope", "policy"],
      properties: {
        policyType: { type: "string" },
        scope: { type: "string" },
        scopeRef: nullableString,
        policy: nullableJsonObject
      }
    },
    MemoryPolicyPatch: {
      type: "object",
      properties: {
        policyType: { type: "string" },
        scope: { type: "string" },
        scopeRef: nullableString,
        policy: nullableJsonObject,
        status: { type: "string" }
      }
    },
    MemoryPolicyList: pageSchema("MemoryPolicy"),
    MemoryPolicyAssignment: {
      type: "object",
      required: ["policyAssignmentId", "tenantId", "policyId", "targetType", "targetId", "priority", "inheritanceMode", "status", "createdAt", "updatedAt", "version"],
      properties: {
        policyAssignmentId: idSchema,
        tenantId: idSchema,
        policyId: idSchema,
        targetType: { type: "string", enum: ["subject", "space", "entity", "binding", "capability_binding", "implementation_profile"] },
        targetId: idSchema,
        priority: { type: "integer", format: "int32" },
        inheritanceMode: { type: "string", enum: ["inherit", "override", "deny", "shadow"] },
        status: { type: "string" },
        validFrom: nullableInstant,
        validTo: nullableInstant,
        createdAt: instant,
        updatedAt: instant,
        version: idSchema
      }
    },
    MemoryPolicyAssignmentRequest: {
      type: "object",
      required: ["policyId", "targetType", "targetId", "inheritanceMode"],
      properties: {
        policyId: idSchema,
        targetType: { type: "string", enum: ["subject", "space", "entity", "binding", "capability_binding", "implementation_profile"] },
        targetId: idSchema,
        priority: { type: "integer", format: "int32" },
        inheritanceMode: { type: "string", enum: ["inherit", "override", "deny", "shadow"] },
        validFrom: nullableInstant,
        validTo: nullableInstant
      }
    },
    MemoryPolicyAssignmentPatch: {
      type: "object",
      properties: {
        priority: { type: "integer", format: "int32" },
        inheritanceMode: { type: "string", enum: ["inherit", "override", "deny", "shadow"] },
        status: { type: "string" },
        validFrom: nullableInstant,
        validTo: nullableInstant
      }
    },
    MemoryPolicyAssignmentList: pageSchema("MemoryPolicyAssignment"),
    MemoryCommercialReadiness: {
      type: "object",
      required: ["readinessId", "tenantId", "score", "state", "createdAt"],
      properties: {
        readinessId: idSchema,
        tenantId: idSchema,
        implementationProfileId: nullableIdSchema,
        score: { type: "number" },
        state: { type: "string" },
        contractCoverage: nullableJsonObject,
        managementCoverage: nullableJsonObject,
        runtimeConformance: nullableJsonObject,
        privacyCoverage: nullableJsonObject,
        auditCoverage: nullableJsonObject,
        sdkCoverage: nullableJsonObject,
        evaluationCoverage: nullableJsonObject,
        observabilityCoverage: nullableJsonObject,
        migrationCoverage: nullableJsonObject,
        blockingFindings: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        warningFindings: { anyOf: [{ type: "array", items: { type: "string" } }, { type: "null" }] },
        createdAt: instant
      }
    },
    MemoryCommercialReadinessRequest: {
      type: "object",
      required: [],
      properties: {
        implementationProfileId: nullableIdSchema
      }
    }
  };
}

function securitySchemesFor(authMode) {
  if (authMode === "api-key") {
    return {
      ApiKey: {
        type: "apiKey",
        in: "header",
        name: "X-API-Key"
      }
    };
  }

  return {
    AuthToken: {
      type: "http",
      scheme: "bearer",
      bearerFormat: "JWT"
    },
    AccessToken: {
      type: "apiKey",
      in: "header",
      name: "Access-Token"
    }
  };
}

function createOpenApi({ title, authority, prefix, sdkFamily, paths, authMode = "dual-token" }) {
  return {
    openapi: "3.1.2",
    info: {
      title,
      version
    },
    servers: [
      { url: "https://api.sdkwork.com", description: "SDKWork production API" },
      { url: "http://localhost:8080", description: "Local/private API" }
    ],
    paths,
    components: {
      schemas: baseSchemas(),
      securitySchemes: securitySchemesFor(authMode)
    },
    "x-sdkwork-owner": owner,
    "x-sdkwork-api-authority": authority,
    "x-sdkwork-sdk-family": sdkFamily,
    "x-sdkwork-owner-only-input": true,
    "x-sdkwork-standard-version": standardVersion,
    "x-sdkwork-domain": domain,
    "x-sdkwork-api-prefix": prefix
  };
}

function openOperation(args) {
  return operation({ ...args, authMode: "api-key" });
}

function writeOpenApi() {
  const paths = {};
  const authority = "sdkwork-memory-open-api";
  const P = `${memoryOpenApiPrefix}/memory`;

  addPath(paths, `${P}/capabilities`, "get", openOperation({
    method: "get",
    authority,
    operationId: "capabilities.retrieve",
    permission: "memory.open.capabilities.read",
    auditEvent: "memory.open.capabilities.read",
    responseSchema: "MemoryCapabilities"
  }));

  addPath(paths, `${P}/events`, "post", openOperation({
    method: "post",
    authority,
    operationId: "events.create",
    permission: "memory.open.events.write",
    auditEvent: "memory.open.event.appended",
    requestSchema: "MemoryEventRequest",
    responseSchema: "MemoryEvent",
    idempotent: true
  }));
  addPath(paths, `${P}/events/{eventId}`, "get", openOperation({
    method: "get",
    authority,
    operationId: "events.retrieve",
    permission: "memory.open.events.read",
    auditEvent: "memory.open.event.read",
    pathParams: [pathParam("eventId")],
    queryParams: [requiredSpaceIdQueryParam()],
    responseSchema: "MemoryEvent"
  }));

  addPath(paths, `${P}/memories`, "get", openOperation({
    method: "get",
    authority,
    operationId: "memories.list",
    permission: "memory.open.records.read",
    auditEvent: "memory.open.record.list",
    // Contract parity (tests/contracts/openapi_query_param_parity_test.mjs):
    // only filters the ListMemoriesQuery DTO + service actually forward to the
    // store are declared. external_subject_ref and memory_type had no
    // service/store implementation and were removed pre-launch.
    queryParams: listParams([
      { name: "space_id", in: "query", required: true, schema: idSchema },
      { name: "show_expired", in: "query", schema: { type: "boolean" } }
    ]),
    responseSchema: "MemoryRecordList"
  }));
  addPath(paths, `${P}/memories`, "post", openOperation({
    method: "post",
    authority,
    operationId: "memories.create",
    permission: "memory.open.records.write",
    auditEvent: "memory.open.record.created",
    requestSchema: "MemoryRecordRequest",
    responseSchema: "MemoryRecord",
    idempotent: true
  }));
  addPath(paths, `${P}/memories/{memoryId}`, "get", openOperation({
    method: "get",
    authority,
    operationId: "memories.retrieve",
    permission: "memory.open.records.read",
    auditEvent: "memory.open.record.read",
    pathParams: [pathParam("memoryId")],
    queryParams: [requiredSpaceIdQueryParam()],
    responseSchema: "MemoryRecord"
  }));
  addPath(paths, `${P}/memories/{memoryId}`, "patch", openOperation({
    method: "patch",
    authority,
    operationId: "memories.update",
    permission: "memory.open.records.write",
    auditEvent: "memory.open.record.updated",
    pathParams: [pathParam("memoryId")],
    queryParams: [requiredSpaceIdQueryParam()],
    requestSchema: "MemoryRecordRequest",
    responseSchema: "MemoryRecord"
  }));
  addPath(paths, `${P}/memories/{memoryId}`, "delete", openOperation({
    method: "delete",
    authority,
    operationId: "memories.delete",
    permission: "memory.open.records.write",
    auditEvent: "memory.open.record.deleted",
    pathParams: [pathParam("memoryId")],
    queryParams: [requiredSpaceIdQueryParam()],
    responseSchema: "MemoryRecord",
    status: "204"
  }));
  addPath(paths, `${P}/memories/delete_all`, "post", openOperation({
    method: "post",
    authority,
    operationId: "memories.deleteAll",
    permission: "memory.open.records.write",
    auditEvent: "memory.open.record.deleted",
    requestSchema: "DeleteAllMemoriesRequest",
    responseSchema: "DeleteAllMemoriesResult",
    status: "200",
    idempotent: true
  }));

  addPath(paths, `${P}/retrievals`, "post", openOperation({
    method: "post",
    authority,
    operationId: "retrievals.create",
    permission: "memory.open.retrievals.write",
    auditEvent: "memory.open.retrieval.created",
    requestSchema: "MemoryRetrievalRequest",
    responseSchema: "MemoryRetrievalResult",
    idempotent: true
  }));
  addPath(paths, `${P}/retrievals/{retrievalId}`, "get", openOperation({
    method: "get",
    authority,
    operationId: "retrievals.retrieve",
    permission: "memory.open.retrievals.read",
    auditEvent: "memory.open.retrieval.read",
    pathParams: [pathParam("retrievalId")],
    responseSchema: "MemoryRetrievalResult"
  }));

  addPath(paths, `${P}/context_packs`, "post", openOperation({
    method: "post",
    authority,
    operationId: "contextPacks.create",
    permission: "memory.open.contextPacks.write",
    auditEvent: "memory.open.context_pack.created",
    requestSchema: "MemoryContextPackRequest",
    responseSchema: "MemoryContextPack",
    idempotent: true
  }));
  addPath(paths, `${P}/context_packs/{contextPackId}`, "get", openOperation({
    method: "get",
    authority,
    operationId: "contextPacks.retrieve",
    permission: "memory.open.contextPacks.read",
    auditEvent: "memory.open.context_pack.read",
    pathParams: [pathParam("contextPackId")],
    responseSchema: "MemoryContextPack"
  }));

  addPath(paths, `${P}/feedback`, "post", openOperation({
    method: "post",
    authority,
    operationId: "feedback.create",
    permission: "memory.open.feedback.write",
    auditEvent: "memory.open.feedback.created",
    requestSchema: "MemoryFeedbackRequest",
    responseSchema: "MemoryFeedback",
    idempotent: true
  }));

  addPath(paths, `${P}/extractions`, "post", openOperation({
    method: "post",
    authority,
    operationId: "extractions.create",
    permission: "memory.open.learning.write",
    auditEvent: "memory.open.extraction.requested",
    requestSchema: "MemoryExtractionRequest",
    responseSchema: "MemoryLearningJob",
    idempotent: true
  }));

  addPath(paths, `${P}/candidates`, "get", openOperation({
    method: "get",
    authority,
    operationId: "candidates.list",
    permission: "memory.open.candidates.read",
    auditEvent: "memory.open.candidate.list",
    // q/decision_state were never implemented by the candidate store query;
    // space_id is required by the service (access::require_list_space_id).
    queryParams: cursorListParams([
      { name: "space_id", in: "query", required: true, schema: idSchema }
    ]),
    responseSchema: "MemoryCandidateList"
  }));
  addPath(paths, `${P}/candidates/{candidateId}`, "get", openOperation({
    method: "get",
    authority,
    operationId: "candidates.retrieve",
    permission: "memory.open.candidates.read",
    auditEvent: "memory.open.candidate.read",
    pathParams: [pathParam("candidateId")],
    responseSchema: "MemoryCandidate"
  }));

  addPath(paths, `${P}/provider_health`, "get", openOperation({
    method: "get",
    authority,
    operationId: "providerHealth.retrieve",
    permission: "memory.open.providerHealth.read",
    auditEvent: "memory.open.provider_health.read",
    responseSchema: "MemoryProviderHealth"
  }));

  // Commercial graph management.
  addPath(paths, `${P}/entities`, "get", openOperation({ method: "get", authority, operationId: "entities.list", permission: "memory.open.entities.read", auditEvent: "memory.open.entity.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: true, schema: idSchema }, { name: "entity_type", in: "query", required: false, schema: { type: "string" } }, { name: "status", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryEntityList" }));
  addPath(paths, `${P}/entities`, "post", openOperation({ method: "post", authority, operationId: "entities.create", permission: "memory.open.entities.write", auditEvent: "memory.open.entity.created", requestSchema: "MemoryEntityRequest", responseSchema: "MemoryEntity", idempotent: true }));
  addPath(paths, `${P}/entities/{entityId}`, "get", openOperation({ method: "get", authority, operationId: "entities.retrieve", permission: "memory.open.entities.read", auditEvent: "memory.open.entity.read", pathParams: [pathParam("entityId")], responseSchema: "MemoryEntity" }));
  addPath(paths, `${P}/entities/{entityId}`, "patch", openOperation({ method: "patch", authority, operationId: "entities.update", permission: "memory.open.entities.write", auditEvent: "memory.open.entity.updated", pathParams: [pathParam("entityId")], requestSchema: "MemoryEntityPatch", responseSchema: "MemoryEntity" }));
  addPath(paths, `${P}/edges`, "get", openOperation({ method: "get", authority, operationId: "edges.list", permission: "memory.open.entities.read", auditEvent: "memory.open.edge.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: true, schema: idSchema }, { name: "source_entity_id", in: "query", required: false, schema: idSchema }, { name: "relation_type", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryEdgeList" }));
  addPath(paths, `${P}/edges`, "post", openOperation({ method: "post", authority, operationId: "edges.create", permission: "memory.open.entities.write", auditEvent: "memory.open.edge.created", requestSchema: "MemoryEdgeRequest", responseSchema: "MemoryEdge", idempotent: true }));
  addPath(paths, `${P}/edges/{edgeId}`, "get", openOperation({ method: "get", authority, operationId: "edges.retrieve", permission: "memory.open.entities.read", auditEvent: "memory.open.edge.read", pathParams: [pathParam("edgeId")], responseSchema: "MemoryEdge" }));
  addPath(paths, `${P}/edges/{edgeId}`, "patch", openOperation({ method: "patch", authority, operationId: "edges.update", permission: "memory.open.entities.write", auditEvent: "memory.open.edge.updated", pathParams: [pathParam("edgeId")], requestSchema: "MemoryEdgePatch", responseSchema: "MemoryEdge" }));
  addPath(paths, `${P}/edges/{edgeId}`, "delete", openOperation({ method: "delete", authority, operationId: "edges.delete", permission: "memory.open.entities.write", auditEvent: "memory.open.edge.deleted", pathParams: [pathParam("edgeId")], responseSchema: "MemoryEdge", status: "204" }));

  writeAlignedOpenApi("sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json", createOpenApi({
    title: "SDKWork Memory Open API",
    authority,
    prefix: memoryOpenApiPrefix,
    sdkFamily: "sdkwork-memory-sdk",
    paths,
    authMode: "api-key"
  }));
}

function writeAppOpenApi() {
  const paths = {};
  const authority = "sdkwork-memory.app";
  const P = "/app/v3/api/memory";
  addPath(paths, `${P}/spaces`, "get", operation({ method: "get", authority, operationId: "spaces.list", permission: "memory.spaces.read", auditEvent: "memory.space.list", queryParams: cursorListParams(), responseSchema: "MemorySpaceList" }));
  addPath(paths, `${P}/spaces`, "post", operation({ method: "post", authority, operationId: "spaces.create", permission: "memory.spaces.write", auditEvent: "memory.space.created", requestSchema: "MemorySpaceRequest", responseSchema: "MemorySpace", idempotent: true }));
  addPath(paths, `${P}/spaces/{spaceId}`, "get", operation({ method: "get", authority, operationId: "spaces.retrieve", permission: "memory.spaces.read", auditEvent: "memory.space.read", pathParams: [pathParam("spaceId")], responseSchema: "MemorySpace" }));
  addPath(paths, `${P}/spaces/{spaceId}`, "patch", operation({ method: "patch", authority, operationId: "spaces.update", permission: "memory.spaces.write", auditEvent: "memory.space.updated", pathParams: [pathParam("spaceId")], requestSchema: "MemorySpaceRequest", responseSchema: "MemorySpace" }));

  addPath(paths, `${P}/events`, "post", operation({ method: "post", authority, operationId: "events.create", permission: "memory.events.write", auditEvent: "memory.event.appended", requestSchema: "MemoryEventRequest", responseSchema: "MemoryEvent", idempotent: true }));
  addPath(paths, `${P}/events/{eventId}`, "get", operation({ method: "get", authority, operationId: "events.retrieve", permission: "memory.events.read", auditEvent: "memory.event.read", pathParams: [pathParam("eventId")], queryParams: [requiredSpaceIdQueryParam()], responseSchema: "MemoryEvent" }));

  addPath(paths, `${P}/memories`, "get", operation({ method: "get", authority, operationId: "memories.list", permission: "memory.records.read", auditEvent: "memory.record.list", queryParams: listParams([{ name: "space_id", in: "query", required: true, schema: idSchema }, { name: "show_expired", in: "query", schema: { type: "boolean" } }]), responseSchema: "MemoryRecordList" }));
  addPath(paths, `${P}/memories`, "post", operation({ method: "post", authority, operationId: "memories.create", permission: "memory.records.write", auditEvent: "memory.record.created", requestSchema: "MemoryRecordRequest", responseSchema: "MemoryRecord", idempotent: true }));
  addPath(paths, `${P}/memories/{memoryId}`, "get", operation({ method: "get", authority, operationId: "memories.retrieve", permission: "memory.records.read", auditEvent: "memory.record.read", pathParams: [pathParam("memoryId")], queryParams: [requiredSpaceIdQueryParam()], responseSchema: "MemoryRecord" }));
  addPath(paths, `${P}/memories/{memoryId}`, "patch", operation({ method: "patch", authority, operationId: "memories.update", permission: "memory.records.write", auditEvent: "memory.record.updated", pathParams: [pathParam("memoryId")], queryParams: [requiredSpaceIdQueryParam()], requestSchema: "MemoryRecordRequest", responseSchema: "MemoryRecord" }));
  addPath(paths, `${P}/memories/{memoryId}`, "delete", operation({ method: "delete", authority, operationId: "memories.delete", permission: "memory.records.write", auditEvent: "memory.record.deleted", pathParams: [pathParam("memoryId")], queryParams: [requiredSpaceIdQueryParam()], responseSchema: "MemoryRecord", status: "204" }));
addPath(paths, `${P}/memories/delete_all`, "post", operation({ method: "post", authority, operationId: "memories.deleteAll", permission: "memory.records.write", auditEvent: "memory.record.deleted", requestSchema: "DeleteAllMemoriesRequest", responseSchema: "DeleteAllMemoriesResult", status: "200", idempotent: true }));
  addPath(paths, `${P}/memories/{memoryId}/sources`, "get", operation({ method: "get", authority, operationId: "memories.sources.list", permission: "memory.records.read", auditEvent: "memory.record.sources.list", pathParams: [pathParam("memoryId")], queryParams: listParams(), responseSchema: "MemoryRecordSourceList" }));

  addPath(paths, `${P}/forget_requests`, "get", operation({ method: "get", authority, operationId: "forgetRequests.list", permission: "memory.forget.read", auditEvent: "memory.forget.list", queryParams: cursorListParams(), responseSchema: "MemoryForgetJobList" }));
  addPath(paths, `${P}/forget_requests`, "post", operation({ method: "post", authority, operationId: "forgetRequests.create", permission: "memory.forget.write", auditEvent: "memory.forget.requested", requestSchema: "MemoryForgetRequest", responseSchema: "MemoryForgetJob", idempotent: true }));
  addPath(paths, `${P}/forget_requests/{forgetRequestId}`, "get", operation({ method: "get", authority, operationId: "forgetRequests.retrieve", permission: "memory.forget.read", auditEvent: "memory.forget.read", pathParams: [pathParam("forgetRequestId")], responseSchema: "MemoryForgetJob" }));

  addPath(paths, `${P}/extractions`, "post", operation({ method: "post", authority, operationId: "extractions.create", permission: "memory.learning.write", auditEvent: "memory.extraction.requested", requestSchema: "MemoryExtractionRequest", responseSchema: "MemoryLearningJob", idempotent: true }));

  addPath(paths, `${P}/candidates`, "get", operation({ method: "get", authority, operationId: "candidates.list", permission: "memory.candidates.read", auditEvent: "memory.candidate.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryCandidateList" }));
  addPath(paths, `${P}/candidates/{candidateId}`, "get", operation({ method: "get", authority, operationId: "candidates.retrieve", permission: "memory.candidates.read", auditEvent: "memory.candidate.read", pathParams: [pathParam("candidateId")], responseSchema: "MemoryCandidate" }));
  addPath(paths, `${P}/candidates/{candidateId}/approve`, "post", operation({ method: "post", authority, operationId: "candidates.approve", permission: "memory.candidates.write", auditEvent: "memory.candidate.approved", pathParams: [pathParam("candidateId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryCandidate", status: "200", idempotent: true }));
  addPath(paths, `${P}/candidates/{candidateId}/reject`, "post", operation({ method: "post", authority, operationId: "candidates.reject", permission: "memory.candidates.write", auditEvent: "memory.candidate.rejected", pathParams: [pathParam("candidateId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryCandidate", status: "200", idempotent: true }));

  addPath(paths, `${P}/habits`, "get", operation({ method: "get", authority, operationId: "habits.list", permission: "memory.habits.read", auditEvent: "memory.habit.list", queryParams: listParams([{ name: "stage", in: "query", schema: { type: "string" } }, { name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryHabitList" }));
  addPath(paths, `${P}/habits/{habitId}`, "get", operation({ method: "get", authority, operationId: "habits.retrieve", permission: "memory.habits.read", auditEvent: "memory.habit.read", pathParams: [pathParam("habitId")], responseSchema: "MemoryHabit" }));
  addPath(paths, `${P}/habits/{habitId}`, "patch", operation({ method: "patch", authority, operationId: "habits.update", permission: "memory.habits.write", auditEvent: "memory.habit.updated", pathParams: [pathParam("habitId")], requestSchema: "MemoryHabitRequest", responseSchema: "MemoryHabit" }));
  addPath(paths, `${P}/habits/{habitId}/confirm`, "post", operation({ method: "post", authority, operationId: "habits.confirm", permission: "memory.habits.write", auditEvent: "memory.habit.confirmed", pathParams: [pathParam("habitId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryHabit", status: "200", idempotent: true }));
  addPath(paths, `${P}/habits/{habitId}/reject`, "post", operation({ method: "post", authority, operationId: "habits.reject", permission: "memory.habits.write", auditEvent: "memory.habit.rejected", pathParams: [pathParam("habitId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryHabit", status: "200", idempotent: true }));

  addPath(paths, `${P}/retrievals`, "post", operation({ method: "post", authority, operationId: "retrievals.create", permission: "memory.retrievals.write", auditEvent: "memory.retrieval.created", requestSchema: "MemoryRetrievalRequest", responseSchema: "MemoryRetrievalResult", idempotent: true }));
  addPath(paths, `${P}/retrievals/{retrievalId}`, "get", operation({ method: "get", authority, operationId: "retrievals.retrieve", permission: "memory.retrievals.read", auditEvent: "memory.retrieval.read", pathParams: [pathParam("retrievalId")], responseSchema: "MemoryRetrievalResult" }));
  addPath(paths, `${P}/context_packs`, "post", operation({ method: "post", authority, operationId: "contextPacks.create", permission: "memory.contextPacks.write", auditEvent: "memory.context_pack.created", requestSchema: "MemoryContextPackRequest", responseSchema: "MemoryContextPack", idempotent: true }));
  addPath(paths, `${P}/context_packs/{contextPackId}`, "get", operation({ method: "get", authority, operationId: "contextPacks.retrieve", permission: "memory.contextPacks.read", auditEvent: "memory.context_pack.read", pathParams: [pathParam("contextPackId")], responseSchema: "MemoryContextPack" }));
  addPath(paths, `${P}/feedback`, "post", operation({ method: "post", authority, operationId: "feedback.create", permission: "memory.feedback.write", auditEvent: "memory.feedback.created", requestSchema: "MemoryFeedbackRequest", responseSchema: "MemoryFeedback", idempotent: true }));
  addPath(paths, `${P}/export_jobs`, "get", operation({ method: "get", authority, operationId: "exportJobs.list", permission: "memory.exports.read", auditEvent: "memory.export.list", queryParams: cursorListParams(), responseSchema: "MemoryExportJobList" }));
  addPath(paths, `${P}/export_jobs`, "post", operation({ method: "post", authority, operationId: "exportJobs.create", permission: "memory.exports.write", auditEvent: "memory.export.requested", requestSchema: "MemoryExportRequest", responseSchema: "MemoryExportJob", idempotent: true }));
  addPath(paths, `${P}/export_jobs/{exportJobId}`, "get", operation({ method: "get", authority, operationId: "exportJobs.retrieve", permission: "memory.exports.read", auditEvent: "memory.export.read", pathParams: [pathParam("exportJobId")], responseSchema: "MemoryExportJob" }));
  addPath(paths, `${P}/learning_settings`, "get", operation({ method: "get", authority, operationId: "learningSettings.retrieve", permission: "memory.learningSettings.read", auditEvent: "memory.learning_settings.read", responseSchema: "MemoryLearningSettings" }));
  addPath(paths, `${P}/learning_settings`, "patch", operation({ method: "patch", authority, operationId: "learningSettings.update", permission: "memory.learningSettings.write", auditEvent: "memory.learning_settings.updated", requestSchema: "MemoryLearningSettingsRequest", responseSchema: "MemoryLearningSettings" }));

  // Commercial management.
  addPath(paths, `${P}/entities`, "get", operation({ method: "get", authority, operationId: "entities.list", permission: "memory.app.entities.read", auditEvent: "memory.app.entity.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }, { name: "entity_type", in: "query", required: false, schema: { type: "string" } }, { name: "status", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryEntityList" }));
  addPath(paths, `${P}/entities`, "post", operation({ method: "post", authority, operationId: "entities.create", permission: "memory.app.entities.write", auditEvent: "memory.app.entity.created", requestSchema: "MemoryEntityRequest", responseSchema: "MemoryEntity", idempotent: true }));
  addPath(paths, `${P}/entities/{entityId}`, "get", operation({ method: "get", authority, operationId: "entities.retrieve", permission: "memory.app.entities.read", auditEvent: "memory.app.entity.read", pathParams: [pathParam("entityId")], responseSchema: "MemoryEntity" }));
  addPath(paths, `${P}/entities/{entityId}`, "patch", operation({ method: "patch", authority, operationId: "entities.update", permission: "memory.app.entities.write", auditEvent: "memory.app.entity.updated", pathParams: [pathParam("entityId")], requestSchema: "MemoryEntityPatch", responseSchema: "MemoryEntity" }));
  addPath(paths, `${P}/policy_assignments`, "get", operation({ method: "get", authority, operationId: "policyAssignments.list", permission: "memory.app.policies.write", auditEvent: "memory.app.policy_assignment.list", queryParams: cursorListParams([{ name: "target_type", in: "query", required: false, schema: { type: "string" } }, { name: "target_id", in: "query", required: false, schema: idSchema }, { name: "policy_id", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryPolicyAssignmentList" }));
  addPath(paths, `${P}/policy_assignments`, "post", operation({ method: "post", authority, operationId: "policyAssignments.create", permission: "memory.app.policies.write", auditEvent: "memory.app.policy_assignment.created", requestSchema: "MemoryPolicyAssignmentRequest", responseSchema: "MemoryPolicyAssignment", idempotent: true }));
  addPath(paths, `${P}/policy_assignments/{policyAssignmentId}`, "patch", operation({ method: "patch", authority, operationId: "policyAssignments.update", permission: "memory.app.policies.write", auditEvent: "memory.app.policy_assignment.updated", pathParams: [pathParam("policyAssignmentId")], requestSchema: "MemoryPolicyAssignmentPatch", responseSchema: "MemoryPolicyAssignment" }));

  writeAlignedOpenApi("sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json", createOpenApi({
    title: "SDKWork Memory App API",
    authority,
    prefix: "/app/v3/api",
    sdkFamily: "sdkwork-memory-app-sdk",
    paths
  }));
}

function writeBackendOpenApi() {
  const paths = {};
  const authority = "sdkwork-memory.backend";
  const P = "/backend/v3/api/memory";
  addPath(paths, `${P}/spaces`, "get", operation({ method: "get", authority, operationId: "spaces.list", permission: "memory.backend.spaces.read", auditEvent: "memory.backend.space.list", queryParams: cursorListParams(), responseSchema: "MemorySpaceList" }));
  addPath(paths, `${P}/spaces/{spaceId}`, "get", operation({ method: "get", authority, operationId: "spaces.retrieve", permission: "memory.backend.spaces.read", auditEvent: "memory.backend.space.read", pathParams: [pathParam("spaceId")], responseSchema: "MemorySpace" }));
  addPath(paths, `${P}/spaces/{spaceId}`, "patch", operation({ method: "patch", authority, operationId: "spaces.update", permission: "memory.backend.spaces.write", auditEvent: "memory.backend.space.updated", pathParams: [pathParam("spaceId")], requestSchema: "MemorySpaceRequest", responseSchema: "MemorySpace" }));
  addPath(paths, `${P}/memories`, "get", operation({ method: "get", authority, operationId: "memories.list", permission: "memory.backend.records.read", auditEvent: "memory.backend.record.list", queryParams: listParams([{ name: "space_id", in: "query", required: false, schema: idSchema }, { name: "show_expired", in: "query", schema: { type: "boolean" } }]), responseSchema: "MemoryRecordList" }));
  addPath(paths, `${P}/memories/{memoryId}`, "get", operation({ method: "get", authority, operationId: "memories.retrieve", permission: "memory.backend.records.read", auditEvent: "memory.backend.record.read", pathParams: [pathParam("memoryId")], queryParams: [requiredSpaceIdQueryParam()], responseSchema: "MemoryRecord" }));
  addPath(paths, `${P}/memories/{memoryId}`, "patch", operation({ method: "patch", authority, operationId: "memories.update", permission: "memory.backend.records.write", auditEvent: "memory.backend.record.updated", pathParams: [pathParam("memoryId")], queryParams: [requiredSpaceIdQueryParam()], requestSchema: "MemoryRecordRequest", responseSchema: "MemoryRecord" }));
  addPath(paths, `${P}/memories/{memoryId}/supersede`, "post", operation({ method: "post", authority, operationId: "memories.supersede", permission: "memory.backend.records.write", auditEvent: "memory.backend.record.superseded", pathParams: [pathParam("memoryId")], requestSchema: "MemoryRecordRequest", responseSchema: "MemoryRecord", status: "200", idempotent: true }));
  addPath(paths, `${P}/events`, "get", operation({ method: "get", authority, operationId: "events.list", permission: "memory.backend.events.read", auditEvent: "memory.backend.event.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryEventList" }));
  addPath(paths, `${P}/events/{eventId}`, "get", operation({ method: "get", authority, operationId: "events.retrieve", permission: "memory.backend.events.read", auditEvent: "memory.backend.event.read", pathParams: [pathParam("eventId")], queryParams: [requiredSpaceIdQueryParam()], responseSchema: "MemoryEvent" }));
  addPath(paths, `${P}/candidates`, "get", operation({ method: "get", authority, operationId: "candidates.list", permission: "memory.backend.candidates.read", auditEvent: "memory.backend.candidate.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryCandidateList" }));
  addPath(paths, `${P}/candidates/{candidateId}/approve`, "post", operation({ method: "post", authority, operationId: "candidates.approve", permission: "memory.backend.candidates.write", auditEvent: "memory.backend.candidate.approved", pathParams: [pathParam("candidateId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryCandidate", status: "200", idempotent: true }));
  addPath(paths, `${P}/candidates/{candidateId}/reject`, "post", operation({ method: "post", authority, operationId: "candidates.reject", permission: "memory.backend.candidates.write", auditEvent: "memory.backend.candidate.rejected", pathParams: [pathParam("candidateId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryCandidate", status: "200", idempotent: true }));
  addPath(paths, `${P}/extraction_jobs`, "get", operation({ method: "get", authority, operationId: "extractionJobs.list", permission: "memory.backend.learning.read", auditEvent: "memory.backend.extraction_job.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryLearningJobList" }));
  addPath(paths, `${P}/extraction_jobs`, "post", operation({ method: "post", authority, operationId: "extractionJobs.create", permission: "memory.backend.learning.write", auditEvent: "memory.backend.extraction_job.created", requestSchema: "MemoryExtractionRequest", responseSchema: "MemoryLearningJob", idempotent: true }));
  addPath(paths, `${P}/extraction_jobs/{jobId}`, "get", operation({ method: "get", authority, operationId: "extractionJobs.retrieve", permission: "memory.backend.learning.read", auditEvent: "memory.backend.extraction_job.read", pathParams: [pathParam("jobId")], responseSchema: "MemoryLearningJob" }));
  addPath(paths, `${P}/consolidation_jobs`, "get", operation({ method: "get", authority, operationId: "consolidationJobs.list", permission: "memory.backend.learning.read", auditEvent: "memory.backend.consolidation_job.list", queryParams: cursorListParams(), responseSchema: "MemoryLearningJobList" }));
  addPath(paths, `${P}/consolidation_jobs`, "post", operation({ method: "post", authority, operationId: "consolidationJobs.create", permission: "memory.backend.learning.write", auditEvent: "memory.backend.consolidation_job.created", requestSchema: "MemoryExtractionRequest", responseSchema: "MemoryLearningJob", idempotent: true }));
  addPath(paths, `${P}/consolidation_jobs/{jobId}`, "get", operation({ method: "get", authority, operationId: "consolidationJobs.retrieve", permission: "memory.backend.learning.read", auditEvent: "memory.backend.consolidation_job.read", pathParams: [pathParam("jobId")], responseSchema: "MemoryLearningJob" }));
  addPath(paths, `${P}/indexes`, "get", operation({ method: "get", authority, operationId: "indexes.list", permission: "memory.backend.indexes.read", auditEvent: "memory.backend.index.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryIndexList" }));
  addPath(paths, `${P}/indexes`, "post", operation({ method: "post", authority, operationId: "indexes.create", permission: "memory.backend.indexes.write", auditEvent: "memory.backend.index.created", requestSchema: "MemoryIndexRequest", responseSchema: "MemoryIndex", idempotent: true }));
  addPath(paths, `${P}/indexes/{indexId}`, "get", operation({ method: "get", authority, operationId: "indexes.retrieve", permission: "memory.backend.indexes.read", auditEvent: "memory.backend.index.read", pathParams: [pathParam("indexId")], responseSchema: "MemoryIndex" }));
  addPath(paths, `${P}/indexes/{indexId}`, "patch", operation({ method: "patch", authority, operationId: "indexes.update", permission: "memory.backend.indexes.write", auditEvent: "memory.backend.index.updated", pathParams: [pathParam("indexId")], requestSchema: "MemoryIndexRequest", responseSchema: "MemoryIndex" }));
  addPath(paths, `${P}/indexes/{indexId}/rebuild`, "post", operation({ method: "post", authority, operationId: "indexes.rebuild", permission: "memory.backend.indexes.write", auditEvent: "memory.backend.index.rebuild_requested", pathParams: [pathParam("indexId")], requestSchema: "MemoryReviewRequest", responseSchema: "MemoryLearningJob", status: "200", idempotent: true }));
  addPath(paths, `${P}/retrieval_profiles`, "get", operation({ method: "get", authority, operationId: "retrievalProfiles.list", permission: "memory.backend.retrievalProfiles.read", auditEvent: "memory.backend.retrieval_profile.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryRetrievalProfileList" }));
  addPath(paths, `${P}/retrieval_profiles`, "post", operation({ method: "post", authority, operationId: "retrievalProfiles.create", permission: "memory.backend.retrievalProfiles.write", auditEvent: "memory.backend.retrieval_profile.created", requestSchema: "MemoryRetrievalProfileRequest", responseSchema: "MemoryRetrievalProfile", idempotent: true }));
  addPath(paths, `${P}/retrieval_profiles/{profileId}`, "get", operation({ method: "get", authority, operationId: "retrievalProfiles.retrieve", permission: "memory.backend.retrievalProfiles.read", auditEvent: "memory.backend.retrieval_profile.read", pathParams: [pathParam("profileId")], responseSchema: "MemoryRetrievalProfile" }));
  addPath(paths, `${P}/retrieval_profiles/{profileId}`, "patch", operation({ method: "patch", authority, operationId: "retrievalProfiles.update", permission: "memory.backend.retrievalProfiles.write", auditEvent: "memory.backend.retrieval_profile.updated", pathParams: [pathParam("profileId")], requestSchema: "MemoryRetrievalProfileRequest", responseSchema: "MemoryRetrievalProfile" }));
  addPath(paths, `${P}/implementation_profiles`, "get", operation({ method: "get", authority, operationId: "implementationProfiles.list", permission: "memory.backend.implementationProfiles.read", auditEvent: "memory.backend.implementation_profile.list", queryParams: cursorListParams(), responseSchema: "MemoryImplementationProfileList" }));
  addPath(paths, `${P}/implementation_profiles`, "post", operation({ method: "post", authority, operationId: "implementationProfiles.create", permission: "memory.backend.implementationProfiles.write", auditEvent: "memory.backend.implementation_profile.created", requestSchema: "MemoryImplementationProfileRequest", responseSchema: "MemoryImplementationProfile", idempotent: true }));
  addPath(paths, `${P}/implementation_profiles/{implementationProfileId}`, "get", operation({ method: "get", authority, operationId: "implementationProfiles.retrieve", permission: "memory.backend.implementationProfiles.read", auditEvent: "memory.backend.implementation_profile.read", pathParams: [pathParam("implementationProfileId")], responseSchema: "MemoryImplementationProfile" }));
  addPath(paths, `${P}/implementation_profiles/{implementationProfileId}`, "patch", operation({ method: "patch", authority, operationId: "implementationProfiles.update", permission: "memory.backend.implementationProfiles.write", auditEvent: "memory.backend.implementation_profile.updated", pathParams: [pathParam("implementationProfileId")], requestSchema: "MemoryImplementationProfileRequest", responseSchema: "MemoryImplementationProfile" }));
  addPath(paths, `${P}/provider_bindings`, "get", operation({ method: "get", authority, operationId: "providerBindings.list", permission: "memory.backend.providerBindings.read", auditEvent: "memory.backend.provider_binding.list", queryParams: cursorListParams(), responseSchema: "MemoryProviderBindingList" }));
  addPath(paths, `${P}/provider_bindings`, "post", operation({ method: "post", authority, operationId: "providerBindings.create", permission: "memory.backend.providerBindings.write", auditEvent: "memory.backend.provider_binding.created", requestSchema: "MemoryProviderBindingRequest", responseSchema: "MemoryProviderBinding", idempotent: true }));
  addPath(paths, `${P}/provider_bindings/{providerBindingId}`, "patch", operation({ method: "patch", authority, operationId: "providerBindings.update", permission: "memory.backend.providerBindings.write", auditEvent: "memory.backend.provider_binding.updated", pathParams: [pathParam("providerBindingId")], requestSchema: "MemoryProviderBindingRequest", responseSchema: "MemoryProviderBinding" }));
  addPath(paths, `${P}/provider_health`, "get", operation({ method: "get", authority, operationId: "providerHealth.retrieve", permission: "memory.backend.providerHealth.read", auditEvent: "memory.backend.provider_health.read", responseSchema: "MemoryProviderHealth" }));
  addPath(paths, `${P}/eval_runs`, "get", operation({ method: "get", authority, operationId: "evalRuns.list", permission: "memory.backend.evalRuns.read", auditEvent: "memory.backend.eval_run.list", queryParams: cursorListParams(), responseSchema: "MemoryEvalRunList" }));
  addPath(paths, `${P}/eval_runs`, "post", operation({ method: "post", authority, operationId: "evalRuns.create", permission: "memory.backend.evalRuns.write", auditEvent: "memory.backend.eval_run.created", requestSchema: "MemoryEvalRunRequest", responseSchema: "MemoryEvalRun", idempotent: true }));
  addPath(paths, `${P}/eval_runs/{evalRunId}`, "get", operation({ method: "get", authority, operationId: "evalRuns.retrieve", permission: "memory.backend.evalRuns.read", auditEvent: "memory.backend.eval_run.read", pathParams: [pathParam("evalRunId")], responseSchema: "MemoryEvalRun" }));
  addPath(paths, `${P}/retrieval_traces`, "get", operation({ method: "get", authority, operationId: "retrievalTraces.list", permission: "memory.backend.retrievalTraces.read", auditEvent: "memory.backend.retrieval_trace.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }]), responseSchema: "MemoryRetrievalTraceList" }));
  addPath(paths, `${P}/retrieval_traces/{traceId}`, "get", operation({ method: "get", authority, operationId: "retrievalTraces.retrieve", permission: "memory.backend.retrievalTraces.read", auditEvent: "memory.backend.retrieval_trace.read", pathParams: [pathParam("traceId")], responseSchema: "MemoryRetrievalTrace" }));
  addPath(paths, `${P}/audit_logs`, "get", operation({ method: "get", authority, operationId: "auditLogs.list", permission: "memory.backend.auditLogs.read", auditEvent: "memory.backend.audit_log.list", queryParams: cursorListParams([{ name: "action", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryAuditLogList" }));
  addPath(paths, `${P}/retention_jobs`, "get", operation({ method: "get", authority, operationId: "retentionJobs.list", permission: "memory.backend.retention.read", auditEvent: "memory.backend.retention_job.list", queryParams: cursorListParams(), responseSchema: "MemoryLearningJobList" }));
  addPath(paths, `${P}/retention_jobs`, "post", operation({ method: "post", authority, operationId: "retentionJobs.create", permission: "memory.backend.retention.write", auditEvent: "memory.backend.retention_job.created", requestSchema: "MemoryRetentionJobRequest", responseSchema: "MemoryLearningJob", idempotent: true }));
  addPath(paths, `${P}/retention_jobs/{retentionJobId}`, "get", operation({ method: "get", authority, operationId: "retentionJobs.retrieve", permission: "memory.backend.retention.read", auditEvent: "memory.backend.retention_job.read", pathParams: [pathParam("retentionJobId")], responseSchema: "MemoryLearningJob" }));
  addPath(paths, `${P}/migration_jobs`, "get", operation({ method: "get", authority, operationId: "migrationJobs.list", permission: "memory.backend.migrations.read", auditEvent: "memory.backend.migration_job.list", queryParams: cursorListParams(), responseSchema: "MemoryLearningJobList" }));
  addPath(paths, `${P}/migration_jobs`, "post", operation({ method: "post", authority, operationId: "migrationJobs.create", permission: "memory.backend.migrations.write", auditEvent: "memory.backend.migration_job.created", requestSchema: "MemoryMigrationJobRequest", responseSchema: "MemoryLearningJob", idempotent: true }));
  addPath(paths, `${P}/migration_jobs/{migrationJobId}`, "get", operation({ method: "get", authority, operationId: "migrationJobs.retrieve", permission: "memory.backend.migrations.read", auditEvent: "memory.backend.migration_job.read", pathParams: [pathParam("migrationJobId")], responseSchema: "MemoryLearningJob" }));

  // Commercial subject, binding, and capability management.
  addPath(paths, `${P}/subjects`, "get", operation({ method: "get", authority, operationId: "subjects.list", permission: "memory.backend.subjects.read", auditEvent: "memory.backend.subject.list", queryParams: cursorListParams([{ name: "subject_type", in: "query", required: false, schema: { type: "string" } }, { name: "status", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemorySubjectList" }));
  addPath(paths, `${P}/subjects`, "post", operation({ method: "post", authority, operationId: "subjects.create", permission: "memory.backend.subjects.write", auditEvent: "memory.backend.subject.created", requestSchema: "MemorySubjectRequest", responseSchema: "MemorySubject", idempotent: true }));
  addPath(paths, `${P}/subjects/{subjectId}`, "get", operation({ method: "get", authority, operationId: "subjects.retrieve", permission: "memory.backend.subjects.read", auditEvent: "memory.backend.subject.read", pathParams: [pathParam("subjectId")], responseSchema: "MemorySubject" }));
  addPath(paths, `${P}/subjects/{subjectId}`, "patch", operation({ method: "patch", authority, operationId: "subjects.update", permission: "memory.backend.subjects.write", auditEvent: "memory.backend.subject.updated", pathParams: [pathParam("subjectId")], requestSchema: "MemorySubjectPatch", responseSchema: "MemorySubject" }));
  addPath(paths, `${P}/subjects/{subjectId}`, "delete", operation({ method: "delete", authority, operationId: "subjects.delete", permission: "memory.backend.subjects.write", auditEvent: "memory.backend.subject.deleted", pathParams: [pathParam("subjectId")], responseSchema: "MemorySubject", status: "204" }));
  addPath(paths, `${P}/bindings`, "get", operation({ method: "get", authority, operationId: "bindings.list", permission: "memory.backend.bindings.read", auditEvent: "memory.backend.binding.list", queryParams: cursorListParams([{ name: "source_subject_id", in: "query", required: false, schema: idSchema }, { name: "target_subject_id", in: "query", required: false, schema: idSchema }, { name: "target_space_id", in: "query", required: false, schema: idSchema }, { name: "binding_kind", in: "query", required: false, schema: { type: "string" } }, { name: "status", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryBindingList" }));
  addPath(paths, `${P}/bindings`, "post", operation({ method: "post", authority, operationId: "bindings.create", permission: "memory.backend.bindings.write", auditEvent: "memory.backend.binding.created", requestSchema: "MemoryBindingRequest", responseSchema: "MemoryBinding", idempotent: true }));
  addPath(paths, `${P}/bindings/{bindingId}`, "get", operation({ method: "get", authority, operationId: "bindings.retrieve", permission: "memory.backend.bindings.read", auditEvent: "memory.backend.binding.read", pathParams: [pathParam("bindingId")], responseSchema: "MemoryBinding" }));
  addPath(paths, `${P}/bindings/{bindingId}`, "delete", operation({ method: "delete", authority, operationId: "bindings.delete", permission: "memory.backend.bindings.write", auditEvent: "memory.backend.binding.deleted", pathParams: [pathParam("bindingId")], responseSchema: "MemoryBinding", status: "204" }));
  addPath(paths, `${P}/capability_bindings`, "get", operation({ method: "get", authority, operationId: "capabilityBindings.list", permission: "memory.backend.capabilityBindings.read", auditEvent: "memory.backend.capability_binding.list", queryParams: cursorListParams([{ name: "capability_code", in: "query", required: false, schema: { type: "string" } }, { name: "target_type", in: "query", required: false, schema: { type: "string" } }, { name: "target_id", in: "query", required: false, schema: idSchema }, { name: "status", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryCapabilityBindingList" }));
  addPath(paths, `${P}/capability_bindings`, "post", operation({ method: "post", authority, operationId: "capabilityBindings.create", permission: "memory.backend.capabilityBindings.write", auditEvent: "memory.backend.capability_binding.created", requestSchema: "MemoryCapabilityBindingRequest", responseSchema: "MemoryCapabilityBinding", idempotent: true }));
  addPath(paths, `${P}/capability_bindings/{capabilityBindingId}`, "get", operation({ method: "get", authority, operationId: "capabilityBindings.retrieve", permission: "memory.backend.capabilityBindings.read", auditEvent: "memory.backend.capability_binding.read", pathParams: [pathParam("capabilityBindingId")], responseSchema: "MemoryCapabilityBinding" }));
  addPath(paths, `${P}/capability_bindings/{capabilityBindingId}`, "delete", operation({ method: "delete", authority, operationId: "capabilityBindings.delete", permission: "memory.backend.capabilityBindings.write", auditEvent: "memory.backend.capability_binding.deleted", pathParams: [pathParam("capabilityBindingId")], responseSchema: "MemoryCapabilityBinding", status: "204" }));
  addPath(paths, `${P}/capabilities/resolve`, "post", operation({ method: "post", authority, operationId: "capabilities.resolve", permission: "memory.backend.capabilityBindings.read", auditEvent: "memory.backend.capabilities.resolved", requestSchema: "MemoryResolveCapabilitiesRequest", responseSchema: "MemoryResolvedCapabilityListResponse", status: "200", idempotent: true }));

  // Commercial graph, policy, and readiness management.
  addPath(paths, `${P}/entities`, "get", operation({ method: "get", authority, operationId: "entities.list", permission: "memory.backend.entities.read", auditEvent: "memory.backend.entity.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }, { name: "entity_type", in: "query", required: false, schema: { type: "string" } }, { name: "status", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryEntityList" }));
  addPath(paths, `${P}/entities`, "post", operation({ method: "post", authority, operationId: "entities.create", permission: "memory.backend.entities.write", auditEvent: "memory.backend.entity.created", requestSchema: "MemoryEntityRequest", responseSchema: "MemoryEntity", idempotent: true }));
  addPath(paths, `${P}/entities/{entityId}`, "get", operation({ method: "get", authority, operationId: "entities.retrieve", permission: "memory.backend.entities.read", auditEvent: "memory.backend.entity.read", pathParams: [pathParam("entityId")], responseSchema: "MemoryEntity" }));
  addPath(paths, `${P}/entities/{entityId}`, "patch", operation({ method: "patch", authority, operationId: "entities.update", permission: "memory.backend.entities.write", auditEvent: "memory.backend.entity.updated", pathParams: [pathParam("entityId")], requestSchema: "MemoryEntityPatch", responseSchema: "MemoryEntity" }));
  addPath(paths, `${P}/edges`, "get", operation({ method: "get", authority, operationId: "edges.list", permission: "memory.backend.entities.read", auditEvent: "memory.backend.edge.list", queryParams: cursorListParams([{ name: "space_id", in: "query", required: false, schema: idSchema }, { name: "source_entity_id", in: "query", required: false, schema: idSchema }, { name: "relation_type", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryEdgeList" }));
  addPath(paths, `${P}/edges`, "post", operation({ method: "post", authority, operationId: "edges.create", permission: "memory.backend.edges.write", auditEvent: "memory.backend.edge.created", requestSchema: "MemoryEdgeRequest", responseSchema: "MemoryEdge", idempotent: true }));
  addPath(paths, `${P}/edges/{edgeId}`, "get", operation({ method: "get", authority, operationId: "edges.retrieve", permission: "memory.backend.entities.read", auditEvent: "memory.backend.edge.read", pathParams: [pathParam("edgeId")], responseSchema: "MemoryEdge" }));
  addPath(paths, `${P}/edges/{edgeId}`, "patch", operation({ method: "patch", authority, operationId: "edges.update", permission: "memory.backend.edges.write", auditEvent: "memory.backend.edge.updated", pathParams: [pathParam("edgeId")], requestSchema: "MemoryEdgePatch", responseSchema: "MemoryEdge" }));
  addPath(paths, `${P}/edges/{edgeId}`, "delete", operation({ method: "delete", authority, operationId: "edges.delete", permission: "memory.backend.edges.write", auditEvent: "memory.backend.edge.deleted", pathParams: [pathParam("edgeId")], responseSchema: "MemoryEdge", status: "204" }));
  addPath(paths, `${P}/policies`, "get", operation({ method: "get", authority, operationId: "policies.list", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy.list", queryParams: cursorListParams([{ name: "policy_type", in: "query", required: false, schema: { type: "string" } }, { name: "scope", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryPolicyList" }));
  addPath(paths, `${P}/policies`, "post", operation({ method: "post", authority, operationId: "policies.create", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy.created", requestSchema: "MemoryPolicyRequest", responseSchema: "MemoryPolicy", idempotent: true }));
  addPath(paths, `${P}/policies/{policyId}`, "get", operation({ method: "get", authority, operationId: "policies.retrieve", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy.read", pathParams: [pathParam("policyId")], responseSchema: "MemoryPolicy" }));
  addPath(paths, `${P}/policies/{policyId}`, "patch", operation({ method: "patch", authority, operationId: "policies.update", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy.updated", pathParams: [pathParam("policyId")], requestSchema: "MemoryPolicyPatch", responseSchema: "MemoryPolicy" }));
  addPath(paths, `${P}/policies/{policyId}`, "delete", operation({ method: "delete", authority, operationId: "policies.delete", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy.deleted", pathParams: [pathParam("policyId")], responseSchema: "MemoryPolicy", status: "204" }));
  addPath(paths, `${P}/policy_assignments`, "get", operation({ method: "get", authority, operationId: "policyAssignments.list", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy_assignment.list", queryParams: cursorListParams([{ name: "target_type", in: "query", required: false, schema: { type: "string" } }, { name: "target_id", in: "query", required: false, schema: idSchema }, { name: "policy_id", in: "query", required: false, schema: { type: "string" } }]), responseSchema: "MemoryPolicyAssignmentList" }));
  addPath(paths, `${P}/policy_assignments`, "post", operation({ method: "post", authority, operationId: "policyAssignments.create", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy_assignment.created", requestSchema: "MemoryPolicyAssignmentRequest", responseSchema: "MemoryPolicyAssignment", idempotent: true }));
  addPath(paths, `${P}/policy_assignments/{policyAssignmentId}`, "get", operation({ method: "get", authority, operationId: "policyAssignments.retrieve", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy_assignment.read", pathParams: [pathParam("policyAssignmentId")], responseSchema: "MemoryPolicyAssignment" }));
  addPath(paths, `${P}/policy_assignments/{policyAssignmentId}`, "patch", operation({ method: "patch", authority, operationId: "policyAssignments.update", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy_assignment.updated", pathParams: [pathParam("policyAssignmentId")], requestSchema: "MemoryPolicyAssignmentPatch", responseSchema: "MemoryPolicyAssignment" }));
  addPath(paths, `${P}/policy_assignments/{policyAssignmentId}`, "delete", operation({ method: "delete", authority, operationId: "policyAssignments.delete", permission: "memory.backend.policies.write", auditEvent: "memory.backend.policy_assignment.deleted", pathParams: [pathParam("policyAssignmentId")], responseSchema: "MemoryPolicyAssignment", status: "204" }));
  addPath(paths, `${P}/commercial_readiness`, "get", operation({ method: "get", authority, operationId: "commercialReadiness.retrieve", permission: "memory.backend.commercialReadiness.read", auditEvent: "memory.backend.commercial_readiness.read", responseSchema: "MemoryCommercialReadiness" }));
  addPath(paths, `${P}/commercial_readiness/rebuild`, "post", operation({ method: "post", authority, operationId: "commercialReadiness.rebuild", permission: "memory.backend.commercialReadiness.write", auditEvent: "memory.commercial_readiness.rebuilt", requestSchema: "MemoryCommercialReadinessRequest", responseSchema: "MemoryCommercialReadiness", status: "200", idempotent: true }));

  writeAlignedOpenApi("sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json", createOpenApi({
    title: "SDKWork Memory Backend API",
    authority,
    prefix: "/backend/v3/api",
    sdkFamily: "sdkwork-memory-backend-sdk",
    paths
  }));
}

const routeSurfaceProfiles = [
  {
    surface: "open-api",
    openapiPath: "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json",
    apisPath: "apis/open-api/memory-open-api.openapi.json",
    packageName: "sdkwork-routes-memory-open-api",
    crateDir: "crates/sdkwork-routes-memory-open-api",
    crateImport: "sdkwork_routes_memory_open_api",
    manifestFn: "open_route_manifest",
    apiAuthority: "sdkwork-memory-open-api",
    sdkFamily: "sdkwork-memory-sdk",
    prefix: memoryOpenApiPrefix,
    routeManifestDir: "sdks/_route-manifests/open-api",
    routeManifestFile: "sdkwork-routes-memory-open-api.route-manifest.json"
  },
  {
    surface: "app-api",
    openapiPath: "sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json",
    apisPath: "apis/app-api/memory-app-api.openapi.json",
    packageName: "sdkwork-routes-memory-app-api",
    crateDir: "crates/sdkwork-routes-memory-app-api",
    crateImport: "sdkwork_routes_memory_app_api",
    manifestFn: "app_route_manifest",
    apiAuthority: "sdkwork-memory.app",
    sdkFamily: "sdkwork-memory-app-sdk",
    prefix: "/app/v3/api",
    routeManifestDir: "sdks/_route-manifests/app-api",
    routeManifestFile: "sdkwork-routes-memory-app-api.route-manifest.json"
  },
  {
    surface: "backend-api",
    openapiPath: "sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json",
    apisPath: "apis/backend-api/memory-backend-api.openapi.json",
    packageName: "sdkwork-routes-memory-backend-api",
    crateDir: "crates/sdkwork-routes-memory-backend-api",
    crateImport: "sdkwork_routes_memory_backend_api",
    manifestFn: "backend_route_manifest",
    apiAuthority: "sdkwork-memory.backend",
    sdkFamily: "sdkwork-memory-backend-sdk",
    prefix: "/backend/v3/api",
    routeManifestDir: "sdks/_route-manifests/backend-api",
    routeManifestFile: "sdkwork-routes-memory-backend-api.route-manifest.json"
  }
];

function readJsonFromDisk(relativePath) {
  return JSON.parse(fs.readFileSync(path.join(root, relativePath), "utf8"));
}

function extractRoutesFromOpenApi(openapi) {
  const routes = [];
  for (const [pathKey, pathItem] of Object.entries(openapi.paths ?? {})) {
    for (const [method, operation] of Object.entries(pathItem ?? {})) {
      if (!["get", "post", "patch", "delete"].includes(method)) {
        continue;
      }
      routes.push({
        method: method.toUpperCase(),
        path: pathKey,
        operationId: operation.operationId,
        tags: operation.tags ?? ["memory"],
        authMode: operation["x-sdkwork-auth-mode"],
        apiSurface: operation["x-sdkwork-api-surface"],
        apiAuthority: operation["x-sdkwork-api-authority"],
        permission: operation["x-sdkwork-permission"] ?? null,
        idempotent: operation["x-sdkwork-idempotent"] === true,
        rateLimitTier: operation["x-sdkwork-rate-limit-tier"] ?? null,
      });
    }
  }
  return routes;
}

function httpRouteAuthHelper(authMode) {
  return authMode === "api-key" ? "api_key" : "dual_token";
}

function httpMethodRust(method) {
  const map = { GET: "Get", POST: "Post", PATCH: "Patch", DELETE: "Delete" };
  return map[method];
}

function writeHttpRouteManifestRust(crateDir, fnName, routes) {
  const lines = [
    "// @generated by tools/materialize_phase1_contracts.mjs — do not edit",
    "",
    "use sdkwork_web_core::{HttpMethod, HttpRoute, HttpRouteManifest, RateLimitTier};",
    "",
    "const HTTP_ROUTES: &[HttpRoute] = &["
  ];
  for (const route of routes) {
    const auth = httpRouteAuthHelper(route.authMode);
    const suffix = [
      route.permission ? `.with_required_permission("${route.permission}")` : "",
      route.idempotent ? ".with_idempotent(true)" : "",
      rateLimitTierRustSuffix(route.rateLimitTier),
    ].join("");
    lines.push(`    HttpRoute::${auth}(`);
    lines.push(`        HttpMethod::${httpMethodRust(route.method)},`);
    lines.push(`        "${route.path}",`);
    lines.push(`        "${route.tags[0] ?? "memory"}",`);
    lines.push(`        "${route.operationId}",`);
    lines.push(`    )${suffix},`);
  }
  lines.push(
    "];",
    "",
    `pub fn ${fnName}() -> HttpRouteManifest {`,
    "    HttpRouteManifest::new(HTTP_ROUTES)",
    "}",
    ""
  );
  writeText(`${crateDir}/src/http_route_manifest.rs`, lines.join("\n"));
}

function writeRouteManifestJson(profile, routes) {
  writeJson(`${profile.routeManifestDir}/${profile.routeManifestFile}`, {
    schemaVersion: 1,
    kind: "sdkwork.route.manifest",
    packageName: profile.packageName,
    surface: profile.surface,
    owner,
    domain,
    capability,
    apiAuthority: profile.apiAuthority,
    sdkFamily: profile.sdkFamily,
    prefix: profile.prefix,
    source: {
      crateRoot: profile.crateDir,
      crateImport: profile.crateImport,
      openApiAuthority: profile.openapiPath
    },
    routes: routes.map((route) => ({
      method: route.method,
      path: route.path,
      operationId: route.operationId,
      tags: route.tags,
      auth: {
        mode: route.authMode,
        required: true
      },
      handler: {
        module: "crate::routes",
        name: null
      },
      ownership: {
        owner,
        apiAuthority: route.apiAuthority
      },
      requestContext: "WebRequestContext",
      apiSurface: route.apiSurface,
      ...(route.rateLimitTier ? { rateLimitTier: route.rateLimitTier } : {}),
    }))
  });
}

function mirrorApisOpenApi(profiles) {
  for (const profile of profiles) {
    const content = fs.readFileSync(path.join(root, profile.openapiPath), "utf8");
    writeText(profile.apisPath, content);
  }
}

function writeRouteArtifacts() {
  for (const profile of routeSurfaceProfiles) {
    const openapi = readJsonFromDisk(profile.openapiPath);
    const routes = extractRoutesFromOpenApi(openapi);
    writeHttpRouteManifestRust(profile.crateDir, profile.manifestFn, routes);
    writeRouteManifestJson(profile, routes);
    console.log(`materialized ${routes.length} routes for ${profile.packageName}`);
  }
  mirrorApisOpenApi(routeSurfaceProfiles);
  writeJson("apis/authority-manifest.json", {
    schemaVersion: 1,
    kind: "sdkwork.api.authority.manifest",
    surfaces: routeSurfaceProfiles.map((profile) => ({
      authorityPath: profile.apisPath,
      sdkPath: profile.openapiPath
    }))
  });
}

function writeVerification() {
  writeText("tools/verify_phase1.ps1", `$ErrorActionPreference = "Stop"

function Read-JsonFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (!(Test-Path $Path)) {
        throw "Missing required file: $Path"
    }
    return Get-Content -Raw $Path | ConvertFrom-Json
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)][string]$Content,
        [Parameter(Mandatory = $true)][string]$Needle,
        [Parameter(Mandatory = $true)][string]$Path
    )
    if (!$Content.Contains($Needle)) {
        throw "$Path must contain: $Needle"
    }
}

$requiredFiles = @(
    "AGENTS.md",
    "CODEX.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".sdkwork/README.md",
    ".sdkwork/skills/README.md",
    ".sdkwork/plugins/README.md",
    "sdkwork.app.config.json",
    "specs/README.md",
    "specs/component.spec.json",
    "${canonicalDesignDoc}",
    "${canonicalSpiDesignDoc}",
    "${legacyDesignStub}",
    "${legacySpiDesignStub}",
    "docs/schema-registry/README.md",
    "docs/schema-registry/tables/001-memory-core.yaml",
    "docs/schema-registry/tables/002-memory-learning.yaml",
    "docs/schema-registry/tables/003-memory-retrieval.yaml",
    "docs/schema-registry/tables/004-memory-provider.yaml",
    "docs/schema-registry/tables/005-memory-governance.yaml",
    "sdks/README.md",
    "sdks/sdkwork-memory-sdk/README.md",
    "sdks/sdkwork-memory-sdk/sdk-manifest.json",
    "sdks/sdkwork-memory-sdk/specs/README.md",
    "sdks/sdkwork-memory-sdk/specs/component.spec.json",
    "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json",
    "sdks/sdkwork-memory-app-sdk/README.md",
    "sdks/sdkwork-memory-app-sdk/sdk-manifest.json",
    "sdks/sdkwork-memory-app-sdk/specs/README.md",
    "sdks/sdkwork-memory-app-sdk/specs/component.spec.json",
    "sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json",
    "sdks/sdkwork-memory-backend-sdk/README.md",
    "sdks/sdkwork-memory-backend-sdk/sdk-manifest.json",
    "sdks/sdkwork-memory-backend-sdk/specs/README.md",
    "sdks/sdkwork-memory-backend-sdk/specs/component.spec.json",
    "sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json",
    "apis/authority-manifest.json",
    "apis/open-api/memory-open-api.openapi.json",
    "apis/app-api/memory-app-api.openapi.json",
    "apis/backend-api/memory-backend-api.openapi.json",
    "sdks/_route-manifests/open-api/sdkwork-routes-memory-open-api.route-manifest.json",
    "sdks/_route-manifests/app-api/sdkwork-routes-memory-app-api.route-manifest.json",
    "sdks/_route-manifests/backend-api/sdkwork-routes-memory-backend-api.route-manifest.json",
    "package.json",
    "sdkwork.workflow.json",
    ".github/workflows/package.yml"
)

foreach ($file in $requiredFiles) {
    if (!(Test-Path $file)) {
        throw "Missing required phase1 contract artifact: $file"
    }
}

foreach ($forbidden in @("sdks/sdkwork-memory-open-api", "sdks/sdkwork-memory-app-api", "sdks/sdkwork-memory-backend-api", "sdks/memory-open-sdk", "sdks/memory-app-sdk", "sdks/memory-backend-sdk")) {
    if (Test-Path $forbidden) {
        throw "Forbidden SDK/API authority directory exists: $forbidden"
    }
}

$appConfig = Read-JsonFile "sdkwork.app.config.json"
if ($appConfig.schemaVersion -ne 3 -or $appConfig.kind -ne "sdkwork.app") {
    throw "sdkwork.app.config.json must use SDKWork app manifest v3"
}
if ($appConfig.app.key -ne "sdkwork-memory") {
    throw "sdkwork.app.config.json app.key must be sdkwork-memory"
}

$rootSpec = Read-JsonFile "specs/component.spec.json"
if ($rootSpec.component.name -ne "sdkwork-memory") {
    throw "Root component spec must identify sdkwork-memory"
}
if ($rootSpec.component.domain -ne "intelligence" -or $rootSpec.component.capability -ne "memory") {
    throw "Root component spec must use domain=intelligence and capability=memory"
}
if (!$rootSpec.contracts.sdkDependencies -or $rootSpec.contracts.sdkDependencies.Count -eq 0) {
    throw "Root component spec must declare sdkDependencies"
}
if ($null -eq $rootSpec.contracts.dependencyApiExports) {
    throw "Root component spec must explicitly declare dependencyApiExports"
}
if ($null -eq $rootSpec.contracts.dependencyApiSurfaces) {
    throw "Root component spec must explicitly declare dependencyApiSurfaces"
}

foreach ($family in @(
    @{ Path = "sdks/sdkwork-memory-sdk"; Authority = "sdkwork-memory-open-api"; Prefix = "${memoryOpenApiPrefix}"; Spec = "openapi/memory-open-api.openapi.json"; Client = "SdkworkMemoryOpenClient" },
    @{ Path = "sdks/sdkwork-memory-app-sdk"; Authority = "sdkwork-memory.app"; Prefix = "/app/v3/api"; Spec = "openapi/memory-app-api.openapi.json"; Client = "SdkworkMemoryAppClient" },
    @{ Path = "sdks/sdkwork-memory-backend-sdk"; Authority = "sdkwork-memory.backend"; Prefix = "/backend/v3/api"; Spec = "openapi/memory-backend-api.openapi.json"; Client = "SdkworkMemoryBackendClient" }
)) {
    $manifest = Read-JsonFile (Join-Path $family.Path "sdk-manifest.json")
    $component = Read-JsonFile (Join-Path $family.Path "specs/component.spec.json")

    if ($manifest.sdkOwner -ne "sdkwork-memory") {
        throw "$($family.Path) manifest sdkOwner mismatch"
    }
    if ($manifest.apiAuthority -ne $family.Authority) {
        throw "$($family.Path) apiAuthority mismatch"
    }
    if ($manifest.generationInputSpec -ne $family.Spec) {
        throw "$($family.Path) generationInputSpec mismatch"
    }
    if ($manifest.discoverySurface.apiPrefix -ne $family.Prefix -or $manifest.apiPrefix -ne $family.Prefix) {
        throw "$($family.Path) apiPrefix mismatch"
    }
    if ($null -eq $component.contracts.sdkDependencies) {
        throw "$($family.Path) component spec must declare sdkDependencies"
    }
    if ($null -eq $component.contracts.dependencyApiExports) {
        throw "$($family.Path) component spec must explicitly declare dependencyApiExports"
    }
    if (!$component.contracts.sdkClients.Contains($family.Client)) {
        throw "$($family.Path) component spec must declare client $($family.Client)"
    }
}

function Verify-OpenApi {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Prefix,
        [Parameter(Mandatory = $true)][string]$Authority,
        [Parameter(Mandatory = $true)][string]$SdkFamily,
        [Parameter(Mandatory = $true)][ValidateSet("dual-token", "api-key")][string]$AuthMode,
        [Parameter(Mandatory = $true)][ValidateSet("open-api", "app-api", "backend-api")][string]$ExpectedApiSurface,
        [Parameter(Mandatory = $true)][string[]]$RequiredOperationIds,
        [Parameter(Mandatory = $true)][string[]]$RequiredSchemas
    )

    $spec = Read-JsonFile $Path
    if (!$spec.openapi.StartsWith("3.1.")) {
        throw "$Path must use OpenAPI 3.1.x"
    }
    if ($spec.'x-sdkwork-owner' -ne "sdkwork-memory" -or $spec.'x-sdkwork-api-authority' -ne $Authority -or $spec.'x-sdkwork-sdk-family' -ne $SdkFamily) {
        throw "$Path root SDKWork ownership metadata mismatch"
    }
    if (!$spec.components -or !$spec.components.schemas -or !$spec.components.securitySchemes) {
        throw "$Path must define schemas and securitySchemes"
    }
    $authToken = $spec.components.securitySchemes.AuthToken
    $accessToken = $spec.components.securitySchemes.AccessToken
    $apiKey = $spec.components.securitySchemes.ApiKey
    if ($AuthMode -eq "dual-token") {
        if (!$authToken -or $authToken.type -ne "http" -or $authToken.scheme -ne "bearer") {
            throw "$Path must define AuthToken as http bearer"
        }
        if (!$accessToken -or $accessToken.type -ne "apiKey" -or $accessToken.in -ne "header" -or $accessToken.name -ne "Access-Token") {
            throw "$Path must define AccessToken as Access-Token apiKey header"
        }
        if ($apiKey) {
            throw "$Path dual-token API must not declare ApiKey security scheme"
        }
    }
    if ($AuthMode -eq "api-key") {
        if (!$apiKey -or $apiKey.type -ne "apiKey" -or $apiKey.in -ne "header" -or $apiKey.name -ne "X-API-Key") {
            throw "$Path must define ApiKey as X-API-Key apiKey header"
        }
        if ($authToken -or $accessToken) {
            throw "$Path open API must not declare app/backend token security schemes"
        }
    }

    $operationIds = New-Object System.Collections.Generic.HashSet[string]
    foreach ($pathProperty in $spec.paths.PSObject.Properties) {
        if (!$pathProperty.Name.StartsWith($Prefix)) {
            throw "$Path contains non-canonical path prefix: $($pathProperty.Name)"
        }
        if ($AuthMode -eq "api-key" -and ($pathProperty.Name.StartsWith("/app/v3/api") -or $pathProperty.Name.StartsWith("/backend/v3/api"))) {
            throw "$Path open API must not use app/backend prefix: $($pathProperty.Name)"
        }
        if (($AuthMode -eq "api-key" -or $Prefix -eq "/backend/v3/api") -and $pathProperty.Name -match "/auth|/login|/sessions|/refresh|/logout") {
            throw "$Path backend/open API must not expose auth/session path: $($pathProperty.Name)"
        }

        foreach ($methodProperty in $pathProperty.Value.PSObject.Properties) {
            $methodName = [string]$methodProperty.Name
            if ($methodName -notin @("get", "post", "patch", "delete")) {
                continue
            }
            $operation = $methodProperty.Value
            $operationId = [string]$operation.operationId
            if ([string]::IsNullOrWhiteSpace($operationId)) {
                throw "$Path operation missing operationId at $($pathProperty.Name)"
            }
            if ($operationId.Contains("_") -or $operationId.Contains("__")) {
                throw "$Path operationId must use dotted lowerCamelCase style: $operationId"
            }
            [void]$operationIds.Add($operationId)
            if ($operation.'x-sdkwork-owner' -ne "sdkwork-memory" -or $operation.'x-sdkwork-api-authority' -ne $Authority) {
                throw "$Path operation ownership mismatch: $operationId"
            }
            if (!$operation.'x-sdkwork-permission' -or !$operation.'x-sdkwork-audit-event' -or !$operation.'x-sdkwork-auth-mode') {
                throw "$Path operation missing permission/audit/auth metadata: $operationId"
            }
            if ($operation.'x-sdkwork-auth-mode' -ne $AuthMode) {
                throw "$Path operation auth mode mismatch for $operationId"
            }
            if ($operation.'x-sdkwork-api-surface' -ne $ExpectedApiSurface) {
                throw "$Path operation api surface mismatch for $operationId"
            }
            if ($operation.'x-sdkwork-request-context' -ne "WebRequestContext") {
                throw "$Path operation must declare WebRequestContext for $operationId"
            }
            $security = $operation.security
            if (!$security -or $security.Count -eq 0) {
                throw "$Path operation missing security declaration: $operationId"
            }
            $firstSecurity = $security[0]
            if ($AuthMode -eq "dual-token" -and (!$firstSecurity.PSObject.Properties["AuthToken"] -or !$firstSecurity.PSObject.Properties["AccessToken"])) {
                throw "$Path operation must require both AuthToken and AccessToken: $operationId"
            }
            if ($AuthMode -eq "api-key" -and !$firstSecurity.PSObject.Properties["ApiKey"]) {
                throw "$Path operation must require ApiKey: $operationId"
            }
            if ($AuthMode -eq "api-key" -and ($firstSecurity.PSObject.Properties["AuthToken"] -or $firstSecurity.PSObject.Properties["AccessToken"])) {
                throw "$Path open API operation must not require app/backend tokens: $operationId"
            }
            foreach ($errorStatus in @("400", "404")) {
                $responseProperty = $operation.responses.PSObject.Properties[$errorStatus]
                if ($responseProperty) {
                    $content = $responseProperty.Value.content
                    if (!$content -or !$content.PSObject.Properties["application/problem+json"]) {
                        throw "$Path error response $errorStatus must include application/problem+json: $operationId"
                    }
                }
            }
            if ($pathProperty.Name.Contains("{") -and (!$operation.parameters -or $operation.parameters.Count -eq 0)) {
                throw "$Path operation with path parameter has no parameters: $operationId"
            }
            if (($methodName -eq "post" -or $methodName -eq "patch") -and !$operation.requestBody) {
                throw "$Path mutating operation has no requestBody: $operationId"
            }
            if ($methodName -eq "post" -and $operation.'x-sdkwork-idempotent') {
                $hasIdempotency = $false
                foreach ($parameter in $operation.parameters) {
                    if ($parameter.name -eq "Idempotency-Key" -and $parameter.in -eq "header") {
                        $hasIdempotency = $true
                    }
                }
                if (!$hasIdempotency) {
                    throw "$Path idempotent POST missing Idempotency-Key header: $operationId"
                }
            }
        }
    }

    foreach ($requiredId in $RequiredOperationIds) {
        if (!$operationIds.Contains($requiredId)) {
            throw "$Path missing required operationId: $requiredId"
        }
    }

    foreach ($schemaName in $RequiredSchemas) {
        if (!$spec.components.schemas.PSObject.Properties[$schemaName]) {
            throw "$Path missing required schema: $schemaName"
        }
    }

    Write-Host "Verified $Path with $($operationIds.Count) operations."
}

$appOpenApiCheck = @{
    Path = "sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json"
    Prefix = "/app/v3/api"
    Authority = "sdkwork-memory.app"
    SdkFamily = "sdkwork-memory-app-sdk"
    AuthMode = "dual-token"
    ExpectedApiSurface = "app-api"
    RequiredOperationIds = @(
        "spaces.create", "spaces.list", "spaces.retrieve", "spaces.update",
        "events.create", "events.retrieve",
        "memories.create", "memories.list", "memories.retrieve", "memories.update", "memories.delete", "memories.deleteAll", "memories.sources.list",
        "forgetRequests.create", "forgetRequests.retrieve",
        "extractions.create",
        "candidates.list", "candidates.retrieve", "candidates.approve", "candidates.reject",
        "habits.list", "habits.retrieve", "habits.update", "habits.confirm", "habits.reject",
        "retrievals.create", "retrievals.retrieve",
        "contextPacks.create", "contextPacks.retrieve",
        "feedback.create",
        "exportJobs.create", "exportJobs.retrieve",
        "learningSettings.retrieve", "learningSettings.update"
    )
    RequiredSchemas = @(
        "ProblemDetail", "MemorySpace", "MemoryEvent", "MemoryRecord", "MemoryCandidate", "MemoryHabit",
        "MemoryRetrievalRequest", "MemoryRetrievalResult", "MemoryContextPackRequest", "MemoryContextPack",
        "MemoryLearningSettings", "MemoryForgetJob", "MemoryExportJob"
    )
}
Verify-OpenApi @appOpenApiCheck

$openApiCheck = @{
    Path = "sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json"
    Prefix = "${memoryOpenApiPrefix}"
    Authority = "sdkwork-memory-open-api"
    SdkFamily = "sdkwork-memory-sdk"
    AuthMode = "api-key"
    ExpectedApiSurface = "open-api"
    RequiredOperationIds = @(
        "capabilities.retrieve",
        "events.create", "events.retrieve",
        "memories.create", "memories.list", "memories.retrieve", "memories.update", "memories.delete", "memories.deleteAll",
        "retrievals.create", "retrievals.retrieve",
        "contextPacks.create", "contextPacks.retrieve",
        "feedback.create",
        "extractions.create",
        "candidates.list", "candidates.retrieve",
        "providerHealth.retrieve"
    )
    RequiredSchemas = @(
        "ProblemDetail", "MemoryCapabilities", "MemoryEvent", "MemoryRecord",
        "MemoryRetrievalRequest", "MemoryRetrievalResult", "MemoryContextPackRequest", "MemoryContextPack",
        "MemoryFeedbackRequest", "MemoryFeedback", "MemoryExtractionRequest", "MemoryLearningJob",
        "MemoryCandidate", "MemoryProviderHealth"
    )
}
Verify-OpenApi @openApiCheck

$backendOpenApiCheck = @{
    Path = "sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json"
    Prefix = "/backend/v3/api"
    Authority = "sdkwork-memory.backend"
    SdkFamily = "sdkwork-memory-backend-sdk"
    AuthMode = "dual-token"
    ExpectedApiSurface = "backend-api"
    RequiredOperationIds = @(
        "spaces.list", "spaces.retrieve", "spaces.update",
        "memories.list", "memories.retrieve", "memories.update", "memories.supersede",
        "events.list", "events.retrieve",
        "candidates.list", "candidates.approve", "candidates.reject",
        "extractionJobs.create", "extractionJobs.retrieve", "consolidationJobs.create",
        "indexes.create", "indexes.list", "indexes.retrieve", "indexes.update", "indexes.rebuild",
        "retrievalProfiles.create", "retrievalProfiles.list", "retrievalProfiles.retrieve", "retrievalProfiles.update",
        "implementationProfiles.create", "implementationProfiles.list", "implementationProfiles.retrieve", "implementationProfiles.update",
        "providerBindings.create", "providerBindings.list", "providerBindings.update",
        "providerHealth.retrieve",
        "evalRuns.create", "evalRuns.list", "evalRuns.retrieve",
        "retrievalTraces.list", "retrievalTraces.retrieve",
        "auditLogs.list", "retentionJobs.create", "migrationJobs.create", "migrationJobs.retrieve"
    )
    RequiredSchemas = @(
        "ProblemDetail", "MemoryIndex", "MemoryRetrievalProfile", "MemoryImplementationProfile",
        "MemoryProviderBinding", "MemoryProviderHealth", "MemoryEvalRun", "MemoryAuditLog",
        "MemoryMigrationJobRequest", "MemoryRetentionJobRequest"
    )
}
Verify-OpenApi @backendOpenApiCheck

foreach ($routeManifestPath in @(
    "sdks/_route-manifests/open-api/sdkwork-routes-memory-open-api.route-manifest.json",
    "sdks/_route-manifests/app-api/sdkwork-routes-memory-app-api.route-manifest.json",
    "sdks/_route-manifests/backend-api/sdkwork-routes-memory-backend-api.route-manifest.json"
)) {
    $routeManifest = Read-JsonFile $routeManifestPath
    if ($routeManifest.kind -ne "sdkwork.route.manifest") {
        throw "$routeManifestPath must use sdkwork.route.manifest kind"
    }
    foreach ($route in $routeManifest.routes) {
        if ($route.requestContext -ne "WebRequestContext") {
            throw "$routeManifestPath route $($route.method) $($route.path) must declare WebRequestContext"
        }
        if ($route.apiSurface -notin @("open-api", "app-api", "backend-api")) {
            throw "$routeManifestPath route $($route.method) $($route.path) must declare canonical apiSurface"
        }
    }
    Write-Host "Verified $routeManifestPath with $($routeManifest.routes.Count) routes."
}

foreach ($schemaPath in Get-ChildItem -Path "docs/schema-registry/tables" -Filter "*.yaml") {
    $content = Get-Content -Raw $schemaPath.FullName
    Assert-Contains -Content $content -Needle "module: memory" -Path $schemaPath.FullName
    Assert-Contains -Content $content -Needle "owner: sdkwork-memory" -Path $schemaPath.FullName
    Assert-Contains -Content $content -Needle "table: ai_" -Path $schemaPath.FullName
}

$allSchemaText = (Get-ChildItem -Path "docs/schema-registry/tables" -Filter "*.yaml" | ForEach-Object { Get-Content -Raw $_.FullName }) -join [Environment]::NewLine
foreach ($requiredTable in @(
    "ai_space", "ai_event", "ai_record", "ai_record_source", "ai_entity", "ai_edge",
    "ai_candidate", "ai_habit", "ai_habit_signal", "ai_learning_job",
    "ai_index", "ai_index_entry", "ai_retrieval_profile", "ai_retrieval_trace", "ai_retrieval_hit", "ai_context_pack",
    "ai_implementation_profile", "ai_provider_binding", "ai_policy",
    "ai_audit_log", "ai_eval_run", "ai_outbox_event"
)) {
    if (!$allSchemaText.Contains("table: $requiredTable")) {
        throw "Schema registry missing required table: $requiredTable"
    }
}

$design = Get-Content -Raw "${canonicalDesignDoc}"
foreach ($snippet in @(
    "Embedding Optional",
    "Multi-Implementation Abstraction",
    "Open API Contract Draft",
    "App API Contract Draft",
    "Backend API Contract Draft",
    "Database And Storage Design",
    "ai_"
)) {
    Assert-Contains -Content $design -Needle $snippet -Path "${canonicalDesignDoc}"
}

$spiDesign = Get-Content -Raw "${canonicalSpiDesignDoc}"
foreach ($snippet in @(
    "MemoryPluginManifest",
    "MemoryRuntimePlugin",
    "MemoryCoreRuntime",
    "Stable Core And Plugin Boundaries",
    "SPI Port Families",
    "Runtime Plugin Manifest",
    "Built-In Plugin Families",
    "Conformance suite",
    "0.1.0 Implementation Decisions",
    "Static Rust registration",
    "JSON manifest plus Rust constant",
    "Runtime plugins are not Codex agent plugins",
    'Do not place runtime Memory plugins under \`.sdkwork/plugins/\`',
    "Industry References"
)) {
    Assert-Contains -Content $spiDesign -Needle $snippet -Path "${canonicalSpiDesignDoc}"
}

foreach ($legacyStub in @("${legacyDesignStub}", "${legacySpiDesignStub}")) {
    $stubContent = Get-Content -Raw $legacyStub
    Assert-Contains -Content $stubContent -Needle "Migrated" -Path $legacyStub
    Assert-Contains -Content $stubContent -Needle "docs/architecture/tech/" -Path $legacyStub
}

if ($spiDesign.Contains("## 17. Open Decisions")) {
    throw "SPI design must resolve first-landing open decisions before runtime implementation starts."
}

Write-Host "SDKWork Memory phase1 contract verification passed."
`);
}

writeAgentEntrypoints();
writeAppManifest();
writeRootSpecs();
writeSdkFamilies();
writeSchemaRegistry();
writeOpenApi();
writeAppOpenApi();
writeBackendOpenApi();
writeRouteArtifacts();

const migrationSync = spawnSync(process.execPath, ["tools/materialize_memory_migrations.mjs"], {
  cwd: root,
  stdio: "inherit"
});
if (migrationSync.status !== 0) {
  process.exit(migrationSync.status ?? 1);
}

writeVerification();
