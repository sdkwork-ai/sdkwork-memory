# SDKWork Memory Search-First Vector Plugin

This plugin provides the mem0-style search-first memory capability for SDKWork
Memory as a provider-neutral runtime plugin: bounded vector candidate retrieval,
a derived vector index projection, and the language-model-driven additive memory
extraction that mem0 V3 is built around.

The runtime manifest is [`sdkwork.memory.plugin.json`](sdkwork.memory.plugin.json).
The module integration contract is [`specs/component.spec.json`](specs/component.spec.json).

## Public surface

Two SPI ports are exported, and nothing else is claimed:

| Port | Responsibility |
| --- | --- |
| `MemoryRetrieverPort` | Bounded, scope-aware vector candidate search through `search_scoped`. |
| `MemoryIndexPort` | Derived vector projection for a memory id already projected by the composition root. |

| Module | Contents |
| --- | --- |
| `manifest` | The authoritative `MemoryPluginManifest` and executable port builders. |
| `config` | `SearchFirstVectorConfig` and its validated bounds. |
| `error` | `SearchFirstVectorError` and its projection onto `MemorySpiError`. |
| `vector_index` | `ScopedVectorIndex`, `ScopeKey`, sensitivity tiers, expiry gating, cosine ranking. |
| `extraction` | Turns to self-contained memories, through `LanguageModelPort`. |
| `additive` | Content-hash deduplication and link resolution for a write batch. |
| `runtime` | `SearchFirstVectorRuntime`, the composition root implementing both ports. |

## The write path is additive

mem0 V3's write path is **ADD-only**. One language-model call turns a batch of
conversation turns into a batch of self-contained memories; each is then either
committed or dropped as a duplicate, and a memory may carry verified links to the
memories already known. There is no `UPDATE`, `DELETE`, or `NOOP`, because the
reference pipeline has no such step — every history entry it writes is `ADD`, and
the only operation the model is asked for is `ADD`.

`SearchFirstVectorRuntime::plan_write` runs the whole path in one call and returns
an `AdditiveWritePlan`:

| Part | Meaning |
| --- | --- |
| `report.memories` | Memories the parser accepted, each with its `attributed_to`. |
| `report.refused` | Emissions that violated the contract, each with its reason. |
| `report.truncated` | How many accepted emissions the `64`-memory bound dropped. |
| `plan.additions` | Memories to commit, with verified links and the digest they were deduplicated by. |
| `plan.suppressed` | Candidates dropped as duplicates, and which text they duplicated. |
| `plan.unverified_links` | Declared links naming a memory the caller never offered. |

Persistence is deliberately **not** here: `ai_record` owns the records, so the
composition root persists each committed addition and then projects it through
`project_record`. Two divergences from the reference implementation are deliberate
and documented in `additive`'s module docs: the content digest is SHA-256 rather
than MD5, and links are validated instead of discarded.

## What this plugin owns, and what it refuses

The plugin is deliberately **not** a canonical store. Per the repository
integration policy, `ai_record` and `ai_event` remain the source of truth and
every index is derived and rebuildable. This plugin therefore declares
`canonicalStore`, `eventLog`, `candidateLifecycle`, `habitLearning`, `auditLog`,
`outboxLog`, and `retrievalTrace` as `false` and exports no port for them. Those
capabilities stay with the native SQL plugin.

## Provider access is injected, never discovered

mem0's published implementation ships its own OpenAI, Anthropic, Ollama,
Hugging Face, Qdrant, Redis, and Postgres clients, each reading credentials from
process environment variables. That shape is rejected here: it violates
`SOURCE_CONFIG_SPEC` (libraries must not discover environment or OS paths) and
`INTEGRATION_SPEC` (provider versions must be explicit and replaceable).

This plugin consumes providers **only** through injected SPI ports:

- `EmbeddingModelPort` — required for any vector projection or search
- `LanguageModelPort` — required for memory extraction and for planning a write
- `RerankModelPort` — optional, consulted only when bound

`providerKinds` declares that bound surface (`language_model`,
`embedding_model`, `rerank_model`) without this plugin offering the `provider`
role itself. `secretRefs` is empty by construction: the composition root resolves
a binding through `ai_provider_binding.endpoint_ref` and
`ai_provider_binding.secret_ref`, so no credential value ever enters this crate.

## Usage

```rust,no_run
use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_memory_plugin_search_first_vector::{
    RecordSensitivity, SearchFirstVectorConfig, SearchFirstVectorRuntime,
    VectorProjectionCommand,
};
use sdkwork_memory_spi::{
    EmbeddingCommand, EmbeddingModelPort, MemoryRetrieverPort, MemoryScopeContext,
    MemorySensitivityReadScope, MemorySpiResult, SearchMemoryCandidatesQuery,
};

/// A provider supplied by the composition root, not built here.
struct MyEmbeddingProvider;

#[async_trait]
impl EmbeddingModelPort for MyEmbeddingProvider {
    fn provider_code(&self) -> &str {
        "my-reviewed-provider"
    }

    fn dimensions(&self) -> usize {
        1024
    }

    async fn embed(&self, _command: EmbeddingCommand) -> MemorySpiResult<Vec<f32>> {
        unimplemented!("call the reviewed provider binding")
    }
}

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let runtime = SearchFirstVectorRuntime::new(
    SearchFirstVectorConfig::default(),
    Some(Arc::new(MyEmbeddingProvider) as Arc<dyn EmbeddingModelPort>),
    None,
    None,
)?;

let scope = MemoryScopeContext {
    tenant_id: 42,
    space_id: 7,
    organization_id: None,
    user_id: Some(7),
};

// Project a record's content. `MemoryIndexPort::index` cannot do this, because
// that port carries only a memory id.
runtime
    .project_record(
        &scope,
        VectorProjectionCommand {
            memory_id: "m-1".to_string(),
            memory_type: "fact".to_string(),
            subject: Some("user".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: "dark mode".to_string(),
            canonical_text: "the user prefers dark mode".to_string(),
            sensitivity: RecordSensitivity::Public,
        },
    )
    .await?;

let result = runtime
    .search_scoped(SearchMemoryCandidatesQuery {
        scope,
        query: "what theme does the user like".to_string(),
        limit: 10,
        retriever_kinds: vec![],
        memory_types: vec![],
        read_scope: MemorySensitivityReadScope::Public,
    })
    .await?;

// `degraded` must be checked: an absent provider binding is reported here, not
// hidden behind an empty result.
assert!(!result.degraded);
# Ok(())
# }
```

## Configuration

All bounds live in `SearchFirstVectorConfig`; none of them is read from the
environment.

| Field | Default | Meaning |
| --- | --- | --- |
| `max_entries_per_scope` | `4096` | Per-`(tenant, space)` vector ceiling. Overflow evicts the smallest `(updated_at, memory_id)`. |
| `default_search_limit` | `32` | Limit used only by the bounded `retrieve_scoped` compatibility helper. |
| `similarity_floor` | `0.0` | Minimum cosine similarity for a candidate to be returned. |
| `use_rerank_when_bound` | `true` | Whether a bound `RerankModelPort` reorders more than one candidate. |
| `include_expired` | `false` | Whether a search returns memories whose `expiration_date` has passed. |

`SearchFirstVectorConfig::validate` rejects a zero or above-ceiling
`max_entries_per_scope`, an out-of-range `default_search_limit`, and a non-finite
or out-of-range `similarity_floor`. `validate_limit` rejects a per-request limit
of `0` or above `MAX_MEMORY_RETRIEVAL_CANDIDATES` rather than clamping it.

## Degradation is reported, never silent

`search_scoped` never substitutes an unrequested retriever kind and never widens
the caller's limit. When a requested capability cannot be honoured, the result
carries `degraded = true`, the affected `unavailable_retriever_kinds`, and a
`degradation_codes` entry:

| Code | Meaning |
| --- | --- |
| `embedding_provider_unbound` | No `EmbeddingModelPort` is bound, so no vector search is possible. |
| `embedding_provider_failed` | The bound embedding provider refused the query embedding. |
| `rerank_provider_failed` | The bound rerank provider refused; vector order is preserved and no candidate is lost. |
| `no_requested_retriever_kind_is_served` | The request asked only for retriever kinds this plugin does not serve. |

`returnsStaleHits` is `false`: the plugin never returns a stale projection as
though it were fresh.

The previous mem0-derived implementation in this repository did the opposite in
sixteen places — accepting a parameter and then discarding or replacing it while
returning `Ok`. The notable case was `QdrantVectorStore::update`, which wrote
`vec![0.0; dimensions]` when no new embedding was supplied, silently destroying
the stored vector. That behaviour is the direct reason for the fail-closed
projection contract below.

## Invariant catalog

This crate contains no `unsafe`. These invariants are load-bearing and are each
covered by a test:

1. **Scope is the bucket key.** A vector lives in exactly one
   `(tenant_id, space_id)` bucket, so isolation is structural rather than a
   post-hoc filter that a large `limit` could bypass.
2. **Filtering precedes truncation.** Sensitivity, memory-type, and expiry filters
   run inside the bucket before `limit`, so truncation can never prefer a
   forbidden or expired record over a live, permitted one.
3. **The sensitivity gate is conservative.** A record stored at the `Owner` tier
   is visible only to a request carrying `Owner` read scope; no read scope is ever
   widened, and `retrieve_scoped` reads at `Public` because it carries no scope
   of its own.
4. **Nothing is fabricated.** A projection is refused rather than padded with
   zeros. A memory id that has never been projected is refused by
   `MemoryIndexPort::index` instead of being given a synthetic vector.
5. **A scope-free lookup refuses ambiguity.** If one memory id is projected into
   more than one scope, `MemoryIndexPort::index` refuses rather than acting on an
   arbitrary scope.
6. **Ranking is deterministic.** Descending cosine similarity, then ascending
   `memory_id`. Bounded eviction removes the smallest `(updated_at, memory_id)`.
7. **A zero vector scores `0.0`, never `NaN`.** Cosine similarity accumulates in
   `f64` and guards zero norm, so ordering cannot become non-deterministic.
8. **The content hash is derived and consulted.** `content_hash` is computed from
   `canonical_text` at write time and is the key duplicate detection actually
   compares, rather than being computed and ignored.
9. **The extraction envelope is not guessed at.** The answer must be an object
   keyed `memory`, optionally fenced. A bare array, prose, or a payload still
   keyed `facts` is refused rather than reinterpreted.
10. **`attributed_to` is a closed vocabulary.** `user` and `assistant` are the
    only accepted values. An unrecognised one refuses that emission instead of
    being relabelled, because provenance that cannot be read is not the same as
    provenance that was never stated.
11. **A model cannot link to a memory the caller did not offer.**
    `resolve_linked_memory_ids` keeps only links naming an offered id and reports
    the rest, so a link can never point the store at an unoffered memory.
12. **One bad emission does not lose the batch.** A refused emission is reported
    through `report.refused` while every other emission in the same response is
    still planned, so the refusal is loud without being destructive.
13. **An unreadable expiration date is refused, not stored.** `upsert` rejects
    anything outside `YYYY-MM-DD` instead of persisting a value whose expiry
    could never be evaluated.

## Qualification

This plugin's `deploymentQualification.state` is `provider-binding-required`: it
is a real implementation, but production selection requires a reviewed
`EmbeddingModelPort` binding, because vector search cannot be served without one.
With no embedding port bound the plugin fails closed rather than degrading to a
non-semantic hash comparison.

## Verification

```powershell
cargo test -p sdkwork-memory-plugin-search-first-vector
node --test tests/contracts/runtime_plugin_layout_contract_test.mjs
```
