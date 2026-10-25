> Status: archived implementation plan. It is not current capability or release authority; use `TECH_ARCHITECTURE.md`.
>
> Migrated from `docs/superpowers/plans/2026-06-10-memory-spi-plugin-runtime-implementation-plan.md` on 2026-06-24.
> Owner: SDKWork maintainers

## Implementation Status (2026-07-06)

**Complete for 0.1.x baseline.** Static plugin registration, native SQL plugin, profile resolver, and conformance harness are implemented.

**Current architecture and production guidance:**

- `docs/architecture/tech/TECH_ARCHITECTURE.md`
- `docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md` §2

The task checklist below is retained as an historical implementation record only. Do not treat unchecked boxes as open work.

---

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development for every runtime/code step. This repository run is explicitly main-agent inline execution per user instruction; do not dispatch subagents for this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first executable SDKWork Memory SPI/plugin runtime layer so implementation profiles can compose provider-neutral ports without changing public Open API, App API, or Backend API contracts.

**Architecture:** Start with static Rust plugin registration for 0.1.0. Keep `sdkwork-memory-spi` framework-neutral and provider-neutral, add a small `sdkwork-memory-profile-resolver` resolver/registry, and create the first `sdkwork-memory-plugin-native-sql` manifest package as the no-embedding baseline. Dynamic loading, backend plugin-list APIs, and provider-specific bridges are deferred until native SQL conformance is real.

**Tech Stack:** Rust workspace, `serde`, `serde_json`, `thiserror`, `async-trait`, Node contract tests, PowerShell Phase 1 verification.

---

## Source Documents

- Agent rules: `AGENTS.md`
- App identity: `agent.app.config.json`
- Component contract: `specs/component.spec.json`
- Architecture design: `docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md`
- Memory design: `docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md`
- Existing MVP plan: `docs/architecture/tech/TECH-2026-06-10-memory-open-api-and-no-embedding-mvp.md`
- Root standards: `../sdkwork-specs/CODE_STYLE_SPEC.md`, `../sdkwork-specs/NAMING_SPEC.md`, `../sdkwork-specs/RUST_CODE_SPEC.md`, `../sdkwork-specs/TEST_SPEC.md`

## File Structure

- Create: `Cargo.toml`
- Create: `crates/sdkwork-memory-spi/Cargo.toml`
- Create: `crates/sdkwork-memory-spi/src/lib.rs`
- Create: `crates/sdkwork-memory-spi/src/error.rs`
- Create: `crates/sdkwork-memory-spi/src/manifest.rs`
- Create: `crates/sdkwork-memory-spi/src/ports.rs`
- Create: `crates/sdkwork-memory-spi/src/registry.rs`
- Create: `crates/sdkwork-memory-spi/src/runtime.rs`
- Create: `crates/sdkwork-memory-spi/tests/manifest_contract.rs`
- Create: `crates/sdkwork-memory-spi/tests/registry_contract.rs`
- Create: `crates/sdkwork-memory-profile-resolver/Cargo.toml`
- Create: `crates/sdkwork-memory-profile-resolver/src/lib.rs`
- Create: `crates/sdkwork-memory-profile-resolver/src/profile.rs`
- Create: `crates/sdkwork-memory-profile-resolver/tests/profile_resolution_contract.rs`
- Create: `crates/sdkwork-memory-test-support/Cargo.toml`
- Create: `crates/sdkwork-memory-test-support/src/lib.rs`
- Create: `plugins/sdkwork-memory-plugin-native-sql/Cargo.toml`
- Create: `plugins/sdkwork-memory-plugin-native-sql/sdkwork.memory.plugin.json`
- Create: `plugins/sdkwork-memory-plugin-native-sql/src/lib.rs`
- Create: `plugins/sdkwork-memory-plugin-native-sql/src/manifest.rs`
- Create: `plugins/sdkwork-memory-plugin-native-sql/tests/manifest_matches_json.rs`
- Modify: `tests/contracts/spi_design_contract_test.mjs`
- Modify: `tools/materialize_phase1_contracts.mjs`
- Generated: `tools/verify_phase1.ps1`

## Invariants

- Static Rust registration only for 0.1.0.
- Runtime plugins live under `plugins/`, never `.sdkwork/plugins/`.
- Plugin manifests contain secret references only, never secret values.
- Core required ports must be present before a runtime serves traffic.
- `native_sql` is the first complete production target and must not require embeddings, vector stores, graph databases, external providers, or LLM providers.
- The SPI crate must not depend on HTTP framework types or generated SDKs for the same Memory authority.
- Generated SDK output remains untouched.

### Task 1: SPI Design Contract Hardening

**Files:**
- Modify: `tests/contracts/spi_design_contract_test.mjs`
- Modify: `docs/architecture/tech/TECH-2026-06-10-memory-spi-plugin-architecture-design.md`
- Modify: `tools/materialize_phase1_contracts.mjs`
- Generated: `tools/verify_phase1.ps1`

- [x] **Step 1: Write a failing contract test**

Run:

```powershell
node tests/contracts/spi_design_contract_test.mjs
```

Expected: FAIL until the SPI design includes `0.1.0 Implementation Decisions`, static registration, JSON manifest plus Rust constant, runtime plugin path separation, and industry references.

- [x] **Step 2: Resolve first-landing design decisions**

Update the SPI design so Phase B can start without open decisions:

```text
Static Rust registration
local_embedded inside native_sql for 0.1.0
JSON manifest plus Rust constant
external bridge eval_only
conformance stored in ai_eval_run initially
plugin-list backend API deferred
native_sql first production target
framework-neutral SPI crate
```

- [x] **Step 3: Propagate checks into Phase 1 verifier**

Patch `tools/materialize_phase1_contracts.mjs` so generated `tools/verify_phase1.ps1` checks the same snippets and rejects `## 17. Open Decisions`.

- [x] **Step 4: Verify design and generated verifier**

Run:

```powershell
node tools/materialize_phase1_contracts.mjs
node tests/contracts/spi_design_contract_test.mjs
powershell -ExecutionPolicy Bypass -File tools/verify_phase1.ps1
```

Expected: PASS.

### Task 2: Rust Workspace And SPI Crate Skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `crates/sdkwork-memory-spi/Cargo.toml`
- Create: `crates/sdkwork-memory-spi/src/lib.rs`
- Test: `cargo metadata --format-version 1`

- [ ] **Step 1: Verify red state**

Run:

```powershell
cargo metadata --format-version 1
```

Expected: FAIL because no root `Cargo.toml` exists.

- [ ] **Step 2: Add minimal workspace and SPI crate manifest**

Create root `Cargo.toml` with `sdkwork-memory-spi`, `sdkwork-memory-profile-resolver`, `sdkwork-memory-test-support`, and `plugins/sdkwork-memory-plugin-native-sql` as members. Use workspace dependencies for `async-trait`, `serde`, `serde_json`, and `thiserror`.

- [ ] **Step 3: Add minimal `lib.rs` exports only**

`crates/sdkwork-memory-spi/src/lib.rs` must only declare modules and re-export stable public SPI types.

- [ ] **Step 4: Verify metadata**

Run:

```powershell
cargo metadata --format-version 1
```

Expected: PASS and lists the workspace members.

### Task 3: Plugin Manifest Contract

**Files:**
- Create: `crates/sdkwork-memory-spi/src/manifest.rs`
- Create: `crates/sdkwork-memory-spi/src/error.rs`
- Test: `crates/sdkwork-memory-spi/tests/manifest_contract.rs`

- [ ] **Step 1: Write failing manifest tests**

Test behaviors:

```rust
use sdkwork_memory_spi::{
    MemoryImplementationKind, MemoryPluginManifest, MemoryPluginRole,
};

#[test]
fn native_sql_manifest_deserializes_and_declares_no_embedding_baseline() {
    let manifest: MemoryPluginManifest = serde_json::from_str(r#"{
      "schemaVersion": 1,
      "kind": "sdkwork.memory.plugin",
      "pluginId": "sdkwork-memory-plugin-native-sql",
      "packageName": "sdkwork-memory-plugin-native-sql",
      "displayName": "SDKWork Memory Native SQL Plugin",
      "version": "0.1.0",
      "owner": "sdkwork-memory",
      "implementationKinds": ["native_sql", "local_embedded"],
      "pluginRoles": ["implementation", "store", "retriever", "index"],
      "deploymentModes": ["server", "container", "private", "local", "test"],
      "portExports": [{"port": "MemoryRecordStorePort", "builder": "build_native_sql_record_store"}],
      "providerKinds": [],
      "retrieverKinds": ["sql", "keyword", "dictionary", "time", "event"],
      "indexKinds": ["sql", "keyword", "dictionary", "time", "event"],
      "requiredCoreVersion": "0.1.0",
      "secretRefs": [],
      "dataClasses": ["tenant", "personal"],
      "capabilities": {
        "canonicalStore": true,
        "eventLog": true,
        "candidateLifecycle": true,
        "habitLearning": true,
        "deletionPropagation": true,
        "auditLog": true,
        "embeddingRequired": false
      },
      "degradation": {"mode": "fail_required_degrade_optional", "returnsStaleHits": false},
      "migration": {"exportSupported": true, "importSupported": true, "dualWriteSupported": false, "shadowReadSupported": true},
      "observability": {"metricsPrefix": "sdkwork_memory_native_sql", "redactsPayloads": true},
      "conformance": {"suite": "sdkwork-memory-plugin-conformance", "suiteVersion": "0.1.0"}
    }"#).unwrap();

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.implementation_kinds.contains(&MemoryImplementationKind::NativeSql));
    assert!(manifest.implementation_kinds.contains(&MemoryImplementationKind::LocalEmbedded));
    assert!(manifest.plugin_roles.contains(&MemoryPluginRole::Implementation));
    assert!(!manifest.capabilities.embedding_required);
    assert!(manifest.validate().is_ok());
}

#[test]
fn manifest_rejects_secret_values_and_agent_plugin_paths() {
    let mut manifest = MemoryPluginManifest::native_sql_for_test();
    manifest.secret_refs.push("literal-token-secret".to_string());
    assert!(manifest.validate().is_err());

    let mut manifest = MemoryPluginManifest::native_sql_for_test();
    manifest.package_name = ".sdkwork/plugins/sdkwork-memory-plugin-native-sql".to_string();
    assert!(manifest.validate().is_err());
}
```

Run:

```powershell
cargo test -p sdkwork-memory-spi --test manifest_contract
```

Expected: FAIL because manifest types do not exist.

- [ ] **Step 2: Implement manifest structs and validation**

Use `serde(rename_all = "camelCase")` for manifest fields and `serde(rename_all = "snake_case")` for enum values. Validation must reject:

```text
kind != sdkwork.memory.plugin
schemaVersion != 1
pluginId not starting with sdkwork-memory-plugin-
pluginId/packageName containing .sdkwork/plugins
empty portExports
secretRefs containing suspicious literal secret markers such as token, password, api_key, private_key
embeddingRequired=true for native_sql
```

- [ ] **Step 3: Verify manifest tests**

Run:

```powershell
cargo test -p sdkwork-memory-spi --test manifest_contract
```

Expected: PASS.

### Task 4: SPI Port Traits

**Files:**
- Create: `crates/sdkwork-memory-spi/src/ports.rs`
- Test: `crates/sdkwork-memory-spi/tests/ports_contract.rs`

- [ ] **Step 1: Write failing port trait tests**

Test that required store ports, retriever ports, index ports, provider ports, context assembler, and evaluation ports are importable and can be implemented by fakes without HTTP framework types.

- [ ] **Step 2: Implement minimal provider-neutral traits**

Traits:

```text
MemoryRecordStorePort
MemoryEventStorePort
MemoryAuditStorePort
MemoryPolicyStorePort
MemoryRetrieverPort
MemoryIndexPort
LanguageModelPort
EmbeddingModelPort
RerankModelPort
ExternalMemoryBridgePort
MemoryContextAssemblerPort
MemoryEvaluationPort
```

Use small command/result structs where needed; do not add persistence or HTTP implementation.

- [ ] **Step 3: Verify port tests**

Run:

```powershell
cargo test -p sdkwork-memory-spi --test ports_contract
```

Expected: PASS.

### Task 5: Plugin Registry

**Files:**
- Create: `crates/sdkwork-memory-spi/src/registry.rs`
- Test: `crates/sdkwork-memory-spi/tests/registry_contract.rs`

- [ ] **Step 1: Write failing registry tests**

Test behaviors:

```text
register plugin by pluginId
reject duplicate pluginId
lookup implementation kind native_sql
lookup required port export MemoryRecordStorePort
fail when active profile requires missing port
```

- [ ] **Step 2: Implement registry**

Implement `MemoryPluginRegistry` with deterministic insertion order and typed errors.

- [ ] **Step 3: Verify registry tests**

Run:

```powershell
cargo test -p sdkwork-memory-spi --test registry_contract
```

Expected: PASS.

### Task 6: Runtime Profile Resolver

**Files:**
- Create: `crates/sdkwork-memory-profile-resolver/Cargo.toml`
- Create: `crates/sdkwork-memory-profile-resolver/src/lib.rs`
- Create: `crates/sdkwork-memory-profile-resolver/src/profile.rs`
- Test: `crates/sdkwork-memory-profile-resolver/tests/profile_resolution_contract.rs`

- [ ] **Step 1: Write failing profile resolver tests**

Test behaviors:

```text
native_sql profile resolves when the native SQL plugin is registered
profile fails before serving when primary implementation plugin is missing
profile fails when required core store port is missing
profile rejects config that carries secret values instead of secret refs
```

- [ ] **Step 2: Implement minimal resolver**

The resolver consumes `MemoryPluginRegistry` and a typed `MemoryImplementationProfileDraft`. It returns `ResolvedMemoryImplementationProfile` or typed error.

- [ ] **Step 3: Verify runtime tests**

Run:

```powershell
cargo test -p sdkwork-memory-profile-resolver --test profile_resolution_contract
```

Expected: PASS.

### Task 7: Native SQL Runtime Plugin Manifest Package

**Files:**
- Create: `plugins/sdkwork-memory-plugin-native-sql/Cargo.toml`
- Create: `plugins/sdkwork-memory-plugin-native-sql/sdkwork.memory.plugin.json`
- Create: `plugins/sdkwork-memory-plugin-native-sql/src/lib.rs`
- Create: `plugins/sdkwork-memory-plugin-native-sql/src/manifest.rs`
- Test: `plugins/sdkwork-memory-plugin-native-sql/tests/manifest_matches_json.rs`

- [ ] **Step 1: Write failing manifest parity test**

Test that:

```text
JSON manifest deserializes into MemoryPluginManifest
Rust `native_sql_manifest()` returns the same pluginId/packageName/version/capabilities
manifest validates
manifest declares native_sql and local_embedded
manifest does not require embeddings
manifest path is under plugins/
```

- [ ] **Step 2: Add JSON manifest and Rust constant**

Keep implementation minimal. Do not add SQL stores yet.

- [ ] **Step 3: Verify native SQL plugin manifest tests**

Run:

```powershell
cargo test -p sdkwork-memory-plugin-native-sql --test manifest_matches_json
```

Expected: PASS.

### Task 8: Conformance Harness Skeleton

**Files:**
- Create: `crates/sdkwork-memory-test-support/Cargo.toml`
- Create: `crates/sdkwork-memory-test-support/src/lib.rs`
- Test: `crates/sdkwork-memory-test-support/tests/conformance_harness_contract.rs`

- [ ] **Step 1: Write failing conformance harness test**

Test that a manifest can be checked for:

```text
manifest validation
required port declarations
no-embedding profile
secret redaction
runtime plugin path separation
```

- [ ] **Step 2: Implement `MemoryPluginConformanceHarness` skeleton**

Do not claim full store/retrieval conformance yet. The harness must return a report with explicit `pending` checks for store CRUD, tenant isolation, deletion propagation, retrieval trace, and audit/outbox.

- [ ] **Step 3: Verify harness test**

Run:

```powershell
cargo test -p sdkwork-memory-test-support --test conformance_harness_contract
```

Expected: PASS with pending checks represented explicitly.

### Task 9: Static Runtime Plugin Path Contract

**Files:**
- Modify: `tests/contracts/spi_design_contract_test.mjs`
- Create: `tests/contracts/runtime_plugin_layout_contract_test.mjs`

- [ ] **Step 1: Write failing layout contract test**

Test that runtime plugin manifests appear under `plugins/` and no runtime memory plugin manifest appears under `.sdkwork/plugins/`.

- [ ] **Step 2: Implement the minimum manifest file placement**

The native SQL plugin manifest from Task 7 should satisfy this.

- [ ] **Step 3: Verify layout contract**

Run:

```powershell
node tests/contracts/runtime_plugin_layout_contract_test.mjs
```

Expected: PASS.

### Task 10: Full First-Slice Verification

**Files:**
- All files touched above

- [ ] **Step 1: Format and check Rust**

Run:

```powershell
cargo fmt --all
cargo test -p sdkwork-memory-spi
cargo test -p sdkwork-memory-profile-resolver
cargo test -p sdkwork-memory-plugin-native-sql
cargo test -p sdkwork-memory-test-support
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 2: Run contract and Phase 1 verification**

Run:

```powershell
node tests/contracts/spi_design_contract_test.mjs
node tests/contracts/runtime_plugin_layout_contract_test.mjs
node tools/materialize_phase1_contracts.mjs
powershell -ExecutionPolicy Bypass -File tools/verify_phase1.ps1
git diff --check
```

Expected: PASS.

- [ ] **Step 3: Record remaining work**

Remaining after this first slice:

```text
native SQL store implementation
SQLite/PostgreSQL migrations from schema registry
retrieval ports and no-embedding conformance
product service orchestration
route crates
typed request-context integration
backend-admin plugin list APIs
external bridge provider review
dynamic plugin loading and signing policy
```
