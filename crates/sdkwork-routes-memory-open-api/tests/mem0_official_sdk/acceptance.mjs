#!/usr/bin/env node
/**
 * The official JavaScript `mem0ai` client driving the mem0 platform wire.
 *
 * Companion to `acceptance.py`: same environment contract, same observation
 * shape, so `tests/mem0_official_sdk_support/mod.rs` can hold **one** set of
 * expectations for both clients. Narrative goes to stderr; the machine-readable
 * result is a single `MEM0_E2E_RESULT `-prefixed JSON line on stdout.
 *
 * Four differences from the Python client are load-bearing, and all four were
 * measured against the installed package rather than assumed:
 *
 *   1. It camel-cases every response key (`_fetchWithErrorHandling` runs
 *      `snakeToCamelKeys`), so this driver maps them back to snake_case. One
 *      observation contract, not two.
 *   2. Its errors expose `errorCode` and keep the **raw response body** as
 *      `message`, where the Python client hands over the parsed `detail`. The
 *      type and the `HTTP_<status>` code agree exactly — only the presentation
 *      differs. That is why the shared asserter requires *containment* of this
 *      surface's wording while the Python test pins the exact string.
 *   3. `batchUpdate` maps each entry to `{memory_id, text}` and drops every other
 *      field, so it cannot patch `metadata` at all. That assertion lives in the
 *      Python test, because a shared one would test something this client has no
 *      way to send.
 *   4. It has no client-side guard on the feedback enum, so an invalid value
 *      reaches the wire and the *server* rejects it (400) — the opposite of the
 *      Python client, which raises locally and never opens a socket. Both
 *      behaviours are asserted, in their own client's test.
 */

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const RESULT_PREFIX = "MEM0_E2E_RESULT ";

function requireEnv(name) {
  const value = process.env[name];
  if (value === undefined || value === "") {
    throw new Error(`the harness must set ${name}`);
  }
  return value;
}

const BASE_URL = requireEnv("MEM0_BASE_URL");
const API_KEY = requireEnv("MEM0_API_KEY");
const EXPECTED_TENANT = requireEnv("MEM0_EXPECT_TENANT");
const CONVERSATION = requireEnv("MEM0_E2E_CONVERSATION");
const UPDATED_TEXT = requireEnv("MEM0_E2E_UPDATED_TEXT");
const BATCH_FIRST = requireEnv("MEM0_E2E_BATCH_FIRST");
const BATCH_SECOND = requireEnv("MEM0_E2E_BATCH_SECOND");
const BATCH_FIRST_UPDATED = requireEnv("MEM0_E2E_BATCH_FIRST_UPDATED");
const BATCH_SECOND_UPDATED = requireEnv("MEM0_E2E_BATCH_SECOND_UPDATED");
const UNKNOWN_ID = requireEnv("MEM0_E2E_UNKNOWN_ID");
const SDK_ENTRY = requireEnv("MEM0_E2E_SDK_ENTRY");

const USER_ID = "alice";
const AGENT_ID = "planner";
const SIBLING_USER = "bob";

const failures = [];

// Counted where the probe is made, never derived from one list's length: the
// named lists are grouped by *why* a call cannot be honoured, so each is a
// partial view and a summary built from one would under-report the run.
let refusalsTotal = 0;
let rejectionsTotal = 0;

function note(message) {
  process.stderr.write(`${message}\n`);
}

function check(name, condition, detail = "") {
  if (condition) {
    note(`  ok   ${name}`);
  } else {
    const rendered = typeof detail === "string" ? detail : JSON.stringify(detail);
    note(`  FAIL ${name} ${rendered}`);
    failures.push(`${name} ${rendered}`.trim());
  }
}

/** The shape a caller sees: type, machine code, human message. */
function describe(error) {
  return {
    type: error?.constructor?.name ?? typeof error,
    error_code: error?.errorCode ?? error?.error_code ?? null,
    message: typeof error?.message === "string" ? error.message : String(error),
  };
}

function expectRefusal(label, call) {
  return call().then(
    (value) => {
      note(`  FAIL ${label} was accepted: ${JSON.stringify(value)}`);
      failures.push(`${label} was accepted instead of refused`);
      return { call: label, accepted: value };
    },
    (error) => {
      refusalsTotal += 1;
      const observed = describe(error);
      note(`  refused ${label}: ${observed.type} ${observed.error_code}`);
      check(`${label} carries a reason`, Boolean(observed.message), observed);
      return { call: label, error: observed };
    },
  );
}

function expectError(label, call) {
  return call().then(
    (value) => {
      note(`  FAIL ${label} was accepted: ${JSON.stringify(value)}`);
      failures.push(`${label} was accepted: ${JSON.stringify(value)}`);
      return { call: label, accepted: value };
    },
    (error) => {
      rejectionsTotal += 1;
      const observed = describe(error);
      note(`  rejected ${label}: ${observed.type} ${observed.error_code}`);
      check(`${label} carries a reason`, Boolean(observed.message), observed);
      return { call: label, error: observed };
    },
  );
}

/**
 * `GET /v1/ping/` read directly, bypassing the client.
 *
 * This client is the reason the field matters — its `ping()` throws unless
 * `status` is exactly `"ok"` — but it never exposes the raw body, so the literal
 * is captured here and asserted by the shared asserter for both clients.
 */
async function rawPing() {
  try {
    const response = await fetch(`${BASE_URL}/v1/ping/`, {
      headers: { Authorization: `Token ${API_KEY}` },
    });
    return await response.json();
  } catch (error) {
    return { error: String(error) };
  }
}

async function main() {
  // Lowercase, and that is not a typo: this SDK tests `=== "false"` where the
  // Python one accepts `"False"`. Getting it wrong only costs a network call to
  // a third party, which is exactly what a hermetic acceptance must not make.
  process.env.MEM0_TELEMETRY = "false";

  const sdk = await import(pathToFileURL(SDK_ENTRY).href);
  const packageRoot = path.resolve(path.dirname(SDK_ENTRY), "..");
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(packageRoot, "package.json"), "utf8"),
  );

  note(`official SDK: mem0ai ${packageJson.version} from ${SDK_ENTRY}`);

  const observed = {};

  // The constructor performs `_resolveIdentity()` and `_validateApiKey()`; if it
  // returns, the credential bridge and the media type already agree with it.
  const client = new sdk.MemoryClient({ apiKey: API_KEY, host: BASE_URL });
  await client.initialized.catch((error) => {
    note(`identity resolution did not complete: ${error?.message ?? error}`);
  });

  const ping = await rawPing();
  observed.ping = {
    org_id: ping.org_id ?? null,
    project_id: ping.project_id ?? null,
    user_email: ping.user_email ?? null,
    status: ping.status ?? null,
  };
  note(`ping: ${JSON.stringify(observed.ping)}`);
  check(
    "ping carries the status literal this client requires",
    observed.ping.status === "ok",
    ping,
  );

  // `ping()` itself is the client's own acceptance test of that response: it
  // throws `APIError("API Key is invalid")` unless `status === "ok"`.
  await client.ping();
  check("ping() accepts the response", true);

  // The field's effect is observable in the client's own state, which is what
  // makes this a compatibility requirement rather than a cosmetic one: a
  // rejected `ping()` leaves the identity unresolved — the constructor only
  // logs it — and every identity-dependent call then fails.
  check(
    "the client resolved its identity from the ping body",
    client.organizationId != null && client.projectId != null,
    { organizationId: client.organizationId, projectId: client.projectId },
  );

  // --- write -----------------------------------------------------------------
  const added = await client.add(
    [
      { role: "user", content: CONVERSATION },
      { role: "assistant", content: "" },
    ],
    { userId: USER_ID, agentId: AGENT_ID, metadata: { topic: "formatting" } },
  );
  const created = added.results[0];
  const memoryId = created.id;
  observed.add = {
    id: created.id,
    event: created.event,
    memory: created.memory,
    user_id: created.userId,
    agent_id: created.agentId,
    hash: created.hash,
    metadata: created.metadata,
  };
  note(`add -> ${observed.add.id}`);
  check("add returns an id", Boolean(created.id), observed.add);
  check("add reports event ADD", created.event === "ADD", observed.add);
  check("add returns the memory text", created.memory === CONVERSATION, observed.add);
  check("add echoes the user scope", created.userId === USER_ID, observed.add);
  check("add keeps caller metadata", created.metadata?.topic === "formatting", observed.add);

  // --- read back -------------------------------------------------------------
  const fetched = await client.get(memoryId);
  observed.get = { id: fetched.id, memory: fetched.memory };
  check("get returns the same id", fetched.id === memoryId, observed.get);

  const found = await client.search("bullet-point summaries", {
    filters: { user_id: USER_ID },
    topK: 5,
  });
  const hits = found.results ?? [];
  observed.search = {
    count: hits.length,
    ids: hits.map((hit) => hit.id),
    first: hits[0] ?? null,
  };
  note(`search -> ${hits.length} hit(s)`);
  check("search finds the stored memory", observed.search.ids.includes(memoryId), observed.search.ids);

  const updated = await client.update(memoryId, { text: UPDATED_TEXT });
  observed.update = { id: updated.id, memory: updated.memory };
  check("update rewrites the text", updated.memory === UPDATED_TEXT, observed.update.memory);

  // --- feedback --------------------------------------------------------------
  // The canonical record accepts a comment but never reads it back, so
  // `feedback_reason` must come home null rather than echoed.
  const feedback = await client.feedback({
    memoryId,
    feedback: "POSITIVE",
    feedbackReason: "matched what the operator asked for",
  });
  observed.feedback = {
    id: feedback.id,
    feedback: feedback.feedback,
    feedback_reason: feedback.feedbackReason,
  };
  note(`feedback -> ${JSON.stringify(observed.feedback)}`);
  check("feedback reports an id", Boolean(feedback.id), observed.feedback);
  check("feedback reports the stored value", feedback.feedback === "POSITIVE", observed.feedback);

  // A withdrawal. Upstream models an omitted `feedback` as clearing it, but the
  // canonical record is only ever written and never cleared, so there is no
  // value to answer with; a 2xx would report a mutation that did not happen.
  observed.feedback_withdrawal = await expectRefusal(
    "feedback(no value, i.e. withdrawal)",
    () => client.feedback({ memoryId }),
  );

  // Unlike the Python client, this one has **no** client-side enum guard, so an
  // invalid value reaches the wire. Recorded as a rejection: the server's own
  // closed-enum check is what must answer, and here it is exercised through a
  // real client rather than a hand-built request.
  observed.feedback_enum_rejection = await expectError(
    "feedback(feedback=MAYBE)",
    () => client.feedback({ memoryId, feedback: "MAYBE" }),
  );

  // --- history ---------------------------------------------------------------
  const history = await client.history(memoryId);
  observed.history = {
    events: history.map((entry) => entry.event),
    entries: history.length,
  };
  note(`history -> ${JSON.stringify(observed.history.events)}`);
  check("history records the add", observed.history.events.includes("ADD"), observed.history.events);
  check("history records the update", observed.history.events.includes("UPDATE"), observed.history.events);

  // --- entity scopes ---------------------------------------------------------
  const users = await client.users();
  observed.users = {
    count: users.count,
    scopes: (users.results ?? []).map((scope) => ({
      type: scope.type,
      name: scope.name,
      owner: scope.owner,
    })),
  };
  note(`users -> ${observed.users.scopes.map((s) => `${s.type}:${s.name}`).join(", ")}`);
  check(
    "users lists the addressed user scope",
    observed.users.scopes.some(
      (scope) => scope.type === "user" && scope.name === USER_ID && scope.owner === EXPECTED_TENANT,
    ),
    observed.users.scopes,
  );

  // --- delete, then prove it took -------------------------------------------
  const deleted = await client.delete(memoryId);
  observed.delete = { message: deleted.message };
  check("delete is acknowledged", Boolean(deleted.message), observed.delete);

  try {
    await client.get(memoryId);
    observed.get_after_delete = { accepted: true };
    failures.push("a deleted memory was still readable");
  } catch (error) {
    observed.get_after_delete = { error: describe(error) };
    check(
      "deleted memory raises MemoryNotFoundError",
      error?.constructor?.name === "MemoryNotFoundError",
      observed.get_after_delete,
    );
  }

  // --- batch update ----------------------------------------------------------
  const batchFirst = (
    await client.add([{ role: "user", content: BATCH_FIRST }], { userId: USER_ID })
  ).results[0].id;
  const batchSecond = (
    await client.add([{ role: "user", content: BATCH_SECOND }], { userId: USER_ID })
  ).results[0].id;

  const acked = await client.batchUpdate([
    { memoryId: batchFirst, text: BATCH_FIRST_UPDATED },
    { memoryId: batchSecond, text: BATCH_SECOND_UPDATED },
  ]);
  const readFirst = await client.get(batchFirst);
  const readSecond = await client.get(batchSecond);
  observed.batch_update = {
    message: acked.message,
    first: readFirst.memory,
    second: readSecond.memory,
  };
  note(`batch_update -> ${observed.batch_update.message}`);
  check(
    "batch_update counts both entries",
    acked.message === "Successfully updated 2 memories",
    observed.batch_update,
  );
  check("batch_update rewrote the first text", readFirst.memory === BATCH_FIRST_UPDATED, observed.batch_update);
  check("batch_update rewrote the second text", readSecond.memory === BATCH_SECOND_UPDATED, observed.batch_update);

  observed.rejections = [
    // An id that names nothing. The batch answers with that entry's own error -
    // 404 naming the id exactly as the caller sent it - while the resolvable
    // entry beside it is applied: each entry commits atomically with its own
    // journal, and the failure message says how many did.
    await expectError(
      "batch_update(unknown memory_id)",
      () =>
        client.batchUpdate([
          { memoryId: batchFirst, text: "must not be written" },
          { memoryId: UNKNOWN_ID, text: "names nothing" },
        ]),
    ),
    // Neither field, so there is nothing to apply.
    await expectError(
      "batch_update(entry with no text or metadata)",
      () => client.batchUpdate([{ memoryId: batchFirst }]),
    ),
    // Upstream declares `maxItems: 1000` on the request array itself.
    await expectError(
      "batch_update(1001 entries)",
      () =>
        client.batchUpdate(
          Array.from({ length: 1001 }, () => ({ memoryId: batchFirst, text: "overflow" })),
        ),
    ),
  ];

  const survived = await client.get(batchFirst);
  check(
    "an aborted batch applied its resolvable entries",
    survived.memory === "must not be written",
    survived.memory,
  );

  // --- batch delete ----------------------------------------------------------
  const deleteAck = await client.batchDelete([batchFirst, batchSecond]);
  observed.batch_delete = { message: deleteAck.message };
  note(`batch_delete -> ${observed.batch_delete.message}`);
  check(
    "batch_delete counts both entries",
    deleteAck.message === "Successfully deleted 2 memories",
    observed.batch_delete,
  );

  observed.batch_delete_proof = [];
  for (const id of [batchFirst, batchSecond]) {
    // Through `expectError`, not a bare catch, so the post-delete read-backs are
    // counted with every other classified probe. Recording them in their own
    // block without counting them is exactly how a summary ends up under-reporting
    // the run — the totals must describe the work, not one list's length.
    observed.batch_delete_proof.push(
      await expectError(`get(${id}, deleted by batch)`, () => client.get(id)),
    );
  }

  // --- refusals: parameters and filters this surface cannot honour -----------
  note("refusals:");
  // A second, unrelated memory so the per-memory refusals have a target that is
  // not one already deleted above.
  const sibling = (
    await client.add([{ role: "user", content: CONVERSATION }], { userId: SIBLING_USER })
  ).results[0].id;

  observed.refusals = [
    await expectRefusal("get_all(filters=user_id)", () =>
      client.getAll({ filters: { user_id: USER_ID } }),
    ),
    await expectRefusal("get_all(page=2)", () => client.getAll({ page: 2 })),
    await expectRefusal("delete_all(user_id=...)", () => client.deleteAll({ userId: USER_ID })),
    await expectRefusal("update(timestamp=...)", () =>
      client.update(sibling, { timestamp: "2026-01-01T00:00:00Z" }),
    ),
    await expectRefusal("delete(delete_linked=True)", () =>
      client.delete(sibling, { deleteLinked: true }),
    ),
  ];

  // --- bulk sweep ------------------------------------------------------------
  const swept = await client.deleteAll();
  observed.delete_all = { message: swept.message };
  check("delete_all is acknowledged", Boolean(swept.message), swept);

  const empty = await client.getAll();
  observed.list_after_sweep = {
    count: empty.count,
    results: (empty.results ?? []).length,
    next: empty.next ?? null,
    previous: empty.previous ?? null,
  };
  check("the sweep really emptied the space", empty.count === 0, observed.list_after_sweep);
  check("an unfiltered listing is answered", observed.list_after_sweep.results === 0);

  const result = {
    sdk: {
      package: "mem0ai",
      version: packageJson.version,
      module: SDK_ENTRY,
      base_url: BASE_URL,
    },
    observed,
    refusals_total: refusalsTotal,
    rejections_total: rejectionsTotal,
    failures,
    ok: failures.length === 0,
  };
  process.stdout.write(`${RESULT_PREFIX}${JSON.stringify(result)}\n`);
  note(
    `official mem0ai ${packageJson.version} (JavaScript) drove the platform wire end to end ` +
      `(${refusalsTotal} refusal(s), ${rejectionsTotal} rejection(s) classified)`,
  );
  return failures.length === 0 ? 0 : 1;
}

try {
  process.exit(await main());
} catch (error) {
  process.stderr.write(`${error?.stack ?? error}\n`);
  process.stdout.write(
    `${RESULT_PREFIX}${JSON.stringify({
      sdk: null,
      observed: {},
      refusals_total: refusalsTotal,
      rejections_total: rejectionsTotal,
      failures: ["the driver itself crashed"],
      ok: false,
    })}\n`,
  );
  process.exit(2);
}
