/**
 * Field-coverage probe: the official JavaScript client with EVERY declared option
 * field set. Companion to `field_coverage.py` and `field_capture_stub.py`.
 *
 * Same contract as the Python driver: one JSON line per call, and the request is
 * read off the capture stub rather than inferred from the source. Two things only
 * hold for this client and are worth knowing while reading the capture:
 *
 *   1. It camel-cases every response key back (`_fetchWithErrorHandling` runs
 *      `snakeToCamelKeys`), but it *sends* snake_case — the capture is of the wire.
 *   2. `search` always adds `output_format: "v1.1"`, and it passes `filters`
 *      through untouched (only `rest` is run through `camelToSnakeKeys`), so the
 *      filter keys are whatever the caller wrote.
 *
 * The stub is started separately:
 *     python field_capture_stub.py 6321 /tmp/capture.jsonl &
 *     MEM0_BASE_URL=http://127.0.0.1:6321 MEM0_API_KEY=probe \
 *       MEM0_E2E_SDK_ENTRY=<abs path to mem0ai/dist/index.mjs> node field_coverage.mjs
 *
 * ESM resolves bare specifiers against this file's directory and `NODE_PATH` does
 * not apply to `import`, so the installed package is addressed by URL — the same
 * reason `mem0_official_js_sdk_flow.rs` resolves rather than assumes a layout.
 */
const BASE = process.env.MEM0_BASE_URL;
const KEY = process.env.MEM0_API_KEY;
const TS = 1700000000;

const SDK_ENTRY = process.env.MEM0_E2E_SDK_ENTRY;
if (!SDK_ENTRY) {
  throw new Error("set MEM0_E2E_SDK_ENTRY to the installed mem0ai ESM entry");
}

const { MemoryClient } = await import(SDK_ENTRY);

const emit = (value) => console.log(JSON.stringify(value));

const attempt = async (label, fn) => {
  try {
    return await fn();
  } catch (error) {
    emit({
      label,
      ok: false,
      exc: error?.constructor?.name,
      code: error?.errorCode,
      msg: String(error?.message).slice(0, 400),
    });
    return null;
  }
};

const client = new MemoryClient({ apiKey: KEY, host: BASE });
// The identity ping is off the request path; give it a moment so the capture log
// reads in call order.
await new Promise((resolve) => setTimeout(resolve, 400));
emit({ label: "client.init()", ok: true });

const added = await attempt("add(full surface)", () =>
  client.add([{ role: "user", content: "probe field coverage" }], {
    userId: "probe-user",
    agentId: "probe-agent",
    appId: "probe-app",
    runId: "probe-run",
    metadata: { probe: "add" },
    infer: true,
    customCategories: [{ probe: "category" }],
    customInstructions: "probe custom instructions",
    agentCustomInstructions: "probe agent instructions",
    timestamp: TS,
    expirationDate: "2030-01-01",
    structuredDataSchema: { type: "object", properties: {} },
  }),
);
if (added !== null) {
  emit({ label: "add(full surface) -> body", ok: true, kind: typeof added });
}

let memoryId = null;
for (const item of Array.isArray(added) ? added : (added?.results ?? [])) {
  if (item?.id) memoryId = item.id;
}

const listed = await attempt("getAll(full surface)", () =>
  client.getAll({
    filters: { userId: "probe-user" },
    page: 2,
    pageSize: 5,
    startDate: "2026-01-01",
    endDate: "2026-12-31",
    categories: ["probe-category"],
    showExpired: true,
    latestOnly: true,
  }),
);
if (listed !== null) {
  emit({ label: "getAll(full surface) -> body", ok: true, kind: typeof listed });
  for (const item of listed?.results ?? []) if (item?.id) memoryId = item.id;
}

memoryId = memoryId ?? "00000000-0000-0000-0000-000000000000";
emit({ label: "resolved memoryId", ok: true, memoryId });

for (const [label, call] of [
  [
    "search(full surface)",
    () =>
      client.search("probe query", {
        filters: { userId: "probe-user" },
        metadata: { probe: "search" },
        topK: 7,
        rerank: true,
        threshold: 0.5,
        fields: ["memory", "id"],
        categories: ["probe-category"],
        showExpired: true,
        referenceDate: "2026-01-01",
        latestOnly: true,
        keywordSearch: true,
        source: "PROBE",
      }),
  ],
  [
    "update(full surface)",
    () =>
      client.update(memoryId, {
        text: "probe updated text",
        metadata: { probe: "update" },
        timestamp: TS,
        expirationDate: "2030-01-01",
      }),
  ],
  ["get(memoryId)", () => client.get(memoryId)],
  ["history(memoryId)", () => client.history(memoryId)],
  ["users(page=2)", () => client.users({ page: 2, pageSize: 5 })],
  [
    "feedback(POSITIVE)",
    () => client.feedback({ memoryId, feedback: "POSITIVE", feedbackReason: "probe reason" }),
  ],
  ["batchUpdate(entry)", () => client.batchUpdate([{ memoryId, text: "probe batch text" }])],
  ["batchDelete(memoryId)", () => client.batchDelete([memoryId])],
  ["deleteAll(userId)", () => client.deleteAll({ userId: "probe-user" })],
  ["delete(deleteLinked=true)", () => client.delete(memoryId, { deleteLinked: true })],
]) {
  const result = await attempt(label, call);
  if (result !== null) {
    emit({ label: `${label} -> body`, ok: true, kind: Array.isArray(result) ? "array" : typeof result });
  }
}

emit({ label: "driver.done", ok: true });
