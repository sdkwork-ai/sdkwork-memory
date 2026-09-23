# SQLite migrations

Add versioned SQL files using `{version}_{name}.up.sql` and a paired `{version}_{name}.down.sql`.

Every numbered migration MUST provide a paired `.down.sql` file (per DATABASE_FRAMEWORK_SPEC §3.4).
The down migration MUST reverse all side effects of the up migration (tables, indexes, columns,
triggers, virtual tables). Layout validation enforces this pairing.
