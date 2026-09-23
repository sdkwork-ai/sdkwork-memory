import { normalizeMemoryItem, normalizeMemoryPage, normalizeMemoryRecord, type MemoryListQuery, type MemoryResourceAction, type MemoryResourceActionContext, type MemoryResourceDataSource, type MemoryResourceFilterDefinition, type MemoryResourceRegistry } from "@sdkwork/memory-pc-commons";
import type { SdkworkAppClient } from "@sdkwork/memory-app-sdk";
import { createContext, useContext, type ReactNode } from "react";

export type MemoryConsoleSdkClient = SdkworkAppClient;

const MemoryConsoleSdkContext = createContext<MemoryConsoleSdkClient | null>(null);

export function MemoryConsoleSdkProvider({ children, client }: { children: ReactNode; client: MemoryConsoleSdkClient }) {
  return <MemoryConsoleSdkContext.Provider value={client}>{children}</MemoryConsoleSdkContext.Provider>;
}

export function useMemoryConsoleSdk(): MemoryConsoleSdkClient {
  const client = useContext(MemoryConsoleSdkContext);
  if (!client) throw new Error("MemoryConsoleSdkProvider is required");
  return client;
}

export function createMemoryConsoleResourceRegistry(client: MemoryConsoleSdkClient): MemoryResourceRegistry {
  // Actions marked `requireIdempotencyKey` reach the SDK contracts that demand a key.
  // The page generates one when the command drawer opens, so this guard only fires if a
  // host ever executes an idempotent action without going through that drawer.
  const idempotency = (context: MemoryResourceActionContext) => {
    const idempotencyKey = context.idempotencyKey?.trim();
    if (!idempotencyKey) {
      throw new Error("Memory console action requires an idempotency key before execution");
    }
    return { idempotencyKey };
  };
  return {
    spaces: withActions(listSource((query) => client.memory.spaces.list(toListParams(query)), {
      detail: async (item) => normalizeMemoryRecord(await client.memory.spaces.retrieve(itemId(item, "spaceId", "id"))),
    }), [
      action("create", "Create space", { ownerSubjectType: "user", ownerSubjectId: "", spaceType: "personal", displayName: "" }, (context) => client.memory.spaces.create(context.body as unknown as Parameters<typeof client.memory.spaces.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update space", { ownerSubjectType: "user", ownerSubjectId: "", spaceType: "personal", displayName: "", lifecycleStatus: "active", version: "" }, (context) => client.memory.spaces.update(selectedId(context, "spaceId"), context.body as unknown as Parameters<typeof client.memory.spaces.update>[1]), { selection: true }),
    ]),
    memories: withActions(listSource((query) => client.memory.list({ ...toListParams(query), spaceId: requireSpaceId(query) }), {
      // Memory type is the console's primary narrowing axis: spaces hold mixed
      // working/semantic/procedural records and reviewers triage by kind.
      filters: [{ param: "memoryType", labelKey: "memory.filters.memoryType", options: ["working", "session", "semantic", "episodic", "procedural", "habit", "relationship", "domain_knowledge"] }],
      detail: async (item, query) => {
        const memoryId = itemId(item, "memoryId", "id");
        const record = normalizeMemoryRecord(await client.memory.retrieve(memoryId, { spaceId: itemSpaceId(item, query) }));
        // Provenance is what separates a traceable memory from an opaque one, so the
        // detail record carries its sources. A sources failure is reported on the
        // record instead of being swallowed or hiding the record itself.
        return { ...record, ...await readMemorySources(client, memoryId) };
      },
    }), [
      action("create", "Create memory", { spaceId: "", scope: "user", memoryType: "semantic", canonicalText: "", sensitivityLevel: "internal" }, (context) => client.memory.create(context.body as unknown as Parameters<typeof client.memory.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update memory", { spaceId: "", canonicalText: "", subject: "", summaryText: "", metadata: {} }, (context) => { const { spaceId, ...patch } = context.body; return client.memory.update(selectedId(context, "memoryId"), patch as unknown as Parameters<typeof client.memory.update>[1], { spaceId: String(spaceId ?? "") }); }, { selection: true }),
      action("delete", "Delete memory", { spaceId: "" }, (context) => client.memory.delete(selectedId(context, "memoryId"), { spaceId: String(context.body.spaceId ?? "") }), { dangerous: true, selection: true }),
    ]),
    events: actionSource([
      action("create", "Ingest event", { spaceId: "", userId: "", actorType: "user", eventType: "message", payload: {}, sensitivityLevel: "internal" }, (context) => client.memory.events.create(context.body as unknown as Parameters<typeof client.memory.events.create>[0], idempotency(context)), { idempotent: true }),
    ]),
    candidates: withActions(listSource((query) => client.memory.candidates.list(toListParams(query)), {
      detail: async (item) => normalizeMemoryRecord(await client.memory.candidates.retrieve(itemId(item, "candidateId", "id"))),
      filters: [{ param: "decisionState", labelKey: "memory.filters.decisionState", options: ["pending", "auto_approved", "approved", "rejected", "expired", "superseded"] }],
    }), [
      action("approve", "Approve candidate", { reason: "" }, (context) => client.memory.candidates.approve(selectedId(context, "candidateId"), context.body as unknown as Parameters<typeof client.memory.candidates.approve>[1], idempotency(context)), { idempotent: true, selection: true }),
      action("reject", "Reject candidate", { reason: "" }, (context) => client.memory.candidates.reject(selectedId(context, "candidateId"), context.body as unknown as Parameters<typeof client.memory.candidates.reject>[1], idempotency(context)), { dangerous: true, idempotent: true, reason: true, selection: true }),
    ]),
    habits: withActions(listSource((query) => client.memory.habits.list(toListParams(query)), {
      detail: async (item) => normalizeMemoryRecord(await client.memory.habits.retrieve(itemId(item, "habitId", "id"))),
      filters: [{ param: "stage", labelKey: "memory.filters.stage", options: ["observing", "emerging", "confirmed", "decaying", "inactive", "rejected"] }],
    }), [
      action("update", "Update habit", { description: "", confidence: 0.5, version: "" }, (context) => client.memory.habits.update(selectedId(context, "habitId"), context.body as unknown as Parameters<typeof client.memory.habits.update>[1]), { selection: true }),
      action("confirm", "Confirm habit", { reason: "" }, (context) => client.memory.habits.confirm(selectedId(context, "habitId"), context.body as unknown as Parameters<typeof client.memory.habits.confirm>[1], idempotency(context)), { idempotent: true, selection: true }),
      action("reject", "Reject habit", { reason: "" }, (context) => client.memory.habits.reject(selectedId(context, "habitId"), context.body as unknown as Parameters<typeof client.memory.habits.reject>[1], idempotency(context)), { dangerous: true, idempotent: true, reason: true, selection: true }),
    ]),
    learningSettings: withActions(itemSource(() => client.memory.learningSettings.retrieve()), [
      action("update", "Update settings", { autoExtractEnabled: true, autoApproveThreshold: 0.9, habitLearningEnabled: true }, (context) => client.memory.learningSettings.update(context.body as unknown as Parameters<typeof client.memory.learningSettings.update>[0])),
    ]),
    retrievals: actionSource([
      action("create", "Run retrieval", { query: "", spaceIds: [], topK: 10, contextBudgetTokens: 2048, includeTrace: true }, (context) => client.memory.retrievals.create(context.body as unknown as Parameters<typeof client.memory.retrievals.create>[0], idempotency(context)), { idempotent: true }),
    ]),
    contextPacks: actionSource([
      action("create", "Create context pack", { query: "", spaceIds: [], contextBudgetTokens: 2048, includeCitations: true }, (context) => client.memory.contextPacks.create(context.body as unknown as Parameters<typeof client.memory.contextPacks.create>[0], idempotency(context)), { idempotent: true }),
    ]),
    feedback: actionSource([
      action("create", "Submit feedback", { targetType: "retrieval", targetId: "", feedbackType: "relevance", score: 1, comment: "" }, (context) => client.memory.feedback.create(context.body as unknown as Parameters<typeof client.memory.feedback.create>[0], idempotency(context)), { idempotent: true }),
    ]),
    entities: withActions(listSource((query) => client.memory.entities.list({ ...toListParams(query), spaceId: query.spaceId }), {
      detail: async (item) => normalizeMemoryRecord(await client.memory.entities.retrieve(itemId(item, "entityId", "id"))),
      // Entity types are tenant-defined, so this stays a free-text filter.
      filters: [{ param: "entityType", labelKey: "memory.filters.entityType" }],
    }), [
      action("create", "Create entity", { spaceId: "", entityType: "person", canonicalName: "", sensitivityLevel: "internal" }, (context) => client.memory.entities.create(context.body as unknown as Parameters<typeof client.memory.entities.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update entity", { canonicalName: "", status: "active" }, (context) => client.memory.entities.update(selectedId(context, "entityId"), context.body as unknown as Parameters<typeof client.memory.entities.update>[1]), { selection: true }),
    ]),
    policyAssignments: withActions(listSource((query) => client.memory.policyAssignments.list(toListParams(query))), [
      action("create", "Assign policy", { policyId: "", targetType: "space", targetId: "", priority: 0, inheritanceMode: "inherit" }, (context) => client.memory.policyAssignments.create(context.body as unknown as Parameters<typeof client.memory.policyAssignments.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update assignment", { priority: 0, inheritanceMode: "inherit", status: "active" }, (context) => client.memory.policyAssignments.update(selectedId(context, "policyAssignmentId"), context.body as unknown as Parameters<typeof client.memory.policyAssignments.update>[1]), { selection: true }),
    ]),
    forgetRequests: withActions(listSource((query) => client.memory.forgetRequests.list(toJobListParams(query)), {
      detail: async (item) => normalizeMemoryRecord(await client.memory.forgetRequests.retrieve(itemId(item, "forgetRequestId", "forgetJobId", "id"))),
    }), [
      action("create", "Create forget request", { scope: "memory", memoryIds: [], reason: "" }, (context) => client.memory.forgetRequests.create(context.body as unknown as Parameters<typeof client.memory.forgetRequests.create>[0], idempotency(context)), { dangerous: true, idempotent: true, reason: true }),
    ]),
    exportJobs: withActions(listSource((query) => client.memory.exportJobs.list(toJobListParams(query)), {
      detail: async (item) => normalizeMemoryRecord(await client.memory.exportJobs.retrieve(itemId(item, "exportJobId", "jobId", "id"))),
    }), [
      action("create", "Create export", { spaceIds: [], format: "json", includeEvents: true }, (context) => client.memory.exportJobs.create(context.body as unknown as Parameters<typeof client.memory.exportJobs.create>[0], idempotency(context)), { idempotent: true }),
    ]),
    extractionJobs: actionSource([
      // Extraction turns ingested events into candidate memories, so it is the command
      // that closes the ingest -> extract -> review loop the console exposes.
      action("create", "Start extraction", { spaceId: "", inputEvents: [], extractionMode: "hybrid", reviewRequired: true }, (context) => client.memory.extractions.create(context.body as unknown as Parameters<typeof client.memory.extractions.create>[0], idempotency(context)), { idempotent: true }),
    ]),
  };
}

function action(id: string, label: string, bodyTemplate: Record<string, unknown>, execute: MemoryResourceAction["execute"], options: { dangerous?: boolean; idempotent?: boolean; reason?: boolean; selection?: boolean } = {}): MemoryResourceAction {
  return { id, label, bodyTemplate, execute, dangerous: options.dangerous, requireAuditReason: options.reason, auditReasonField: options.reason ? "reason" : undefined, requireIdempotencyKey: options.idempotent, requiresSelection: options.selection };
}

function withActions(source: MemoryResourceDataSource, actions: readonly MemoryResourceAction[]): MemoryResourceDataSource {
  return { ...source, actions };
}

function requireSpaceId(query: MemoryListQuery): string {
  if (query.spaceId?.trim()) return query.spaceId.trim();
  throw Object.assign(new Error("spaceId is required"), { code: 40003 });
}

function selectedId(context: MemoryResourceActionContext, ...keys: string[]): string {
  for (const key of keys) {
    const value = context.selectedItem?.[key];
    if (typeof value === "string" || typeof value === "number") return String(value);
  }
  throw new Error("Selected resource id is unavailable");
}

type DetailLoader = NonNullable<MemoryResourceDataSource["loadDetail"]>;

interface ListSourceOptions {
  detail?: DetailLoader;
  filters?: readonly MemoryResourceFilterDefinition[];
}

function listSource(load: (query: MemoryListQuery) => Promise<unknown>, options: ListSourceOptions = {}): MemoryResourceDataSource {
  return {
    kind: "list",
    ...(options.detail ? { loadDetail: options.detail } : {}),
    ...(options.filters ? { filters: options.filters } : {}),
    async load(query) { return normalizeMemoryPage(await load(query)); },
  };
}

function itemSource(load: () => Promise<unknown>): MemoryResourceDataSource {
  return { kind: "retrieve", async load() { return normalizeMemoryItem(await load()); } };
}

/**
 * Action-only resource: the owning API exposes commands but no collection endpoint
 * (retrievals, context packs, feedback, event ingestion, extraction runs).
 *
 * Rendering an empty page is deliberate — the commands are the capability, and the
 * block defaults to the action toolbar when a collection is unavailable.
 */
function actionSource(actions: readonly MemoryResourceAction[]): MemoryResourceDataSource {
  return { actions, kind: "retrieve", async load() { return { items: [], pageInfo: { mode: "cursor", hasMore: false } }; } };
}

/** Reads the first id-looking field of a list row, failing loudly when none exists. */
function itemId(item: Record<string, unknown>, ...keys: string[]): string {
  for (const key of keys) {
    const value = item[key];
    if (typeof value === "string" && value.trim()) return value.trim();
    if (typeof value === "number") return String(value);
  }
  throw new Error(`Memory console detail requires one of: ${keys.join(", ")}`);
}

/** Reads the space scope from the selected row, preferring the caller-supplied scope. */
function itemSpaceId(item: Record<string, unknown>, query: MemoryListQuery): string {
  const scoped = query.spaceId?.trim();
  if (scoped) return scoped;
  const value = item.spaceId;
  return typeof value === "string" || typeof value === "number" ? String(value) : "";
}

/**
 * Reads the provenance of a memory for the detail drawer.
 *
 * Sources are supplementary: a failure is reported as a field on the record rather
 * than hiding the record or being swallowed, so an operator can still see that
 * provenance exists and why it could not be read.
 */
async function readMemorySources(
  client: MemoryConsoleSdkClient,
  memoryId: string,
): Promise<{ sources: readonly unknown[] } | { sourcesUnavailable: string }> {
  try {
    const page = await client.memory.sources.list(memoryId, { pageSize: 20 });
    return { sources: page.items };
  } catch (reason) {
    return { sourcesUnavailable: reason instanceof Error ? reason.message : String(reason) };
  }
}

function toListParams(query: MemoryListQuery) {
  return { q: query.q, cursor: query.cursor, pageSize: query.pageSize, ...query.filterValues };
}

function toJobListParams(query: MemoryListQuery) {
  return { cursor: query.cursor, pageSize: query.pageSize };
}
