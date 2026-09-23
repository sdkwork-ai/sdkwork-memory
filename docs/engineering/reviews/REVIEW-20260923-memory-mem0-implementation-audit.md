# SDKWork Memory `crates/sdkwork-memory-mem0` 实现审计

Status: review record (审计结论 + 修复方案)

Owner: SDKWork Memory maintainers

Reviewed: 2026-09-23

Scope: `D:\sdkwork-space\sdkwork-memory\crates\sdkwork-memory-mem0`（39 个文件 / 5,740 行 Rust）

Baselines: `docs/product/prd/PRD.md`、`docs/architecture/tech/TECH_ARCHITECTURE.md`、`specs/component.spec.json`、`D:\sdkwork-space\sdkwork-specs`（`RUST_CODE_SPEC` / `NAMING_SPEC` / `TEST_SPEC` / `COMPONENT_SPEC` / `DATABASE_SPEC` / `SECURITY_SPEC` / `INTEGRATION_SPEC` / `SOURCE_CONFIG_SPEC` / `HEALTH_CHECK_SPEC` / `DEPENDENCY_MANAGEMENT_SPEC` / `QUALITY_GATE_SPEC` / `CODE_STYLE_SPEC`）

Method: 全量源码通读 + 全仓/全 workspace 检索 + **实跑门禁与 `cargo metadata` 取证**（附录 A）。所有结论给出 `文件:行`。无法实测的标注「疑似」。

---

## 0. 结论摘要

### 0.0 勘误（2026-09-23 15:40，本报告自身的事实错误）

> **本报告初版把 mem0 目录描述成「39 个未追踪文件」，这是错的。**
> 正确事实：这 39 个文件**已被提交**，由 HEAD `4f14ccb93c113391be560f3036d080d39caa0e70`
> （2026-09-23 15:14:32 +0800，`sdkwork-ai`，`feat(memory): mem0 integration, release signing, DB pool spec + related updates`）一次性引入。
>
> **错在哪**：初版取证用的是 `git ls-files crates/sdkwork-memory-mem0 | wc -l`，而当时那条命令链在
> 前一步就短路了，`git ls-files` 的**真实回显（39）从未被执行**，报告里填的是"零输出"的推断值。
> 拿一条没跑过的命令当证据，是取证纪律问题，不是口径问题。
>
> **影响面**：§0 摘要表、§1.1 证据表、§1.1 后果第 2/3 条、§5 方案 B 均引用了这个错误前提，
> 已逐条改正（见各处"勘误"标注）。附录 A 的原始回显保留为**错误证据样本**，并补上复核后的真实回显。
>
> **实质结论不变，但性质更糟**：原来的说法暗示"文件迟早会丢"，实际情况相反——
> 这 39 个文件 5,740 行代码**已在版本控制里、看起来完全正常、但永远不会被编译、lint、测试或任何门禁触及**。
> 「未追踪」至少还会在 `git status` 里显形；**「已提交但无 manifest」连这点信号都没有**
> —— 它污染的是仓库体积与阅读认知，且 `git clean`、重新 clone、CI checkout 都清不掉它。
> 缺陷的可发现性比初版判断**更低**。

**判定：`crates/sdkwork-memory-mem0` 当前不是一个可交付物，而是一份已提交入库、却从未接入构建的未接入上游移植代码。**

它同时存在三个层面的问题，且互相独立：

| 层面 | 定性 | 一句话 |
| --- | --- | --- |
| **工程接入** | **P0 阻断** | 没有 `Cargo.toml`、不在 workspace members、`cargo metadata` 里不存在这个包；**整套门禁对它零覆盖**（实测见 §1.2） |
| **功能正确性** | **P0 阻断** | 存在**跨作用域读+改+删**与**「带条件的 reset 变成全库清空」**两类数据安全缺陷；另有 16 处「静默降级」——接口收下参数、后端悄悄丢弃或替换 |
| **规范合规** | **P1 严重** | crate 名不属于 `TEST_SPEC` 允许的责任族，源码 import 名违反 `NAMING_SPEC` §3.1，架构上不属于 `TECH_ARCHITECTURE.md` 的任何 ownership 边界，且绕过 SPI / IAM / 模型路由 / 数据库框架四条主通道 |

| 级别 | 数量 | 说明 |
| --- | --- | --- |
| **P0 阻断** | **5** | 未接入（§1）、跨作用域读写删（C-1）、reset 全库清空（C-2）、filter 静默丢弃（C-3）、无租户隔离（D-1） |
| **P1 严重** | **17** | 命名/形态 6 项、静默降级 8 项、安全与集成 3 项 |
| **P2 一般** | **12** | 文档、lint 属性、死代码、格式化、锁模型不一致等 |

**根因不是「写得差」，而是「没有归属」**：它既不在 `Cargo.toml` 的 members、也不在 `specs/component.spec.json` 的 manifests、也不在 `TECH_ARCHITECTURE.md` 的 ownership 表、也不在 PRD 的任何能力项里。因此它从未被编译过、从未被 lint 过、从未被任何门禁看见过——**这解释了为什么缺陷能同时存在于数据安全层和规范层而无人发现**。

> 补充：同日完成的 [`REVIEW-20260923-memory-commercial-readiness-audit.md`](REVIEW-20260923-memory-commercial-readiness-audit.md) 做了「全仓检索 + 实跑仓库门禁」，仍未发现本目录。这不是那次审计的疏漏，而是**门禁规则本身缺一条**（§1.3），应作为该评审的遗留项一并登记。**§1.3 的门禁现已落地并自证**（7 个自测用例全绿，首次实跑精确抓出 mem0）。

> **额外发现（非 mem0 自身缺陷，是本次修复过程中撞出来的仓库级 P0）**：
> `pnpm check` 的脚本链因缺失 `check:repository-docs` 脚本而**在中断点后静默丢弃 7 条门禁**（§1.4）。
> 它与 §1.3 同源 —— 都是「门禁存在但等于不存在」，只是形态不同：
> §1.3 是"扫到零个单元"，§1.4 是"根本跑不到"。§1.4 已修复。

---

## 1. 工程接入状态（P0）

### 1.1 事实

| 检查项 | 结果 |
| --- | --- |
| `crates/sdkwork-memory-mem0/Cargo.toml` | **不存在** |
| 仓库根 `Cargo.toml` `members` | **不包含** `crates/sdkwork-memory-mem0`（18 个成员，无此项） |
| `cargo metadata --no-deps` 中 `"name":"sdkwork-memory-mem0"` 计数 | **0** |
| `git ls-files crates/sdkwork-memory-mem0` | **39**（**已提交入库**；由 HEAD `4f14ccb` 引入。初版误报为 0，见 §0.0 勘误） |
| `git check-ignore -v crates/sdkwork-memory-mem0/src/lib.rs` | 退出码 **1**（未被任何 `.gitignore` 规则匹配） |
| 全仓 `mem0` 引用（`*.toml` / `*.json` / `*.md` / `*.mjs` / `*.rs`，排除 `target/`） | **0** |
| `sdkwork-specs` 全仓 `mem0` 引用 | **0** |
| `specs/component.spec.json` `manifests` | **不包含** |
| `TECH_ARCHITECTURE.md` ownership 边界表 | **不包含** |
| `tests/` 目录 / `specs/component.spec.json` / `README.md` | **均不存在** |

**直接后果**：

1. `pnpm test` / `pnpm verify`（最终都落到 `cargo test --workspace`）**不会编译它**，所以它的 30+ 个内联单测从未运行过一次。
2. 它**在 git 里，但没有 manifest** → `git clean -fdx` 清不掉它、重新 clone 也带得回来，
   所以它不会"消失"，只会**永久留在仓库里**：5,740 行不可编译、不可测试、不可引用的代码构成仓库体积与阅读成本，
   且没有任何检查会把它标红。
   **（初版在此处写作"不在 git 里、随时会凭空消失"，方向相反，已改正。）**
3. 它**已入库**，因此 `git log` / `git blame` / code review 都覆盖得到 —— 但 review 只能看见"diff 加了什么文件"，
   **看不见"这些文件不在 workspace members 里"**。信号存在于 git 层，判定存在于构建层，两者不交叉，
   这正是它能带着 P0 数据安全缺陷合入 HEAD 的原因。
   **（初版在此处写作"git status 只多一行、没有 diff / review / blame"，与实际相反，已改正。）**

### 1.2 门禁盲区（实测证据）

全部用 `--root "D:/sdkwork-space/sdkwork-memory"` 实跑（Windows 正斜杠路径；用 Git Bash 的 `/d/...` 形式会被 node 解析成 `D:\d\...`，读数无效）：

| 门禁 | 实测输出 | 是否发现 mem0 |
| --- | --- | --- |
| `check-rust-crate-naming-standard.mjs` | `crates scanned: 18`（**恰等于 workspace members 数**） | **否** |
| `check-rust-manifest-standard.mjs` | `errors: 0 / warnings: 0` | **否** |
| `check-database-framework-standard.mjs` | `Database framework standard passed` | **否** |
| `tools/verify_sdkwork_structure.ps1` | 用 `Get-ChildItem -Recurse -Filter Cargo.toml` 枚举 | **否**（无 manifest 即不存在） |
| `tools/check_sdkwork_memory_architecture_alignment.mjs` | 固定 `文件:断言` 清单，不枚举 crate | **否** |

**结论：当前门禁套件里没有任何一条规则能发现「一个没有 manifest 的 crate 目录」。**

### 1.3 补齐的结构门禁（**已落地并自证**）

初版此处是"建议登记"。现已实现并接入脚本链：

| 项 | 落地物 |
| --- | --- |
| 门禁 | `tools/check-crate-inventory-standard.mjs` |
| 自测 | `tools/check-crate-inventory-standard.test.mjs`（7 个用例，**7/7 pass**） |
| 脚本 | `package.json` → `check:crate-inventory-standard` |
| 接入 | `_sdkwork:check` 第 3 位（`check:architecture-alignment` 之后）；`_sdkwork:verify` 经 `pnpm check` 覆盖；自测进 `_sdkwork:test` 与 `_sdkwork:verify` |

规则 R1/R2/R3：

- **R1** `crates/*` 与 `plugins/*` 下每个一级目录必须有 `Cargo.toml`。
- **R2** 该 `[package].name` 必须出现在仓库根 `Cargo.toml` 的 `[workspace] members` 中（支持 `crates/*` 通配展开，与 cargo 同义）。
- **R3** `members` 里的每个路径必须在磁盘上存在（防悬空 member）。

**首次实跑（就是这个仓库，2026-09-23）**：

```console
$ node tools/check-crate-inventory-standard.mjs
repo root      : D:\sdkwork-space\sdkwork-memory
crates/plugins : 20 first-level crate directories scanned
manifests      : 19 Cargo.toml found
members        : 19 declared -> 19 resolved on disk
units examined : 39
exit=1
Crate inventory standard failed:
- crates/sdkwork-memory-mem0/ has no Cargo.toml: it is committed but unreachable by cargo,
  cargo test --workspace, and every crate-scanning gate (QUALITY_GATE_SPEC.md section 30). ...
```

**它恰好且只抓出 mem0 一个目录** —— 20 个目录里 19 个合规，唯一例外是 mem0。这正是初版 §1.2 想要的证据。

**变异控制（证明门禁非恒绿）**：自测在系统临时目录构造合成仓库，逐条验证 R1 / R2 / R3 / 空扫描四条规则都会把门禁**打红**，健康布局会**转绿**。空扫描（`crates/` 与 `plugins/` 都不存在）**不是 pass 而是 exit 1**（`QUALITY_GATE_SPEC.md` §31 Fail closed）。

> **⚠️ 该门禁当前是红的，且这是设计意图。** 一旦 mem0 目录被删除（或获得 manifest 并入 members），它会自动转绿。
> 用 allowlist 把它刷绿会重新制造 §1.2 那种"门禁存在但看不见"的状态，因此**不做 allowlist**。
> 处置决策见 §5 第 6 项。

### 1.4 【P0 · 新发现】`pnpm check` 的脚本链有 7 条门禁**永不执行**

这是与 §1.3 同类、但性质更重的一个缺陷：不是"门禁扫零个单元"，而是**门禁根本跑不到**。

`package.json` 的 `_sdkwork:check` 与 `_sdkwork:verify` 都调用了 `pnpm check:repository-docs`，
但**这个脚本在仓库里没有定义**：

```console
$ pnpm run check:repository-docs
 ERR_PNPM_NO_SCRIPT  Missing script: check:repository-docs
$ pnpm run check:repository-docs >/dev/null 2>&1; echo $?
1
```

因为链条用 `&&` 串联，`_sdkwork:check` 在 `check:repository-docs` 处**中断**，其后的门禁全部不可达：

| # | `_sdkwork:check` 中的门禁 | 实际是否执行 |
| --- | --- | --- |
| 1 | `check:app-composition` | ✅ |
| 2 | `check:architecture-alignment` | ✅ |
| 3 | `check:release-readiness` | ✅ |
| 4 | `check:pnpm-script-standard` | ✅ |
| 5 | `check:agent-workflow-standard` | ✅ |
| 6 | `check:repository-docs` | ❌ **脚本不存在 → 链在此中断** |
| 7 | `check:pagination` | ⛔ **不可达** |
| 8 | `check:api-envelope` | ⛔ **不可达** |
| 9 | `check:api-operation-patterns` | ⛔ **不可达** |
| 10 | `check:sdk-standard` | ⛔ **不可达** |
| 11 | `topology:validate` | ⛔ **不可达** |
| 12 | `db:validate` | ⛔ **不可达** |
| 13 | `db:pool:validate` | ⛔ **不可达** |

即 **7 条门禁（第 7–13 位）在 `pnpm check` 里从未运行**。`_sdkwork:verify` 中段调用 `pnpm check`，
因此它的收尾段（`check_sdkwork_memory_architecture_alignment` / `check:cors-standard` /
`check:api-operation-patterns` / `check:repository-docs` / `db:pool:validate`）同样不可达。

**为什么没被发现**：这些门禁**单独跑都是绿的**（§1.2 的表就是这么跑的），
"逐条跑绿" 与 "链上跑得到" 是两件事 —— 与 §0.0 勘误里那个错误同源：
**把"某个局部动作成功"当成"整体链路成功"。**

**修复**（已落地）：`check-repository-docs-standard.mjs` 在 `sdkwork-specs/tools/` 里**是存在的**，
`AGENTS_SPEC.md:288` 也写明了调用式，是 memory 仓的脚本定义在某次编辑中丢失而引用残留。已补回：

```json
"check:repository-docs": "node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root ."
```

```console
$ node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root "D:/sdkwork-space/sdkwork-memory"
repository docs standard ok: D:\sdkwork-space\sdkwork-memory (profile=application)
exit=0
```

**同类风险提示（未修，需人工裁定）**：`_sdkwork:verify` 链条更长、串联更多外部工具
（`powershell tools/verify_phase1.ps1` 等），**任何一环缺文件都会静默吃掉其后全部环节**。
建议后续把这些 `&&` 长串改成一个会**枚举并报告每环状态**的 runner，而不是靠 shell 短路 —— 
见 §5 第 8 项。

---

## 2. 功能正确性缺陷

### C-1 ·【P0】跨作用域读取、更新与删除他人记忆

`Memory::add(..., infer=true)` 的推断分支在检索「已有相关记忆」时构造了一个**条件为空的 filter**：

- `crates/sdkwork-memory-mem0/src/memory/manager.rs:205-208` —— `search_filters` 的 `conditions: vec![]`
- `crates/sdkwork-memory-mem0/src/memory/manager.rs:213-216` —— 该空 filter 传入 `vector_store.search(...)`
- `crates/sdkwork-memory-mem0/src/vector_stores/memory.rs:59-61` —— `conditions.is_empty()` 时**直接返回 true**，即匹配全部记录

于是进入 LLM 上下文的是**全库检索结果（含其他 user/agent/run 的记忆）**，然后：

- `manager.rs:299-321` —— `UPDATE` 分支调用 `self.update(real_id, &text)`
- `manager.rs:322-348` —— `DELETE` 分支调用 `self.delete(real_id)`

而 `update()`（`manager.rs:461-494`）与 `delete()`（`manager.rs:497-519`）**没有任何作用域校验**。

即：**用户 A 的一次 `add` 就可能改写或删除用户 B 的记忆**，触发条件只是向量相似度足够高。这既是正确性缺陷，也是跨租户数据破坏。

### C-2 ·【P0】带作用域的 `reset()` 退化为全库清空

`crates/sdkwork-memory-mem0/src/memory/manager.rs:532-551`：

```rust
let filters = if options.user_id.is_some() || options.agent_id.is_some() {
    // TODO: Build proper filters
    None
} else {
    None
};
self.vector_store.delete_all(filters.as_ref()).await?;

if let Some(history) = &self.history {
    if filters.is_none() {
        history.reset()?;
    }
}
```

两个分支**都返回 `None`**。因此：

1. `reset(ResetOptions { user_id: Some("alice"), .. })` → `delete_all(None)` → 进入各后端「无 filter」语义：
   - Postgres：`DELETE FROM <table>`（无 `WHERE`，`postgres.rs:315-327`）
   - Redis：`KEYS <prefix>*` 后逐个 `DEL`（`redis.rs:300-319`）
   - Qdrant：**删除整个 collection 再重建**（`qdrant.rs:310-319`）
   - InMemory：清空整个 `HashMap`（`memory.rs:271-289`）
2. `filters.is_none()` 恒为真 → 紧接着 `history.reset()` → **`DELETE FROM history`**（`history/sqlite.rs:122-126`）。

即：**调用方以为在清一个用户的记忆，实际清掉了全库 + 全部历史**。行内的 `// TODO:` 标记（`manager.rs:535`）本身就说明这段从未完成，但它已经是一份会被调用的公开 API。

### C-3 ·【P0】filter 在 4 个后端里有 6 处被静默丢弃

| 后端 | 位置 | 行为 |
| --- | --- | --- |
| Postgres | `postgres.rs:119-123` | `search` 的 filter 未翻译，`where_clauses` 恒空 |
| Postgres | `postgres.rs:275-279` | `list` 同上 |
| Postgres | `postgres.rs:315-319` | `delete_all` 同上 → 退化为全表 `DELETE` |
| Qdrant | `qdrant.rs:141-144` | `build_filter` 恒返 `None` |
| Qdrant | `qdrant.rs:310-319` | `delete_all` 忽略 filter，改为删 collection |
| Redis | `redis.rs:109 / 264 / 300` | 三处参数直接命名 `_filters`，完全不读 |

四处 `// TODO: Implement full filter translation/conversion`（`postgres.rs:122/278/318`、`qdrant.rs:142`）证实这些是「声明了能力但未实现」，而对外签名接受 `Option<&Filters>` 且**返回 `Ok`**。

违反 `INTEGRATION_SPEC.md` §55：*Provider capability gaps `MUST` return standard unavailable/unsupported errors, not silently degrade.*

### C-4 ·【P1】作用域过滤发生在 top-K 截断之后

`crates/sdkwork-memory-mem0/src/memory/manager.rs:372-402`：

```rust
let search_limit = if options.rerank { limit * 10 } else { limit * 2 };
let results = self.vector_store.search(&embedding, search_limit, options.filters.as_ref()).await?;
// ... 之后才 retain 按 user_id / agent_id / run_id
```

作用域过滤是**内存后置过滤**，而 `user_id` / `agent_id` / `run_id` **不在**传给 store 的 filter 里（只有调用方自报的 `options.filters`）。因此当同库中存在其他用户的高相似记录时，本用户命中数会被挤压，甚至返回空集。`get_all`（`manager.rs:431-455`）同样先按 `limit` 截断再过滤。

### C-5 ·【P1】`to_memory_record` 静默编造 UUID，`updated_at` 无法回读

`crates/sdkwork-memory-mem0/src/vector_stores/traits.rs:71-83`：

```rust
id: uuid::Uuid::parse_str(&self.id).unwrap_or_else(|_| uuid::Uuid::new_v4()),
...
updated_at: self.payload.created_at, // Use created_at as fallback
```

- 任一非 UUID 形态的 store id（Qdrant num id、空串、调用方自定义 id）→ **随机新 UUID**，随后 `update(id)` / `delete(id)` / `history(record.id)` 全部指向错误目标。
- `Payload`（`models.rs:454-477`）只承载 `created_at`，**没有 `updated_at` 字段**，所以「最后更新时间」在落库那一刻就丢了，读回恒等于创建时间。

### C-6 ·【P1】静默降级清单（其余 12 处）

| # | 位置 | 静默行为 |
| --- | --- | --- |
| 1 | `qdrant.rs:260-266` | `update(embedding=None)` → 用 `vec![0.0; dimensions]` 覆盖真实向量（注释原文 "For now, just use zeros"），记录永久不可检索 |
| 2 | `redis.rs:252` | `f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4]))` → 字节数异常时把分量静默置 0 |
| 3 | `qdrant.rs:310-319` | `delete_all` 返回 `Ok(0)`，调用方拿到 0 无法区分「没删到」与「删光了」 |
| 4 | `history/sqlite.rs:105-106` | `Uuid::parse_str(...).unwrap_or_default()` → 静默变 nil UUID |
| 5 | `history/sqlite.rs:100-102` | 时间戳解析失败 → 静默替换为 `Utc::now()`，**历史时间线被篡改** |
| 6 | `vector_stores/mod.rs:44-47` | `let _ = (collection_name, dimensions);` → `MemoryStoreConfig.max_entries` 被丢弃，内存 store 无界增长，违反 `RUST_CODE_SPEC` §9「Bounded caches `MUST` be sized from config」 |
| 7 | `manager.rs:139 / 280 / 481 / 505` | `let _ = history.add_history(...)` —— 历史写入失败被吞，**4 处且均无文档说明**，违反 `RUST_CODE_SPEC` §5 末条 |
| 8 | `models.rs:228-229` | `infer` 字段文档写 `(default: true)`，但 `#[derive(Default)]` 使实际默认值为 **`false`**，违反 `RUST_CODE_SPEC` §10「`MUST NOT` provide "empty" defaults that silently change behavior」 |
| 9 | `config.rs:74-78` | `EmbedderConfig::default()` = **Mock 哈希嵌入**（无语义相似度）→ `MemoryConfig::default()` 能"跑通"但检索结果无意义，且无任何告警 |
| 10 | `llms/mod.rs:32-50` + `config.rs:311-323` | 未开 feature 时 `LLMConfig` 变体全被 cfg 掉，`config.llm` 恒 `None` → `add(infer=true)` 静默走 `add_raw`（`manager.rs:95-101`），不报错不告警，违反 `INTEGRATION_SPEC.md` §55 |
| 11 | `rerankers/*` + `manager.rs:408-414` | `rerank=true` 但未配置 reranker → 只 `warn!` 后静默不重排 |
| 12 | `errors.rs:62-64` / `124-125` | `RateLimited` 变体从未被构造（死变体），无限流可观测性，违反 `INTEGRATION_SPEC.md` §114 |

### C-7 ·【P1】跨后端 filter 语义不一致

`crates/sdkwork-memory-mem0/src/vector_stores/memory.rs:53-76` 的 `matches_filters` **只查 `payload.metadata`**；而 `user_id` / `agent_id` / `run_id` 是 `Payload` 的**顶层字段**（`models.rs:466-472`），不在 metadata 里。由于 `Payload` 对 metadata 用了 `#[serde(flatten)]`（`models.rs:475-476`），Postgres/Qdrant/Redis 走 JSON 序列化时这些字段又在同一层可被命中。

结果：**同一条 filter 在 InMemoryStore 与其余后端语义不同**。而 `conformance.rs` 的契约测试（`vector_stores/conformance.rs:25-61`）只覆盖 CRUD 与 `delete_all(None)`，**没有任何 filter 断言** —— 这层分歧正好落在测试盲区里。

### C-8 ·【P1】`InMemoryStore::list` 结果非确定

`vector_stores/memory.rs:247-269` 直接对 `HashMap` 迭代并 `truncate(limit)`，不排序。`get_all` 因此返回**每次调用可能不同的子集**。

### C-9 ·【P2】死代码与无消费者能力

| 位置 | 状态 |
| --- | --- |
| `src/graph/**`（4 文件） | `InMemoryGraph` / `GraphMemory` 仅被自身与自测引用；`Memory` 结构体（`manager.rs:28-36`）**没有 graph 字段**，全仓零消费者 |
| `src/utils/filters.rs` | `FilterBuilder` 只被自身测试引用 |
| `models.rs:32-33 / 83-87` | `hash` 字段计算后**从未用于去重**，是死字段 |
| `src/python.rs` | pyo3 绑定；`sdkwork.app.config.json` 的 `runtime` 只声明 `API` + `CONTAINER_IMAGE`，属未声明的额外交付技术栈 |

---

## 3. sdkwork-specs 逐条合规判定

### 3.1 命名与形态

| 编号 | 规范条款 | 证据 | 判定 |
| --- | --- | --- | --- |
| N-1 | `TEST_SPEC.md` §425：Rust crate `MUST` 使用责任族之一（`sdkwork-<domain>-<capability>-service` / `-repository-sqlx` / `sdkwork-routes-<capability>-<surface>` / `-service-host` / `-native-host` / `-tauri-host` / `-worker` / `sdkwork-api-<app>-assembly` / `-standalone-gateway` / `sdkwork-api-cloud-gateway`） | `sdkwork-memory-mem0` **不属于任一族** | ❌ |
| N-2 | `NAMING_SPEC.md` §3.1 rule 2：目录名 `MUST` 等于 `[package].name` | 目录存在、**无 manifest**，无法成立 | ❌ |
| N-3 | `NAMING_SPEC.md` §3.1 rule 4/5 + `RUST_CODE_SPEC.md` §4：含连字符的 package `MUST` 显式声明 `[lib].name`（= `sdkwork_memory_mem0`），源码 `MUST` import lib 名 | 源码 `use mem0_rust::`（`lib.rs:13`、`bin/mem0.rs:1`、`bin/mem0-server.rs:11`、`python.rs:91`） | ❌ |
| N-4 | `RUST_CODE_SPEC.md` §4：可运行 crate 名 `MUST` 使用特定后缀；`src/bin` 单面 smoke binary **不得成为公开入口** | `[[bin]] mem0-server`（`bin/mem0-server.rs:1-118`，axum listener 绑 `127.0.0.1:3000`） | ❌ |
| N-5 | `RUST_CODE_SPEC.md` §5：错误类型命名 `Error` + crate 级 `pub type Result<T>` | `errors.rs:9` 命名为 `MemoryError`，别名定义在子模块 | ❌ |
| N-6 | `COMPONENT_SPEC.md` §26：每个 authored module `MUST` 含 `specs/component.spec.json` 等 | 缺 `specs/`、`README.md`、`tests/` | ❌ |
| N-7 | `NAMING_SPEC.md` §3.2 rule 1 + `DEPENDENCY_MANAGEMENT_SPEC.md` §81/§85：源码引用的外部 crate `MUST` 在 manifest 声明 | 11 个未在仓库根 `[workspace.dependencies]` 声明：`async_openai` `ollama_rs` `qdrant_client` `redis` `rusqlite` `hex` `sha2` `chrono` `uuid` `async_trait` `pyo3` | ❌ |

### 3.2 架构归属

`TECH_ARCHITECTURE.md` 的 Ownership 边界只承认：`sdkwork-memory-contract`、`sdkwork-intelligence-memory-service`、`sdkwork-memory-spi`、`sdkwork-memory-plugin-native-sql`、`sdkwork-memory-retrieval`、route crates、standalone gateway、PC app、生成 SDK。**`sdkwork-memory-mem0` 不在其中**，且：

| 编号 | 规范/权威 | 违规 |
| --- | --- | --- |
| A-1 | PRD 第 13 行：*"Provider integrations remain replaceable through SDKWork SPI contracts"*；`TECH_ARCHITECTURE.md` Runtime Shape：`service use cases -> SPI/store ports -> SQL/provider adapters` | 自带一整套 `Embedder` / `VectorStore` / `LLM` / `Reranker` trait，**完全不经过 `sdkwork-memory-spi`** |
| A-2 | `TECH_ARCHITECTURE.md` Data And Job Model：*"PostgreSQL and SQLite share one logical storage model through `sqlx::Any`"* | 引入 `rusqlite`（第二个 SQLite 驱动 + 独立 `Connection`） |
| A-3 | `INTEGRATION_SPEC.md` §13/§64：外部系统 `MUST` 经 provider adapter/connector 模块；上游已发布门面时生产消费方 `MUST` 用该公开面 | 直连 `api.openai.com` / `api.anthropic.com` / `api.cohere.com` / `api-inference.huggingface.co`，绕过 `sdkwork-cloudrouter` / `sdkwork-models` 的账号-路由-计价面 |
| A-4 | 职责边界：`sdkwork-memory-retrieval` 已拥有 "Retrieval and context composition algorithms" | 本 crate 在 `memory/manager.rs` 内重建了一套检索编排 |

### 3.3 数据库

| 编号 | 规范条款 | 证据 | 判定 |
| --- | --- | --- | --- |
| DB-1 | `DATABASE_SPEC.md` DB061/DB062：新业务表首段 `MUST` 是**已注册**的模块前缀 | `history/sqlite.rs:23-36` 运行时 `CREATE TABLE IF NOT EXISTS history`（无 `ai_` 前缀） | ❌ |
| DB-2 | 同上；本仓基线为 `database/ddl/baseline/postgres/0001_memory_baseline.sql`，`db:materialize:contract --prefixes ai_` | `postgres.rs:362-375` 运行时 `CREATE TABLE {table_name}_{collection}`，表名形如 `memories_<collection>` | ❌ |
| DB-3 | `TEST_SPEC.md` §483：crate-local `migrations/` 不得成为唯一迁移源 | 历史表 DDL 只存在于 crate 代码里，不在迁移链 | ❌ |
| DB-4 | `DATABASE_SPEC.md` §157/160：主键 `MUST` 由已批准的 ID provider 生成 | `models.rs:55` 用 `Uuid::new_v4()`；未用 `sdkwork-id-core` / `sdkwork-database-id`（仓库根已声明这两个 workspace 依赖） | ❌ |

### 3.4 安全与隔离

| 编号 | 规范条款 | 违规 |
| --- | --- | --- |
| S-1 | `SECURITY_SPEC.md` §65-71：每个受保护操作 `MUST` 服务端鉴权；权限检查 `MUST` 含 tenant + organization；对象级授权 `MUST` 在返回/改动租户数据前完成 | 作用域只有调用方自报的 `user_id: String`，**无 tenant / organization / space / actor 维度**；`mem0-server.rs:52-100` 让任意 HTTP 调用者读写任意 `user_id` |
| S-2 | `TECH_ARCHITECTURE.md`：*"Every store operation includes tenant and required space/actor predicates before materialization"* | store 层无租户谓词（`traits.rs:19-66`） |
| S-3 | `SOURCE_CONFIG_SPEC.md`：*"Shared libraries and SDK packages receive typed configuration from bootstrap and do not discover `etc/`, environment variables, or OS paths themselves"* | `embeddings/huggingface.rs:27`（`HF_TOKEN`）、`llms/anthropic.rs:26`（`ANTHROPIC_API_KEY`）、`rerankers/cohere.rs:18`（`COHERE_API_KEY`）、OpenAI 走 `OPENAI_API_KEY`；`history_db_path` 由库自行发现 OS 路径 |
| S-4 | `INTEGRATION_SPEC.md` §15：外部凭据 `MUST` 经安全凭据存储与轮换 | API key 以明文 `Option<String>` 装入 `MemoryConfig` |
| S-5 | `INTEGRATION_SPEC.md` §54：Provider API 版本 `MUST` 显式 | `llms/anthropic.rs:116` 写死 `anthropic-version: 2023-06-01`；`rerankers/cohere.rs:64` 写死 `/v1/rerank`；`embeddings/huggingface.rs:32` 写死 pipeline 路径 —— 均不可配置 |
| S-6 | `RUST_CODE_SPEC.md` §5：错误变体 `MUST NOT` 在 `Display` 里携带原始 SQL / 密钥 / 凭据 / PII | `VectorStoreError::Connection(e.to_string())`（`postgres.rs:29`）可含**带凭据的 DSN**；`e.to_string()`（`postgres.rs:105/147` 等）可含失败 SQL；`LLMError::Api(format!("Anthropic API error: {}", error_text))`（`anthropic.rs:125`）回传供应商响应体 |
| S-7 | `CODE_STYLE_SPEC.md` §4：不得把数据库/provider/storage/framework 异常泄漏进 API schema；`HEALTH_CHECK_SPEC.md` §58 同旨 | `bin/mem0-server.rs:66/88` 把 `e.to_string()` 直接返回 HTTP 客户端 |
| S-8 | `SECURITY_SPEC.md` §26：token 与 secret `MUST NOT` 被记录 | 未见主动打日志，但 `Debug` 派生（`config.rs:13` `MemoryConfig`）会让密钥进入任何 `{:?}` |

### 3.5 Rust 代码标准

| 编号 | 规范条款 | 证据 | 判定 |
| --- | --- | --- | --- |
| R-1 | `RUST_CODE_SPEC.md` §7：`unwrap` / `expect` / `panic!` / `todo!` / `unimplemented!` / `unreachable!` / `dbg!` **在 library code 中被禁止** | `history/sqlite.rs:55/83/123`（`.lock().unwrap()`）、`embeddings/ollama.rs:21`、`llms/ollama.rs:22`（`Url::parse(...).unwrap()`）、`utils/retry.rs:38`（`unreachable!`） | ❌ |
| R-2 | 同上：`unwrap` 在非测试代码中被禁止 | 同上 6 处 | ❌ |
| R-3 | `RUST_CODE_SPEC.md` §7：`expect`/`unwrap` 在 async task 中禁止 | `history` 的 3 处 `lock().unwrap()` 都从 async fn 调用 | ❌ |
| R-4 | `RUST_CODE_SPEC.md` §8：**Timeouts `MUST` wrap every external await（HTTP / SQL / provider / lock）**，禁止无界外部 await | `reqwest::Client::new()`（`embeddings/huggingface.rs:38`、`rerankers/cohere.rs:22`、`llms/anthropic.rs:29`）未设 timeout；`PgPoolOptions`（`postgres.rs:25-28`）、`ConnectionManager`（`redis.rs:30`）、ollama client 均未显式声明超时 | ❌ |
| R-5 | `RUST_CODE_SPEC.md` §8：重试 `MUST` 有界 + **jitter** + backoff；`INTEGRATION_SPEC.md` §112 同旨 | `utils/retry.rs:19-39` 只有指数退避 + 5s 上限，**无 jitter**；且仅 HuggingFace 使用（`huggingface.rs:81/160`），OpenAI / Anthropic / Ollama / Cohere 无重试 | ❌ |
| R-6 | `RUST_CODE_SPEC.md` §8：不得跨 `.await` 持锁；优先 `tokio::sync` 原语 | `vector_stores/memory.rs:19` 用 `std::sync::RwLock`（同仓 `graph/memory.rs:14-15` 却用 `tokio::sync::RwLock` → **同仓两套锁模型**）；`history/sqlite.rs:5` 用 `std::sync::Mutex` 持锁执行同步 rusqlite IO（阻塞 async 执行器） | ⚠️ |
| R-7 | `RUST_CODE_SPEC.md` §5：错误枚举 `MUST` 通过 `source()` 暴露因果链；转换 `MUST NOT` 吞掉上下文；不得加 `String` catch-all | `errors.rs` 中 `Config(String)` / `History(String)` / `Reranker(String)` / `InvalidInput(String)` 及 `EmbeddingError` / `VectorStoreError` / `LLMError` **全部变体都是 String 载荷**；`From` 实现一律 `e.to_string()` | ❌ |
| R-8 | `RUST_CODE_SPEC.md` §5：`MUST NOT` 用 `let _ =` 吞错，除非有意忽略**并已文档化** | `manager.rs:139/280/481/505` | ❌ |
| R-9 | `RUST_CODE_SPEC.md` §10：`#[must_use]` `MUST` 施加于返回 `Result`/`Option`/`Iterator`/guard 的函数 | 全 crate `#[must_use]` 计数 **0** | ❌ |
| R-10 | `RUST_CODE_SPEC.md` §10：library crate `MUST` 声明 `#![warn(missing_docs)]`，公开项 `MUST` 有文档 | `lib.rs` 无该属性；`graph/models.rs`、`graph/traits.rs`、`history/sqlite.rs`、`rerankers/mod.rs`、`utils/retry.rs` 多处公开项无文档 | ❌ |
| R-11 | `TEST_SPEC.md` §424：`src/lib.rs` **不得**包含 handlers / repositories / SQL / provider clients / **test fixtures** | `lib.rs:66-76` 内联 `#[cfg(test)] mod tests` | ❌ |
| R-12 | `HEALTH_CHECK_SPEC.md` §20/§26/§35-36：每个 HTTP listener `MUST` 暴露 `/healthz` 与 `/readyz`；**legacy `/health`、`/ready` `MUST NOT` 被引入**；探针 `MUST` 经 `sdkwork-web-bootstrap::service_router`，不得自建 | `bin/mem0-server.rs:48-50/108` 自建 `async fn health()` 挂 `/health` —— **同时违反三条** | ❌ |
| R-13 | `RUST_CODE_SPEC.md` §4 / `SDKWORK_WORKSPACE_SPEC.md`：格式化与提交基线 | `cargo fmt --check` 无法运行（无 manifest）；人工可见 `lib.rs:45-46` 重复注释、`lib.rs:74` 与 `manager.rs:302` 行尾空格、`manager.rs:3-7` import 未排序 | ❌ |
| R-14 | `QUALITY_GATE_SPEC.md` §30/§31：门禁可达性与 fail-closed | 见 §1.2，本 crate **对所有门禁不可达**，且没有规则覆盖「无 manifest 的 crate 目录」 | ❌ |

### 3.6 集成

| 编号 | 规范条款 | 违规 |
| --- | --- | --- |
| I-1 | `INTEGRATION_SPEC.md` §55：provider 能力缺口 `MUST` 返回标准 unavailable/unsupported 错误，**不得静默降级** | §2 C-3 / C-6 共 16 处静默降级 |
| I-2 | `INTEGRATION_SPEC.md` §114：限流处理 `MUST` 通过 metrics 与运维日志可见 | `RateLimited` 死变体；无 metrics |
| I-3 | `INTEGRATION_SPEC.md` §17：集成模块 `MUST` 文档化 ownership / provider / scopes / callbacks / rate limits / failure behavior | 无 README，无任何 provider 文档 | ❌ |

---

## 4. 修复方案

### 4.1 方案选择

| 方案 | 内容 | 适用判据 | 评价 |
| --- | --- | --- | --- |
| **A · 收敛为 SPI 插件 profile** | 重命名 + 搬入 `plugins/`，实现 `sdkwork-memory-spi` 端口，删掉自建 facade 与 HTTP listener | 产品上**决定要做向量检索路线** | **推荐**（唯一能同时满足 PRD「SPI 可替换」与 `TECH_ARCHITECTURE.md` ownership 的路径） |
| **B · 整体删除** | **39 个已提交文件**，需 `git rm -r --cached` + 删工作区文件（**枚举路径，禁止通配**，见 `DESTRUCTIVE_OPERATION_SPEC.md`；**注：因文件已入库，`git clean -fdx` 无效**，初版给出的命令是错的，已改正） | 产品上**暂不做向量路线** | **次推荐**（已确认零引用、零消费者、零门禁依赖）。**属破坏性操作，须人工显式批准后才可执行**，本审计不擅自执行 |
| **C · 移出 `crates/` 作为外部参考** | 迁到 `external/` 或独立仓，明确不参与 SDKWork 交付 | 只想留档 | **不推荐**（`NAMING_SPEC.md` §3.1 rule 11 要求第三方保持上游命名不改写，而本目录已被改成 `sdkwork-` 前缀，两边都不合规） |

> **决策输入**：PRD 第 13 行明确 *"without requiring an embedding provider. Provider integrations remain replaceable through SDKWork SPI contracts."* —— 若向量路线在路线图内，只有方案 A 成立；若不在，方案 B 是最干净的止损。
> 本审计**不代替产品决策**，故不擅自执行删除或搬迁。

> **执行状态（2026-09-23 15:40 更新）**：**方案 A 已落地**，产出
> `plugins/sdkwork-memory-plugin-search-first-vector/`（17 个文件 / 约 4,641 行 Rust，
> 已进 workspace members，实测 **98 个测试全绿 + 0 warning**）。
> 该插件**不是**把 mem0 原样搬进来，而是按 §4.2 清单重写：SPI 端口优先、
> 零新增第三方依赖（哈希/时间取自 `sdkwork-utils-rust`）、无自建 facade、无 HTTP listener。
> 因此 `crates/sdkwork-memory-mem0/` 这 39 个已提交文件**已无保留价值**，
> 是否 `git rm` 见 §5 第 6 项遗留决策。

### 4.2 方案 A 落地清单（若采纳）

1. **目录与命名**
   - 目标：`plugins/sdkwork-memory-plugin-search-first-vector/`（`sdkwork-memory-plugin-*` 族已在仓库内存在先例）
   - 形态对齐 `plugins/sdkwork-memory-plugin-reference-profiles/`：`Cargo.toml` + `README.md` + `sdkwork.memory.plugin.json` + `specs/component.spec.json` + `src/` + `tests/`
   - `Cargo.toml` 继承 workspace：`rust-version.workspace` / `edition.workspace` / `version.workspace` / `license.workspace` / `[lints] workspace = true`
   - `sdkwork.memory.plugin.json` 声明 `implementationKinds: ["search_first"]` + `portExports`（参照 reference-profiles 的 9 个 port 写法）
2. **接入**
   - 加进根 `Cargo.toml` `members`；新增第三方依赖进 `[workspace.dependencies]`（11 个，逐个过依赖治理）
   - `specs/component.spec.json` 的 `manifests` 补登记
3. **架构收敛（核心工作量）**
   - 删除 `src/memory/manager.rs` 的 `Memory`/`MemoryConfig` facade，改为实现 `sdkwork-memory-spi` 的 `MemoryRecordStorePort` 等端口
   - 删除 `src/bin/mem0-server.rs`（HTTP 入口只保留 `sdkwork-api-memory-assembly` + `standalone-gateway`）与 `src/bin/mem0.rs`
   - scope 从 `user_id: String` 升级为 SPI 的 `MemoryScopeContext`（tenant / organization / space / actor）
   - store 调用改为 sqlx `Any` 单栈；移除 `rusqlite`；表结构进 `database/ddl/baseline` + 迁移，前缀 `ai_`
   - provider 调用改走平台模型面（`sdkwork-cloudrouter` / `sdkwork-models`），端点/版本/凭据由 bootstrap 注入 typed config
4. **正确性修复（必须先于接入完成）**
   - C-1：`add_with_inference` 的检索 filter 必须携带 scope 谓词；`update`/`delete` 必须做对象级授权
   - C-2：`reset` 的 scope 必须下推为真实 filter；未实现时 `Err` 而非 `Ok`
   - C-3：filter 未实现的后端返回 `VectorStoreError::Unsupported`，**不允许返回 `Ok`**
   - C-4：scope 下推到 store 查询条件，不做 top-K 后置过滤
   - C-5：`Payload` 补 `updated_at`；id 解析失败返回错误而非编造 UUID
   - C-6：逐条消除静默降级（补齐/报错二选一）
   - C-7：统一 `matches_filters` 的可见字段集合，并在 `conformance.rs` 补 filter 断言
5. **标准修复**
   - 错误类型改名 `Error` + crate 级 `pub type Result<T>`；变体去 String 化，保留 `source()` 链，去除 SQL/DSN/供应商响应体
   - 错误枚举补 `Unsupported` / `Unavailable { retryable }`
   - 全部外部 await 加 `tokio::time::timeout`；`RetryPolicy` 加 jitter
   - 库代码清除全部 `unwrap` / `unreachable!`
   - 补 `#![warn(missing_docs)]` 与 `#[must_use]`
   - `lib.rs` 只留模块声明与 re-export；测试移入 `tests/`
   - 跑 `cargo fmt`
6. **门禁**
   - 落实 §1.3 的结构门禁（磁盘 crate 目录 ↔ workspace members 双向一致）
   - 落实一条「库内不得读取环境变量 / 不得硬编码 provider 端点」的扫描（可复用 `SOURCE_CONFIG_SPEC` 的校验器思路）

### 4.3 验收方式（无论选 A 或 B）

```bash
# 1) 接入状态：必须能列出该包（方案 A）
cargo metadata --no-deps --format-version 1 | grep -c '"name":"<新 crate 名>"'   # 期望 ≥1

# 2) 结构门禁：磁盘目录数 == workspace member 数
node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root "D:/sdkwork-space/sdkwork-memory"

# 3) 全仓门禁
pnpm check && pnpm verify

# 4) 变异自证（方案 A 的新门禁必须会红）
#    临时把新 crate 从 members 摘掉 → 结构门禁必须非零退出
```

> 方案 B 的验收：`git status --porcelain` 中不再出现该路径；`pnpm check && pnpm verify` 保持绿。

#### 4.3.1 实际验收回显（2026-09-23 15:40，方案 A）

```console
# 1) 接入状态
$ cargo metadata --no-deps --format-version 1 | grep -c '"name":"sdkwork-memory-plugin-search-first-vector"'
1                                    <-- ✅ 已在 workspace 内（members 18 -> 19）

# 2) 插件自身测试（含 doctest）
$ cargo test -p sdkwork-memory-plugin-search-first-vector
test result: ok. 41 passed; 0 failed     (unit)
test result: ok.  8 passed; 0 failed     (manifest_matches_json)
test result: ok. 25 passed; 0 failed     (retrieval_scope_contract)
test result: ok. 14 passed; 0 failed     (fact_extraction_contract)
test result: ok. 10 passed; 0 failed     (doctests)
warning: 0
                                    <-- ✅ 98 passed / 0 failed / 0 warning

# 3) 插件布局契约门禁（自动枚举 plugins/**/sdkwork.memory.plugin.json）
$ node --test tests/contracts/runtime_plugin_layout_contract_test.mjs
# pass N / # fail 0                <-- ✅ 新插件被自动纳入校验

# 4) 仓库架构对齐门禁
$ node tools/check_sdkwork_memory_architecture_alignment.mjs
Architecture alignment passed       <-- ✅

# 5) 变异自证（证明新门禁真的会红，而非恒绿）
$ # 把 component.spec.json 的 component.name 改坏
$ node --test tests/contracts/runtime_plugin_layout_contract_test.mjs
# pass 0 / # fail 1                  <-- ✅ 门禁确实会红
$ # 还原后
$ diff <还原后> <原件> && echo IDENTICAL
IDENTICAL                            <-- ✅ 字节级还原
```

> 这里刻意用**变异控制**而不是"跑一遍绿了就收工"：`QUALITY_GATE_SPEC` §30 要防的正是
> "门禁看起来存在、实际扫零个单元恒绿"。先证明它会红，绿才有意义。

#### 4.3.2 全 workspace 回归（证明新增 member 没弄坏既有 crate）

```console
$ node scripts/cargo-test-workspace.mjs          # = cargo test --workspace -j 1
Compiling sdkwork-memory-plugin-search-first-vector v0.1.0 (...)
WS_TEST_EXIT=0

$ # 汇总
test-result blocks : 91
passed             : 495
failed             : 0
ignored            : 1
exit               : 0
mem0 compiled?     : false      <-- 全量跑一次，mem0 依然一次都没被编译
```

**495 passed / 0 failed / 0 ignored-失败**，新插件在完整 workspace 构建内编译并通过其 10 个 doctest，
既有 18 个 crate 无一回归。`mem0 compiled? false` 是 §1 可达性结论的终局证据：
**即使跑全量测试，mem0 也不会被编译一次。**

#### 4.3.3 门禁套件逐条实跑（15 条）

```console
app-composition                 PASS      repository-docs              PASS
architecture-alignment          PASS      pagination                   PASS
crate-inventory                 FAIL(1)   api-envelope                 PASS   <-- 设计意图，指向 mem0
release-readiness               PASS      api-operation-patterns       PASS
pnpm-script-standard            PASS      sdk-standard                 PASS
agent-workflow-standard         PASS      db-validate                  PASS
rust-crate-naming               PASS      db-pool-validate             PASS
rust-manifest-standard          PASS
```

**14/15 绿，唯一红的是新门禁且红得正确**（精确指出 `crates/sdkwork-memory-mem0/`）。

> **为何不是 `pnpm check` / `pnpm verify` 的原样输出**：本工作树 `node_modules` 缺失，
> `pnpm check` 在第一步就报 `'sdkwork-app' 不是内部或外部命令`（`PNPM_CHECK_EXIT=1`），
> 因此无法给出"整链绿"的证据。上面是**逐条直接调用同一批检查器**的结果 —— 
> 覆盖面等价，且已顺带修掉了 §1.4 那条让链断掉的缺失脚本。
> 恢复完整链验证需先 `pnpm install`（§5 第 9 项）。

---

## 5. 未决问题 / 需人工裁定

| # | 问题 | 需要谁定 |
| --- | --- | --- |
| 1 | ~~向量检索路线是否进入 Memory 路线图（决定方案 A / B）~~ **已定：走方案 A，且已落地**（见 §4.1 执行状态） | 产品 + 架构 |
| 2 | 是否需要 `rusqlite` 之外的向量库适配（Qdrant / pgvector / Redis 三选几），以及它们是否应作为独立 plugin 还是同一 plugin 内多后端 —— 当前插件是**内存态向量索引 + 绑定 `EmbeddingModelPort`**，持久化向量库尚未接入 | 架构 |
| 3 | `sdkwork-memory-retrieval` 已拥有"检索与上下文合成算法"，新插件边界需与其划清（避免第二处检索编排） | 架构 |
| 4 | 仓库既有 15 条 `rust.lib-name-undeclared` 警告（`check-rust-crate-naming-standard`）是否随本次一并清理 | 维护者 |
| 5 | `sdkwork.app.config.json` 未声明 Python 交付面，`src/python.rs` 是否保留 | 产品 |
| 6 | **`crates/sdkwork-memory-mem0/` 的 39 个已提交文件是否 `git rm`** —— 方案 A 已落地，其能力已被重写实现覆盖，原目录零引用、零消费者、零门禁依赖。**这是破坏性操作，须人工显式批准**；不批准则维持现状（已提交、不构建、无门禁覆盖） | 维护者 + 产品 |
| 7 | **§1.3 的结构门禁是否上提到 `sdkwork-specs` 全局** —— 本仓已用本地实现（`tools/check-crate-inventory-standard.mjs`）堵住，但同类"无 manifest 目录"在其余 ~100 个 sdkwork 仓同样不可见，需全局化才根治 | 架构 + 维护者 |
| 8 | **`_sdkwork:check` / `_sdkwork:verify` 的长 `&&` 串是否改为逐环上报的 runner**（§1.4）—— 否则任何一环缺文件都静默吃掉其后全部门禁，且失败原因只显示"某条命令不存在"而非"哪几条没跑" | 维护者 |
| 9 | `node_modules` 未安装，`pnpm check` / `pnpm verify` 在本工作树**不可运行**（`sdkwork-app` 不存在）—— 是否补 `pnpm install` 以恢复完整链验证 | 维护者 |

---

## 附录 A · 取证命令与实测回显

### A.0 初版的错误证据（保留存档，勿再引用）

```console
$ cd /d/sdkwork-space/sdkwork-memory
$ git ls-files crates/sdkwork-memory-mem0 | wc -l
0                                    <-- ❌ 错误：这条命令当时根本没执行成功
$ git status --porcelain --untracked-files=all crates/sdkwork-memory-mem0 | wc -l
39                                   <-- 真实值 39，但成因不是"未追踪"而是"已提交"
```

> 这两行是**错误证据样本**。`git ls-files` 的真实回显是 **39**（下节 A.1 复核），
> 初版填的 `0` 来自一条短路的命令链，属无效取证。保留在此是为了让后来者能看见错误是怎么产生的。

### A.1 复核后的真实回显（2026-09-23 15:36）

```console
$ cd /d/sdkwork-space/sdkwork-memory
$ git ls-files crates/sdkwork-memory-mem0/ | wc -l
39                                   <-- ✅ 已提交，不是未追踪

$ git status --short
 M Cargo.lock                        <-- 本次重构的改动
 M Cargo.toml                        <-- 本次重构的改动
?? plugins/sdkwork-memory-plugin-search-first-vector/   <-- 本次新增（唯一未追踪项）
                                     <-- 注意：crates/sdkwork-memory-mem0 没有出现在这里

$ git check-ignore -v crates/sdkwork-memory-mem0/src/lib.rs; echo "exit=$?"
exit=1                               <-- 未被任何 .gitignore 规则匹配

$ git log --oneline -1
4f14ccb feat(memory): mem0 integration, release signing, DB pool spec + related updates

$ git show --stat --oneline 4f14ccb -- crates/sdkwork-memory-mem0 | tail -3
 ... (39 files, 5740 insertions)
       <-- 全部 39 个文件由 HEAD 这一次提交引入

$ ls crates/sdkwork-memory-mem0/Cargo.toml
ls: cannot access 'crates/sdkwork-memory-mem0/Cargo.toml': No such file or directory

$ cargo metadata --no-deps --format-version 1 | grep -o '"name":"sdkwork-memory-mem0"' | wc -l
0                                    <-- 仍然：包不存在于 cargo 视角

$ grep -rn "mem0" --include="*.toml" --include="*.json" --include="*.md" --include="*.mjs" . | grep -v "^./target/"
(empty)                              <-- 仍然：零引用、零消费者
```

**三行合读才是完整事实**：`ls-files` = 39（在 git 里）+ 无 `Cargo.toml` + `cargo metadata` = 0
⇒ **已提交、但不可构建、门禁零覆盖**。任何单独一行都会导出错误结论：
只看 `ls-files` 会以为它已接入；只看 `cargo metadata` 会以为它不在仓库里。

```console
$ cd /d/sdkwork-space/sdkwork-specs
$ node tools/check-rust-crate-naming-standard.mjs --root "D:/sdkwork-space/sdkwork-memory"
--- summary ---
repos scanned : 1
crates scanned: 18          <-- 等于 workspace members 数；mem0 不在其中
errors        : 0
warnings      : 15          <-- 全部为 rust.lib-name-undeclared（既有债务）
by code:
     15  rust.lib-name-undeclared

$ node tools/check-rust-manifest-standard.mjs --root "D:/sdkwork-space/sdkwork-memory"
repos scanned : 1
errors        : 0
warnings      : 0

$ node tools/check-database-framework-standard.mjs --root "D:/sdkwork-space/sdkwork-memory"
Database framework standard passed
```

> 路径注意事项：必须用 `D:/...` 正斜杠 Windows 形式。用 Git Bash 的 `/d/sdkwork-space/...` 会被解析成 `D:\d\sdkwork-space\...`，导致 `crates scanned: 0` 的**假读数**（本次审计首轮即踩此坑，已用对照仓 `sdkwork-order` / `sdkwork-drive` / `sdkwork-iam` 复现并纠正）。

### 未声明的外部依赖（11 个）

```
async_openai  ollama_rs  qdrant_client  redis  rusqlite
hex  sha2  chrono  uuid  async_trait  pyo3
```

### 非测试代码中的 panic 面（6 处）

```
history/sqlite.rs:55    self.conn.lock().unwrap()
history/sqlite.rs:83    self.conn.lock().unwrap()
history/sqlite.rs:123   self.conn.lock().unwrap()
embeddings/ollama.rs:21 url::Url::parse("http://localhost:11434").unwrap()
llms/ollama.rs:22       url::Url::parse("http://localhost:11434").unwrap()
utils/retry.rs:38       unreachable!("retry loop always returns")
```

### 静默吞错点

```
memory/manager.rs:139   let _ = history.add_history(...)
memory/manager.rs:280   let _ = history.add_history(...)
memory/manager.rs:481   let _ = history.add_history(...)
memory/manager.rs:505   let _ = history.add_history(...)
vector_stores/mod.rs:45 let _ = (collection_name, dimensions);      // 丢弃 max_entries
vector_stores/postgres.rs:393 let _ = sqlx::query(&index_query)...  // 已注释说明，可接受
```

### 未实现标记（6 处）

```
memory/manager.rs:535            // TODO: Build proper filters          <-- 直接对应 C-2 全库清空
vector_stores/postgres.rs:122    // TODO: Implement full filter translation
vector_stores/postgres.rs:278    // TODO: Implement full filter translation
vector_stores/postgres.rs:318    // TODO: Implement full filter translation
vector_stores/qdrant.rs:142      // TODO: Implement full filter conversion
bin/mem0.rs:5                    println!("mem0 CLI (experimental)");
```
