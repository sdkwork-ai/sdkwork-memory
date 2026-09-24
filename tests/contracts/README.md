# Contract Tests

Node test-runner (`node --test`) contracts that keep materialized artifacts,
route manifests, and the Rust contract crate in lockstep. Run from the
repository root; every check listed here is also wired into repository
verification (`pnpm verify` / CI).

## Tests

| Test | Guards |
| --- | --- |
| `route_manifest_openapi_parity_test.mjs` | Route manifests and generated Rust `http_route_manifest.rs` files mirror the three surface OpenAPI authorities byte-for-byte (paths, methods, operationIds, auth metadata). |
| `openapi_query_param_parity_test.mjs` | For every GET `*.list` operation on the three surfaces, the declared query parameter set equals the deserializable field set of the `sdkwork-memory-contract` query DTO bound to the handler. Bidirectional: a declared parameter the DTO rejects (`deny_unknown_fields` -> 400) fails, and an implemented DTO filter missing from OpenAPI fails. The `operationId -> DTO` map is an explicit enumeration; shared-DTO fields the service provably rejects or ignores are listed in `UNDECLARED_DTO_FIELDS` with the service behavior cited. |
| `open_api_prefix_contract_test.mjs` | The open surface keeps the `/mem/v3/api` prefix, the SDK manifest declares no unimplemented discovery `schemaUrl`, and no markdown reintroduces the legacy `/memory/v3/...` identifiers. |
| `openapi_phase1_contract_test.mjs` | Phase 1 OpenAPI materialization invariants (ownership metadata, pagination defaults, envelope shapes). |
| `memory_database_migration_parity_test.mjs`, `native_sql_migration_contract_test.mjs`, `schema_registry_phase1_contract_test.mjs` | Database contract, migration, and schema-registry parity. |
| `runtime_plugin_layout_contract_test.mjs`, `rust_spi_boundary_contract_test.mjs`, `spi_design_contract_test.mjs` | Runtime plugin layout and SPI boundary contracts. |

## Regeneration flow

The authority for the three OpenAPI surfaces is `apis/<surface>/memory-<surface>-api.openapi.json`,
produced by `node tools/materialize_phase1_contracts.mjs` (the `sdks/*/openapi/*.openapi.json`
copies are byte-identical generation-input mirrors). After changing the tool or
the contract DTOs, run:

```bash
node tools/materialize_phase1_contracts.mjs
pnpm sdk:generate
node tests/contracts/route_manifest_openapi_parity_test.mjs
node tests/contracts/openapi_query_param_parity_test.mjs
pnpm check:sdk-standard
```
