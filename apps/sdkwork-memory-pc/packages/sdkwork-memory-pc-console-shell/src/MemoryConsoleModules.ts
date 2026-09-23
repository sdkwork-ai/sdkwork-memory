import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { memoryModule as consoleGovernanceModule } from "@sdkwork/memory-pc-console-governance";
import { memoryModule as consoleKnowledgeModule } from "@sdkwork/memory-pc-console-knowledge";
import { memoryModule as consoleLearningModule } from "@sdkwork/memory-pc-console-learning";
import { memoryModule as consoleMemoryModule } from "@sdkwork/memory-pc-console-memory";
import { memoryModule as consoleOverviewModule } from "@sdkwork/memory-pc-console-overview";
import { memoryModule as consoleRetrievalModule } from "@sdkwork/memory-pc-console-retrieval";

/**
 * Canonical user-console Memory module catalog.
 *
 * This is the composition boundary for the Memory user console: the module list,
 * their order, their routes, and their permission requirements are owned here by
 * sdkwork-memory, so an embedding host never has to know which capability packages
 * exist. A host mounts the block with this catalog and inherits every current and
 * future Memory console module.
 *
 * Ordering is intentional and mirrors the user journey:
 * overview (what do I have) -> memory (inspect and correct) -> learning (pending
 * decisions) -> retrieval (prove recall) -> knowledge (entities) -> governance
 * (retention, forget, export).
 */
export const memoryConsoleModules: readonly MemoryPcModuleDefinition[] = assertUniqueConsoleModules([
  consoleOverviewModule,
  consoleMemoryModule,
  consoleLearningModule,
  consoleRetrievalModule,
  consoleKnowledgeModule,
  consoleGovernanceModule,
]);

/**
 * Resolves a module by its route segment, so a host can deep-link
 * `/console/memory/retrieval` without hardcoding the module id.
 */
export function findMemoryConsoleModuleByRoute(
  route: string | undefined,
): MemoryPcModuleDefinition | undefined {
  if (!route) return undefined;
  return memoryConsoleModules.find((candidate) => candidate.route === route);
}

/**
 * Fails fast on duplicate ids or duplicate `surface:route` pairs. A duplicate would
 * make the module switcher render two entries that route to the same page, and a
 * duplicate route would make deep links ambiguous.
 */
function assertUniqueConsoleModules(
  modules: readonly MemoryPcModuleDefinition[],
): readonly MemoryPcModuleDefinition[] {
  const ids = new Set<string>();
  const routes = new Set<string>();
  for (const module of modules) {
    if (ids.has(module.id)) throw new Error(`Duplicate Memory console module id: ${module.id}`);
    const routeKey = `${module.surface}:${module.route}`;
    if (routes.has(routeKey)) throw new Error(`Duplicate Memory console module route: ${routeKey}`);
    ids.add(module.id);
    routes.add(routeKey);
  }
  return modules;
}
