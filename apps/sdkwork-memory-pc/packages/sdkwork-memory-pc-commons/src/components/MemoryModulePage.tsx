import { AlertTriangle, ChevronLeft, ChevronRight, Database, Play, RefreshCw, Search, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { uuid } from "@sdkwork/utils/id";

import { useMemoryI18n } from "../i18n/runtime.tsx";
import type { MemoryListQuery, MemoryPageResult, MemoryPcModuleDefinition, MemoryPcResourceKey, MemoryResourceAction, MemoryResourceFilterDefinition, MemoryResourceRegistry, MemorySpaceOption } from "../types.ts";

const DEFAULT_PAGE: MemoryPageResult = { items: [], pageInfo: { mode: "cursor", hasMore: false } };
const EMPTY_FILTERS: readonly MemoryResourceFilterDefinition[] = [];

export interface MemoryModulePageProps {
  module: MemoryPcModuleDefinition;
  registry: MemoryResourceRegistry;
}

export function MemoryModulePage({ module, registry }: MemoryModulePageProps) {
  const { locale, translate } = useMemoryI18n();
  const [resource, setResource] = useState<MemoryPcResourceKey>(module.resources[0] ?? "spaces");
  const [q, setQ] = useState("");
  // The server-backed list effect consumes the debounced value so typing does not
  // fire one request per keystroke (FRONTEND_CODE_SPEC §6: debounce network inputs).
  const debouncedQuery = useDebouncedValue(q.trim(), 300);
  const [spaceId, setSpaceId] = useState("");
  const [spaceOptions, setSpaceOptions] = useState<readonly MemorySpaceOption[]>([]);
  const [filterValues, setFilterValues] = useState<Record<string, string>>({});
  const [cursor, setCursor] = useState<string>();
  const [cursorHistory, setCursorHistory] = useState<string[]>([]);
  const [pageSize, setPageSize] = useState(20);
  const [refreshVersion, setRefreshVersion] = useState(0);
  const [page, setPage] = useState(DEFAULT_PAGE);
  const [detailRow, setDetailRow] = useState<Record<string, unknown> | null>(null);
  const [detailPayload, setDetailPayload] = useState<Record<string, unknown> | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [activeAction, setActiveAction] = useState<MemoryResourceAction>();
  const [actionBody, setActionBody] = useState("{}");
  const [auditReason, setAuditReason] = useState("");
  const [idempotencyKey, setIdempotencyKey] = useState("");
  const [actionConfirmed, setActionConfirmed] = useState(false);
  const [actionRunning, setActionRunning] = useState(false);
  const [actionError, setActionError] = useState<string>();
  const dataSource = registry[resource];
  /** Registry resource that enumerates selectable spaces, when the active source declares one. */
  const spaceOptionsSource = dataSource?.spaceOptionsSource;
  // The dropdown replaces the input only while it can represent the current value: a
  // hand-typed id that is not among the listed spaces falls back to the manual input.
  const spaceSelectorAvailable = spaceOptions.length > 0 && (spaceId.trim() === "" || spaceOptions.some((option) => option.spaceId === spaceId.trim()));
  /**
   * The record shown in the detail drawer: the authoritative detail payload once the
   * owning API has answered, otherwise the list projection that opened the drawer.
   */
  const selectedItem = detailPayload ?? detailRow;
  const parsedActionBody = useMemo(() => parseActionBody(actionBody), [actionBody]);
  // Stable identity while the resource stays the same: an inline `[]` default would
  // change every render and re-trigger the load effect forever.
  const declaredFilters = dataSource?.filters ?? EMPTY_FILTERS;

  /**
   * Filter values the active resource actually declares.
   *
   * Switching resource tabs must not leak a foreign parameter into the request, so
   * values are filtered against the declared set instead of being sent verbatim.
   */
  const activeFilterValues = useMemo(() => {
    const entries = declaredFilters
      .map((filter) => [filter.param, filterValues[filter.param]?.trim() ?? ""] as const)
      .filter(([, value]) => value.length > 0);
    return entries.length > 0 ? Object.fromEntries(entries) : undefined;
  }, [declaredFilters, filterValues]);

  function message(key: string, fallback: string): string {
    const value = translate(key);
    return value === key ? fallback : value;
  }

  function resourceLabel(value: string): string {
    return message(`memory.resources.${value}`, formatResourceName(value));
  }

  function fieldLabel(value: string): string {
    return message(`memory.fields.${value}`, formatResourceName(value));
  }

  function filterLabel(filter: MemoryResourceFilterDefinition): string {
    return message(filter.labelKey, formatResourceName(filter.param));
  }

  function filterOptionLabel(filter: MemoryResourceFilterDefinition, option: string): string {
    return message(`memory.filters.${filter.param}.${option}`, formatResourceName(option));
  }

  function updateFilterValue(param: string, value: string): void {
    setFilterValues((current) => {
      if (value) return { ...current, [param]: value };
      const next = { ...current };
      delete next[param];
      return next;
    });
  }

  function actionLabel(action: MemoryResourceAction): string {
    const verb = message(`memory.actions.${action.id}`, action.label);
    const target = resourceLabel(resource);
    return locale === "zh-CN" ? `${target}${verb}` : `${verb} ${target}`;
  }

  useEffect(() => {
    setCursor(undefined);
    setCursorHistory([]);
    setDetailRow(null);
    setDetailPayload(null);
    setDetailError(undefined);
    closeAction();
  }, [resource, q, spaceId, filterValues, pageSize]);

  useEffect(() => {
    if (!dataSource) {
      setPage(DEFAULT_PAGE);
      setLoading(false);
      setError(undefined);
      return;
    }
    const controller = new AbortController();
    const query: MemoryListQuery = {
      pageSize,
      ...(debouncedQuery ? { q: debouncedQuery } : {}),
      ...(spaceId.trim() ? { spaceId: spaceId.trim() } : {}),
      ...(cursor ? { cursor } : {}),
      ...(activeFilterValues ? { filterValues: activeFilterValues } : {}),
    };
    setLoading(true);
    setError(undefined);
    void dataSource.load(query, controller.signal).then((result) => {
      if (!controller.signal.aborted) setPage(result);
    }).catch((reason: unknown) => {
      if (!controller.signal.aborted) setError(readSafeError(reason, translate));
    }).finally(() => {
      if (!controller.signal.aborted) setLoading(false);
    });
    return () => controller.abort();
  }, [activeFilterValues, cursor, dataSource, debouncedQuery, pageSize, refreshVersion, spaceId]);

  // Space options come from the registry's own spaces data source, so the scope
  // selector lists real spaces instead of relying on a hand-typed id. They are an
  // aid, not a requirement: on failure the manual input stays as the fallback.
  useEffect(() => {
    const source = spaceOptionsSource ? registry[spaceOptionsSource] : undefined;
    if (!spaceOptionsSource || !source) {
      setSpaceOptions([]);
      return;
    }
    const controller = new AbortController();
    void source.load({ pageSize: 100 }, controller.signal).then((result) => {
      if (controller.signal.aborted) return;
      setSpaceOptions(result.items.map(readSpaceOption).filter((option): option is MemorySpaceOption => option !== null));
    }).catch(() => {
      if (!controller.signal.aborted) setSpaceOptions([]);
    });
    return () => controller.abort();
  }, [refreshVersion, registry, spaceOptionsSource]);

  // The drawer opens on the list projection immediately and is then replaced by the
  // authoritative single-record payload. Resources without a detail operation keep
  // showing the row, so the drawer never blocks on a capability the resource lacks.
  useEffect(() => {
    if (!detailRow || !dataSource?.loadDetail) return;
    const controller = new AbortController();
    const query: MemoryListQuery = {
      pageSize,
      ...(spaceId.trim() ? { spaceId: spaceId.trim() } : {}),
      ...(activeFilterValues ? { filterValues: activeFilterValues } : {}),
    };
    setDetailLoading(true);
    setDetailError(undefined);
    void dataSource.loadDetail(detailRow, query, controller.signal).then((detail) => {
      if (!controller.signal.aborted) setDetailPayload(detail);
    }).catch((reason: unknown) => {
      if (!controller.signal.aborted) setDetailError(readSafeError(reason, translate));
    }).finally(() => {
      if (!controller.signal.aborted) setDetailLoading(false);
    });
    return () => controller.abort();
    // `translate` is intentionally absent: hosts may pass a fresh `setLocale` closure
    // on every render, so depending on the memoized translator would re-run this effect
    // forever. The list effect above omits it for the same reason.
  }, [activeFilterValues, dataSource, detailRow, pageSize, spaceId]);

  const columns = useMemo(() => resolveColumns(page.items), [page.items]);
  const nextCursor = page.pageInfo.nextCursor;

  function openNextPage(): void {
    if (!nextCursor) return;
    setCursorHistory((history) => [...history, cursor ?? ""]);
    setCursor(nextCursor);
  }

  function openPreviousPage(): void {
    if (cursorHistory.length === 0) return;
    // Computed from the current render's state before both setters: calling
    // setCursor inside the setCursorHistory updater would be a side effect in a
    // state updater, which React expects to stay pure.
    const previous = cursorHistory.at(-1);
    setCursor(previous || undefined);
    setCursorHistory((history) => history.slice(0, -1));
  }

  function openDetail(item: Record<string, unknown>): void {
    setDetailRow(item);
    // Drop the previous payload so the drawer cannot flash a stale record while the
    // authoritative one is still in flight.
    setDetailPayload(null);
  }

  function closeDetail(): void {
    setDetailRow(null);
    setDetailPayload(null);
    setDetailError(undefined);
  }

  function openAction(action: MemoryResourceAction): void {
    setActiveAction(action);
    const body = { ...action.bodyTemplate };
    if (selectedItem?.version !== undefined && Object.hasOwn(body, "version")) {
      body.version = selectedItem.version;
    }
    setActionBody(JSON.stringify(body, null, 2));
    setAuditReason("");
    setIdempotencyKey(uuid());
    setActionConfirmed(false);
    setActionError(undefined);
  }

  function closeAction(): void {
    setActiveAction(undefined);
    setActionRunning(false);
    setActionError(undefined);
  }

  async function executeAction(): Promise<void> {
    if (!activeAction || actionRunning) return;
    if (activeAction.requiresSelection && !selectedItem) {
      setActionError(translate("memory.commons.selectResourceError"));
      return;
    }
    if (activeAction.requireAuditReason && !auditReason.trim()) {
      setActionError(translate("memory.commons.auditReasonError"));
      return;
    }
    if (activeAction.requireIdempotencyKey && !idempotencyKey.trim()) {
      setActionError(translate("memory.commons.idempotencyKeyError"));
      return;
    }
    if (activeAction.dangerous && !actionConfirmed) {
      setActionError(translate("memory.commons.confirmationError"));
      return;
    }
    let body: Record<string, unknown>;
    try {
      const parsed = JSON.parse(actionBody) as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error();
      body = parsed as Record<string, unknown>;
    } catch {
      setActionError(translate("memory.commons.requestBodyError"));
      return;
    }
    if (activeAction.auditReasonField && auditReason.trim()) {
      body[activeAction.auditReasonField] = auditReason.trim();
    }
    setActionRunning(true);
    setActionError(undefined);
    try {
      const result = await activeAction.execute({
        body,
        ...(selectedItem ? { selectedItem } : {}),
        ...(auditReason.trim() ? { auditReason: auditReason.trim() } : {}),
        ...(idempotencyKey.trim() ? { idempotencyKey: idempotencyKey.trim() } : {}),
      });
      if (result && typeof result === "object") setDetailPayload(result as Record<string, unknown>);
      setRefreshVersion((version) => version + 1);
      closeAction();
    } catch (reason) {
      setActionError(readSafeError(reason, translate) || translate("memory.commons.commandFailed"));
      setActionRunning(false);
    }
  }

  function updateActionField(key: string, value: unknown): void {
    const body = parsedActionBody ?? { ...activeAction?.bodyTemplate };
    setActionBody(JSON.stringify({ ...body, [key]: value }, null, 2));
  }

  return (
    <section className="module-page" aria-labelledby="module-title">
      <header className="module-header">
        <div>
          <p className="module-kicker">{module.surface === "backend-admin" ? translate("memory.commons.operatorSurface") : translate("memory.commons.userSurface")}</p>
          <h1 id="module-title">{translate(module.titleKey)}</h1>
        </div>
        <button className="icon-button" type="button" title={translate("memory.commons.refresh")} aria-label={translate("memory.commons.refresh")} onClick={() => setRefreshVersion((version) => version + 1)}>
          <RefreshCw size={17} aria-hidden="true" />
        </button>
      </header>

      <div className="resource-tabs" role="tablist" aria-label={translate(module.titleKey)}>
        {module.resources.map((candidate) => (
          <button key={candidate} type="button" role="tab" aria-selected={candidate === resource} className={candidate === resource ? "resource-tab active" : "resource-tab"} onClick={() => setResource(candidate)}>
            {resourceLabel(candidate)}
          </button>
        ))}
      </div>

      <div className="resource-toolbar">
        <label className="field-control search-field">
          <span>{translate("memory.commons.search")}</span>
          <div><Search size={16} aria-hidden="true" /><input value={q} onChange={(event) => setQ(event.target.value)} placeholder={translate("memory.commons.searchPlaceholder")} /></div>
        </label>
        <label className="field-control scope-field">
          <span>{translate("memory.commons.spaceId")}</span>
          {spaceSelectorAvailable ? (
            <select value={spaceId} onChange={(event) => setSpaceId(event.target.value)}>
              <option value="">{translate("memory.commons.spaceSelectPrompt")}</option>
              {spaceOptions.map((option) => <option key={option.spaceId} value={option.spaceId}>{option.displayName}</option>)}
            </select>
          ) : (
            <input value={spaceId} onChange={(event) => setSpaceId(event.target.value)} placeholder={translate("memory.commons.spaceIdPlaceholder")} />
          )}
        </label>
        {declaredFilters.map((filter) => (
          <label className="field-control filter-field" key={filter.param}>
            <span>{filterLabel(filter)}</span>
            {filter.options?.length ? (
              <select value={filterValues[filter.param] ?? ""} onChange={(event) => updateFilterValue(filter.param, event.target.value)}>
                <option value="">{translate("memory.commons.filterAny")}</option>
                {filter.options.map((option) => <option key={option} value={option}>{filterOptionLabel(filter, option)}</option>)}
              </select>
            ) : (
              <input value={filterValues[filter.param] ?? ""} onChange={(event) => updateFilterValue(filter.param, event.target.value)} />
            )}
          </label>
        ))}
        <label className="field-control page-size-field">
          <span>{translate("memory.commons.pageSize")}</span>
          <select value={pageSize} onChange={(event) => setPageSize(Number(event.target.value))}>
            <option value={20}>20</option>
            <option value={50}>50</option>
            <option value={100}>100</option>
          </select>
        </label>
        {dataSource?.actions?.length ? (
          <div className="resource-actions" aria-label={translate("memory.commons.actions")}>
            {dataSource.actions.map((action) => (
              <button className={action.dangerous ? "command-button danger" : "command-button"} disabled={action.requiresSelection && !selectedItem} key={action.id} onClick={() => openAction(action)} type="button">
                <Play size={15} aria-hidden="true" />{actionLabel(action)}
              </button>
            ))}
          </div>
        ) : null}
      </div>

      <div className="resource-stage" aria-live="polite" aria-busy={loading}>
        {!dataSource ? <StatusState icon={<Database size={24} />} message={translate("memory.commons.unavailable")} /> : null}
        {dataSource && loading ? <div className="loading-line"><span className="spinner" />{translate("memory.commons.loading")}</div> : null}
        {dataSource && error ? <StatusState icon={<AlertTriangle size={24} />} message={`${translate("memory.commons.error")} ${error}`} tone="danger" /> : null}
        {dataSource && !loading && !error && page.items.length === 0 ? <StatusState icon={<Database size={24} />} message={translate("memory.commons.empty")} /> : null}
        {dataSource && !loading && !error && page.items.length > 0 ? (
          <div className="table-scroll">
            <table>
              <thead><tr>{columns.map((column) => <th key={column}>{fieldLabel(column)}</th>)}</tr></thead>
              <tbody>{page.items.map((item, rowIndex) => (
                <tr key={readRowKey(item, rowIndex)} tabIndex={0} onClick={() => openDetail(item)} onKeyDown={(event) => { if (event.key === "Enter") openDetail(item); }}>
                  {columns.map((column) => <td key={column}>{formatValue(item[column])}</td>)}
                </tr>
              ))}</tbody>
            </table>
          </div>
        ) : null}
      </div>

      <footer className="pagination-bar">
        <span>{page.pageInfo.totalItems ?? translate("memory.commons.pageRowsLabel").replace("{n}", String(page.items.length))}</span>
        <div>
          <button className="icon-button" type="button" disabled={cursorHistory.length === 0} title={translate("memory.commons.previous")} aria-label={translate("memory.commons.previous")} onClick={openPreviousPage}><ChevronLeft size={17} /></button>
          <button className="icon-button" type="button" disabled={!nextCursor} title={translate("memory.commons.next")} aria-label={translate("memory.commons.next")} onClick={openNextPage}><ChevronRight size={17} /></button>
        </div>
      </footer>

      {selectedItem ? (
        <aside className="detail-drawer" aria-label={translate("memory.commons.details")}>
          <header><h2>{translate("memory.commons.details")}</h2><button className="icon-button" type="button" title={translate("memory.commons.close")} aria-label={translate("memory.commons.close")} onClick={closeDetail}><X size={17} /></button></header>
          {detailLoading ? <p className="detail-status" role="status">{translate("memory.commons.detailLoading")}</p> : null}
          {detailError ? <p className="command-error">{detailError}</p> : null}
          <dl>{Object.entries(selectedItem).map(([key, value]) => <div key={key}><dt>{fieldLabel(key)}</dt><dd>{formatLongValue(value)}</dd></div>)}</dl>
        </aside>
      ) : null}

      {activeAction ? (
        <aside className="command-drawer" aria-label={actionLabel(activeAction)}>
          <header><h2>{actionLabel(activeAction)}</h2><button className="icon-button" type="button" aria-label={translate("memory.commons.close")} title={translate("memory.commons.close")} onClick={closeAction}><X size={17} /></button></header>
          {activeAction.requiresSelection ? <dl className="command-impact"><div><dt>{translate("memory.commons.resource")}</dt><dd>{resourceLabel(resource)}</dd></div><div><dt>{translate("memory.commons.resourceId")}</dt><dd>{formatValue(readSelectedId(selectedItem))}</dd></div><div><dt>{translate("memory.commons.operation")}</dt><dd>{message(`memory.actions.${activeAction.id}`, activeAction.label)}</dd></div>{spaceId.trim() ? <div><dt>{translate("memory.commons.spaceId")}</dt><dd>{spaceId.trim()}</dd></div> : null}</dl> : null}
          <div className="command-fields">
            {Object.entries(activeAction.bodyTemplate).filter(([key]) => key !== activeAction.auditReasonField).map(([key, template]) => <ActionField key={key} label={fieldLabel(key)} template={template} value={parsedActionBody?.[key] ?? template} onChange={(value) => updateActionField(key, value)} />)}
          </div>
          {activeAction.requireAuditReason ? <label className="field-control"><span>{translate("memory.commons.auditReason")}</span><input value={auditReason} onChange={(event) => setAuditReason(event.target.value)} /></label> : null}
          {activeAction.requireIdempotencyKey ? <label className="field-control"><span>{translate("memory.commons.idempotencyKey")}</span><input value={idempotencyKey} onChange={(event) => setIdempotencyKey(event.target.value)} /></label> : null}
          <details className="advanced-json"><summary>{translate("memory.commons.advancedJson")}</summary><label className="field-control"><span>{translate("memory.commons.requestBody")}</span><textarea rows={10} value={actionBody} onChange={(event) => setActionBody(event.target.value)} spellCheck={false} /></label></details>
          {activeAction.dangerous ? <label className="confirm-control"><input type="checkbox" checked={actionConfirmed} onChange={(event) => setActionConfirmed(event.target.checked)} /><span>{translate("memory.commons.confirmAffectedScope")}</span></label> : null}
          {actionError ? <p className="command-error">{actionError}</p> : null}
          <footer><button className={activeAction.dangerous ? "command-button danger" : "command-button"} disabled={actionRunning} onClick={() => void executeAction()} type="button"><Play size={15} />{actionLabel(activeAction)}</button></footer>
        </aside>
      ) : null}
    </section>
  );
}

function StatusState({ icon, message, tone = "neutral" }: { icon: React.ReactNode; message: string; tone?: "danger" | "neutral" }) {
  return <div className={`status-state ${tone}`}>{icon}<p>{message}</p></div>;
}

/**
 * Mirrors `value` into state only after typing has settled for `delayMs`.
 *
 * Search inputs hit the server through the list effect, so the effect must observe
 * the debounced value instead of every keystroke. The pending timer is cleared on
 * every change (and unmount), so only the settled value survives.
 */
function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs, value]);
  return debounced;
}

/** Projects a spaces list row into a selectable option, or null when it has no id. */
function readSpaceOption(item: Record<string, unknown>): MemorySpaceOption | null {
  const value = item.spaceId ?? item.id;
  if (typeof value !== "string" && typeof value !== "number") return null;
  const candidate = item.displayName ?? item.name;
  const displayName = typeof candidate === "string" && candidate.trim() ? candidate : String(value);
  return { spaceId: String(value), displayName };
}

function ActionField({ label, onChange, template, value }: { label: string; onChange(value: unknown): void; template: unknown; value: unknown }) {
  if (typeof template === "boolean") {
    return <label className="confirm-control action-boolean"><input type="checkbox" checked={Boolean(value)} onChange={(event) => onChange(event.target.checked)} /><span>{label}</span></label>;
  }
  if (typeof template === "number") {
    return <label className="field-control"><span>{label}</span><input type="number" value={typeof value === "number" ? value : template} onChange={(event) => onChange(Number(event.target.value))} /></label>;
  }
  if (Array.isArray(template)) {
    const items = Array.isArray(value) ? value : template;
    return <label className="field-control"><span>{label}</span><input value={items.map(String).join(", ")} onChange={(event) => onChange(event.target.value.split(",").map((item) => item.trim()).filter(Boolean))} /></label>;
  }
  if (template && typeof template === "object") {
    return <JsonActionField label={label} value={value} onChange={onChange} />;
  }
  return <label className="field-control"><span>{label}</span><input value={typeof value === "string" ? value : ""} onChange={(event) => onChange(event.target.value)} /></label>;
}

function JsonActionField({ label, onChange, value }: { label: string; onChange(value: unknown): void; value: unknown }) {
  const [text, setText] = useState(() => JSON.stringify(value ?? {}, null, 2));
  const [invalid, setInvalid] = useState(false);
  useEffect(() => setText(JSON.stringify(value ?? {}, null, 2)), [value]);

  function commit(): void {
    try {
      const parsed = JSON.parse(text) as unknown;
      if (!parsed || typeof parsed !== "object") throw new Error();
      setInvalid(false);
      onChange(parsed);
    } catch {
      setInvalid(true);
    }
  }

  return <label className="field-control"><span>{label}</span><textarea aria-invalid={invalid} rows={5} value={text} onBlur={commit} onChange={(event) => setText(event.target.value)} spellCheck={false} /></label>;
}

function parseActionBody(value: string): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed as Record<string, unknown> : null;
  } catch {
    return null;
  }
}

function readSelectedId(item: Record<string, unknown> | null): unknown {
  if (!item) return "-";
  for (const key of ["id", "uuid", "spaceId", "memoryId", "jobId", "candidateId", "habitId", "subjectId", "bindingId", "entityId", "edgeId", "policyId", "policyAssignmentId"]) {
    if (item[key] !== undefined) return item[key];
  }
  return "-";
}

function resolveColumns(items: readonly Record<string, unknown>[]): string[] {
  const preferred = ["id", "uuid", "displayName", "name", "title", "content", "status", "state", "type", "updatedAt", "createdAt"];
  const keys = new Set(items.flatMap((item) => Object.keys(item)));
  return [...preferred.filter((key) => keys.has(key)), ...[...keys].filter((key) => !preferred.includes(key))].slice(0, 7);
}

function readRowKey(item: Record<string, unknown>, index: number): string {
  const value = item.id ?? item.uuid;
  return typeof value === "string" || typeof value === "number" ? String(value) : `row-${index}`;
}

function formatResourceName(value: string): string {
  return value.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replace(/[-_]/g, " ").replace(/^./, (letter) => letter.toUpperCase());
}

function formatValue(value: unknown): string {
  const text = formatLongValue(value);
  return text.length > 96 ? `${text.slice(0, 93)}...` : text;
}

function formatLongValue(value: unknown): string {
  if (value === null || value === undefined) return "-";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value, null, 2);
}

function readSafeError(value: unknown, translate: (key: string) => string): string {
  if (typeof value !== "object" || value === null) return "";
  const error = value as { code?: unknown; traceId?: unknown };
  const code = typeof error.code === "number" ? `${translate("memory.commons.errorCode")} ${error.code}.` : "";
  const trace = typeof error.traceId === "string" ? ` ${translate("memory.commons.traceId")} ${error.traceId}.` : "";
  return `${code}${trace}`.trim();
}
