# SQLite migrations

Reserved for post-baseline versioned changes. `baselineStrategy` is
`baseline-plus-migrations`, so a fresh install applies the consolidated baseline at
`ddl/baseline/sqlite/0001_memory_baseline.sql` and then applies the migrations here in
version order. Do not restate the full schema in this directory.

Add versioned SQL files using `{version}_{name}.up.sql`. SQLite has no `ALTER TABLE ... IF NOT
EXISTS` for columns, so a migration that adds a column MUST be guarded by the dialect's own
precondition check rather than relying on re-running to be a no-op.

A paired `.down.sql` is **optional**. Ship one only for a bounded, tested, data-preserving
reversal, and declare `-- reversible: true` with `-- rollback: down-migration`. Irreversible or
lossy changes MUST declare `-- reversible: false` with a `rollback` token of `forward-fix` or
`restore-cutover` and MUST NOT ship a down file.

Authority: `DATABASE_SPEC.md` section 586, `DATABASE_FRAMEWORK_SPEC.md` section 7.1.
