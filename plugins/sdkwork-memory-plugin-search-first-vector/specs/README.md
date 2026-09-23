# Search-First Vector Plugin Specs

[`component.spec.json`](component.spec.json) is the machine-readable module
contract. The runtime capability and port manifest remains
[`../sdkwork.memory.plugin.json`](../sdkwork.memory.plugin.json).

The canonical SPI design is
`../../../docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md`.
The canonical root specs are linked from `component.spec.json` through
`../../../sdkwork-specs/`.

## Narrowing rules

**Two ports only.** This plugin exports `MemoryRetrieverPort` and
`MemoryIndexPort`. It owns no canonical store, no event log, no candidate
lifecycle, no habit store, and no retrieval trace store. Those remain owned by
`sdkwork-memory-plugin-native-sql`. A consumer that needs them composes both
plugins; it must not expect this plugin to grow them.

**Vector candidates are not truth.** Per the SPI contract, candidates returned
from `search_scoped` are projections. The service must rehydrate every
candidate through `MemoryRecordStorePort` before returning it to a caller or
assembling context. This plugin makes no canonical claim about `subject`,
`predicate`, or `object_text`.

**Scope is explicit.** `search_scoped` reads tenant and space solely from
`SearchMemoryCandidatesQuery::scope`. It never infers scope from ambient state
and never widens a `read_scope`. The tier gate is conservative: a record stored
at the `Owner` tier is visible only to a request carrying `Owner` read scope.

**Limits are bounded at the index boundary.** `limit` is clamped to the
`MAX_MEMORY_RETRIEVAL_CANDIDATES` ceiling and applied inside the scope bucket
after filtering, so truncation can never drop a permitted record in favour of
a forbidden one.

**No fabricated vectors.** A memory id that has never been projected is
refused with a structured error. The plugin will not synthesise a zero vector
for it, because a zero vector has no similarity relationship and would make
scoring silently meaningless.

**No discovered providers.** Credentials and endpoints are never read from the
environment and never stored as literal values. `secretRefs` is empty by
construction: provider access arrives as an injected port whose binding the
composition root resolves through `ai_provider_binding.endpoint_ref` and
`ai_provider_binding.secret_ref`.

**Determinism.** Ranking is ordered by descending cosine similarity and then
by ascending `memory_id`, so equal-scoring candidates have a stable order.
Bounded eviction removes the entry with the smallest `(updated_at, memory_id)`,
which is likewise deterministic.
