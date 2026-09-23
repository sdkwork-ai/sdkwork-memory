export type MemoryPcSurface = "app-console" | "backend-admin";

export type MemoryPcResourceKey =
  | "auditLogs"
  | "bindings"
  | "candidates"
  | "capabilities"
  | "capabilityBindings"
  | "commercialReadiness"
  | "consolidationJobs"
  | "contextPacks"
  | "edges"
  | "entities"
  | "evalRuns"
  | "events"
  | "exportJobs"
  | "extractionJobs"
  | "feedback"
  | "forgetRequests"
  | "habits"
  | "implementationProfiles"
  | "indexes"
  | "learningSettings"
  | "memories"
  | "migrationJobs"
  | "policies"
  | "policyAssignments"
  | "providerBindings"
  | "providerHealth"
  | "retentionJobs"
  | "retrievalProfiles"
  | "retrievals"
  | "retrievalTraces"
  | "spaces"
  | "subjects";

export type MemoryMessageCatalog = Readonly<Record<string, string>>;

export interface MemoryPcModuleDefinition {
  descriptionKey: string;
  id: string;
  messages: Readonly<Record<string, MemoryMessageCatalog>>;
  permission: string;
  resources: readonly MemoryPcResourceKey[];
  route: string;
  surface: MemoryPcSurface;
  titleKey: string;
}

export interface MemoryPageInfo {
  cursor?: string;
  hasMore?: boolean;
  mode: "cursor" | "offset";
  nextCursor?: string;
  page?: number;
  pageSize?: number;
  totalItems?: string;
  totalPages?: number;
}

export interface MemoryPageResult {
  items: readonly Record<string, unknown>[];
  pageInfo: MemoryPageInfo;
}

export interface MemoryListQuery {
  cursor?: string;
  /**
   * Resource-specific filter values, keyed by the query parameter they map onto
   * (`memoryType`, `entityType`, `decisionState`, `stage`, …).
   *
   * The registry declares which filters a resource supports through
   * {@link MemoryResourceDataSource.filters}; the page renders them generically and
   * hands the selected values back here, so a new server-side filter never needs a
   * page change.
   */
  filterValues?: Readonly<Record<string, string>>;
  pageSize: number;
  q?: string;
  spaceId?: string;
}

/**
 * A server-side list filter a resource supports.
 *
 * Ownership stays with the registry that binds the owning SDK operation: the page
 * only renders the control. `options` lists the accepted enum values, so the page
 * can render a closed select instead of a free-text box.
 */
export interface MemoryResourceFilterDefinition {
  /** Query parameter the selected value is sent as. */
  param: string;
  /** Message key of the filter label; falls back to the formatted `param`. */
  labelKey: string;
  /** Accepted values. Empty or omitted means a free-text filter. */
  options?: readonly string[];
}

export interface MemoryResourceDataSource {
  actions?: readonly MemoryResourceAction[];
  /**
   * Fetches the authoritative single-record payload for a row.
   *
   * List rows are projections: they may omit provenance, version chains, or
   * server-computed fields. Declaring this lets the detail drawer show the record
   * as the owning API returns it, without the host knowing which resource needs it.
   */
  loadDetail?(item: Record<string, unknown>, query: MemoryListQuery, signal?: AbortSignal): Promise<Record<string, unknown>>;
  filters?: readonly MemoryResourceFilterDefinition[];
  kind: "list" | "retrieve";
  load(query: MemoryListQuery, signal?: AbortSignal): Promise<MemoryPageResult>;
}

export interface MemoryResourceActionContext {
  auditReason?: string;
  body: Record<string, unknown>;
  idempotencyKey?: string;
  selectedItem?: Record<string, unknown>;
}

export interface MemoryResourceAction {
  auditReasonField?: string;
  bodyTemplate: Record<string, unknown>;
  dangerous?: boolean;
  execute(context: MemoryResourceActionContext): Promise<unknown>;
  id: string;
  label: string;
  requireAuditReason?: boolean;
  requireIdempotencyKey?: boolean;
  requiresSelection?: boolean;
}

export type MemoryResourceRegistry = Partial<Record<MemoryPcResourceKey, MemoryResourceDataSource>>;
