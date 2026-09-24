import { memoryModule as adminControlPlaneModule } from "@sdkwork/memory-pc-admin-control-plane";
import { memoryModule as adminEvaluationModule } from "@sdkwork/memory-pc-admin-evaluation";
import { memoryModule as adminGovernanceModule } from "@sdkwork/memory-pc-admin-governance";
import { memoryModule as adminKnowledgeGraphModule } from "@sdkwork/memory-pc-admin-knowledge-graph";
import { memoryModule as adminLearningModule } from "@sdkwork/memory-pc-admin-learning";
import { memoryModule as adminMemoryModule } from "@sdkwork/memory-pc-admin-memory";
import { memoryModule as adminOverviewModule } from "@sdkwork/memory-pc-admin-overview";
import { memoryModule as adminProvidersModule } from "@sdkwork/memory-pc-admin-providers";
import { memoryModule as adminRetrievalModule } from "@sdkwork/memory-pc-admin-retrieval";
import { MemoryAdminShell } from "@sdkwork/memory-pc-admin-shell";
import { MemoryI18nProvider, type MemoryLocale, type MemoryPcModuleDefinition, type MemoryResourceRegistry } from "@sdkwork/memory-pc-commons";
import { memoryModule as consoleGovernanceModule } from "@sdkwork/memory-pc-console-governance";
import { memoryModule as consoleKnowledgeModule } from "@sdkwork/memory-pc-console-knowledge";
import { memoryModule as consoleLearningModule } from "@sdkwork/memory-pc-console-learning";
import { memoryModule as consoleMemoryModule } from "@sdkwork/memory-pc-console-memory";
import { memoryModule as consoleOverviewModule } from "@sdkwork/memory-pc-console-overview";
import { memoryModule as consoleRetrievalModule } from "@sdkwork/memory-pc-console-retrieval";
import { MemoryConsoleShell } from "@sdkwork/memory-pc-console-shell";
import { StrictMode, useState } from "react";
import { createRoot } from "react-dom/client";
import { MemoryRouter } from "react-router-dom";

import "../../src/index.css";

const consoleModules = [consoleOverviewModule, consoleMemoryModule, consoleLearningModule, consoleRetrievalModule, consoleKnowledgeModule, consoleGovernanceModule] satisfies readonly MemoryPcModuleDefinition[];
const adminModules = [adminOverviewModule, adminMemoryModule, adminLearningModule, adminRetrievalModule, adminProvidersModule, adminEvaluationModule, adminKnowledgeGraphModule, adminControlPlaneModule, adminGovernanceModule] satisfies readonly MemoryPcModuleDefinition[];
const fixtureItems = [
  { id: "mem_01", displayName: "Response style preference", type: "preference", status: "stable", confidence: 0.94, scope: "user", updatedAt: "2026-07-20T08:31:00Z" },
  { id: "mem_02", displayName: "SDKWork API conventions", type: "semantic", status: "confirmed", confidence: 0.91, scope: "project", updatedAt: "2026-07-20T07:55:00Z" },
  { id: "mem_03", displayName: "Use composed SDK facades", type: "procedural", status: "stable", confidence: 0.98, scope: "organization", updatedAt: "2026-07-19T16:42:00Z" },
  { id: "mem_04", displayName: "No-embedding retrieval profile", type: "configuration", status: "active", confidence: 0.87, scope: "space", updatedAt: "2026-07-19T14:20:00Z" },
  { id: "mem_05", displayName: "Privacy export preference", type: "governance", status: "confirmed", confidence: 1, scope: "user", updatedAt: "2026-07-18T11:05:00Z" },
];
const fixtureDataSource = {
  kind: "list" as const,
  actions: [{ id: "forget", label: "Forget selected memory", bodyTemplate: { spaceId: "space_demo", reason: "" }, dangerous: true, requireAuditReason: true, auditReasonField: "reason", requireIdempotencyKey: true, requiresSelection: true, execute: async () => ({ accepted: true }) }],
  load: async () => ({ items: fixtureItems, pageInfo: { mode: "cursor" as const, nextCursor: "fixture-next", hasMore: true } }),
};
const fixtureRegistry = new Proxy({}, { get: () => fixtureDataSource }) as MemoryResourceRegistry;

function VisualFixture() {
  const [locale, setLocale] = useState<MemoryLocale>("zh-CN");
  const admin = new URLSearchParams(window.location.search).get("surface") === "admin";
  const modules = admin ? adminModules : consoleModules;
  return (
    <MemoryI18nProvider locale={locale} modules={[...consoleModules, ...adminModules]} setLocale={setLocale}>
      <MemoryRouter initialEntries={[admin ? "/admin/overview" : "/console/overview"]}>
        {admin
          ? <MemoryAdminShell modules={modules} permissionScope={["*"]} registry={fixtureRegistry} userLabel="Operations reviewer" />
          : <MemoryConsoleShell modules={modules} permissionScope={["*"]} registry={fixtureRegistry} userLabel="Memory owner" />}
      </MemoryRouter>
    </MemoryI18nProvider>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("Visual fixture root is missing");
createRoot(root).render(<StrictMode><VisualFixture /></StrictMode>);
