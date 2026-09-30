# MEMORY Database Module

Canonical lifecycle assets for `sdkwork-memory` under `DATABASE_FRAMEWORK_SPEC.md`.

- moduleId: `memory`
- serviceCode: `MEMORY`
- tablePrefix: `ai_`
- engines: PostgreSQL (`authoritative-server` role). SQLite is the client-local/test
  plane and, per `DATABASE_FRAMEWORK_SPEC` section 5.2/section 309, its assets live
  outside this authoritative root — they are owned by
  `plugins/sdkwork-memory-plugin-native-sql` and materialized from
  `tests/fixtures/database/sqlite/`.
- SQLite runtime profile: the pool is pinned to one connection (the `sqlx::Any`
  pool cannot re-apply per-connection PRAGMAs on recycled connections, so the
  store pins one), `foreign_keys = ON`, `busy_timeout = 5s`, and the default
  rollback journal is kept (no WAL). Requests therefore serialize through one
  writer by design, and a long keyset sweep blocks the local plane for its
  duration; the local plane is a single-process convenience surface, and the
  server-role engine admission rule that rejects SQLite for server roles is the
  load-bearing guard. Concurrent multi-process access to one SQLite file is not
  a supported deployment shape.

## Migration Authority

PostgreSQL is the authoritative lifecycle. Schema changes are authored only in the
application-root directories:

1. **Initialization state**: `database/ddl/baseline/postgres/0001_memory_baseline.sql`
   is the full DDL snapshot and the authority for greenfield deployments.
2. `database/migrations/postgres/` holds the post-baseline incremental deltas
   (`0001_organization_id_not_null`, `0002_claim_order_and_keyset_indexes`,
   `0003_hard_delete_cleanup_indexes`,
   `0004_candidate_job_linkage`); paired up/down files there are the
   incremental authority. Under the committed `baselineStrategy:
   baseline-plus-migrations`, the checked-in baseline stays the authoritative
   DDL source and is never regenerated from these deltas: greenfield
   deployments apply the baseline and then replay the deltas, and a delta is
   only absorbed into the baseline by a reviewed, deliberate baseline edit
   (the folded blocks carry `-- source:` markers). `pnpm
   db:materialize:baseline` validates this state and re-folds the SQLite
   fixture projection.
3. The plugin's embedded compatibility runner
   (`plugins/sdkwork-memory-plugin-native-sql`, version keys in `store.rs`) is a
   test/local-tool bootstrap only. It embeds the baseline plus every
   post-baseline PostgreSQL delta (0001-0005) — including
   `0004_candidate_job_linkage`, which adds `ai_candidate.learning_job_uuid`
   and `ai_eval_run.version`, and `0005_usage_daily`, which adds the
   `ai_usage_daily` usage-fact table (mirrored by SQLite fixture migration
   0018) — and a structural test asserts the embedded set
   matches the application-root delta directory, so a locally bootstrapped
   PostgreSQL carries the same schema the application-root lifecycle applies.
   Production never runs it: the store connects
   with `apply_migration=false` and the application-root lifecycle
   (`sdkwork-memory-database-host`) applies the baseline. Its private bookkeeping
   table `ops_memory_schema_version` is not part of the application schema;
   production never depends on it, and an externally initialized SQLite pool
   that lacks the quota-serialization row is rejected at startup with a named
   diagnostic instead of failing at first space create.

`pnpm verify` checks that the baseline and the contract are current.

## Physical Profile

- Every business-table `id` is an application-generated SDKWork Snowflake `BIGINT`;
  neither engine allocates row IDs.
- JSON and UTC instant logical values use validated `TEXT` in the native SQL
  profile (`compliance_level: L2`, registered in `contract/table-registry.json`).
  JSON is validated at application boundaries; on PostgreSQL, metadata filters
  cast `TEXT` to `jsonb` per row at query time — a high-frequency filter key must
  therefore be promoted to a real column through a reviewed schema change rather
  than left inside `*_json`. `DATABASE_SPEC` section 8.2 requires native
  `JSONB`/`TIMESTAMPTZ` for new L2+ tables; this profile is a registered
  cross-engine exemption for the memory domain until such a promotion lands.
- `TEXT` instants sort correctly because every writer emits the same fixed-width
  UTC format (`%Y-%m-%dT%H:%M:%S%.3fZ`); any writer that emits a different format
  breaks time comparisons silently.
- Unique constraints that must tolerate `NULL` pairs use PostgreSQL 15+
  `NULLS NOT DISTINCT`; the deployment minimum PostgreSQL version is 15, and the
  data-plane bootstrap asserts `server_version_num >= 150000` at startup. SQLite
  mirrors the same uniqueness with a `-1` sentinel for "any user" preference rows
  (see `preference_scope_user_binding` in the native SQL store).
- Continuous confidence, ranking, and weight values use PostgreSQL
  `DOUBLE PRECISION` and SQLite `REAL`. They are not monetary or exact-decimal
  values.
- Outbox deliveries, learning jobs, and eval runs persist owner/token/expiry
  leases with an `attempt_count` bound; completion, acknowledgement, and failure
  marking are all fenced by the current unexpired token, and stale rows past the
  attempt ceiling land in a terminal `dead` state. Learning jobs carry
  `next_attempt_at` for attempt-aware requeue backoff; eval runs mirror
  `ai_learning_job.error_json` for dead-letter reasons and carry an optimistic
  `ai_eval_run.version` (PostgreSQL delta 0004, mirrored by SQLite fixture
  migration 0017) that every state transition increments.
- `ai_candidate.learning_job_uuid` links extracted candidates to their
  originating learning job, so an extraction retry cannot duplicate candidates:
  a requeued run inserts only candidates whose job linkage is new, and a run
  that creates zero candidates completes as success.
- A scheduled retention worker hard-deletes terminal and derived rows in
  bounded batches: terminal outbox events, terminal learning/eval jobs,
  retrieval traces with their hits and context packs, and audit logs — each on
  its own configurable window (`SDKWORK_MEMORY_RETENTION_*`, 0 disables a
  sweep). Canonical records and events are never touched by mechanical
  retention; they follow the space retention job and forget workflows.
- Cross-dialect schema parity (tables, columns, constraints, indexes) is
  machine-checked by `tools/check-database-dialect-parity.mjs` with a declared
  divergence allowlist, and the SQLite folded baseline under
  `tests/fixtures/database/sqlite/ddl/baseline/` is regenerated from the
  fixture migrations by `pnpm db:materialize:baseline` so it cannot go stale.
- Runtime pods keep auto-migration disabled. Production applies the baseline
  through the release migration job before rolling out runtime Pods.

## Initialization state

This module is in **initialization state** for greenfield deployments:

1. **Baseline** — `database/ddl/baseline/postgres/0001_memory_baseline.sql` contains
   the full DDL snapshot.
2. **Migrations** — `database/migrations/postgres/` holds the post-baseline
   incremental deltas (paired up/down files); under `baselineStrategy:
   baseline-plus-migrations` the checked-in baseline stays the authoritative
   DDL source and greenfield deployments apply the baseline and then replay
   the deltas.
3. **Drift** — run `pnpm db:drift:check` before release.

## Commands

```bash
pnpm run db:validate
pnpm run db:materialize:contract
pnpm run db:plan
pnpm run db:init
pnpm run db:migrate
pnpm run db:seed
pnpm run db:status
pnpm run db:drift:check
```
