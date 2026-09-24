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

## Migration Authority

PostgreSQL is the authoritative lifecycle. Schema changes are authored only in the
application-root directories:

1. **Initialization state**: `database/ddl/baseline/postgres/0001_memory_baseline.sql`
   is the full DDL snapshot and the authority for greenfield deployments.
2. `database/migrations/postgres/` is reserved for post-GA incremental schema
   changes and is intentionally empty until the first post-GA migration; once a
   migration exists, paired up/down files there are the incremental authority and
   `pnpm db:materialize:baseline` folds them into the baseline.
3. The plugin's embedded compatibility runner
   (`plugins/sdkwork-memory-plugin-native-sql`, version keys in `store.rs`) is a
   test/local-tool bootstrap only. Production never runs it: the store connects
   with `apply_migration=false` and the application-root lifecycle
   (`sdkwork-memory-database-host`) applies the baseline. Its private bookkeeping
   table `ops_memory_schema_version` is not part of the application schema and
   nothing in production depends on it.

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
  `NULLS NOT DISTINCT`; the deployment minimum PostgreSQL version is 15. SQLite
  mirrors the same uniqueness with a `-1` sentinel for "any user" preference rows
  (see `preference_scope_user_binding` in the native SQL store).
- Continuous confidence, ranking, and weight values use PostgreSQL
  `DOUBLE PRECISION` and SQLite `REAL`. They are not monetary or exact-decimal
  values.
- Outbox deliveries, learning jobs, and eval runs persist owner/token/expiry
  leases with an `attempt_count` bound; completion and acknowledgement are fenced
  by the current unexpired token, and stale rows past the attempt ceiling land in
  a terminal `dead` state.
- Runtime pods keep auto-migration disabled. Production applies the baseline
  through the release migration job before rolling out runtime Pods.

## Initialization state

This module is in **initialization state** for greenfield deployments:

1. **Baseline** — `database/ddl/baseline/postgres/0001_memory_baseline.sql` contains
   the full DDL snapshot.
2. **Migrations** — `database/migrations/postgres/` is reserved for post-GA
   incremental schema changes only. It is intentionally empty at initialization
   apart from data-shape migrations that ran before the baseline was folded.
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
