import { describe, expect, it, vi } from "vitest";

import { createMemoryAdminResourceRegistry, type MemoryAdminSdkClient } from "@sdkwork/memory-pc-admin-core";
import { hasPermissionHint, normalizeMemoryPage } from "@sdkwork/memory-pc-commons";
import { createMemoryConsoleResourceRegistry, type MemoryConsoleSdkClient } from "@sdkwork/memory-pc-console-core";

describe("Memory resource contracts", () => {
  it("normalizes SDKWork list data without client-side pagination", () => {
    const result = normalizeMemoryPage({ items: [{ id: "1" }, { id: "2" }], pageInfo: { mode: "cursor", nextCursor: "next", hasMore: true } });
    expect(result.items).toHaveLength(2);
    expect(result.pageInfo.nextCursor).toBe("next");
    expect(result.pageInfo.hasMore).toBe(true);
  });

  it("rejects malformed SDK list data instead of fabricating an empty page", () => {
    expect(() => normalizeMemoryPage(undefined)).toThrow("Memory SDK contract violation");
    expect(() => normalizeMemoryPage({ items: "invalid", pageInfo: { mode: "cursor" } })).toThrow("data.items");
    expect(() => normalizeMemoryPage({ items: [], pageInfo: { mode: "cursor", pageSize: 201 } })).toThrow("pageInfo.pageSize");
  });

  it("passes console pagination to the generated app SDK", async () => {
    const list = vi.fn().mockResolvedValue({ items: [], pageInfo: { mode: "cursor" } });
    const client = { memory: { spaces: { list } } } as unknown as MemoryConsoleSdkClient;
    const source = createMemoryConsoleResourceRegistry(client).spaces;
    await source?.load({ q: "preference", cursor: "opaque", pageSize: 50 });
    expect(list).toHaveBeenCalledWith({ q: "preference", cursor: "opaque", pageSize: 50 }, { signal: undefined });
  });

  it("forwards the abort signal from the console registry to the generated app SDK", async () => {
    const list = vi.fn().mockResolvedValue({ items: [], pageInfo: { mode: "cursor" } });
    const client = { memory: { list } } as unknown as MemoryConsoleSdkClient;
    const controller = new AbortController();
    const source = createMemoryConsoleResourceRegistry(client).memories;
    await source?.load({ pageSize: 20, spaceId: "space-1" }, controller.signal);
    expect(list).toHaveBeenCalledWith({ pageSize: 20, spaceId: "space-1" }, { signal: controller.signal });
  });

  it("rejects a missing required memory space without calling the App SDK", async () => {
    const list = vi.fn();
    const client = { memory: { list } } as unknown as MemoryConsoleSdkClient;
    const source = createMemoryConsoleResourceRegistry(client).memories;
    await expect(source?.load({ pageSize: 20 })).rejects.toMatchObject({ code: 40003 });
    expect(list).not.toHaveBeenCalled();
  });

  it("passes admin pagination to the generated backend SDK", async () => {
    const list = vi.fn().mockResolvedValue({ items: [], pageInfo: { mode: "cursor" } });
    const client = { memory: { auditLogs: { list } } } as unknown as MemoryAdminSdkClient;
    const source = createMemoryAdminResourceRegistry(client).auditLogs;
    await source?.load({ q: "policy", cursor: "opaque", pageSize: 20 });
    expect(list).toHaveBeenCalledWith({ q: "policy", cursor: "opaque", pageSize: 20 }, { signal: undefined });
  });

  it("forwards the abort signal from the admin registry to the generated backend SDK", async () => {
    const list = vi.fn().mockResolvedValue({ items: [], pageInfo: { mode: "cursor" } });
    const client = { memory: { auditLogs: { list } } } as unknown as MemoryAdminSdkClient;
    const controller = new AbortController();
    const source = createMemoryAdminResourceRegistry(client).auditLogs;
    await source?.load({ q: "policy", pageSize: 20 }, controller.signal);
    expect(list).toHaveBeenCalledWith({ q: "policy", pageSize: 20 }, { signal: controller.signal });
  });

  it("exposes guarded Console mutations through the App SDK registry", async () => {
    const create = vi.fn().mockResolvedValue({ spaceId: "space-1" });
    const client = { memory: { spaces: { create, list: vi.fn() } } } as unknown as MemoryConsoleSdkClient;
    const actions = createMemoryConsoleResourceRegistry(client).spaces?.actions ?? [];
    const createAction = actions.find((action) => action.id === "create");
    expect(createAction?.requireIdempotencyKey).toBe(true);
    await createAction?.execute({ body: { ownerSubjectType: "user", ownerSubjectId: "u1", spaceType: "personal", displayName: "Personal" }, idempotencyKey: "request-1" });
    expect(create).toHaveBeenCalledWith(expect.not.objectContaining({ tenantId: expect.anything() }), { idempotencyKey: "request-1" });
  });

  it("marks DELETE mutations as selection and confirmation guarded without fake audit input", () => {
    const client = { memory: { subjects: { list: vi.fn() } } } as unknown as MemoryAdminSdkClient;
    const actions = createMemoryAdminResourceRegistry(client).subjects?.actions ?? [];
    const deleteAction = actions.find((action) => action.id === "delete");
    expect(deleteAction).toMatchObject({ dangerous: true, requiresSelection: true });
    expect(deleteAction?.requireAuditReason).not.toBe(true);
  });

  it("binds destructive command audit reasons to the typed request field", () => {
    const client = { memory: { retentionJobs: { list: vi.fn(), create: vi.fn() } } } as unknown as MemoryAdminSdkClient;
    const actions = createMemoryAdminResourceRegistry(client).retentionJobs?.actions ?? [];
    expect(actions.find((action) => action.id === "create")).toMatchObject({
      auditReasonField: "reason",
      dangerous: true,
      requireAuditReason: true,
      requireIdempotencyKey: true,
    });
  });
});

describe("Memory route permission hints", () => {
  it("supports exact and prefix wildcard permission hints", () => {
    expect(hasPermissionHint(["memory.records.read"], "memory.records.read")).toBe(true);
    expect(hasPermissionHint(["memory.backend.*"], "memory.backend.auditLogs.read")).toBe(true);
    expect(hasPermissionHint(["memory.records.read"], "memory.backend.records.read")).toBe(false);
  });
});
