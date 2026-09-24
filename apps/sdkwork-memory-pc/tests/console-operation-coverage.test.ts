import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { createMemoryConsoleResourceRegistry, type MemoryConsoleSdkClient } from "@sdkwork/memory-pc-console-core";
import { memoryConsoleModules } from "@sdkwork/memory-pc-console-shell";

const appRoot = resolve(import.meta.dirname, "..");
const repositoryRoot = resolve(appRoot, "..", "..");

interface OpenApiOperation {
  operationId?: string;
  "x-sdkwork-permission"?: string;
}

interface OpenApiDocument {
  components?: { schemas?: Record<string, unknown> };
  paths?: Record<string, Record<string, OpenApiOperation>>;
}

const HTTP_METHODS = ["delete", "get", "patch", "post", "put"] as const;

function readJson(relativePath: string): unknown {
  return JSON.parse(readFileSync(resolve(repositoryRoot, relativePath), "utf8")) as unknown;
}

const appApi = readJson("apis/app-api/memory-app-api.openapi.json") as OpenApiDocument;

/** Every operation the Memory app-api contract declares, with its permission code. */
const appApiOperations = new Map<string, string>();
for (const item of Object.values(appApi.paths ?? {})) {
  for (const method of HTTP_METHODS) {
    const operation = item[method];
    if (!operation?.operationId) continue;
    appApiOperations.set(operation.operationId, operation["x-sdkwork-permission"] ?? "");
  }
}

/**
 * Operations the user console wires, keyed by the registry resource that owns them.
 *
 * Provenance is `memories.sources.list`, which the memory detail loader calls to show
 * where a memory came from — the console equivalent of mem0's per-memory history.
 */
const CONSOLE_OPERATION_COVERAGE: Readonly<Record<string, readonly string[]>> = {
  spaces: ["spaces.create", "spaces.list", "spaces.retrieve", "spaces.update"],
  memories: ["memories.create", "memories.delete", "memories.list", "memories.retrieve", "memories.sources.list", "memories.update"],
  events: ["events.create"],
  candidates: ["candidates.approve", "candidates.list", "candidates.reject", "candidates.retrieve"],
  habits: ["habits.confirm", "habits.list", "habits.reject", "habits.retrieve", "habits.update"],
  learningSettings: ["learningSettings.retrieve", "learningSettings.update"],
  retrievals: ["retrievals.create"],
  contextPacks: ["contextPacks.create"],
  feedback: ["feedback.create"],
  entities: ["entities.create", "entities.list", "entities.retrieve", "entities.update"],
  policyAssignments: ["policyAssignments.create", "policyAssignments.list", "policyAssignments.update"],
  forgetRequests: ["forgetRequests.create", "forgetRequests.list", "forgetRequests.retrieve"],
  exportJobs: ["exportJobs.create", "exportJobs.list", "exportJobs.retrieve"],
  extractionJobs: ["extractions.create"],
};

/**
 * Operations the user console deliberately does not reach, with the reason.
 *
 * A validator cannot judge whether an omission is intentional, so every omission is
 * recorded here: a newly added operation that is neither wired nor listed fails the
 * suite instead of shipping an unreachable management endpoint.
 */
const CONSOLE_EXCLUDED_OPERATIONS: Readonly<Record<string, string>> = {
  "events.retrieve":
    "app-api exposes no event collection, so the console cannot obtain an event id to retrieve; ingestion is exposed as the events.create command.",
  "memories.deleteAll":
    "Bulk memory purge is a destructive tenant-wide operation without a row selection; the user console deliberately offers per-memory delete (memories.delete) instead, so delete_all stays out of the user surface.",
  "retrievals.retrieve":
    "Retrievals are commands: retrievals.create already returns the full result, and app-api exposes no retrieval collection to browse.",
  "contextPacks.retrieve":
    "Context packs are commands: contextPacks.create already returns the pack, and app-api exposes no collection endpoint.",
};

/**
 * The registry is a pure builder: every SDK call it wires lives inside a closure, so a
 * placeholder client is enough to inspect which resources it declares.
 */
const consoleRegistry = createMemoryConsoleResourceRegistry({} as MemoryConsoleSdkClient);

describe("Memory user-console operation coverage", () => {
  it("reads the app-api contract", () => {
    expect(appApiOperations.size).toBeGreaterThan(0);
  });

  it("wires or explicitly excludes every app-api operation", () => {
    const covered = new Set(Object.values(CONSOLE_OPERATION_COVERAGE).flat());
    const unreachable = [...appApiOperations.keys()].filter(
      (operationId) => !covered.has(operationId) && !Object.hasOwn(CONSOLE_EXCLUDED_OPERATIONS, operationId),
    );
    expect(unreachable).toEqual([]);
  });

  it("keeps no stale coverage or exclusion entry", () => {
    const declared = [...Object.values(CONSOLE_OPERATION_COVERAGE).flat(), ...Object.keys(CONSOLE_EXCLUDED_OPERATIONS)];
    const unknown = declared.filter((operationId) => !appApiOperations.has(operationId));
    expect(unknown).toEqual([]);
  });

  it("never both wires and excludes the same operation", () => {
    const covered = new Set(Object.values(CONSOLE_OPERATION_COVERAGE).flat());
    const contradictory = Object.keys(CONSOLE_EXCLUDED_OPERATIONS).filter((operationId) => covered.has(operationId));
    expect(contradictory).toEqual([]);
  });

  it("binds every covered operation to a registry resource that exists", () => {
    const missing = Object.keys(CONSOLE_OPERATION_COVERAGE).filter(
      (resource) => !Object.hasOwn(consoleRegistry, resource),
    );
    expect(missing).toEqual([]);
  });

  it("surfaces every wired resource in at least one console module", () => {
    // Coverage without a module is an orbit without a surface: the registry could wire
    // an operation that no user-facing module ever renders.
    const shown = new Set<string>(
      memoryConsoleModules.flatMap((module) => [...module.resources] as string[]),
    );
    const unreachable = Object.keys(CONSOLE_OPERATION_COVERAGE).filter((resource) => !shown.has(resource));
    expect(unreachable).toEqual([]);
  });

  it("binds every resource a console module shows to a registry data source", () => {
    const unbound: string[] = [];
    for (const module of memoryConsoleModules) {
      for (const resource of module.resources) {
        if (!Object.hasOwn(consoleRegistry, resource)) unbound.push(`${module.id}:${resource}`);
      }
    }
    expect(unbound).toEqual([]);
  });

  it("keeps every wired operation inside the memory permission catalog", () => {
    const manifest = readJson("specs/iam.module.manifest.json") as {
      permissions?: { catalog?: { code?: string }[] };
    };
    const declaredCodes = new Set(
      (manifest.permissions?.catalog ?? [])
        .map((entry) => entry.code)
        .filter((code): code is string => typeof code === "string"),
    );
    expect(declaredCodes.size).toBeGreaterThan(0);

    const codes = [...appApiOperations.keys()]
      .filter((operationId) => Object.values(CONSOLE_OPERATION_COVERAGE).flat().includes(operationId))
      .map((operationId) => appApiOperations.get(operationId) ?? "");
    const unknown = [...new Set(codes)].filter((code) => !declaredCodes.has(code));
    expect(unknown).toEqual([]);
  });
});
