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
import {
  MEMORY_COMMONS_CATALOGS,
  MEMORY_SUPPORTED_LOCALES,
  type MemoryLocale,
  type MemoryMessageCatalog,
  type MemoryPcModuleDefinition,
} from "@sdkwork/memory-pc-commons";
import { memoryConsoleModules } from "@sdkwork/memory-pc-console-shell";

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

function keySet(catalog: MemoryMessageCatalog): string[] {
  return Object.keys(catalog).sort();
}

describe("Memory PC i18n catalog parity", () => {
  it("ships commons catalogs for every supported locale", () => {
    for (const locale of MEMORY_SUPPORTED_LOCALES) {
      const catalog = MEMORY_COMMONS_CATALOGS[locale];
      expect(Object.keys(catalog).length, `commons catalog ${locale} must not be empty`).toBeGreaterThan(0);
    }
  });

  it("keeps every commons catalog aligned with the en-US key set", () => {
    const fallbackKeys = keySet(MEMORY_COMMONS_CATALOGS["en-US"]);
    expect(fallbackKeys.length).toBeGreaterThan(0);
    for (const locale of MEMORY_SUPPORTED_LOCALES) {
      expect(keySet(MEMORY_COMMONS_CATALOGS[locale]), `commons catalog ${locale} keys drift`).toEqual(fallbackKeys);
    }
  });

  it("wires module catalogs for every supported locale with the en-US key set", () => {
    for (const module of allModules) {
      expect(module.messages["en-US"], `${module.id} must declare an en-US catalog`).toBeDefined();
      const fallbackKeys = keySet(module.messages["en-US"] ?? {});
      for (const locale of MEMORY_SUPPORTED_LOCALES as readonly MemoryLocale[]) {
        const catalog = module.messages[locale];
        expect(catalog, `${module.id} must declare a ${locale} catalog`).toBeDefined();
        expect(keySet(catalog ?? {}), `${module.id} ${locale} keys drift`).toEqual(fallbackKeys);
      }
    }
  });
});
