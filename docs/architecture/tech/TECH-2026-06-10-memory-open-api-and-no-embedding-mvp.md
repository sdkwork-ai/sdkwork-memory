> Status: archived implementation plan. It is not current capability or release authority; use `TECH_ARCHITECTURE.md`.
>
> Migrated from `docs/superpowers/plans/2026-06-10-memory-open-api-and-no-embedding-mvp.md` on 2026-06-24.
> Owner: SDKWork maintainers

## Implementation Status (2026-07-06)

**Complete.** The no-embedding MVP described in this plan is implemented and verified (`cargo test --workspace`, `pnpm verify`).

**Current architecture and production guidance:**

- `docs/architecture/tech/TECH_ARCHITECTURE.md`
- `docs/architecture/tech/TECH-2026-06-10-commercial-memory-management-design.md` §2

The task checklist below is retained as an historical implementation record only. Do not treat unchecked boxes as open work.

---

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first runnable SDKWork Memory backend that supports the generated Open API, App API, and Backend API contracts while proving memory retrieval works without embeddings.

**Architecture:** Keep `ai_record` and `ai_event` as canonical source-of-truth tables. Build a Rust service with explicit contract, product, storage, retrieval, and route boundaries; all search, vector, graph, file, or external-provider indexes remain pluggable derived capabilities behind ports. Open API clients use API key context, App/Backend API clients use dual-token context, and all three surfaces call the same product services.

**Tech Stack:** Rust workspace, Axum-compatible route crates, SQLx, PostgreSQL, SQLite, OpenAPI 3.1.2, generated TypeScript SDK families under `sdks/`, PowerShell/Node contract verification.

---

## Source Documents

- Design spec: `docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md`
- Generator: `tools/materialize_phase1_contracts.mjs`
- Phase 1 verifier: `tools/verify_phase1.ps1`
- Open API authority: `sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json`
- App API authority: `sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json`
- Backend API authority: `sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json`
- Schema registry: `docs/schema-registry/tables/*.yaml`

## File Structure

Create or modify these implementation boundaries:

- Modify: `tools/materialize_phase1_contracts.mjs`
- Modify: `tools/verify_phase1.ps1` only through `node tools/materialize_phase1_contracts.mjs`
- Create: `Cargo.toml`
- Create: `crates/sdkwork-memory-contract/Cargo.toml`
- Create: `crates/sdkwork-memory-contract/src/lib.rs`
- Create: `crates/sdkwork-memory-contract/src/ids.rs`
- Create: `crates/sdkwork-memory-contract/src/dto.rs`
- Create: `crates/sdkwork-memory-contract/src/ports.rs`
- Create: `crates/sdkwork-memory-retrieval/Cargo.toml`
- Create: `crates/sdkwork-memory-retrieval/src/lib.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/mod.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/sql.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/keyword.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/dictionary.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/time.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/event.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/fusion.rs`
- Create: `crates/sdkwork-memory-retrieval/src/context_pack.rs`
- Create: `crates/sdkwork-memory-retrieval/src/learning.rs`
- Create: `crates/sdkwork-memory-test-support/Cargo.toml`
- Create: `crates/sdkwork-memory-test-support/src/lib.rs`
- Create: `crates/sdkwork-intelligence-memory-service/Cargo.toml`
- Create: `crates/sdkwork-intelligence-memory-service/src/lib.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/memory_service.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/retrieval_service.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/context_pack_service.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/learning_service.rs`
- Create: `crates/sdkwork-intelligence-memory-repository-sqlx/Cargo.toml`
- Create: `crates/sdkwork-intelligence-memory-repository-sqlx/src/lib.rs`
- Create: `crates/sdkwork-intelligence-memory-repository-sqlx/src/repositories.rs`
- Create: `database/migrations/postgres/0001_memory_phase1.up.sql`
- Create: `tests/fixtures/database/sqlite/migrations/0001_memory_phase1.up.sql`
- Create: `crates/sdkwork-routes-memory-open-api/Cargo.toml`
- Create: `crates/sdkwork-routes-memory-open-api/src/lib.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/paths.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/routes.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/manifest.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/handlers.rs`
- Create: `crates/sdkwork-routes-memory-app-api/Cargo.toml`
- Create: `crates/sdkwork-routes-memory-app-api/src/lib.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/paths.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/routes.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/manifest.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/handlers.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/Cargo.toml`
- Create: `crates/sdkwork-routes-memory-backend-api/src/lib.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/paths.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/routes.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/manifest.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/handlers.rs`
- Create: `tests/contracts/openapi_phase1_contract_test.mjs`
- Create: `tests/contracts/schema_registry_phase1_contract_test.mjs`
- Create: `tests/no_embedding/memory_no_embedding_mvp_test.rs`
- Create: `tests/api/open_api_smoke_test.rs`
- Create: `tests/api/app_backend_api_smoke_test.rs`

## Invariants

- `ai_record` and `ai_event` are canonical; all indexes are derived and rebuildable.
- Embedding is optional. Phase 1 must pass without an embedding provider, vector table, vector database, or vector retriever.
- Retrieval profile defaults to SQL, keyword, dictionary, time, and event retrievers.
- Open API uses `ApiKey` / `X-API-Key`; it must not accept app login token fallback.
- App API and Backend API use dual-token context.
- Backend-only controls stay out of Open API.
- Generated SDK output under `generated/server-openapi` is never hand-edited.

### Task 1: Contract Guard For Open API

**Files:**
- Modify: `tools/materialize_phase1_contracts.mjs`
- Generated: `tools/verify_phase1.ps1`
- Test: `tools/verify_phase1.ps1`

- [ ] **Step 1: Write the failing guard**

Run:

```powershell
$path = 'tools\materialize_phase1_contracts.mjs'
if (-not (Select-String -Path $path -Pattern '"Open API Contract Draft"' -SimpleMatch -Quiet)) {
  Write-Error 'RED: generator design snippet list is missing "Open API Contract Draft"'
  exit 1
}
```

Expected: FAIL until the generator requires `Open API Contract Draft`.

- [ ] **Step 2: Add the minimal generator check**

Add `"Open API Contract Draft"` to the design snippet list in `writeVerification()`:

```js
foreach ($snippet in @(
    "Embedding Optional",
    "Multi-Implementation Abstraction",
    "Open API Contract Draft",
    "App API Contract Draft",
    "Backend API Contract Draft",
    "Database And Storage Design",
    "ai_"
)) {
```

- [ ] **Step 3: Verify the guard passes**

Run the same PowerShell guard.

Expected: PASS with a message that the generator requires `Open API Contract Draft`.

- [ ] **Step 4: Regenerate and run full phase 1 verification**

Run:

```powershell
node --check tools\materialize_phase1_contracts.mjs
node tools\materialize_phase1_contracts.mjs
powershell -ExecutionPolicy Bypass -File tools\verify_phase1.ps1
```

Expected: all three OpenAPI surfaces verify, including 17 Open API operations.

### Task 2: Rust Workspace Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: crate and service `Cargo.toml` files listed in File Structure
- Test: `cargo metadata --format-version 1`

- [ ] **Step 1: Write the failing workspace check**

Run:

```powershell
cargo metadata --format-version 1
```

Expected: FAIL with missing `Cargo.toml`.

- [ ] **Step 2: Add root workspace manifest**

Create `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
  "crates/sdkwork-memory-contract",
  "crates/sdkwork-memory-retrieval",
  "crates/sdkwork-memory-test-support",
  "crates/sdkwork-intelligence-memory-service",
  "crates/sdkwork-intelligence-memory-repository-sqlx",
  "crates/sdkwork-routes-memory-open-api",
  "crates/sdkwork-routes-memory-app-api",
  "crates/sdkwork-routes-memory-backend-api",
]

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "UNLICENSED"

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "macros", "postgres", "sqlite", "chrono", "json"] }
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
uuid = { version = "1", features = ["serde", "v7"] }
```

- [ ] **Step 3: Add minimal crate manifests**

Each crate starts with only dependencies it uses. Example `crates/sdkwork-memory-contract/Cargo.toml`:

```toml
[package]
name = "sdkwork-memory-contract"
edition.workspace = true
version.workspace = true
license.workspace = true

[dependencies]
async-trait.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
```

- [ ] **Step 4: Verify workspace metadata**

Run:

```powershell
cargo metadata --format-version 1
```

Expected: PASS and lists every member.

### Task 3: Contract DTOs And Ports

**Files:**
- Create: `crates/sdkwork-memory-contract/src/lib.rs`
- Create: `crates/sdkwork-memory-contract/src/ids.rs`
- Create: `crates/sdkwork-memory-contract/src/dto.rs`
- Create: `crates/sdkwork-memory-contract/src/ports.rs`
- Test: `crates/sdkwork-memory-contract/tests/contract_surface.rs`

- [ ] **Step 1: Write failing tests for contract surface**

Create `crates/sdkwork-memory-contract/tests/contract_surface.rs`:

```rust
use sdkwork_memory_contract::{
    MemoryEventRequest, MemoryImplementationCapabilities, MemoryRecordRequest,
    MemoryRetrieveRequest, RetrieverKind,
};

#[test]
fn phase1_retrievers_do_not_require_embedding() {
    let request = MemoryRetrieveRequest {
        query: "preferred editor".to_string(),
        retrievers: vec![
            RetrieverKind::Sql,
            RetrieverKind::Keyword,
            RetrieverKind::Dictionary,
            RetrieverKind::Time,
            RetrieverKind::Event,
        ],
        embedding_required: false,
        limit: 10,
    };

    assert!(!request.embedding_required);
    assert!(!request.retrievers.contains(&RetrieverKind::Vector));
}

#[test]
fn implementation_capabilities_mark_vector_as_optional() {
    let capabilities = MemoryImplementationCapabilities::native_sql_phase1();
    assert!(capabilities.record_crud);
    assert!(capabilities.event_log);
    assert!(capabilities.vector_optional);
    assert!(!capabilities.vector_required);
}

#[test]
fn create_requests_are_serializable_contract_dtos() {
    let event = MemoryEventRequest::new_text("evt-1", "User prefers concise answers");
    let record = MemoryRecordRequest::new_fact("rec-1", "answer_style", "concise");
    serde_json::to_value(event).unwrap();
    serde_json::to_value(record).unwrap();
}
```

Run:

```powershell
cargo test -p sdkwork-memory-contract --test contract_surface
```

Expected: FAIL because the crate and DTOs do not exist.

- [ ] **Step 2: Add minimal DTOs and enums**

Create `src/lib.rs`:

```rust
pub mod dto;
pub mod ids;
pub mod ports;

pub use dto::*;
pub use ids::*;
pub use ports::*;
```

Create `src/dto.rs` with only Phase 1 fields:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrieverKind {
    Sql,
    Keyword,
    Dictionary,
    Time,
    Event,
    Vector,
    Graph,
    FileGrep,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRetrieveRequest {
    pub query: String,
    pub retrievers: Vec<RetrieverKind>,
    pub embedding_required: bool,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryImplementationCapabilities {
    pub event_log: bool,
    pub record_crud: bool,
    pub candidate_lifecycle: bool,
    pub habit_learning: bool,
    pub context_assembly: bool,
    pub vector_optional: bool,
    pub vector_required: bool,
    pub graph_optional: bool,
    pub file_grep_optional: bool,
}

impl MemoryImplementationCapabilities {
    pub fn native_sql_phase1() -> Self {
        Self {
            event_log: true,
            record_crud: true,
            candidate_lifecycle: true,
            habit_learning: true,
            context_assembly: true,
            vector_optional: true,
            vector_required: false,
            graph_optional: true,
            file_grep_optional: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEventRequest {
    pub event_id: String,
    pub content_text: String,
}

impl MemoryEventRequest {
    pub fn new_text(event_id: impl Into<String>, content_text: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            content_text: content_text.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecordRequest {
    pub memory_id: String,
    pub subject: String,
    pub object_text: String,
}

impl MemoryRecordRequest {
    pub fn new_fact(
        memory_id: impl Into<String>,
        subject: impl Into<String>,
        object_text: impl Into<String>,
    ) -> Self {
        Self {
            memory_id: memory_id.into(),
            subject: subject.into(),
            object_text: object_text.into(),
        }
    }
}
```

- [ ] **Step 3: Add repository and provider ports**

Create `src/ports.rs`:

```rust
use async_trait::async_trait;

use crate::{MemoryEventRequest, MemoryRecordRequest, MemoryRetrieveRequest};

#[async_trait]
pub trait MemoryEventStore: Send + Sync {
    async fn append(&self, request: MemoryEventRequest) -> anyhow::Result<String>;
    async fn retrieve(&self, event_id: &str) -> anyhow::Result<Option<MemoryEventRequest>>;
}

#[async_trait]
pub trait MemoryRecordStore: Send + Sync {
    async fn create(&self, request: MemoryRecordRequest) -> anyhow::Result<String>;
    async fn retrieve(&self, memory_id: &str) -> anyhow::Result<Option<MemoryRecordRequest>>;
}

#[async_trait]
pub trait MemoryRetriever: Send + Sync {
    async fn retrieve(&self, request: &MemoryRetrieveRequest) -> anyhow::Result<Vec<String>>;
}

pub trait EmbeddingModel: Send + Sync {}
pub trait LanguageModel: Send + Sync {}
pub trait RerankModel: Send + Sync {}
```

- [ ] **Step 4: Verify contract crate**

Run:

```powershell
cargo test -p sdkwork-memory-contract --test contract_surface
```

Expected: PASS.

### Task 4: Phase 1 SQL Migrations

**Files:**
- Create: `database/migrations/postgres/0001_memory_phase1.up.sql`
- Create: `tests/fixtures/database/sqlite/migrations/0001_memory_phase1.up.sql`
- Test: `tests/contracts/schema_registry_phase1_contract_test.mjs`

- [ ] **Step 1: Write failing schema contract test**

Create `tests/contracts/schema_registry_phase1_contract_test.mjs`:

```js
import assert from "node:assert/strict";
import fs from "node:fs";

const requiredTables = [
  "ai_space",
  "ai_event",
  "ai_record",
  "ai_record_source",
  "ai_candidate",
  "ai_habit",
  "ai_retrieval_trace",
  "ai_retrieval_hit",
  "ai_context_pack",
];

for (const file of [
  "database/migrations/postgres/0001_memory_phase1.up.sql",
  "tests/fixtures/database/sqlite/migrations/0001_memory_phase1.up.sql",
]) {
  const sql = fs.readFileSync(file, "utf8").toLowerCase();
  for (const table of requiredTables) {
    assert.match(sql, new RegExp(`create\\s+table\\s+${table}\\b`), `${file} missing ${table}`);
  }
  assert.doesNotMatch(sql, /vector|embedding\(/, `${file} must not require vector or embedding storage in Phase 1`);
}
```

Run:

```powershell
node tests\contracts\schema_registry_phase1_contract_test.mjs
```

Expected: FAIL because migration files do not exist.

- [ ] **Step 2: Add minimal PostgreSQL migration**

Create the required tables from the schema registry. Start with the core fields required by the product code:

```sql
CREATE TABLE ai_space (
  id BIGINT PRIMARY KEY,
  uuid VARCHAR(64) NOT NULL,
  tenant_id BIGINT NOT NULL,
  owner_subject_type VARCHAR(64) NOT NULL,
  owner_subject_id VARCHAR(128) NOT NULL,
  space_type VARCHAR(64) NOT NULL,
  display_name VARCHAR(200) NOT NULL,
  lifecycle_status VARCHAR(32) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  version BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE ai_event (
  id BIGINT PRIMARY KEY,
  uuid VARCHAR(64) NOT NULL,
  tenant_id BIGINT NOT NULL,
  space_id BIGINT NOT NULL REFERENCES ai_space(id),
  event_type VARCHAR(64) NOT NULL,
  event_time TIMESTAMPTZ NOT NULL,
  content_text_redacted TEXT,
  structured_payload JSONB,
  sensitivity VARCHAR(64) NOT NULL,
  lifecycle_status VARCHAR(32) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  version BIGINT NOT NULL DEFAULT 0
);

CREATE TABLE ai_record (
  id BIGINT PRIMARY KEY,
  uuid VARCHAR(64) NOT NULL,
  tenant_id BIGINT NOT NULL,
  space_id BIGINT NOT NULL REFERENCES ai_space(id),
  memory_type VARCHAR(64) NOT NULL,
  subject VARCHAR(256),
  predicate VARCHAR(128),
  object_text TEXT,
  content TEXT NOT NULL,
  keywords JSONB,
  aliases JSONB,
  tags JSONB,
  confidence DOUBLE PRECISION NOT NULL,
  importance DOUBLE PRECISION NOT NULL,
  sensitivity VARCHAR(64) NOT NULL,
  lifecycle_status VARCHAR(32) NOT NULL,
  created_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL,
  deleted_at TIMESTAMPTZ,
  version BIGINT NOT NULL DEFAULT 0
);
```

Add the remaining required Phase 1 tables in the same migration: `ai_record_source`, `ai_candidate`, `ai_habit`, `ai_retrieval_trace`, `ai_retrieval_hit`, and `ai_context_pack`.

- [ ] **Step 3: Add minimal SQLite migration**

Mirror the same logical schema with SQLite-compatible types:

```sql
CREATE TABLE ai_space (
  id INTEGER PRIMARY KEY,
  uuid TEXT NOT NULL,
  tenant_id INTEGER NOT NULL,
  owner_subject_type TEXT NOT NULL,
  owner_subject_id TEXT NOT NULL,
  space_type TEXT NOT NULL,
  display_name TEXT NOT NULL,
  lifecycle_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 0
);
```

Add the same required table set as PostgreSQL.

- [ ] **Step 4: Verify migration contract**

Run:

```powershell
node tests\contracts\schema_registry_phase1_contract_test.mjs
powershell -ExecutionPolicy Bypass -File tools\verify_phase1.ps1
```

Expected: PASS.

### Task 5: SQLx Repository Implementation

**Files:**
- Create: `crates/sdkwork-intelligence-memory-repository-sqlx/src/lib.rs`
- Create: `crates/sdkwork-intelligence-memory-repository-sqlx/src/repositories.rs`
- Test: `crates/sdkwork-intelligence-memory-repository-sqlx/tests/repository_sqlite_test.rs`

- [ ] **Step 1: Write failing repository test**

Create `repository_sqlite_test.rs`:

```rust
use sdkwork_memory_contract::{MemoryEventRequest, MemoryEventStore, MemoryRecordRequest, MemoryRecordStore};
use sdkwork_memory_storage_sqlx::SqlxMemoryRepository;

#[tokio::test]
async fn stores_events_and_records_without_embedding_tables() {
    let repo = SqlxMemoryRepository::new_in_memory_sqlite().await.unwrap();

    let event_id = repo
        .append(MemoryEventRequest::new_text("evt-1", "User prefers concise answers"))
        .await
        .unwrap();
    let record_id = repo
        .create(MemoryRecordRequest::new_fact("rec-1", "answer_style", "concise"))
        .await
        .unwrap();

    assert_eq!(event_id, "evt-1");
    assert_eq!(record_id, "rec-1");
    assert!(repo.retrieve("evt-1").await.unwrap().is_some());
    assert!(MemoryRecordStore::retrieve(&repo, "rec-1").await.unwrap().is_some());
}
```

Run:

```powershell
cargo test -p sdkwork-memory-storage-sqlx --test repository_sqlite_test
```

Expected: FAIL because the repository does not exist.

- [ ] **Step 2: Implement minimal SQLite-backed repository**

Implement `SqlxMemoryRepository` with `new_in_memory_sqlite()`, migration application, and the `MemoryEventStore` / `MemoryRecordStore` traits.

- [ ] **Step 3: Verify repository test**

Run:

```powershell
cargo test -p sdkwork-memory-storage-sqlx --test repository_sqlite_test
```

Expected: PASS.

### Task 6: Deterministic No-Embedding Retrievers

**Files:**
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/sql.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/keyword.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/dictionary.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/time.rs`
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/event.rs`
- Test: `crates/sdkwork-memory-retrieval/tests/no_embedding_retrievers_test.rs`

- [ ] **Step 1: Write failing retriever tests**

Create tests proving each retriever can return candidates without embeddings:

```rust
use sdkwork_memory_contract::{MemoryRetrieveRequest, RetrieverKind};
use sdkwork_memory_retrieval::retrieval::{DictionaryRetriever, KeywordRetriever, TimeRetriever};

#[tokio::test]
async fn keyword_dictionary_and_time_retrievers_run_without_embedding_provider() {
    let request = MemoryRetrieveRequest {
        query: "concise answer style".to_string(),
        retrievers: vec![RetrieverKind::Keyword, RetrieverKind::Dictionary, RetrieverKind::Time],
        embedding_required: false,
        limit: 5,
    };

    assert!(!KeywordRetriever::default().retrieve(&request).await.unwrap().is_empty());
    assert!(!DictionaryRetriever::default().retrieve(&request).await.unwrap().is_empty());
    assert!(!TimeRetriever::default().retrieve(&request).await.unwrap().is_empty());
}
```

Run:

```powershell
cargo test -p sdkwork-memory-retrieval --test no_embedding_retrievers_test
```

Expected: FAIL because retrievers do not exist.

- [ ] **Step 2: Implement minimal retrievers**

Each retriever returns scored `MemoryHit` values from supplied records or repository query results. Keep scoring deterministic:

```rust
pub struct MemoryHit {
    pub memory_id: String,
    pub score: f32,
    pub retriever: RetrieverKind,
    pub reason: String,
}
```

Initial scoring:

- SQL: exact subject/predicate/status filters.
- Keyword: token overlap with `content`, `keywords`, and `tags`.
- Dictionary: alias and canonical term match.
- Time: recency and validity window.
- Event: event source and session proximity.

- [ ] **Step 3: Verify retrievers**

Run:

```powershell
cargo test -p sdkwork-memory-retrieval --test no_embedding_retrievers_test
```

Expected: PASS.

### Task 7: Retrieval Orchestrator And Fusion

**Files:**
- Create: `crates/sdkwork-memory-retrieval/src/retrieval/fusion.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/retrieval_service.rs`
- Test: `crates/sdkwork-intelligence-memory-service/tests/retrieval_service_test.rs`

- [ ] **Step 1: Write failing fusion test**

Create a test proving multiple retriever outputs are merged, deduped, and sorted:

```rust
use sdkwork_memory_product::RetrievalService;

#[tokio::test]
async fn fuses_hits_from_multiple_no_embedding_retrievers() {
    let service = RetrievalService::with_phase1_fake_retrievers();
    let result = service.retrieve("concise answer style").await.unwrap();

    assert!(!result.hits.is_empty());
    assert!(result.hits.windows(2).all(|pair| pair[0].score >= pair[1].score));
    assert!(result.retrievers_used.contains(&"keyword".to_string()));
    assert!(!result.retrievers_used.contains(&"vector".to_string()));
}
```

Run:

```powershell
cargo test -p sdkwork-memory-product --test retrieval_service_test
```

Expected: FAIL because the service does not exist.

- [ ] **Step 2: Implement reciprocal-rank style fusion**

Keep the first implementation simple:

```rust
pub fn fuse_hits(mut hits: Vec<MemoryHit>, limit: usize) -> Vec<MemoryHit> {
    hits.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.memory_id.cmp(&b.memory_id)));
    hits.dedup_by(|a, b| a.memory_id == b.memory_id);
    hits.truncate(limit);
    hits
}
```

- [ ] **Step 3: Verify service test**

Run:

```powershell
cargo test -p sdkwork-memory-product --test retrieval_service_test
```

Expected: PASS.

### Task 8: Context Pack Assembler

**Files:**
- Create: `crates/sdkwork-memory-retrieval/src/context_pack.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/context_pack_service.rs`
- Test: `crates/sdkwork-intelligence-memory-service/tests/context_pack_service_test.rs`

- [ ] **Step 1: Write failing context pack test**

Create:

```rust
use sdkwork_memory_product::ContextPackService;

#[test]
fn context_pack_contains_traceable_selected_memories() {
    let service = ContextPackService::default();
    let pack = service.assemble_for_test(vec!["rec-1".into(), "rec-2".into()]);

    assert_eq!(pack.memory_ids, vec!["rec-1", "rec-2"]);
    assert!(!pack.context_text.is_empty());
    assert!(pack.debug.retrievers_used.len() > 0);
}
```

Run:

```powershell
cargo test -p sdkwork-memory-product --test context_pack_service_test
```

Expected: FAIL.

- [ ] **Step 2: Implement minimal assembler**

Assembler responsibilities:

- stable order
- max item count
- redacted content only
- source memory ids
- retrieval trace id when available
- debug metadata gated by caller permission

- [ ] **Step 3: Verify**

Run:

```powershell
cargo test -p sdkwork-memory-product --test context_pack_service_test
```

Expected: PASS.

### Task 9: Candidate Learning And Habit Memory

**Files:**
- Create: `crates/sdkwork-memory-retrieval/src/learning.rs`
- Create: `crates/sdkwork-intelligence-memory-service/src/learning_service.rs`
- Test: `crates/sdkwork-intelligence-memory-service/tests/learning_service_test.rs`

- [ ] **Step 1: Write failing learning tests**

Create tests for repeated signal promotion:

```rust
use sdkwork_memory_product::LearningService;

#[test]
fn repeated_behavior_promotes_habit_candidate_without_llm() {
    let mut service = LearningService::deterministic_for_test();

    service.observe("answer_style", "concise");
    service.observe("answer_style", "concise");
    service.observe("answer_style", "concise");

    let candidates = service.pending_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].habit_key, "answer_style=concise");
}
```

Run:

```powershell
cargo test -p sdkwork-memory-product --test learning_service_test
```

Expected: FAIL.

- [ ] **Step 2: Implement deterministic habit signal accumulator**

Rules:

- promote after configurable threshold
- decay stale signals
- never auto-promote sensitive data
- keep candidate approval explicit unless tenant policy enables auto-approval

- [ ] **Step 3: Verify**

Run:

```powershell
cargo test -p sdkwork-memory-product --test learning_service_test
```

Expected: PASS.

### Task 10: LLM, Embedding, And Rerank Provider Abstractions

**Files:**
- Modify: `crates/sdkwork-memory-contract/src/ports.rs`
- Create: `crates/sdkwork-memory-contract/tests/provider_ports_test.rs`

- [ ] **Step 1: Write failing provider abstraction test**

Create:

```rust
use sdkwork_memory_contract::{EmbeddingModel, LanguageModel, RerankModel};

struct FakeLanguageModel;
struct FakeEmbeddingModel;
struct FakeRerankModel;

impl LanguageModel for FakeLanguageModel {
    fn provider_code(&self) -> &str {
        "fake-language"
    }
}

impl EmbeddingModel for FakeEmbeddingModel {
    fn provider_code(&self) -> &str {
        "fake-embedding"
    }

    fn dimensions(&self) -> usize {
        384
    }
}

impl RerankModel for FakeRerankModel {
    fn provider_code(&self) -> &str {
        "fake-rerank"
    }
}

#[test]
fn provider_ports_are_independent_and_optional() {
    fn reads_language_model(provider: &impl LanguageModel) -> &str {
        provider.provider_code()
    }

    fn reads_embedding_model(provider: &impl EmbeddingModel) -> (&str, usize) {
        (provider.provider_code(), provider.dimensions())
    }

    fn reads_rerank_model(provider: &impl RerankModel) -> &str {
        provider.provider_code()
    }

    assert_eq!(reads_language_model(&FakeLanguageModel), "fake-language");
    assert_eq!(reads_embedding_model(&FakeEmbeddingModel), ("fake-embedding", 384));
    assert_eq!(reads_rerank_model(&FakeRerankModel), "fake-rerank");
}
```

Run:

```powershell
cargo test -p sdkwork-memory-contract --test provider_ports_test
```

Expected: FAIL because the initial marker traits do not expose `provider_code()` or `dimensions()`.

- [ ] **Step 2: Add provider ports without binding product services to concrete providers**

Keep the first port traits small:

```rust
pub trait LanguageModel: Send + Sync {
    fn provider_code(&self) -> &str;
}

pub trait EmbeddingModel: Send + Sync {
    fn provider_code(&self) -> &str;
    fn dimensions(&self) -> usize;
}

pub trait RerankModel: Send + Sync {
    fn provider_code(&self) -> &str;
}
```

Update the fake implementations in the test.

- [ ] **Step 3: Verify**

Run:

```powershell
cargo test -p sdkwork-memory-contract --test provider_ports_test
```

Expected: PASS.

### Task 11: Open API Route Crate

**Files:**
- Create: `crates/sdkwork-routes-memory-open-api/src/paths.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/routes.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/manifest.rs`
- Create: `crates/sdkwork-routes-memory-open-api/src/handlers.rs`
- Test: `crates/sdkwork-routes-memory-open-api/tests/route_manifest_test.rs`

- [ ] **Step 1: Write failing route manifest test**

Create:

```rust
use sdkwork_routes_memory_open_api::manifest::route_manifest;

#[test]
fn open_api_manifest_uses_mem_prefix_and_api_key_auth() {
    let manifest = route_manifest();
    assert_eq!(manifest.package_name, "sdkwork-routes-memory-open-api");
    assert_eq!(manifest.surface, "open-api");
    assert_eq!(manifest.api_authority, "sdkwork-memory-open-api");
    assert_eq!(manifest.sdk_family, "sdkwork-memory-sdk");
    assert_eq!(manifest.prefix, "/mem/v3/api");
    assert!(manifest.routes.iter().all(|route| route.path.starts_with("/mem/v3/api")));
    assert!(manifest.routes.iter().all(|route| route.auth_mode == "api-key"));
}
```

Run:

```powershell
cargo test -p sdkwork-routes-memory-open-api --test route_manifest_test
```

Expected: FAIL.

- [ ] **Step 2: Add path constants**

Create `paths.rs`:

```rust
pub const PREFIX: &str = "/mem/v3/api";
pub const MEMORY_BASE: &str = "/mem/v3/api/memory";
pub const CAPABILITIES: &str = "/mem/v3/api/memory/capabilities";
pub const EVENTS: &str = "/mem/v3/api/memory/events";
pub const MEMORIES: &str = "/mem/v3/api/memory/memories";
pub const RETRIEVALS: &str = "/mem/v3/api/memory/retrievals";
pub const CONTEXT_PACKS: &str = "/mem/v3/api/memory/context_packs";
pub const FEEDBACK: &str = "/mem/v3/api/memory/feedback";
pub const EXTRACTIONS: &str = "/mem/v3/api/memory/extractions";
pub const CANDIDATES: &str = "/mem/v3/api/memory/candidates";
pub const PROVIDER_HEALTH: &str = "/mem/v3/api/memory/provider_health";
```

- [ ] **Step 3: Add route manifest**

Manifest entries must match the 17 generated Open API operations:

```rust
pub fn operation_ids() -> [&'static str; 17] {
    [
        "capabilities.retrieve",
        "events.create",
        "events.retrieve",
        "memories.list",
        "memories.create",
        "memories.retrieve",
        "memories.update",
        "memories.delete",
        "retrievals.create",
        "retrievals.retrieve",
        "contextPacks.create",
        "contextPacks.retrieve",
        "feedback.create",
        "extractions.create",
        "candidates.list",
        "candidates.retrieve",
        "providerHealth.retrieve",
    ]
}
```

- [ ] **Step 4: Verify manifest**

Run:

```powershell
cargo test -p sdkwork-routes-memory-open-api --test route_manifest_test
```

Expected: PASS.

### Task 12: App API And Backend API Route Crates

**Files:**
- Create: `crates/sdkwork-routes-memory-app-api/src/lib.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/paths.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/routes.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/manifest.rs`
- Create: `crates/sdkwork-routes-memory-app-api/src/handlers.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/lib.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/paths.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/routes.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/manifest.rs`
- Create: `crates/sdkwork-routes-memory-backend-api/src/handlers.rs`
- Test: route manifest tests under both route crates

- [ ] **Step 1: Write failing app/backend route manifest tests**

App test assertions:

```rust
assert_eq!(manifest.package_name, "sdkwork-routes-memory-app-api");
assert_eq!(manifest.surface, "app-api");
assert_eq!(manifest.api_authority, "sdkwork-memory-app-api"); // early drafts showed the retired dotted alias sdkwork-memory.app
assert_eq!(manifest.sdk_family, "sdkwork-memory-app-sdk");
assert_eq!(manifest.prefix, "/app/v3/api");
assert!(manifest.routes.iter().all(|route| route.auth_mode == "dual-token"));
```

Backend test assertions:

```rust
assert_eq!(manifest.package_name, "sdkwork-routes-memory-backend-api");
assert_eq!(manifest.surface, "backend-api");
assert_eq!(manifest.api_authority, "sdkwork-memory-backend-api"); // early drafts showed the retired dotted alias sdkwork-memory.backend
assert_eq!(manifest.sdk_family, "sdkwork-memory-backend-sdk");
assert_eq!(manifest.prefix, "/backend/v3/api");
assert!(manifest.routes.iter().all(|route| route.auth_mode == "dual-token"));
assert!(manifest.routes.iter().all(|route| !route.path.contains("/auth")));
```

Run:

```powershell
cargo test -p sdkwork-routes-memory-app-api --test route_manifest_test
cargo test -p sdkwork-routes-memory-backend-api --test route_manifest_test
```

Expected: FAIL.

- [ ] **Step 2: Add app/backend manifests and path constants**

Mirror the OpenAPI authorities generated by `tools/materialize_phase1_contracts.mjs`.

- [ ] **Step 3: Verify**

Run:

```powershell
cargo test -p sdkwork-routes-memory-app-api --test route_manifest_test
cargo test -p sdkwork-routes-memory-backend-api --test route_manifest_test
```

Expected: PASS.

### Task 13: API Context And Smoke Tests

**Files:**
- Create: `tests/api/open_api_smoke_test.rs`
- Create: `tests/api/app_backend_api_smoke_test.rs`
- Modify: route handlers after failing tests

- [ ] **Step 1: Write failing Open API credential test**

Create a smoke test that calls a protected Open API route without `X-API-Key`, then with a fake valid API-key resolver:

```rust
#[tokio::test]
async fn open_api_rejects_missing_api_key_and_does_not_use_dual_token_fallback() {
    let app = sdkwork_routes_memory_open_api::test_router();

    let response = app
        .oneshot(request_without_api_key("/mem/v3/api/memory/capabilities"))
        .await
        .unwrap();

    assert_eq!(response.status(), 401);
}
```

Run:

```powershell
cargo test --test open_api_smoke_test
```

Expected: FAIL until the route test harness exists.

- [ ] **Step 2: Write failing app/backend dual-token tests**

Test missing auth token, missing access token, and successful fake context injection.

- [ ] **Step 3: Implement minimal context resolvers**

Use test-only resolvers for Phase 1 route smoke tests. Production IAM/appbase integration must be a later explicit task and must not trust raw tenant/user headers.

- [ ] **Step 4: Verify API smoke tests**

Run:

```powershell
cargo test --test open_api_smoke_test
cargo test --test app_backend_api_smoke_test
```

Expected: PASS.

### Task 14: Contract Drift And SDK Artifact Checks

**Files:**
- Create: `tests/contracts/openapi_phase1_contract_test.mjs`
- Modify: `tools/verify_phase1.ps1` only through generator if the test proves a missing guard

- [ ] **Step 1: Write OpenAPI drift test**

Create:

```js
import assert from "node:assert/strict";
import fs from "node:fs";

const open = JSON.parse(fs.readFileSync("sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json", "utf8"));

const operations = Object.values(open.paths).flatMap((pathItem) =>
  ["get", "post", "patch", "delete"].filter((method) => pathItem[method]).map((method) => pathItem[method])
);

assert.equal(operations.length, 17);
assert.ok(open.components.securitySchemes.ApiKey);
assert.ok(!open.components.securitySchemes.AuthToken);
assert.ok(!open.components.securitySchemes.AccessToken);
for (const operation of operations) {
  assert.deepEqual(operation.security, [{ ApiKey: [] }]);
  assert.equal(operation["x-sdkwork-auth-mode"], "api-key");
  assert.equal(operation["x-sdkwork-api-authority"], "sdkwork-memory-open-api");
}
```

Run:

```powershell
node tests\contracts\openapi_phase1_contract_test.mjs
```

Expected: PASS with current generated OpenAPI. If it fails, fix the generator and regenerate.

- [ ] **Step 2: Verify full contract materialization**

Run:

```powershell
node --check tools\materialize_phase1_contracts.mjs
node tools\materialize_phase1_contracts.mjs
powershell -ExecutionPolicy Bypass -File tools\verify_phase1.ps1
```

Expected: PASS.

### Task 15: End-To-End No-Embedding MVP Flow

**Files:**
- Create: `tests/no_embedding/memory_no_embedding_mvp_test.rs`
- Modify: product/storage/core services only after failing test

- [ ] **Step 1: Write failing end-to-end test**

Create:

```rust
use sdkwork_memory_product::MemoryProductRuntime;

#[tokio::test]
async fn remembers_retrieves_and_builds_context_without_embeddings() {
    let runtime = MemoryProductRuntime::new_sqlite_phase1().await.unwrap();

    runtime.append_event("evt-1", "User prefers concise answers").await.unwrap();
    runtime.create_memory("rec-1", "answer_style", "concise").await.unwrap();

    let retrieval = runtime.retrieve("How should the assistant answer?").await.unwrap();
    assert!(retrieval.hits.iter().any(|hit| hit.memory_id == "rec-1"));
    assert!(!retrieval.retrievers_used.contains(&"vector".to_string()));

    let pack = runtime.create_context_pack(retrieval.trace_id).await.unwrap();
    assert!(pack.context_text.contains("concise"));
}
```

Run:

```powershell
cargo test --test memory_no_embedding_mvp_test
```

Expected: FAIL.

- [ ] **Step 2: Wire storage, retrievers, orchestrator, and context assembler**

Build `MemoryProductRuntime::new_sqlite_phase1()` with:

- SQLx SQLite repository
- SQL, keyword, dictionary, time, and event retrievers
- no embedding provider
- context pack assembler
- retrieval trace writer

- [ ] **Step 3: Verify end-to-end flow**

Run:

```powershell
cargo test --test memory_no_embedding_mvp_test
```

Expected: PASS.

### Task 16: Documentation And Evidence Bundle

**Files:**
- Modify: `docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md`
- Modify: `docs/architecture/tech/TECH-2026-06-10-memory-open-api-and-no-embedding-mvp.md`
- Modify: `specs/README.md` only through generator if contract artifact list changes

- [ ] **Step 1: Update implementation evidence**

Add a short evidence section to the design or implementation notes with:

```text
node --check tools\materialize_phase1_contracts.mjs
node tools\materialize_phase1_contracts.mjs
powershell -ExecutionPolicy Bypass -File tools\verify_phase1.ps1
cargo test --workspace
```

- [ ] **Step 2: Run all contract and Rust checks**

Run:

```powershell
node --check tools\materialize_phase1_contracts.mjs
node tools\materialize_phase1_contracts.mjs
powershell -ExecutionPolicy Bypass -File tools\verify_phase1.ps1
cargo test --workspace
```

Expected:

- Open API verifies with 17 operations.
- App API and Backend API verify with their current operation counts.
- No no-embedding test uses vector or embedding storage.
- Rust workspace tests pass.

- [ ] **Step 3: Record unresolved risks**

Record any deferred items explicitly:

- production API key resolver
- production appbase dual-token resolver
- PostgreSQL integration test database
- generated TypeScript compile after `sdkgen` is wired
- external provider bridge deletion propagation tests
- OpenSearch/vector/graph/file retrievers

## Execution Notes

- Do not hand-edit generated SDK output.
- Do not add vector/embedding schema to Phase 1 migrations.
- Any table or migration change needs human review before running against persistent databases.
- This repository currently may not be initialized as a git repository; if `git status` fails, skip commit steps and report that limitation.
- If implementation continues in the main agent, execute tasks in order and keep each task red-green before moving to the next one.
