import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const workspaceRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appRoot = resolve(workspaceRoot, "apps", "sdkwork-memory-pc");

const infrastructurePackages = [
  { id: "core", surface: "pc-runtime", dependencies: { "@sdkwork/auth-pc-react": "workspace:*", "@sdkwork/auth-runtime-pc-react": "workspace:*", "@sdkwork/iam-app-sdk": "workspace:*", "@sdkwork/memory-app-sdk": "workspace:*", "@sdkwork/memory-pc-commons": "workspace:*", "@sdkwork/sdk-common": "workspace:*" } },
  {
    id: "commons",
    surface: "pc-shared",
    // `./styles.css` is the package-owned presentation scope for every user-facing
    // Memory console surface. It is imported by the standalone PC application and by
    // host applications that embed the block, so hosts never restate Memory UI rules.
    assetExports: { "./styles.css": "./src/styles/memory-console.css" },
    dependencies: { "@sdkwork/utils": "workspace:*", "lucide-react": "catalog:", "react": "catalog:", "react-router-dom": "^7.14.0" },
  },
  { id: "console-core", surface: "app-console", dependencies: { "@sdkwork/memory-app-sdk": "workspace:*", "@sdkwork/memory-pc-commons": "workspace:*", react: "catalog:" } },
  {
    id: "console-shell",
    surface: "app-console",
    // The console shell owns "console route composition" (APP_PC_ARCHITECTURE_SPEC.md),
    // so it also owns the canonical console module catalog (`MemoryConsoleModules.ts`).
    // Keeping the catalog here means an embedding host imports one package to mount the
    // whole Memory console instead of hardcoding the memory capability package list.
    dependencies: {
      "@sdkwork/memory-pc-commons": "workspace:*",
      "@sdkwork/memory-pc-console-core": "workspace:*",
      "@sdkwork/memory-pc-console-governance": "workspace:*",
      "@sdkwork/memory-pc-console-knowledge": "workspace:*",
      "@sdkwork/memory-pc-console-learning": "workspace:*",
      "@sdkwork/memory-pc-console-memory": "workspace:*",
      "@sdkwork/memory-pc-console-overview": "workspace:*",
      "@sdkwork/memory-pc-console-retrieval": "workspace:*",
      react: "catalog:",
      "react-router-dom": "^7.14.0",
    },
  },
  { id: "admin-core", surface: "backend-admin", dependencies: { "@sdkwork/memory-backend-sdk": "workspace:*", "@sdkwork/memory-pc-commons": "workspace:*", "@sdkwork/sdk-common": "workspace:*", react: "catalog:" } },
  { id: "admin-shell", surface: "backend-admin", dependencies: { "@sdkwork/memory-pc-admin-core": "workspace:*", "@sdkwork/memory-pc-commons": "workspace:*", react: "catalog:", "react-router-dom": "^7.14.0" } },
];

const capabilityPackages = [
  { id: "console-overview", surface: "app-console", title: "Overview", permission: "memory.spaces.read", resources: ["spaces", "candidates", "habits"] },
  { id: "console-memory", surface: "app-console", title: "Memory", permission: "memory.records.read", resources: ["spaces", "memories"] },
  { id: "console-learning", surface: "app-console", title: "Learning", permission: "memory.candidates.read", resources: ["candidates", "habits", "learningSettings"] },
  { id: "console-retrieval", surface: "app-console", title: "Retrieval", permission: "memory.retrievals.write", resources: ["retrievals", "contextPacks", "feedback"] },
  { id: "console-knowledge", surface: "app-console", title: "Knowledge", permission: "memory.app.entities.read", resources: ["entities"] },
  { id: "console-governance", surface: "app-console", title: "Governance", permission: "memory.app.policies.write", resources: ["policyAssignments", "forgetRequests", "exportJobs"] },
  { id: "admin-overview", surface: "backend-admin", title: "Operations", permission: "memory.backend.commercialReadiness.read", resources: ["providerHealth", "commercialReadiness"] },
  { id: "admin-memory", surface: "backend-admin", title: "Memory operations", permission: "memory.backend.records.read", resources: ["spaces", "memories", "events"] },
  { id: "admin-learning", surface: "backend-admin", title: "Learning operations", permission: "memory.backend.candidates.read", resources: ["candidates", "extractionJobs", "consolidationJobs"] },
  { id: "admin-retrieval", surface: "backend-admin", title: "Retrieval operations", permission: "memory.backend.indexes.read", resources: ["indexes", "retrievalProfiles", "retrievalTraces"] },
  { id: "admin-providers", surface: "backend-admin", title: "Providers", permission: "memory.backend.providerBindings.read", resources: ["implementationProfiles", "providerBindings", "providerHealth"] },
  { id: "admin-evaluation", surface: "backend-admin", title: "Evaluation", permission: "memory.backend.evalRuns.read", resources: ["evalRuns"] },
  { id: "admin-knowledge-graph", surface: "backend-admin", title: "Knowledge graph", permission: "memory.backend.entities.read", resources: ["entities", "edges"] },
  { id: "admin-control-plane", surface: "backend-admin", title: "Control plane", permission: "memory.backend.subjects.read", resources: ["subjects", "bindings", "capabilityBindings", "capabilities"] },
  { id: "admin-governance", surface: "backend-admin", title: "Governance", permission: "memory.backend.auditLogs.read", resources: ["policies", "policyAssignments", "auditLogs", "retentionJobs", "migrationJobs"] },
];

materializeAppAgentEntrypoints();

for (const definition of [...infrastructurePackages, ...capabilityPackages]) {
  materializePackage(definition);
}

function materializeAppAgentEntrypoints() {
  writeText(resolve(appRoot, "AGENTS.md"), `# SDKWork Memory PC Application

<!-- SDKWORK-AGENTS-GENERATED: v2 -->

## SDKWORK Soul

Read \`../../../sdkwork-specs/SOUL.md\` before executing tasks in this root. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Canonical SDKWork standards from this application root:

- \`../../../sdkwork-specs/README.md\`
- \`../../../sdkwork-specs/SOUL.md\`
- \`../../../sdkwork-specs/AGENTS_SPEC.md\`
- \`../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md\`
- \`../../../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md\`
- \`../../../sdkwork-specs/CODE_STYLE_SPEC.md\`
- \`../../../sdkwork-specs/NAMING_SPEC.md\`
- \`../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md\`

Do not copy global spec bodies into this application root. If these relative paths do not resolve, stop and report the broken workspace layout.

## Application Identity

Read \`sdkwork.app.config.json\` when work touches the PC application's behavior, SDK wiring, release metadata, app-owned capabilities, packaging, or deployment. Read \`etc/\` for concrete browser runtime and deployment configuration; the application manifest is not runtime configuration authority.

## Local Dictionary Structure

- \`AGENTS.md\`: application agent entrypoint.
- \`sdkwork.app.config.json\`: PC application identity, release, and capability metadata.
- \`etc/\`: deployable-root source configuration and browser runtime templates.
- \`specs/\`: PC application contract and narrowing rules.
- \`packages/\`: infrastructure, shell, Console capability, and Admin capability packages.
- \`src/\`: composition root, IAM boundary, and lazy surface entrypoints.
- \`tests/\`: runtime, SDK-boundary, pagination, and visual verification assets.
- \`.sdkwork/\`: application-local AI workspace metadata.

## Spec Resolution Order

Use dynamic progressive loading: read this file and the local dictionary first, then \`sdkwork.app.config.json\` or local component specs only when the task touches them, then task-specific files from \`../../../sdkwork-specs/README.md\`, and only then implementation files. Language-specific specs are on-demand; do not load unrelated Rust, Java, native, or mobile standards for PC React work.

## Required Specs By Task Type

- PC architecture and package changes: \`../../../sdkwork-specs/APP_PC_ARCHITECTURE_SPEC.md\`, \`../../../sdkwork-specs/APP_PC_REACT_UI_SPEC.md\`, and \`../../../sdkwork-specs/MODULE_SPEC.md\`.
- Console SDK changes: \`../../../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md\` and \`../../../sdkwork-specs/SDK_SPEC.md\`.
- Admin SDK changes: \`../../../sdkwork-specs/SDK_SPEC.md\`, \`../../../sdkwork-specs/BACKEND_UI_SPEC.md\`, and the backend SDK integration skill.
- Frontend code: \`../../../sdkwork-specs/FRONTEND_CODE_SPEC.md\`, \`../../../sdkwork-specs/TYPESCRIPT_CODE_SPEC.md\`, \`../../../sdkwork-specs/I18N_SPEC.md\`, and \`../../../sdkwork-specs/TEST_SPEC.md\`.
- Source config and deployment: \`../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md\`, \`../../../sdkwork-specs/CONFIG_SPEC.md\`, \`../../../sdkwork-specs/ENVIRONMENT_SPEC.md\`, and \`../../../sdkwork-specs/DEPLOYMENT_SPEC.md\`.
- Package scripts and workflows: \`../../../sdkwork-specs/PNPM_SCRIPT_SPEC.md\`, \`../../../sdkwork-specs/GITHUB_WORKFLOW_SPEC.md\`, and \`../../../sdkwork-specs/TEST_SPEC.md\`.

## Code Style Rules

Read \`../../../sdkwork-specs/CODE_STYLE_SPEC.md\` and \`../../../sdkwork-specs/NAMING_SPEC.md\` before code changes. Generated SDK output must not be hand-edited. Console packages consume only \`@sdkwork/memory-app-sdk\` through Console core; Admin packages consume only \`@sdkwork/memory-backend-sdk\` through Admin core. Do not add raw HTTP, manual auth headers, local SDK forks, or cross-surface business imports.

## Build, Test, and Verification

Use the application package scripts and root SDKWork validators:

\`\`\`powershell
pnpm --dir apps/sdkwork-memory-pc check
node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
node ../sdkwork-specs/tools/check-source-config-standard.mjs --root apps/sdkwork-memory-pc
\`\`\`

## Agent Execution Rules

Fail closed when runtime configuration, IAM state, SDK authority, route permission hints, or surface ownership is ambiguous. The server remains the authorization authority; frontend permission hints are navigation and visibility aids only. Preserve lazy loading so the Console entry path does not load Backend SDK code.

## Task-Specific Standards

API work loads \`../../../sdkwork-specs/API_SPEC.md\`; list/search work loads \`../../../sdkwork-specs/PAGINATION_SPEC.md\`; SDK consumer work loads \`../../../sdkwork-specs/APP_SDK_INTEGRATION_SPEC.md\` and \`../../../sdkwork-specs/SDK_SPEC.md\`; source configuration work loads \`../../../sdkwork-specs/SOURCE_CONFIG_SPEC.md\`. Link the authority and validator instead of copying normative bodies here.

## Human Review Rules

Human review is required for breaking public API changes, privacy/security exceptions, generated SDK ownership changes, production IAM policy changes, and destructive filesystem or data operations.
`);

  const shim = (toolName) => `# ${toolName} Entry

This file is a compatibility shim for ${toolName}. The authoritative application entrypoint is \`AGENTS.md\`.

Read in this order:

1. \`AGENTS.md\`
2. \`../../../sdkwork-specs/SOUL.md\`
3. \`../../../sdkwork-specs/AGENTS_SPEC.md\`
4. Task-specific files from \`../../../sdkwork-specs/README.md\`

Do not duplicate or override SDKWork rules here.
`;

  writeText(resolve(appRoot, "CODEX.md"), shim("Codex"));
  writeText(resolve(appRoot, "CLAUDE.md"), shim("Claude Code"));
  writeText(resolve(appRoot, "GEMINI.md"), shim("Gemini"));
}

function materializePackage(definition) {
  const directoryName = `sdkwork-memory-pc-${definition.id}`;
  const packageRoot = resolve(appRoot, "packages", directoryName);
  const packageName = `@sdkwork/memory-pc-${definition.id}`;
  const isCapability = "resources" in definition;
  const surfaceCore = definition.surface === "backend-admin"
    ? "@sdkwork/memory-pc-admin-core"
    : "@sdkwork/memory-pc-console-core";
  const dependencies = isCapability
    ? { "@sdkwork/memory-pc-commons": "workspace:*", [surfaceCore]: "workspace:*", react: "catalog:" }
    : definition.dependencies;

  writeJson(resolve(packageRoot, "package.json"), {
    name: packageName,
    version: "0.1.0",
    private: true,
    type: "module",
    main: "./src/index.ts",
    exports: {
      ".": {
        types: "./src/index.ts",
        import: "./src/index.ts",
        default: "./src/index.ts",
      },
      ...(definition.assetExports ?? {}),
      ...(definition.id.endsWith("core") ? coreSubpathExports() : {}),
    },
    dependencies,
    sdkwork: {
      applicationCode: "memory",
      architecture: "pc-react",
      capability: definition.id,
      surface: definition.surface,
      managedBy: "tools/materialize_memory_pc_packages.mjs",
    },
  });

  writeJson(resolve(packageRoot, "specs", "component.spec.json"), componentSpec(definition, packageName, directoryName));
  writeText(resolve(packageRoot, "specs", "README.md"), `# ${packageName}\n\nMachine authority: \`component.spec.json\`. Global standards are referenced through \`canonicalSpecs\` and are not copied here.\n`);

  if (isCapability) {
    const capability = definition.id.replace(/^(console|admin)-/, "");
    writeText(resolve(packageRoot, "src", "i18n", "en-US", "memory", capability, "module.ts"), messageSource(definition, "en-US"));
    writeText(resolve(packageRoot, "src", "i18n", "zh-CN", "memory", capability, "module.ts"), messageSource(definition, "zh-CN"));
    writeText(resolve(packageRoot, "src", "module.ts"), capabilityModuleSource(definition));
    writeText(resolve(packageRoot, "src", "index.ts"), "export { memoryModule } from \"./module.ts\";\n");
  }
}

function componentSpec(definition, packageName, directoryName) {
  const isBackend = definition.surface === "backend-admin";
  return {
    schemaVersion: 1,
    kind: "sdkwork.component.spec",
    component: {
      name: packageName,
      displayName: `SDKWork Memory PC ${definition.id}`,
      version: "0.1.0",
      type: "node-package",
      root: `sdkwork-memory/apps/sdkwork-memory-pc/packages/${directoryName}`,
      domain: "intelligence",
      capability: definition.id,
      surface: definition.surface,
      languages: ["typescript"],
      generated: false,
      private: true,
      status: "active",
      manifests: ["package.json", "specs/component.spec.json"],
    },
    canonicalSpecs: [
      spec("COMPONENT_SPEC.md", "Module-local component contract."),
      spec("APP_PC_ARCHITECTURE_SPEC.md", "PC package taxonomy and surface boundaries."),
      spec("APP_PC_REACT_UI_SPEC.md", "React PC package implementation rules."),
      spec("APP_SDK_INTEGRATION_SPEC.md", "Injected SDK client and consumer import rules."),
      spec("I18N_SPEC.md", "Package-owned locale fragment rules."),
      spec("TEST_SPEC.md", "Verification requirements."),
    ],
    contracts: {
      publicExports: ["src/index.ts"],
      runtimeEntrypoints: [],
      routeManifest: null,
      sdkClients: [],
      sdkDependencies: definition.id.endsWith("core")
        ? [{
            workspace: isBackend ? "sdkwork-memory-backend-sdk" : "sdkwork-memory-app-sdk",
            surface: isBackend ? "backend-api" : "app-api",
            credentialMode: isBackend ? "authenticated-backend-admin" : "authenticated-app-api",
          }]
        : [],
      ...(definition.id.endsWith("core") ? {
        permissionComposition: {
          inheritanceMode: "module-catalog-with-overrides",
          applicationModule: { manifestRef: "../../../../../specs/iam.module.manifest.json" },
          moduleCatalogRefs: [{
            moduleId: "memory",
            manifestRef: "../../../../../specs/iam.module.manifest.json",
            inheritPermissions: true,
            inheritRoles: true,
          }],
          bootstrapAccessTokenScope: {
            inheritFrom: "sdkwork.app.config.json#backend.accessTokenPermissionScope",
            supplement: [],
            overrideReplace: false,
          },
          routePermissionHints: {
            inheritFromOpenApi: true,
            inheritFromModuleManifests: true,
            overrides: [],
          },
          consumerPolicy: {
            forbidLocalPermissionCatalogForDependencyDomains: true,
            allowExplicitOverridesOnly: true,
            allowFrontendHintsWithoutServerDuplication: true,
          },
        },
      } : {}),
      events: [],
      configKeys: [],
      permissions: "permission" in definition ? [definition.permission] : [],
    },
    integration: {
      authority: "Root SDKWork specs remain authoritative.",
      dependencyPolicy: "Consume sibling modules through package public exports only.",
      sdkPolicy: isBackend
        ? "Backend-admin services consume the injected composed Memory backend SDK through admin-core."
        : "Console services consume the injected composed Memory app SDK through console-core.",
    },
    verification: {
      commands: ["pnpm --dir apps/sdkwork-memory-pc typecheck", "pnpm --dir apps/sdkwork-memory-pc test"],
    },
    metadata: {
      managedBy: "tools/materialize_memory_pc_packages.mjs",
      standardVersion: "2026-07-20",
    },
  };
}

function spec(file, purpose) {
  return { file, path: `../../../../../sdkwork-specs/${file}`, purpose };
}

function capabilityModuleSource(definition) {
  const route = definition.id.replace(/^(console|admin)-/, "");
  const constantName = definition.id.replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
  return `import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";\nimport { messages as enUS } from "./i18n/en-US/memory/${route}/module.ts";\nimport { messages as zhCN } from "./i18n/zh-CN/memory/${route}/module.ts";\n\nexport const ${constantName}Module = {\n  id: "${definition.id}",\n  surface: "${definition.surface}",\n  route: "${route}",\n  titleKey: "memory.${definition.id}.title",\n  descriptionKey: "memory.${definition.id}.description",\n  permission: "${definition.permission}",\n  resources: ${JSON.stringify(definition.resources)},\n  messages: { "en-US": enUS, "zh-CN": zhCN },\n} as const satisfies MemoryPcModuleDefinition;\n\nexport const memoryModule = ${constantName}Module;\n`;
}

function messageSource(definition, locale) {
  const descriptions = {
    "console-overview": "Monitor the state of your memory workspace and pending decisions.",
    "console-memory": "Inspect, correct, and trace memories stored in your spaces.",
    "console-learning": "Review learned candidates, habits, and learning preferences.",
    "console-retrieval": "Test retrieval and inspect context assembled for AI workflows.",
    "console-knowledge": "Maintain the entities recognized in your memory spaces.",
    "console-governance": "Control policies, exports, retention, and forgetting requests.",
    "admin-overview": "Track service health, readiness, and operational risk.",
    "admin-memory": "Investigate canonical memories, events, and supersession chains.",
    "admin-learning": "Operate extraction, candidate review, and consolidation jobs.",
    "admin-retrieval": "Manage indexes, profiles, and explainable retrieval traces.",
    "admin-providers": "Manage implementation profiles, provider bindings, and health.",
    "admin-evaluation": "Measure retrieval, learning, habit, and end-to-end quality.",
    "admin-knowledge-graph": "Inspect entities, edges, and relationship integrity.",
    "admin-control-plane": "Manage subjects, bindings, and capability resolution.",
    "admin-governance": "Operate policy, audit, retention, and migration controls.",
  };
  const zhDescriptions = {
    "console-overview": "查看记忆工作区状态与待处理决策。",
    "console-memory": "检查、纠正并追溯空间中的记忆。",
    "console-learning": "审核学习候选、习惯与学习偏好。",
    "console-retrieval": "验证检索效果并检查 AI 工作流的上下文组装。",
    "console-knowledge": "维护记忆空间中识别出的实体。",
    "console-governance": "管理策略、导出、保留与遗忘请求。",
    "admin-overview": "跟踪服务健康、商业就绪度与运营风险。",
    "admin-memory": "排查规范记忆、事件与替代链路。",
    "admin-learning": "运营抽取、候选审核与合并任务。",
    "admin-retrieval": "管理索引、检索配置与可解释 Trace。",
    "admin-providers": "管理实现配置、Provider 绑定与健康状态。",
    "admin-evaluation": "评估检索、学习、习惯与端到端质量。",
    "admin-knowledge-graph": "检查实体、关系边与图完整性。",
    "admin-control-plane": "管理 Subject、绑定与能力解析。",
    "admin-governance": "运营策略、审计、保留与迁移控制。",
  };
  const title = locale === "zh-CN" ? translateTitle(definition.title) : definition.title;
  const description = locale === "zh-CN" ? zhDescriptions[definition.id] : descriptions[definition.id];
  return `export const messages = ${JSON.stringify({
    [`memory.${definition.id}.title`]: title,
    [`memory.${definition.id}.description`]: description,
  }, null, 2)} as const;\n`;
}

function translateTitle(title) {
  const titles = {
    "Overview": "概览",
    "Memory": "记忆",
    "Learning": "学习",
    "Retrieval": "检索",
    "Knowledge": "知识实体",
    "Governance": "治理",
    "Operations": "运营概览",
    "Memory operations": "记忆运营",
    "Learning operations": "学习运营",
    "Retrieval operations": "检索运营",
    "Providers": "Provider 管理",
    "Evaluation": "质量评测",
    "Knowledge graph": "知识图谱",
    "Control plane": "控制平面",
  };
  return titles[title] ?? title;
}

function coreSubpathExports() {
  return Object.fromEntries(["sdk", "modules", "host", "session", "composition"].map((subpath) => [
    `./${subpath}`,
    {
      types: `./src/${subpath}/index.ts`,
      import: `./src/${subpath}/index.ts`,
      default: `./src/${subpath}/index.ts`,
    },
  ]));
}

function writeJson(path, value) {
  writeText(path, `${JSON.stringify(value, null, 2)}\n`);
}

function writeText(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  if (safeRead(path) === value) return;
  writeFileSync(path, value, "utf8");
}

function safeRead(path) {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}
