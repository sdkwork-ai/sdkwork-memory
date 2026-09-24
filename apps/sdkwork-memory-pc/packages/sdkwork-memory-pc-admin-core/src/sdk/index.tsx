import { normalizeMemoryItem, normalizeMemoryPage, type MemoryListQuery, type MemoryResourceAction, type MemoryResourceActionContext, type MemoryResourceDataSource, type MemoryResourceRegistry } from "@sdkwork/memory-pc-commons";
import { createClient, type SdkworkBackendClient } from "@sdkwork/memory-backend-sdk";
import type { AuthTokenManager } from "@sdkwork/sdk-common";
import { createContext, useContext, type ReactNode } from "react";

export type MemoryAdminSdkClient = SdkworkBackendClient;

const MemoryAdminSdkContext = createContext<MemoryAdminSdkClient | null>(null);

export function createMemoryAdminSdkClient(baseUrl: string, tokenManager: AuthTokenManager): MemoryAdminSdkClient {
  return createClient({ baseUrl, authMode: "dual-token", platform: "pc", tokenManager });
}

export function MemoryAdminSdkProvider({ children, client }: { children: ReactNode; client: MemoryAdminSdkClient }) {
  return <MemoryAdminSdkContext.Provider value={client}>{children}</MemoryAdminSdkContext.Provider>;
}

export function useMemoryAdminSdk(): MemoryAdminSdkClient {
  const client = useContext(MemoryAdminSdkContext);
  if (!client) throw new Error("MemoryAdminSdkProvider is required");
  return client;
}

export function createMemoryAdminResourceRegistry(client: MemoryAdminSdkClient): MemoryResourceRegistry {
  // Generated contracts require a string idempotency key on every command
  // operation; fail closed with the same guard the console registry applies so
  // a host bypassing the command drawer cannot emit unguarded writes.
  const idempotency = (context: MemoryResourceActionContext) => {
    const idempotencyKey = context.idempotencyKey?.trim();
    if (!idempotencyKey) {
      throw new Error("Memory admin action requires an idempotency key before execution");
    }
    return { idempotencyKey };
  };
  return {
    spaces: withActions(listSource((query) => client.memory.spaces.list(toListParams(query))), [
      action("update", "Update space", { ownerSubjectType: "user", ownerSubjectId: "", spaceType: "personal", displayName: "", lifecycleStatus: "active", version: "" }, (context) => client.memory.spaces.update(selectedId(context, "spaceId"), context.body as unknown as Parameters<typeof client.memory.spaces.update>[1]), { selection: true }),
    ]),
    memories: withActions(listSource((query) => client.memory.list(toListParams(query))), [
      action("update", "Update memory", { spaceId: "", canonicalText: "", subject: "", summaryText: "", metadata: {} }, (context) => { const { spaceId, ...patch } = context.body; return client.memory.update(selectedId(context, "memoryId"), patch as unknown as Parameters<typeof client.memory.update>[1], { spaceId: String(spaceId ?? "") }); }, { selection: true }),
      action("supersede", "Supersede memory", { spaceId: "", scope: "user", memoryType: "semantic", canonicalText: "", sensitivityLevel: "internal", version: "" }, (context) => client.memory.supersede(selectedId(context, "memoryId"), context.body as unknown as Parameters<typeof client.memory.supersede>[1], idempotency(context)), { dangerous: true, idempotent: true, selection: true }),
    ]),
    events: listSource((query) => client.memory.events.list(toListParams(query))),
    candidates: withActions(listSource((query) => client.memory.candidates.list(toListParams(query))), [
      action("approve", "Approve candidate", { reason: "" }, (context) => client.memory.candidates.approve(selectedId(context, "candidateId"), context.body as unknown as Parameters<typeof client.memory.candidates.approve>[1], idempotency(context)), { idempotent: true, selection: true }),
      action("reject", "Reject candidate", { reason: "" }, (context) => client.memory.candidates.reject(selectedId(context, "candidateId"), context.body as unknown as Parameters<typeof client.memory.candidates.reject>[1], idempotency(context)), { dangerous: true, idempotent: true, reason: true, selection: true }),
    ]),
    extractionJobs: withActions(listSource((query) => client.memory.extractionJobs.list(toJobListParams(query, true))), [action("create", "Start extraction", { spaceId: "", inputEvents: [], mode: "incremental" }, (context) => client.memory.extractionJobs.create(context.body as unknown as Parameters<typeof client.memory.extractionJobs.create>[0], idempotency(context)), { idempotent: true })]),
    consolidationJobs: withActions(listSource((query) => client.memory.consolidationJobs.list(toJobListParams(query))), [action("create", "Start consolidation", { spaceId: "", inputEvents: [], mode: "canonical" }, (context) => client.memory.consolidationJobs.create(context.body as unknown as Parameters<typeof client.memory.consolidationJobs.create>[0], idempotency(context)), { idempotent: true })]),
    indexes: withActions(listSource((query) => client.memory.indexes.list(toListParams(query))), [
      action("create", "Create index", { spaceId: "", indexKind: "keyword", schemaVersion: "1" }, (context) => client.memory.indexes.create(context.body as unknown as Parameters<typeof client.memory.indexes.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update index", { status: "active", version: "" }, (context) => client.memory.indexes.update(selectedId(context, "indexId"), context.body as unknown as Parameters<typeof client.memory.indexes.update>[1]), { selection: true }),
      action("rebuild", "Rebuild index", { reason: "" }, (context) => client.memory.indexes.rebuild(selectedId(context, "indexId"), context.body as unknown as Parameters<typeof client.memory.indexes.rebuild>[1], idempotency(context)), { dangerous: true, idempotent: true, reason: true, selection: true }),
    ]),
    retrievalProfiles: withActions(listSource((query) => client.memory.retrievalProfiles.list(toListParams(query))), [
      action("create", "Create retrieval profile", { name: "", strategy: "balanced", retrievers: ["keyword"], topK: 10, contextBudgetTokens: 2048 }, (context) => client.memory.retrievalProfiles.create(context.body as unknown as Parameters<typeof client.memory.retrievalProfiles.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update retrieval profile", { name: "", strategy: "balanced", retrievers: ["keyword"], topK: 10, contextBudgetTokens: 2048, version: "" }, (context) => client.memory.retrievalProfiles.update(selectedId(context, "retrievalProfileId", "profileId"), context.body as unknown as Parameters<typeof client.memory.retrievalProfiles.update>[1]), { selection: true }),
    ]),
    retrievalTraces: listSource((query) => client.memory.retrievalTraces.list(toListParams(query))),
    implementationProfiles: withActions(listSource((query) => client.memory.implementationProfiles.list(toListParams(query))), [
      action("create", "Create implementation profile", { name: "", implementationKind: "native_sql", role: "primary", capabilities: ["keyword_retrieval"], status: "active" }, (context) => client.memory.implementationProfiles.create(context.body as unknown as Parameters<typeof client.memory.implementationProfiles.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update implementation profile", { name: "", implementationKind: "native_sql", role: "primary", capabilities: ["keyword_retrieval"], status: "active", version: "" }, (context) => client.memory.implementationProfiles.update(selectedId(context, "implementationProfileId"), context.body as unknown as Parameters<typeof client.memory.implementationProfiles.update>[1]), { selection: true }),
    ]),
    providerBindings: withActions(listSource((query) => client.memory.providerBindings.list(toListParams(query))), [
      action("create", "Create provider binding", { providerKind: "embedding", providerCode: "", displayName: "", capabilities: ["embedding"], status: "active" }, (context) => client.memory.providerBindings.create(context.body as unknown as Parameters<typeof client.memory.providerBindings.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update provider binding", { providerKind: "embedding", providerCode: "", displayName: "", capabilities: ["embedding"], status: "active", version: "" }, (context) => client.memory.providerBindings.update(selectedId(context, "providerBindingId"), context.body as unknown as Parameters<typeof client.memory.providerBindings.update>[1]), { selection: true }),
    ]),
    providerHealth: itemSource(() => client.memory.providerHealth.retrieve()),
    evalRuns: withActions(listSource((query) => client.memory.evalRuns.list(toListParams(query))), [
      action("create", "Create evaluation run", { evalType: "retrieval_quality", config: { cases: [] } }, (context) => client.memory.evalRuns.create(context.body as unknown as Parameters<typeof client.memory.evalRuns.create>[0], idempotency(context)), { idempotent: true }),
    ]),
    auditLogs: listSource((query) => client.memory.auditLogs.list(toListParams(query))),
    subjects: withActions(listSource((query) => client.memory.subjects.list(toListParams(query))), [
      action("create", "Create subject", { subjectType: "user", subjectRef: "", displayName: "" }, (context) => client.memory.subjects.create(context.body as unknown as Parameters<typeof client.memory.subjects.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update subject", { displayName: "", status: "active" }, (context) => client.memory.subjects.update(selectedId(context, "subjectId"), context.body as unknown as Parameters<typeof client.memory.subjects.update>[1]), { selection: true }),
      action("delete", "Delete subject", {}, (context) => client.memory.subjects.delete(selectedId(context, "subjectId")), { dangerous: true, selection: true }),
    ]),
    bindings: withActions(listSource((query) => client.memory.bindings.list(toListParams(query))), [
      action("create", "Create binding", { bindingKind: "access", bindingRole: "viewer" }, (context) => client.memory.bindings.create(context.body as unknown as Parameters<typeof client.memory.bindings.create>[0], idempotency(context)), { idempotent: true }),
      action("delete", "Delete binding", {}, (context) => client.memory.bindings.delete(selectedId(context, "bindingId")), { dangerous: true, selection: true }),
    ]),
    capabilityBindings: withActions(listSource((query) => client.memory.capabilityBindings.list(toListParams(query))), [
      action("create", "Create capability binding", { capabilityCode: "", targetType: "subject", targetId: "", mode: "allow", priority: 0 }, (context) => client.memory.capabilityBindings.create(context.body as unknown as Parameters<typeof client.memory.capabilityBindings.create>[0], idempotency(context)), { idempotent: true }),
      action("delete", "Delete capability binding", {}, (context) => client.memory.capabilityBindings.delete(selectedId(context, "capabilityBindingId")), { dangerous: true, selection: true }),
    ]),
    capabilities: actionSource([action("resolve", "Resolve capabilities", { targetType: "subject", targetId: "" }, (context) => client.memory.capabilities.resolve(context.body as unknown as Parameters<typeof client.memory.capabilities.resolve>[0], idempotency(context)), { idempotent: true })]),
    entities: withActions(listSource((query) => client.memory.entities.list({ ...toListParams(query), spaceId: query.spaceId })), [
      action("create", "Create entity", { spaceId: "", entityType: "person", canonicalName: "", sensitivityLevel: "internal" }, (context) => client.memory.entities.create(context.body as unknown as Parameters<typeof client.memory.entities.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update entity", { canonicalName: "", status: "active" }, (context) => client.memory.entities.update(selectedId(context, "entityId"), context.body as unknown as Parameters<typeof client.memory.entities.update>[1]), { selection: true }),
    ]),
    edges: withActions(listSource((query) => client.memory.edges.list({ ...toListParams(query), spaceId: query.spaceId })), [
      action("create", "Create edge", { spaceId: "", sourceEntityId: "", targetEntityId: "", relationType: "related_to" }, (context) => client.memory.edges.create(context.body as unknown as Parameters<typeof client.memory.edges.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update edge", { relationType: "related_to", status: "active" }, (context) => client.memory.edges.update(selectedId(context, "edgeId"), context.body as unknown as Parameters<typeof client.memory.edges.update>[1]), { selection: true }),
      action("delete", "Delete edge", {}, (context) => client.memory.edges.delete(selectedId(context, "edgeId")), { dangerous: true, selection: true }),
    ]),
    policies: withActions(listSource((query) => client.memory.policies.list(toListParams(query))), [
      action("create", "Create policy", { policyType: "retention", scope: "tenant", policy: {} }, (context) => client.memory.policies.create(context.body as unknown as Parameters<typeof client.memory.policies.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update policy", { status: "active", policy: {} }, (context) => client.memory.policies.update(selectedId(context, "policyId"), context.body as unknown as Parameters<typeof client.memory.policies.update>[1]), { selection: true }),
      action("delete", "Delete policy", {}, (context) => client.memory.policies.delete(selectedId(context, "policyId")), { dangerous: true, selection: true }),
    ]),
    policyAssignments: withActions(listSource((query) => client.memory.policyAssignments.list(toListParams(query))), [
      action("create", "Assign policy", { policyId: "", targetType: "space", targetId: "", priority: 0, inheritanceMode: "inherit" }, (context) => client.memory.policyAssignments.create(context.body as unknown as Parameters<typeof client.memory.policyAssignments.create>[0], idempotency(context)), { idempotent: true }),
      action("update", "Update assignment", { priority: 0, inheritanceMode: "inherit", status: "active" }, (context) => client.memory.policyAssignments.update(selectedId(context, "policyAssignmentId"), context.body as unknown as Parameters<typeof client.memory.policyAssignments.update>[1]), { selection: true }),
      action("delete", "Delete assignment", {}, (context) => client.memory.policyAssignments.delete(selectedId(context, "policyAssignmentId")), { dangerous: true, selection: true }),
    ]),
    retentionJobs: withActions(listSource((query) => client.memory.retentionJobs.list(toJobListParams(query))), [action("create", "Start retention", { scope: "tenant", reason: "", dryRun: true }, (context) => client.memory.retentionJobs.create(context.body as unknown as Parameters<typeof client.memory.retentionJobs.create>[0], idempotency(context)), { dangerous: true, idempotent: true, reason: true })]),
    migrationJobs: withActions(listSource((query) => client.memory.migrationJobs.list(toJobListParams(query))), [action("create", "Start migration", { sourceImplementationProfileId: "", targetImplementationProfileId: "", mode: "shadow", reason: "", dryRun: true }, (context) => client.memory.migrationJobs.create(context.body as unknown as Parameters<typeof client.memory.migrationJobs.create>[0], idempotency(context)), { dangerous: true, idempotent: true, reason: true })]),
    commercialReadiness: withActions(itemSource(() => client.memory.commercialReadiness.retrieve()), [action("rebuild", "Rebuild readiness", {}, (context) => client.memory.commercialReadiness.rebuild(context.body as unknown as Parameters<typeof client.memory.commercialReadiness.rebuild>[0], idempotency(context)), { idempotent: true })]),
  };
}

function action(id: string, label: string, bodyTemplate: Record<string, unknown>, execute: MemoryResourceAction["execute"], options: { dangerous?: boolean; idempotent?: boolean; reason?: boolean; selection?: boolean } = {}): MemoryResourceAction {
  return { id, label, bodyTemplate, execute, dangerous: options.dangerous, requireAuditReason: options.reason, auditReasonField: options.reason ? "reason" : undefined, requireIdempotencyKey: options.idempotent, requiresSelection: options.selection };
}

function withActions(source: MemoryResourceDataSource, actions: readonly MemoryResourceAction[]): MemoryResourceDataSource {
  return { ...source, actions };
}

function actionSource(actions: readonly MemoryResourceAction[]): MemoryResourceDataSource {
  return { actions, kind: "retrieve", async load() { return { items: [], pageInfo: { mode: "cursor", hasMore: false } }; } };
}

function selectedId(context: MemoryResourceActionContext, ...keys: string[]): string {
  for (const key of keys) {
    const value = context.selectedItem?.[key];
    if (typeof value === "string" || typeof value === "number") return String(value);
  }
  throw new Error("Selected resource id is unavailable");
}

function listSource(load: (query: MemoryListQuery) => Promise<unknown>): MemoryResourceDataSource {
  return { kind: "list", async load(query) { return normalizeMemoryPage(await load(query)); } };
}

function itemSource(load: () => Promise<unknown>): MemoryResourceDataSource {
  return { kind: "retrieve", async load() { return normalizeMemoryItem(await load()); } };
}

function toListParams(query: MemoryListQuery) {
  return { q: query.q, cursor: query.cursor, pageSize: query.pageSize };
}

function toJobListParams(query: MemoryListQuery, includeSpaceId = false) {
  return {
    cursor: query.cursor,
    pageSize: query.pageSize,
    ...(includeSpaceId && query.spaceId ? { spaceId: query.spaceId } : {}),
  };
}
