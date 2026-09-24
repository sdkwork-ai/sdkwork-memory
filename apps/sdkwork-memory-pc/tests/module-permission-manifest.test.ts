import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { memoryModule as adminControlPlaneModule } from "@sdkwork/memory-pc-admin-control-plane";
import { memoryModule as adminEvaluationModule } from "@sdkwork/memory-pc-admin-evaluation";
import { memoryModule as adminGovernanceModule } from "@sdkwork/memory-pc-admin-governance";
import { memoryModule as adminKnowledgeGraphModule } from "@sdkwork/memory-pc-admin-knowledge-graph";
import { memoryModule as adminLearningModule } from "@sdkwork/memory-pc-admin-learning";
import { memoryModule as adminMemoryModule } from "@sdkwork/memory-pc-admin-memory";
import { memoryModule as adminOverviewModule } from "@sdkwork/memory-pc-admin-overview";
import { memoryModule as adminProvidersModule } from "@sdkwork/memory-pc-admin-providers";
import { memoryModule as adminRetrievalModule } from "@sdkwork/memory-pc-admin-retrieval";
import { memoryConsoleModules } from "@sdkwork/memory-pc-console-shell";
import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";

const appRoot = resolve(import.meta.dirname, "..");

interface MemoryPcAppManifest {
  backend?: {
    accessTokenPermissionScope?: readonly string[];
  };
}

const adminModules: readonly MemoryPcModuleDefinition[] = [
  adminOverviewModule,
  adminMemoryModule,
  adminLearningModule,
  adminRetrievalModule,
  adminProvidersModule,
  adminEvaluationModule,
  adminKnowledgeGraphModule,
  adminControlPlaneModule,
  adminGovernanceModule,
];

const allModules: readonly MemoryPcModuleDefinition[] = [...memoryConsoleModules, ...adminModules];

describe("Memory PC module permission manifest coverage", () => {
  it("grants every module permission in the app manifest access token scope", () => {
    const manifest = JSON.parse(
      readFileSync(resolve(appRoot, "sdkwork.app.config.json"), "utf8"),
    ) as MemoryPcAppManifest;
    const granted = new Set(manifest.backend?.accessTokenPermissionScope ?? []);
    expect(granted.size).toBeGreaterThan(0);

    const required = allModules.map((module) => module.permission);
    expect(required.length).toBeGreaterThan(0);

    // A module whose permission hint is missing from the manifest scope would render
    // permanently denied for every granted session, so the drift fails here instead.
    const missing = [...new Set(required)].filter((permission) => !granted.has(permission));
    expect(missing).toEqual([]);
  });
});
