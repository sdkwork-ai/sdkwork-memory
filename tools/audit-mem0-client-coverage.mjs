#!/usr/bin/env node
// SDKWORK-MEM0-CLIENT-COVERAGE-SAMPLER
//
// Recomputes, from the official clients' own source, which of their wire calls
// this repository serves — so the "served / unrouted" split quoted in the
// compatibility review is *recomputed* instead of remembered.
//
// Why a sampler and not a gate: the clients are vendored under `external/mem0/`,
// which is gitignored. A gate has to pass on a fresh checkout, and this check
// cannot — the reference tree may legitimately be absent. So it reports drift
// instead of enforcing, and its exit codes separate "clean" from "could not
// sample at all":
//
//   0  the account matches the recorded expectation
//   1  drift — a client call, a route, or the classification moved
//   2  usage error
//   3  the mem0 client sources are not vendored, so nothing could be sampled
//
// Exit code 3 must never collapse into 0: "nothing was sampled" and "sampled and
// clean" must not look alike.
//
// The recorded expectation below is NOT a copy of the review prose. It is the
// account this sampler produced when it was written, and it exists so that a
// change in either side is *reported* rather than silently absorbed. Refreshing
// it is a deliberate edit, never an automatic rewrite.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const REPO = path.resolve(new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"));

const PY_CLIENT = path.join(REPO, "external", "mem0", "mem0", "client", "main.py");
const TS_CLIENT = path.join(REPO, "external", "mem0", "mem0-ts", "src", "client", "mem0.ts");
const PATHS_RS = path.join(
  REPO,
  "crates",
  "sdkwork-routes-memory-open-api",
  "src",
  "paths.rs",
);
const EVIDENCE_DIR = path.join(REPO, "target", "mem0-client-coverage");

// ---------------------------------------------------------------------------
// Routes this repository serves, read from the true source.
// ---------------------------------------------------------------------------
// Digits are in the class because the refusal constants are named for the
// protocol version (`MEM0_V2_ENTITIES`) and a class of `[A-Z_]` alone stops at
// the `2` — which reads as "no such constant" rather than as a name.
const ROUTE_RE = /pub const (MEM0_[A-Z0-9_]+): &str = "([^"]+)";/g;
const ROUTE_VERBS = {
  MEM0_PING: ["GET"],
  MEM0_ENTITIES: ["GET"],
  MEM0_DELETE_ALL: ["DELETE"],
  MEM0_MEMORY: ["GET", "PUT", "DELETE"],
  MEM0_HISTORY: ["GET"],
  MEM0_ADD: ["POST"],
  MEM0_SEARCH: ["POST"],
  MEM0_LIST: ["POST"],
  MEM0_FEEDBACK: ["POST"],
  MEM0_BATCH: ["PUT", "DELETE"],
  // Refusal-only constants. Named in `paths::MEM0_REFUSED_PATHS`, so a
  // call landing on one of them is `refused`, not `served`: the route exists but
  // answers `501 by name`, which is a third outcome and must not be counted as an
  // implementation. `main` fails if a declared route constant has no entry here,
  // because the fallback (`[]`) would silently reclassify its calls.
  MEM0_V2_ENTITIES: ["DELETE"],
  MEM0_V2_ENTITY_PROFILE: ["GET"],
  MEM0_V2_PROFILE_JOBS: ["POST"],
  MEM0_V2_PROFILE_JOB: ["GET"],
  MEM0_V2_PROFILE_SETTINGS: ["GET", "POST"],
  MEM0_V1_ENTITIES: ["DELETE"],
  MEM0_EXPORTS: ["POST"],
  MEM0_EXPORTS_GET: ["POST"],
  MEM0_SUMMARY: ["POST"],
  MEM0_PROJECT_WEBHOOKS: ["GET", "POST"],
  MEM0_WEBHOOK: ["PUT", "DELETE"],
  MEM0_PROJECT: ["GET", "PATCH"],
};

// The prefixes the surface declares as the mem0 compatibility wire, read from
// `paths.rs` rather than copied. This list decides whether a client-callable path
// is credential-bridged and re-typed to `application/json`, so a copy that drifted
// from the constant would make the framing account below wrong in the direction of
// looking fine.
const PREFIX_RE = /pub const MEM0_PATH_PREFIXES: \[&str; \d+\] = \[([^\]]*)\];/;
function readDeclaredPrefixes() {
  const text = fs.readFileSync(PATHS_RS, "utf8");
  const match = PREFIX_RE.exec(text);
  if (!match) throw new Error("MEM0_PATH_PREFIXES not found in paths.rs");
  const prefixes = [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  if (prefixes.length === 0) throw new Error("MEM0_PATH_PREFIXES is empty");
  return prefixes;
}

/**
 * Whether a client-built path is under a declared mem0 prefix.
 *
 * Anchored at the start on purpose. `/api/v1/webhooks/...` sits beside `/v1/...`
 * and only became part of the compatibility wire when `/api/v1/` was declared as
 * its own prefix — a substring test would have reported the platform-account
 * family as bridged back when it was not, hiding exactly the gap this dimension
 * exists to record.
 */
function isUnderAnyPrefix(rawPath, prefixes) {
  return prefixes.some((prefix) => rawPath.startsWith(prefix));
}

// The paths this surface answers with a named `501` instead of implementing,
// read from `paths.rs` (`MEM0_REFUSED_PATHS`). That constant names the route
// constants rather than repeating their strings, so the names are resolved here
// through the same route table the classifier uses — and a name with no constant
// behind it is an error rather than a silently missing entry.
const REFUSAL_RE = /pub const MEM0_REFUSED_PATHS: \[&str; \d+\] = \[([^\]]*)\];/;
function readRefusedPaths(routes) {
  const text = fs.readFileSync(PATHS_RS, "utf8");
  const match = REFUSAL_RE.exec(text);
  if (!match) throw new Error("MEM0_REFUSED_PATHS not found in paths.rs");
  const names = match[1]
    .split(",")
    .map((token) => token.trim())
    .filter((token) => token.length > 0);
  if (names.length === 0) throw new Error("MEM0_REFUSED_PATHS is empty");
  const paths = new Set();
  for (const name of names) {
    const resolved = routes.get(name);
    if (!resolved) {
      throw new Error(
        `MEM0_REFUSED_PATHS names ${name}, which is not a declared route constant in paths.rs`,
      );
    }
    paths.add(resolved);
  }
  return paths;
}

// ---------------------------------------------------------------------------
// Recorded expectation — produced by this sampler, not quoted from prose.
// ---------------------------------------------------------------------------
// Five buckets for a client *method*, because a method is in exactly one of them
// and collapsing any of them into another would misstate the account:
//
//   served    the call matches a route this repo declares and implements, verb
//             included
//   refused   the call matches a route this repo declares that answers `501` by
//             name — a named boundary, not an implementation
//   unrouted  the path is known and this repo declares nothing on it
//   unparsed  the path is assembled at runtime and cannot be decided statically
//   inert     the method reaches the wire through no call of its own
//
// `served` and `refused` are kept apart on purpose. Both are routes this repo
// declares, so a verb-aware path match alone cannot tell them apart — and a
// route that exists only to refuse is *not* an implementation. Folding the two
// would put the refusals in the "compatibility surface implemented" column,
// which is the one place they must never appear.
//
// `getProfileJob` is the only `unparsed` entry, and it is genuinely undecidable:
// it accepts either a caller-supplied `statusUrl` or a job id and builds the URL
// from whichever it got. (Python's `get_profile_job` stages the same path through
// a local and *is* decidable — it lands on the `/v2/` refusal.)
//
// Python ships two client classes sharing one wire; the async one is asserted to
// mirror the sync one, so only the sync account is recorded. Its `ping` lives
// inside the private `_validate_api_key`, so it appears in the Python *call*
// list but not in the Python *method* list — which is why Python reads 12 served
// where TypeScript reads 13.
//
// `framing` is a second axis over the same calls, not a sixth bucket: it asks
// whether the path is under a declared mem0 prefix, which is what decides whether
// the credential bridge and the problem-document bridge run at all. A path outside
// every declared prefix is answered by the framework's surface classifier *before*
// either bridge sees it, so a mem0 caller gets `401` with
// `application/problem+json` instead of the mem0 `{"detail": ...}` framed as
// `application/json`. Measured against the real clients: the Python driver reads
// that body as a raw problem-document string because
// `mem0/client/utils.py` only unwraps `detail` when the content type starts with
// `application/json` (`startswith`, so `application/problem+json` misses).
//
// `/v2/` and `/api/v1/` have since been declared, and every path the clients
// build under the previously undeclared `/api/v1/` platform-account plane is
// now answered by a named refusal — so nothing is left in this set. The set is
// recorded rather than filtered so the boundary stays visible, and a *new* path
// landing there goes red instead of quietly widening the gap.
const EXPECTED = {
  // Read from `paths.rs`. Pinned by value because the declared prefix set is what
  // decides framing for every call below: a prefix appearing or disappearing here
  // changes what the account means, so it is drift and must be reported.
  prefixes: ["/v1/", "/v2/", "/v3/", "/api/v1/"],
  python: {
    served: [
      "add",
      "batch_delete",
      "batch_update",
      "delete",
      "delete_all",
      "feedback",
      "get",
      "get_all",
      "history",
      "search",
      "update",
      "users",
    ],
    refused: [
      "create_memory_export",
      "create_webhook",
      "delete_users",
      "delete_webhook",
      "generate_profile",
      "get_memory_export",
      "get_profile",
      "get_profile_job",
      "get_profile_settings",
      "get_project",
      "get_summary",
      "get_webhooks",
      "sample_profiles",
      "update_profile_settings",
      "update_project",
      "update_webhook",
    ],
    unrouted: [],
    unparsed: [],
    unbridged: [],
    inert: ["chat", "reset"],
    classes: ["MemoryClient", "AsyncMemoryClient"],
  },
  typescript: {
    served: [
      "add",
      "batchDelete",
      "batchUpdate",
      "delete",
      "deleteAll",
      "feedback",
      "get",
      "getAll",
      "history",
      "ping",
      "search",
      "update",
      "users",
    ],
    refused: [
      "createMemoryExport",
      "createWebhook",
      "deleteUser",
      "deleteUsers",
      "deleteWebhook",
      "generateProfile",
      "getMemoryExport",
      "getProfile",
      "getProfileSettings",
      "getProject",
      "getWebhooks",
      "sampleProfiles",
      "updateProfileSettings",
      "updateProject",
      "updateWebhook",
    ],
    unrouted: [],
    unparsed: ["getProfileJob"],
    unbridged: [],
    inert: ["constructor"],
    classes: ["client"],
  },
};

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------
function readRoutes() {
  const text = fs.readFileSync(PATHS_RS, "utf8");
  const routes = new Map();
  for (const [, name, p] of text.matchAll(ROUTE_RE)) routes.set(name, p);
  return routes;
}

function normalise(raw) {
  let p = raw.split("?")[0];
  p = p.replace(/\$\{[^}]*\}/g, "{id}");
  p = p.replace(/\{[^}]*\}/g, "{id}");
  if (!p.startsWith("/") && /^[a-z]/.test(p)) p = `/${p}`;
  return p;
}

/**
 * Whether a client-built path lands on a declared route pattern.
 *
 * Segment-wise, with a route's `{param}` matching any one client segment —
 * because that is how the router matches, and string equality is *not*
 * equivalent. The Python client hardcodes the entity type in
 * `/v2/entities/user/{id}/profile/`; upstream declares that route as
 * `/v2/entities/{entity_type}/{entity_id}/profile/`, and the two are the same
 * route but never the same string. Comparing strings reported the call as
 * landing nowhere, which read as "this service declares nothing there" while it
 * in fact answers it with a named refusal.
 */
function pathMatchesRoute(routePath, callPath) {
  const route = routePath.split("/");
  const call = callPath.split("/");
  if (route.length !== call.length) return false;
  for (let i = 0; i < route.length; i += 1) {
    const segment = route[i];
    if (segment.startsWith("{") && segment.endsWith("}")) continue;
    if (segment !== call[i]) return false;
  }
  return true;
}

/**
 * Decide whether one client call lands on a route this repo declares.
 *
 * Returns a structure rather than a bare value, because three outcomes have to
 * stay distinguishable: *served* (declared and implemented), *refused* (declared
 * and answered `501` by name), and plain *absent*. "Path matches but the verb is
 * not declared" is a fourth that folds into absent, and it carries a reason
 * because a bare `null` would let a caller count it as served.
 */
function classifyCall(rawPath, verb, routes, refusals = new Set()) {
  const shape = normalise(rawPath);
  for (const [name, routed] of routes) {
    if (!pathMatchesRoute(routed, shape)) continue;
    const declared = ROUTE_VERBS[name] ?? [];
    if (!declared.includes(verb)) {
      return {
        served: false,
        refused: false,
        route: null,
        detail: `${name} declares [${declared.join(", ")}] but the call uses ${verb}`,
      };
    }
    if (refusals.has(routed)) {
      return { served: false, refused: true, route: name, detail: null };
    }
    return { served: true, refused: false, route: name, detail: null };
  }
  return { served: false, refused: false, route: null, detail: null };
}

/** End of a member's body: the start of the next member at the same level. */
function memberEnd(lines, start, isMember) {
  for (let k = start + 1; k < lines.length; k += 1) if (isMember(lines[k])) return k;
  return lines.length;
}

const PY_MEMBER = /^ {4}(?:@|(?:async )?def )/;
const PY_DEF = /^ {4}(?:async )?def ([A-Za-z_][A-Za-z0-9_]*)\s*\(/;
const PY_CLASS = /^class ([A-Za-z_][A-Za-z0-9_]*)/;
const PY_SYNC_CALL = /self\.client\.(get|post|put|delete|patch)\(/g;
const PY_ASYNC_CALL = /self\.async_client\.(get|post|put|delete|patch)\(/g;
const PY_REQUEST = /self\.(?:async_)?client\.request\(\s*"(GET|POST|PUT|DELETE)"/g;

// A call's path is the first string literal after the call opens. Two shapes
// exist and both must be handled: inline literals, and multi-line bodies where
// the literal sits on a following line or is a module constant.
const PY_LITERAL = /f?"([^"]*)"/g;
const PY_CONST = /^([A-Z_][A-Z0-9_]*)\s*=\s*"([^"]*)"\s*$/gm;

function pyConstants() {
  const text = fs.readFileSync(PY_CLIENT, "utf8");
  const constants = new Map();
  for (const [, name, value] of text.matchAll(PY_CONST)) constants.set(name, value);
  return constants;
}

function extractPython(routes, refusals) {
  const lines = fs.readFileSync(PY_CLIENT, "utf8").split("\n");
  const constants = pyConstants();
  const byClass = new Map();
  let klass = "(module)";

  for (let i = 0; i < lines.length; i += 1) {
    const cls = lines[i].match(PY_CLASS);
    if (cls) {
      klass = cls[1];
      continue;
    }
    const def = lines[i].match(PY_DEF);
    if (!def) continue;
    const name = def[1];
    if (name.startsWith("_")) continue;
    const end = memberEnd(lines, i, (l) => PY_MEMBER.test(l));
    const body = lines.slice(i, end).join("\n");

    const calls = [];
    const unparsed = [];
    // Some methods build the path into a local first, then pass the variable:
    //   path = f"{PROFILE_JOBS_PATH}{path}/"
    //   response = self.client.get(path)
    // Resolving those locals is what keeps the account from silently dropping a
    // method whose path is assembled one line earlier.
    const locals = new Map();
    for (const m of body.matchAll(/^[ \t]*([a-z_][A-Za-z0-9_]*)\s*=\s*f?"([^"]*)"[ \t]*$/gm)) {
      locals.set(
        m[1],
        m[2].replace(/\{([A-Z_][A-Z0-9_]*)\}/g, (whole, name) => constants.get(name) ?? whole),
      );
    }
    const collect = (re, wrapper) => {
      for (const m of body.matchAll(re)) {
        const verb = m[1].toUpperCase();
        // Only the call's *first* argument can be the path. Scanning further
        // into the call reaches the JSON payload, where a stray string like
        // `"operation"` would be mistaken for a route — which is how a
        // generate-profile call first read as `POST /operation`.
        const first = body.slice(m.index + m[0].length).replace(/^\s+/, "");
        const asLiteral = first.match(/^f?"([^"]*)"/);
        const asIdent = first.match(/^([A-Za-z_][A-Za-z0-9_]*)/);
        let literal = null;
        if (asLiteral) literal = asLiteral[1];
        else if (asIdent && locals.has(asIdent[1])) literal = locals.get(asIdent[1]);
        else if (asIdent && constants.has(asIdent[1])) literal = constants.get(asIdent[1]);
        if (literal !== null) {
          // `f"{PROFILE_JOBS_PATH}{suffix}/"` written inline rather than staged
          // through a local.
          literal = literal.replace(/\{([A-Z_][A-Z0-9_]*)\}/g, (whole, name) =>
            constants.get(name) ?? whole,
          );
          calls.push({ verb, path: normalise(literal), via: wrapper });
        } else {
          unparsed.push(`L${i + 1} ${klass}.${name}: ${verb} with no resolvable path`);
        }
      }
    };
    collect(PY_SYNC_CALL, "client");
    collect(PY_ASYNC_CALL, "async_client");
    for (const m of body.matchAll(PY_REQUEST)) {
      const tail = body.slice(m.index + m[0].length);
      const lit = tail.match(/^\s*,\s*f?"([^"]*)"/);
      if (lit) calls.push({ verb: m[1], path: normalise(lit[1]), via: "request" });
      else unparsed.push(`L${i + 1} ${klass}.${name}: request(${m[1]}) with no resolvable path`);
    }

    if (!byClass.has(klass)) byClass.set(klass, []);
    byClass.get(klass).push({ method: name, line: i + 1, calls, unparsed });
  }

  const account = {
    served: new Set(),
    refused: new Set(),
    unrouted: new Set(),
    unparsed: [],
    inert: new Set(),
    calls: [],
    mixed: new Map(),
  };
  for (const [owner, members] of byClass) {
    for (const member of members) {
      account.unparsed.push(...member.unparsed);
      // A public method that issues no request of its own: a delegator such as
      // `reset` (which reaches the wire through `delete_users`) or a stub such
      // as `chat`. Recorded by name so that a new one is drift, not silence.
      if (member.calls.length === 0) account.inert.add(member.method);
      const dispositions = new Set();
      for (const call of member.calls) {
        const verdict = classifyCall(call.path, call.verb, routes, refusals);
        const record = { owner, method: member.method, verb: call.verb, path: call.path, route: verdict.route, refused: verdict.refused, note: verdict.detail, line: member.line };
        account.calls.push(record);
        if (verdict.served) dispositions.add("served");
        else if (verdict.refused) dispositions.add("refused");
        else dispositions.add("unrouted");
      }
      recordDispositions(account, member.method, dispositions);
    }
  }
  return { account, classes: [...byClass.keys()] };
}

/**
 * Place one method in exactly one bucket, and keep a method that reached more
 * than one as an explicit finding.
 *
 * A method with no call of its own (a pure delegator) is `inert` and belongs to
 * no bucket. Precedence — served beats refused beats unrouted — only decides
 * which single bucket a mixed method lands in; the mixture itself is *reported*
 * rather than silently resolved, because "the call list disagrees with itself"
 * is exactly the kind of drift a single bucket would hide.
 */
function recordDispositions(account, method, dispositions) {
  if (dispositions.size === 0) return;
  if (dispositions.size > 1) {
    account.mixed.set(method, [...dispositions].sort().join("+"));
  }
  if (dispositions.has("served")) account.served.add(method);
  else if (dispositions.has("refused")) account.refused.add(method);
  else account.unrouted.add(method);
}

const TS_MEMBER =
  /^ {2}(?:@|(?:private |public |protected |readonly |static |abstract )*(?:async )?(?:constructor|get |set )?[A-Za-z_][A-Za-z0-9_]*\s*[(:<=?])/;
// Statement keywords sit at the same indent as members in a few places and
// would otherwise be read as methods named `if` / `for` / `return`.
// Statement keywords that can be followed by `(` and so look like a member
// declaration. Only those belong here: `delete` is a legitimate method name in
// this client, and listing it would drop a served route.
const TS_KEYWORDS = new Set([
  "if",
  "for",
  "while",
  "switch",
  "catch",
  "return",
  "await",
  "typeof",
  "new",
  "super",
  "throw",
  "function",
  "do",
  "else",
  "case",
  "try",
  "finally",
  "break",
  "continue",
]);
const TS_DEF =
  /^ {2}(?:private |public |protected |readonly |static )*(?:async )?([A-Za-z_][A-Za-z0-9_]*)\s*\(/;
const TS_URL = /`\$\{this\.host\}([^`]*)`/g;
const TS_METHOD_AFTER = /method:\s*"([A-Z]+)"/;
const TS_CONST = /^(?:export )?const ([A-Z_][A-Z0-9_]*)\s*=\s*"([^"]*)"/gm;

function tsConstants() {
  const text = fs.readFileSync(TS_CLIENT, "utf8");
  const constants = new Map();
  for (const [, name, value] of text.matchAll(TS_CONST)) constants.set(name, value);
  return constants;
}

/**
 * Reduce a template path to a routed shape.
 *
 * Three interpolation kinds have to be told apart, because treating them alike
 * either drops every route or invents routes that do not exist:
 *
 *   `${encodePathSegment(x)}`  an id segment          -> `{id}`
 *   `${PROFILE_JOBS_PATH}`      the whole path         -> the constant's value
 *   `${query ? "?" + q : ""}`   a query suffix         -> truncated away
 *
 * The last one is why truncation happens *after* the query split: cutting at
 * the first `?` removes the conditional suffix without touching the path.
 */
function resolveTsPath(raw, constants) {
  if (raw.includes("\u0000")) return null;
  let p = raw.split("?")[0];
  const whole = p.match(/^\$\{([A-Z_][A-Z0-9_]*)\}$/);
  if (whole) return constants.has(whole[1]) ? normalise(constants.get(whole[1])) : null;
  p = p.replace(/\$\{([A-Z_][A-Z0-9_]*)\}/g, (token, name) =>
    constants.has(name) ? constants.get(name) : token,
  );
  // Id segments: `${encodePathSegment(x)}`, `${this.projectId}`, `${entity.type}`.
  // A *simple* expression can only be an id; anything carrying an operator or a
  // whitespace is a conditional and is handled by the truncation below.
  p = p.replace(/\$\{([^{}]*)\}/g, (token, expr) => {
    const inner = expr.trim();
    if (/^(?:encodePathSegment|encodeURIComponent)\(.*\)$/.test(inner)) return "{id}";
    if (/^[A-Za-z_][A-Za-z0-9_.]*$/.test(inner)) return "{id}";
    return token;
  });
  if (p.includes("${")) {
    const cut = p.lastIndexOf("/", p.indexOf("${") - 1);
    if (cut <= 0) return null;
    p = p.slice(0, cut + 1);
  }
  if (!p.startsWith("/")) return null;
  return normalise(p);
}

function extractTypeScript(routes, refusals) {
  const lines = fs.readFileSync(TS_CLIENT, "utf8").split("\n");
  const constants = tsConstants();
  const account = {
    served: new Set(),
    refused: new Set(),
    unrouted: new Set(),
    unparsed: [],
    inert: new Set(),
    calls: [],
    mixed: new Map(),
  };
  for (let i = 0; i < lines.length; i += 1) {
    const def = lines[i].match(TS_DEF);
    if (!def) continue;
    const name = def[1];
    if (TS_KEYWORDS.has(name)) continue;
    if (name.startsWith("_")) continue;
    const end = memberEnd(lines, i, (l) => TS_MEMBER.test(l));
    const body = lines.slice(i, end).join("\n");
    const urls = [...body.matchAll(TS_URL)];
    // A public member that reaches no template — a constructor, or a delegator.
    // It issues no request of its own, so it is recorded as inert by name rather
    // than dropped: a new one appearing is a change to the account.
    if (urls.length === 0) {
      account.inert.add(`${name}@L${i + 1}`);
      continue;
    }
    const verbMatch = body.match(TS_METHOD_AFTER);
    const verb = verbMatch ? verbMatch[1] : "GET";
    let matched = 0;
    const dispositions = new Set();
    for (const url of urls) {
      const literal = resolveTsPath(url[1], constants);
      if (literal === null) continue;
      matched += 1;
      const verdict = classifyCall(literal, verb, routes, refusals);
      account.calls.push({ method: name, verb, path: literal, route: verdict.route, refused: verdict.refused, note: verdict.detail, line: i + 1 });
      if (verdict.served) dispositions.add("served");
      else if (verdict.refused) dispositions.add("refused");
      else dispositions.add("unrouted");
    }
    if (matched === 0) {
      account.unparsed.push(`L${i + 1} ${name}: path assembled at runtime`);
    }
    recordDispositions(account, name, dispositions);
  }
  return { account, classes: ["client"] };
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------
function diffSets(actual, expected) {
  const a = new Set(actual);
  const e = new Set(expected);
  return {
    missing: [...e].filter((x) => !a.has(x)).sort(),
    extra: [...a].filter((x) => !e.has(x)).sort(),
  };
}

function compare(label, { account, classes }, expected, prefixes) {
  const findings = [];
  const served = diffSets(account.served, expected.served);
  const refused = diffSets(account.refused, expected.refused);
  const unrouted = diffSets(account.unrouted, expected.unrouted);
  if (served.missing.length) findings.push(`${label}: no longer served: ${served.missing.join(", ")}`);
  if (served.extra.length) findings.push(`${label}: newly served: ${served.extra.join(", ")}`);
  if (refused.missing.length) findings.push(`${label}: no longer refused by name: ${refused.missing.join(", ")}`);
  if (refused.extra.length) findings.push(`${label}: newly refused by name: ${refused.extra.join(", ")}`);
  if (unrouted.missing.length) findings.push(`${label}: no longer unrouted: ${unrouted.missing.join(", ")}`);
  if (unrouted.extra.length) findings.push(`${label}: newly unrouted: ${unrouted.extra.join(", ")}`);

  // `unparsed` is an identity set, not a count: a method moving into or out of
  // it changes what the account can claim, and a count would hide a swap.
  const unparsedNames = account.unparsed.map((line) => line.replace(/^L\d+ (?:[\w.]+\.)?/, "").split(":")[0]);
  const unparsed = diffSets(unparsedNames, expected.unparsed);
  if (unparsed.missing.length) findings.push(`${label}: no longer unparsed: ${unparsed.missing.join(", ")}`);
  if (unparsed.extra.length) findings.push(`${label}: newly unparsed: ${unparsed.extra.join(", ")}`);

  // A method whose calls do not agree on a disposition is reported with the
  // dispositions it reached, because the bucket it lands in is a precedence
  // choice and the disagreement is the thing worth seeing.
  const mixed = [...account.mixed].sort().map(([method, kinds]) => `${method} (${kinds})`);
  if (mixed.length) findings.push(`${label}: classified more than one way: ${mixed.join(", ")}`);

  const inertNames = [...account.inert].map((n) => n.replace(/@L\d+$/, ""));
  const inert = diffSets(inertNames, expected.inert);
  if (inert.missing.length) findings.push(`${label}: no longer inert: ${inert.missing.join(", ")}`);
  if (inert.extra.length) findings.push(`${label}: newly inert: ${inert.extra.join(", ")}`);

  const framed = { bridged: new Set(), refused: new Set(), outside: new Set() };
  for (const call of account.calls) {
    if (!call.path) continue;
    const key = `${call.verb} ${call.path}`;
    if (!isUnderAnyPrefix(call.path, prefixes)) framed.outside.add(key);
    else if (call.refused) framed.refused.add(key);
    else framed.bridged.add(key);
  }
  const framing = diffSets([...framed.outside], expected.unbridged);
  if (framing.missing.length) {
    findings.push(`${label}: no longer outside every declared prefix: ${framing.missing.join(", ")}`);
  }
  if (framing.extra.length) {
    findings.push(`${label}: newly outside every declared prefix: ${framing.extra.join(", ")}`);
  }

  const classDrift = diffSets(classes, expected.classes);
  if (classDrift.missing.length) findings.push(`${label}: client class gone: ${classDrift.missing.join(", ")}`);
  if (classDrift.extra.length) findings.push(`${label}: new client class: ${classDrift.extra.join(", ")}`);

  return {
    findings,
    served: [...account.served].sort(),
    refused: [...account.refused].sort(),
    unrouted: [...account.unrouted].sort(),
    unparsed: unparsedNames.sort(),
    unbridged: [...framed.outside].sort(),
    refusedShapes: [...framed.refused].sort(),
    inert: inertNames.sort(),
  };
}

/** The async client must speak exactly the same wire as the sync one. */
function checkAsyncMirror(calls) {
  const byOwner = new Map();
  for (const c of calls) {
    if (!byOwner.has(c.owner)) byOwner.set(c.owner, new Set());
    byOwner.get(c.owner).add(`${c.method} ${c.verb} ${c.path}`);
  }
  const sync = byOwner.get("MemoryClient") ?? new Set();
  const async = byOwner.get("AsyncMemoryClient") ?? new Set();
  if (sync.size === 0 || async.size === 0) return [];
  const findings = [];
  for (const x of sync) if (!async.has(x)) findings.push(`async client is missing: ${x}`);
  for (const x of async) if (!sync.has(x)) findings.push(`async client adds: ${x}`);
  return findings;
}

// ---------------------------------------------------------------------------
// Self-test
// ---------------------------------------------------------------------------
// The extraction rules are where this sampler can lie: a rule written one
// character too wide silently drops every route, and one written too narrow
// invents routes that do not exist. Both failure modes have already happened
// here, so each rule is pinned with the case that exposed it plus a positive
// control.
function selfTest() {
  const constants = new Map([["PROFILE_JOBS_PATH", "/v2/profiles/jobs/"]]);
  const cases = [
    // A plain path must survive. The first draft rejected anything containing a
    // lowercase letter as "a template", which rejected every route in the repo.
    ["plain route survives", "/v1/ping/", "/v1/ping/"],
    // Id interpolation, in both spellings this client uses.
    [
      "encodePathSegment is an id",
      "/v1/memories/${encodePathSegment(memoryId)}/",
      "/v1/memories/{id}/",
    ],
    ["encodeURIComponent is an id", "/v1/entities/${encodeURIComponent(id)}/", "/v1/entities/{id}/"],
    ["member id is an id", "/api/v1/webhooks/projects/${this.projectId}/", "/api/v1/webhooks/projects/{id}/"],
    ["nested member id is an id", "/v2/entities/${entity.type}/${entity.name}/", "/v2/entities/{id}/{id}/"],
    // A conditional query suffix is truncated, and must not eat the path.
    [
      "conditional suffix is truncated",
      '/v1/memories/${encodePathSegment(memoryId)}/${query ? `?${query}` : ""}',
      "/v1/memories/{id}/",
    ],
    [
      "conditional query keeps the collection path",
      '/v3/memories/${queryParams.length ? `?${queryParams.join("&")}` : ""}',
      "/v3/memories/",
    ],
    ["query before interpolation", "/v1/memories/?${params}", "/v1/memories/"],
    // A whole-path constant resolves; an unknown variable does not.
    ["constant path resolves", "${PROFILE_JOBS_PATH}", "/v2/profiles/jobs/"],
    ["runtime variable is undecidable", "${path}", null],
    // Not a path at all.
    ["telemetry credential key is not a path", "\u0000${this.apiKey}", null],
  ];

  const failures = [];
  for (const [name, input, want] of cases) {
    const got = resolveTsPath(input, constants);
    if (got !== want) failures.push(`${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
  }

  // `delete` is a method in this client. Listing it as a keyword drops a route.
  if (TS_KEYWORDS.has("delete")) {
    failures.push("`delete` must not be treated as a statement keyword");
  }
  // …while the statement keywords that really do appear at member indent are.
  for (const kw of ["if", "for", "return", "catch"]) {
    if (!TS_KEYWORDS.has(kw)) failures.push(`\`${kw}\` must be treated as a statement keyword`);
  }

  // Classification must be verb-aware: the same path with an undeclared verb is
  // not served, and the failure has to say so rather than reporting `null`.
  const routes = readRoutes();
  const verbCases = [
    ["GET /v1/ping/ is served", "/v1/ping/", "GET", true],
    ["DELETE /v1/ping/ is not", "/v1/ping/", "DELETE", false],
    ["PATCH is not declared anywhere", "/v1/memories/{id}/", "PATCH", false],
    ["PUT /v1/memories/{id}/ is served", "/v1/memories/{id}/", "PUT", true],
  ];
  for (const [name, p, verb, wantServed] of verbCases) {
    const got = classifyCall(p, verb, routes).served;
    if (got !== wantServed) failures.push(`${name}: got served=${got}`);
  }

  // Refusal must be a third outcome, not folded into `served`. A path+verb match
  // on a refusal route *is* a declared route, so a classifier that only asked
  // "does a route exist" would file the `/v2/` boundary under "implemented" —
  // the single most misleading reading this axis can produce.
  const refusals = readRefusedPaths(routes);
  const refusalCases = [
    ["a refusal route is refused, not served", "/v2/profiles/jobs/", "POST", false, true],
    ["a refusal route with an undeclared verb is neither", "/v2/profiles/jobs/", "GET", false, false],
    ["the settings pair refuses on read", "/v2/profiles/settings/", "GET", false, true],
    ["the settings pair refuses on write", "/v2/profiles/settings/", "POST", false, true],
    ["the v2 entity delete is refused", "/v2/entities/{id}/{id}/", "DELETE", false, true],
    ["an implemented route is served, not refused", "/v1/ping/", "GET", true, false],
    ["the deprecated v1 entity delete is refused", "/v1/entities/{id}/{id}/", "DELETE", false, true],
    ["the v1 entity collection keeps serving its listing", "/v1/entities/", "GET", true, false],
    ["a webhook read is refused", "/api/v1/webhooks/projects/{id}/", "GET", false, true],
    ["a webhook registration is refused", "/api/v1/webhooks/projects/{id}/", "POST", false, true],
    ["the project pair refuses on read", "/api/v1/orgs/organizations/{id}/projects/{id}/", "GET", false, true],
    ["the project pair refuses on patch", "/api/v1/orgs/organizations/{id}/projects/{id}/", "PATCH", false, true],
  ];
  for (const [name, p, verb, wantServed, wantRefused] of refusalCases) {
    const got = classifyCall(p, verb, routes, refusals);
    if (got.served !== wantServed || got.refused !== wantRefused) {
      failures.push(`${name}: got served=${got.served} refused=${got.refused}`);
    }
  }
  // Structural invariant: every `/v2/` route must be declared a refusal. A `/v2/`
  // route missing from `MEM0_REFUSED_PATHS` would be counted as implemented, and
  // the account would report a capability the surface only refuses.
  let v2Routes = 0;
  for (const [name, p] of routes) {
    if (!p.startsWith("/v2/")) continue;
    v2Routes += 1;
    if (!refusals.has(p)) failures.push(`/v2/ route ${name} is not listed in MEM0_REFUSED_PATHS`);
  }
  if (v2Routes === 0) failures.push("no /v2/ route is declared, so the refusal axis is untested");
  // The refusal set grew beyond `/v2/` (the `/v1/` export/summary/entity shapes
  // and the `/api/v1/` platform-account plane joined it), so it must now be a
  // superset of the `/v2/` routes — every refusal path it names must still be a
  // declared route constant, which `readRefusedPaths` already enforces.
  if (refusals.size < v2Routes) {
    failures.push(`MEM0_REFUSED_PATHS holds ${refusals.size} paths but ${v2Routes} /v2/ routes exist`);
  }

  // Route matching must mirror the router: a `{param}` stands for any one
  // segment, and everything else must be equal. String equality is the wrong
  // model and produced a false "lands nowhere" for the hardcoded entity type.
  const routeCases = [
    ["a declared route matches itself", "/v1/ping/", "/v1/ping/", true],
    [
      "a hardcoded segment matches the parameterised route",
      "/v2/entities/{entity_type}/{entity_id}/profile/",
      "/v2/entities/user/{id}/profile/",
      true,
    ],
    [
      "an extra segment is not the same route",
      "/v2/entities/{entity_type}/{entity_id}/",
      "/v2/entities/user/{id}/profile/",
      false,
    ],
    ["a different static segment is not the same route", "/v3/memories/add/", "/v3/memories/search/", false],
    ["a parameter does not swallow a shorter path", "/v1/memories/{memory_id}/", "/v1/memories/", false],
  ];
  for (const [name, routePath, callPath, want] of routeCases) {
    const got = pathMatchesRoute(routePath, callPath);
    if (got !== want) failures.push(`${name}: got ${got}, want ${want}`);
  }

  // Framing: prefix membership decides whether either mem0 bridge runs, so the
  // test is anchored at the start and must not fold unrelated path spaces in.
  // The prefix list is the one `paths.rs` declares — a fixture here would let the
  // test pass against a set the service does not use.
  const probePrefixes = readDeclaredPrefixes();
  const prefixCases = [
    ["declared /v1/ path is bridged", "/v1/ping/", true],
    ["declared /v3/ path is bridged", "/v3/memories/add/", true],
    ["the platform-account family is now bridged under /api/v1/", "/api/v1/webhooks/projects/{id}/", true],
    ["a longer segment is not the prefix", "/v10/thing/", false],
    ["a sibling api version is not declared", "/api/v2/webhooks/projects/{id}/", false],
  ];
  for (const [name, p, want] of prefixCases) {
    const got = isUnderAnyPrefix(p, probePrefixes);
    if (got !== want) failures.push(`${name}: got ${got}, want ${want}`);
  }
  // Structural invariant: the prefix set that drives both halves of the mem0
  // exemption is pinned by value, because a version appearing or disappearing
  // here silently changes the framing of every call under it.
  for (const required of ["/v1/", "/v2/", "/v3/", "/api/v1/"]) {
    if (!probePrefixes.includes(required)) {
      failures.push(`paths.rs does not declare ${required} as a mem0 prefix`);
    }
  }

  const counted = cases.length + verbCases.length + prefixCases.length + refusalCases.length + routeCases.length;
  if (failures.length === 0) {
    process.stdout.write(
      `self-test: ${counted} cases pass (+5 keyword checks, +2 structural invariants)\n`,
    );
    return 0;
  }
  process.stderr.write(`self-test FAILED (${failures.length}):\n`);
  for (const f of failures) process.stderr.write(`  ${f}\n`);
  return 1;
}

// ---------------------------------------------------------------------------
function main() {
  if (process.argv.includes("--self-test")) return selfTest();

  const missing = [PY_CLIENT, TS_CLIENT, PATHS_RS].filter((p) => !fs.existsSync(p));
  if (missing.length) {
    process.stderr.write(
      `not vendored, nothing sampled (run from a checkout with external/mem0/):\n` +
        missing.map((p) => `  missing ${path.relative(REPO, p)}`).join("\n") +
        "\n",
    );
    return 3;
  }

  const prefixes = readDeclaredPrefixes();
  const routes = readRoutes();
  const refusals = readRefusedPaths(routes);
  const py = extractPython(routes, refusals);
  const ts = extractTypeScript(routes, refusals);

  const results = {
    python: compare("python", py, EXPECTED.python, prefixes),
    typescript: compare("typescript", ts, EXPECTED.typescript, prefixes),
  };
  const mirror = checkAsyncMirror(py.account.calls);

  // The prefix set decides framing for every call below, so a change to it is
  // drift even when no client call moved.
  const prefixDrift = diffSets(prefixes, EXPECTED.prefixes);
  const prefixFindings = [];
  if (prefixDrift.missing.length) {
    prefixFindings.push(`declared prefixes dropped: ${prefixDrift.missing.join(", ")}`);
  }
  if (prefixDrift.extra.length) {
    prefixFindings.push(`declared prefixes added: ${prefixDrift.extra.join(", ")}`);
  }
  // A declared route constant with no verb list would silently reclassify every
  // call that lands on it, because the classifier's fallback is "declares []".
  const uncoveredRoutes = [...routes.keys()].filter((name) => !(name in ROUTE_VERBS)).sort();
  if (uncoveredRoutes.length) {
    prefixFindings.push(`route constants with no declared verbs: ${uncoveredRoutes.join(", ")}`);
  }
  // `/v1/` and `/v3/` carry implementations; `/v2/` carries refusals and nothing
  // else, which `paths::MEM0_REFUSED_PATHS` enumerates. A `/v2/` route missing
  // from that list is the most dangerous shape this account can take: the
  // classifier sees a declared route with a declared verb and files it under
  // `served`, so the surface would report a capability it only refuses.
  const undeclaredRefusals = [...routes.entries()]
    .filter(([, path]) => path.startsWith("/v2/") && !refusals.has(path))
    .map(([name]) => name)
    .sort();
  if (undeclaredRefusals.length) {
    prefixFindings.push(`/v2/ routes not declared as refusals: ${undeclaredRefusals.join(", ")}`);
  }

  const findings = [
    ...results.python.findings,
    ...results.typescript.findings,
    ...prefixFindings,
    ...mirror,
  ];

  const refusedShapes = [
    ...new Set([...results.python.refusedShapes, ...results.typescript.refusedShapes]),
  ].sort();

  process.stdout.write(
    `routes served (from paths.rs): ${[...routes.keys()].length}\n` +
      `python      served=${results.python.served.length} refused=${results.python.refused.length} unrouted=${results.python.unrouted.length}\n` +
      `typescript  served=${results.typescript.served.length} refused=${results.typescript.refused.length} unrouted=${results.typescript.unrouted.length}\n` +
      `declared prefixes (from paths.rs): ${prefixes.join(" ")}\n` +
      `refusal routes (from paths.rs): ${refusals.size}\n` +
      `refused shapes reached by a client: ${refusedShapes.length}\n` +
      `outside every prefix (unbridged framing): ${[...new Set([...results.python.unbridged, ...results.typescript.unbridged])].length}\n`,
  );

  const evidence = {
    sampledAt: new Date().toISOString(),
    prefixes,
    refusalPaths: [...refusals].sort(),
    refusedShapes,
    routes: Object.fromEntries(routes),
    python: {
      classes: py.classes,
      served: results.python.served,
      refused: results.python.refused,
      unrouted: results.python.unrouted,
      unbridged: results.python.unbridged,
      calls: py.account.calls,
    },
    typescript: {
      served: results.typescript.served,
      refused: results.typescript.refused,
      unrouted: results.typescript.unrouted,
      unbridged: results.typescript.unbridged,
      calls: ts.account.calls,
    },
    findings,
  };
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
  const stamp = evidence.sampledAt.replace(/[:.]/g, "-");
  const evidencePath = path.join(EVIDENCE_DIR, `${stamp}.json`);
  fs.writeFileSync(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  process.stdout.write(`evidence: ${path.relative(REPO, evidencePath)}\n`);

  if (findings.length === 0) {
    process.stdout.write("account matches the recorded expectation\n");
    return 0;
  }
  process.stderr.write(`drift detected (${findings.length}):\n`);
  for (const f of findings) process.stderr.write(`  ${f}\n`);
  return 1;
}

process.exit(main());
