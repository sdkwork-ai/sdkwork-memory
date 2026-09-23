import type { MemoryPageInfo, MemoryPageResult } from "../types.ts";

export function normalizeMemoryPage(value: unknown): MemoryPageResult {
  if (!isRecord(value)) throw contractError("list response must be an object");
  const data = isRecord(value.data) ? value.data : value;
  if (!Array.isArray(data.items) || !data.items.every(isRecord)) {
    throw contractError("list response data.items must be an array of objects");
  }
  if (!isRecord(data.pageInfo)) {
    throw contractError("list response data.pageInfo must be an object");
  }
  return {
    items: data.items,
    pageInfo: normalizePageInfo(data.pageInfo),
  };
}

export function normalizeMemoryItem(value: unknown): MemoryPageResult {
  if (!isRecord(value)) throw contractError("resource response must be an object");
  const data = isRecord(value.data) ? value.data : value;
  const item = isRecord(data.item) ? data.item : data;
  if (!isRecord(item)) throw contractError("resource response data.item must be an object");
  return {
    items: [item],
    pageInfo: { mode: "offset", page: 1, pageSize: 1, totalItems: "1", totalPages: 1 },
  };
}

export function emptyMemoryPage(): MemoryPageResult {
  return { items: [], pageInfo: { mode: "cursor", hasMore: false } };
}

/**
 * Normalizes a single-resource response into the plain record the detail drawer renders.
 *
 * Unwraps the SDKWork envelope (`data.item`, then `data`) the same way the list
 * normalizer does, and rejects anything that is not an object: a detail loader must not
 * be able to hand the drawer a shape the contract never promised.
 */
export function normalizeMemoryRecord(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) throw contractError("resource response must be an object");
  const data = isRecord(value.data) ? value.data : value;
  const item = isRecord(data.item) ? data.item : data;
  if (!isRecord(item)) throw contractError("resource response data.item must be an object");
  return item;
}

function normalizePageInfo(value: Record<string, unknown>): MemoryPageInfo {
  if (value.mode !== "cursor" && value.mode !== "offset") {
    throw contractError("pageInfo.mode must be cursor or offset");
  }
  assertOptionalString(value, "cursor");
  assertOptionalString(value, "nextCursor", true);
  assertOptionalBoolean(value, "hasMore");
  assertOptionalInteger(value, "page", 1);
  assertOptionalInteger(value, "pageSize", 1, 200);
  assertOptionalInteger(value, "totalPages", 0);
  if (value.totalItems !== undefined && (typeof value.totalItems !== "string" || !/^\d+$/u.test(value.totalItems))) {
    throw contractError("pageInfo.totalItems must be an unsigned integer string");
  }
  return {
    mode: value.mode,
    ...(typeof value.cursor === "string" ? { cursor: value.cursor } : {}),
    ...(typeof value.nextCursor === "string" ? { nextCursor: value.nextCursor } : {}),
    ...(typeof value.hasMore === "boolean" ? { hasMore: value.hasMore } : {}),
    ...(typeof value.page === "number" ? { page: value.page } : {}),
    ...(typeof value.pageSize === "number" ? { pageSize: value.pageSize } : {}),
    ...(typeof value.totalItems === "string" ? { totalItems: value.totalItems } : {}),
    ...(typeof value.totalPages === "number" ? { totalPages: value.totalPages } : {}),
  };
}

function assertOptionalString(value: Record<string, unknown>, field: string, nullable = false): void {
  const candidate = value[field];
  if (candidate !== undefined && !(nullable && candidate === null) && typeof candidate !== "string") {
    throw contractError(`pageInfo.${field} must be a string${nullable ? " or null" : ""}`);
  }
}

function assertOptionalBoolean(value: Record<string, unknown>, field: string): void {
  if (value[field] !== undefined && typeof value[field] !== "boolean") {
    throw contractError(`pageInfo.${field} must be a boolean`);
  }
}

function assertOptionalInteger(value: Record<string, unknown>, field: string, minimum: number, maximum?: number): void {
  const candidate = value[field];
  if (candidate === undefined) return;
  if (typeof candidate !== "number" || !Number.isInteger(candidate) || candidate < minimum || (maximum !== undefined && candidate > maximum)) {
    throw contractError(`pageInfo.${field} is outside its contract range`);
  }
}

function contractError(detail: string): TypeError {
  return new TypeError(`Memory SDK contract violation: ${detail}`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
