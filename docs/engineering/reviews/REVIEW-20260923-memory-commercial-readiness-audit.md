# SDKWork Memory 商业化就绪审计

Status: review record (审计结论 + 修复台账)

Owner: SDKWork Memory maintainers

Reviewed: 2026-09-23

Revised: 2026-09-23（同日两轮：首轮为审计，次轮为逐项修复与复验；差异见 §1）

Scope: `D:\sdkwork-space\sdkwork-memory` 全仓（Rust 服务 + SPI/插件 + 三份权威 OpenAPI + 生成 SDK + PC 客户端 + 数据库契约 + 部署资产）

Baselines: `docs/product/prd/PRD.md`、`docs/architecture/tech/TECH_ARCHITECTURE.md`、`D:\sdkwork-space\sdkwork-specs`（PAGINATION_SPEC / API_SPEC / DATABASE_SPEC / DATABASE_FRAMEWORK_SPEC / ENVIRONMENT_SPEC / SECURITY_SPEC / PRIVACY_SPEC / PERFORMANCE_SPEC）

Method: 静态通读 + 全仓检索 + **实跑仓库门禁、契约测试与工作空间测试**（附录 A）。所有结论均给出 `文件:行` 证据；无法实测的标注「疑似」。**每项修复都用变异测试自证门禁会红**，再还原并校验字节级还原。

---

## 0. 结论摘要

**判定：当前仍不具备商业化生产落地能力**——与仓库自述一致（PRD §Release State：`internal release candidate, not an active production release`）。

但**整条「解阻断」路径已走完**：首轮审计列出的 5 项 P0 阻断全部关闭，发布就绪门禁由红转绿，全量测试由 390 增至 **397 通过 / 0 失败**（新增 7 个测试全部针对本轮修复）。当前距商用底线的差距已从「硬阻断 + 门禁假绿」转为「运营证据与容量验证缺失」，属于**可排期补齐**而非**架构返工**。

| 级别 | 首轮 | 本轮后 | 说明 |
| --- | --- | --- | --- |
| **P0 阻断发布** | 5 | **0** | 全部关闭；其中 2 项的原始处方被本轮证据推翻（见 §2 P0-2 / P0-3） |
| **P1 严重** | 9 | **4 关闭、1 部分、4 未动** | 部署配置、停机、生成物入库、无界请求已修；重试/死信、明文 HTTP、索引、跨租户测试仍开 |
| **P2 一般** | 36 | 33 未动 | 首轮清点基本有效；其中 3 项经复核**不是缺陷**（§1.3） |

**首轮根因已消除**：`sdkwork-specs` 的规范校验器过去**全部通过**却没有覆盖「文档化 query 参数名 ↔ Rust 反序列化字段名」这一层。本轮在该层新增门禁 `check:api-operation-patterns` 的 query 词汇分类（`sdkwork-specs/tools/lib/api-operation-patterns.mjs`），并在 workspace 全量跑出 **8 个兄弟仓**存在同类违规——即首轮「门禁假绿」的判断已被独立复现，且不止 Memory 一仓。

---

## 1. 修订记录

### 1.1 本轮关闭

| 项 | 首轮判定 | 本轮结论 | 关键修复位置 |
| --- | --- | --- | --- |
| P0-1 | 7 个列表操作过滤参数契约↔实现互斥，官方 SDK 必然 400 | **已关闭** | 三份权威 OpenAPI 的 query 参数已全部 `lower_snake_case`；生成 SDK 线格式 `space_id`、TS 属性 `spaceId`；新增 query 词汇门禁 |
| P0-2 | SQLite 生产 schema 来自测试夹具，双栈权威不同源 | **已关闭，且原始处方被推翻** | parity 门禁重写为按引擎基线断言；数据库宿主新增 `ensure_pool_engine_is_declared` 失败关闭。**不再是缺陷**，见 §1.3 |
| P0-3 | SQLite 强制单连接，与「大规模集群高并发」冲突 | **已关闭，且处方venue纠正** | `reject_sqlite_server_role_engine` 在三条服务端入口统一 fail-fast；档位声明位置是 `database/contract/schema.yaml`，不是 `component.spec.json` |
| P0-4 | 发布就绪门禁自身为红（3 errors） | **已关闭** | `specs/process-database-pool.spec.json` 落地；README 跨 await 持锁的全局 IAM 池已移除；`check:release-readiness` 现为 `passed (candidate mode)` |
| P0-5 | PC 客户端 `prebuild` 引用不存在的脚本 | **已关闭** | `apps/sdkwork-memory-pc/scripts/materialize-runtime-env.mjs` 落地并委托共享实现；Vite 改用 `createBrowserRuntimeEnvVitePlugin`；新增「脚本引用必须存在」门禁 |
| P1-1 | k8s `SDKWORK_DATABASE_URL` 同名 3 次 | **已关闭** | 收敛为单一统一身份；退役的模块级预算键改为进程级 `SDKWORK_DATABASE_MAX_CONNECTIONS` |
| P1-4 | 优雅停机仅 3 秒盲等 | **已关闭** | 停机句柄交回宿主（`shutdown` + 有界 `drain`）；顺序改为 HTTP 先排空；5 个契约测试 |
| P1-6 | 被声明「never committed」的生成物已入库 | **已关闭** | `sdks/**/generated/server-openapi/src/**/*.js`、`*.d.ts` 追踪数均降为 **0**；`src/**/*.js` 为 **0** |
| P1-7 | forget `memory_ids` 无上限、`purged_events` 恒 0 | **已关闭（后半为误判）** | 上限已加（契约 `maxItems` + 运行时校验 + 端到端测试）；`purged_events: 0` 经复核是**结构性正确**，见 §1.3 |

### 1.2 本轮部分处理

| 项 | 状态 | 已做 / 未做 |
| --- | --- | --- |
| P1-2 | **部分** | 已做：准入上限与进程池预算对齐（256→64，池 32，2 副本 ⇒ 64，满足「并发布 ≤ 池 × 2」），并显式声明 `SDKWORK_DATABASE_ACQUIRE_TIMEOUT`。未做：外层独立 `TimeoutLayer` 兜底、worker 与请求路径的池配额隔离 |

### 1.3 首轮判定被推翻的三项（明确更正）

**（1）P0-2「SQLite 权威缺失」不是缺陷，原处方会让仓库自家门禁变红。**

首轮主张把 `tests/fixtures/database/sqlite/migrations/*` 迁到 `database/ddl/baseline/sqlite/`、并给 `engines` 增列 `sqlite`。该主张与权威规范直接冲突：

- `DATABASE_FRAMEWORK_SPEC.md` §344/§421：`authoritative-server` 根**必须恰好声明一个引擎**（此处 `["postgres"]`）；SQLite 方言由 `client-local` 插件组合，**在 `tests/fixtures/database/sqlite/` 下演练，绝不在 `database/` 下**。
- 本仓两道门禁**主动断言**该路径不得存在：`tools/check_sdkwork_memory_architecture_alignment.mjs:234-245`、`tests/contracts/memory_database_migration_parity_test.mjs:138-145`。执行首轮处方会同时打红门禁与规范。

首轮的真实缺陷是**另外两条**，均已修复：parity 门禁自身因 `readdirSync("database/migrations/sqlite")` 抛 ENOENT 而**从未真正校验过双引擎一致**（已重写为按引擎基线断言，见 `memory_database_migration_parity_test.mjs`）；以及数据库宿主对**未声明引擎的池失败开放**，服务器指向 SQLite 会「引导成功」到一个空 schema（已由 `ensure_pool_engine_is_declared` 改为失败关闭）。

**（2）P0-3 的档位声明 venue 不对。**

首轮要求「写入 `specs/component.spec.json`」。`COMPONENT_SPEC.md` §3 的 `component.spec.json` schema **没有任何引擎/部署档位字段**，注入自定义键即为规范外形态；而规范规定的声明位置是数据库契约本身——`database/contract/schema.yaml` 已声明 `database_role: authoritative-server` 与 `engines: [postgres]`，`database/database.manifest.json` 同。fail-fast 实现与集成测试均已存在（`reject_sqlite_server_role_engine`，由 `api_server_smoke_test.rs:207` 断言）。

**（3）P1-7 的「`purged_events` 恒 0 故统计不真实」不成立。**

`ai_event` 在权威基线中**只引用 `ai_space`，不引用 `ai_record`**（`database/ddl/baseline/postgres/0001_memory_baseline.sql:37-58`）。因此定向删除单条记忆既不会级联到事件、**也不应**级联——一条事件可能喂出多条记忆，未被点名的记忆仍在服役。事件清理是**整空间 / 整用户**作用域的属性，已由 `delete_events_in_space` / `delete_events_for_user_*` 实现。本轮以端到端测试把两侧都钉住：定向遗忘报 `purgedEvents: 0`，其后空间级遗忘仍能清掉≥2 条事件。

---

## 2. P0 — 阻断商业化发布（已全部关闭）

### P0-1　query 参数契约↔实现互斥

**原始判定**：OpenAPI 声明 camelCase，Rust 结构体 snake_case 且 `deny_unknown_fields` → 按文档发 `?spaceId=` 被拒，真实可用的 `?space_id=` 未声明。

**当前证据**（实测）：

```text
三份权威 OpenAPI 的 query 参数命名违规数 = 0（按门禁同一正则判定）
check-api-operation-patterns --workspace .  →  api operation patterns check passed
生成 SDK：{ name: 'space_id', value: params?.spaceId, style: 'form' }   ← 线格式 snake_case，TS 属性 camelCase
SDK openapi ↔ 权威 apis/：三份 sha256 全部 SAME
```

**新增门禁**：`sdkwork-specs/tools/lib/api-operation-patterns.mjs` 的 `classifyQueryParameterVocabulary`（解析 `#/components/parameters/*` `$ref`、仅取 `in: query`、正则 `^[a-z][a-z0-9]*(?:_[a-z0-9]+)*$`），已自证：内联 `spaceId` + `$ref` `spaceId` 恰报 2 条、`page_size` 通过、path 参数跳过。

### P0-2　双栈权威与 parity 门禁

**当前证据**：

```text
tests/contracts/memory_database_migration_parity_test.mjs    → PASS（重写后，ENOENT 已消失）
assertBaselineInvariants("postgres", database/ddl/baseline/postgres/0001_memory_baseline.sql)
assertBaselineInvariants("sqlite",   tests/fixtures/database/sqlite/ddl/baseline/0001_memory_baseline.sql)
断言 database/ddl/baseline/sqlite 与 database/migrations/sqlite 不得存在（§344）
断言插件遗留 migrations 壳只剩 postgres 目录、无 .sql、有 DEPRECATED.md
```

**失败关闭修复**（`crates/sdkwork-memory-database-host/src/lib.rs`）：在 `init()` 之前调用 `ensure_pool_engine_is_declared(pool.engine(), &manifest)`，诊断文本给出 `DATABASE_FRAMEWORK_SPEC section 344` 与 `SDKWORK_DATABASE_URL`。对应测试 `crates/sdkwork-memory-database-host/tests/engine_admission.rs`（由 `sqlite_lifecycle.rs` `git mv` 而来）断言：诊断含「engine admission failed / declares engines ["postgres"] / cannot run its lifecycle on a sqlite pool」，且 `ops_schema_migration_history` 不存在（证明拒绝发生在 `init()` 之前）。

**规范依据**：`DATABASE_FRAMEWORK_SPEC` §344/§421、`ENVIRONMENT_SPEC` §7.2。

### P0-3　SQLite 档位与 fail-fast

**当前证据**：`crates/sdkwork-intelligence-memory-repository-sqlx/src/bootstrap.rs:63` `reject_sqlite_server_role_engine(dialect, deployment_mode)`——三处服务端入口（运行时数据面引导、数据库宿主引导、assembly 的 `db-migrate` 生命周期）全部经此收口，诊断一致。集成测试 `api_server_smoke_test.rs:162,207` 断言 `authoritative-server Memory rejects the SQLite engine`。

**档位声明**：`database/contract/schema.yaml`（`database_role: authoritative-server`、`engines: [postgres]`）+ `database/database.manifest.json`。这是 §344 规定的声明位置（首轮处方 venue 已在 §1.3 更正）。

**文档措辞**：`TECH_ARCHITECTURE.md:96` 现表述为「PostgreSQL and SQLite share one logical storage model through `sqlx::Any`」。建议补一句并发档位限定（「same logical model, different concurrency tiers; SQLite is the client-local / test plane」）以免读者误推 SQLite 具集群并发能力——**该项仍待办，见 §9**。

### P0-4　发布就绪门禁

**当前证据**：

```text
node tools/check_sdkwork_memory_release_readiness.mjs
[release-readiness] passed (candidate mode)
[release-readiness] warning: release candidate is correctly blocked from production publication   ← 仅信息性
EXIT=0
```

三项 error 全消。`specs/process-database-pool.spec.json` 已落地（1 进程 `sdkwork-api-memory-standalone-gateway`、`driver: sdkwork_database_sqlx::DatabasePool`、3 消费者、6 `productionSourceRoots`），由 `sdkwork-specs/tools/check-process-shared-database-pool.mjs` 校验并接入 `db:pool:validate`。`crates/sdkwork-routes-memory-support/src/readiness.rs` 中跨 await 持锁的进程全局 `Mutex<Option<Arc<PgPool>>>` 已移除。

### P0-5　PC 客户端构建链

**当前证据**：`apps/sdkwork-memory-pc/scripts/materialize-runtime-env.mjs` 已落地，刻意做薄委托到 `sdkwork-specs/tools/build-browser-client.mjs`（无 `#!` shebang，规避 CRLF/esbuild 转换问题）；`apps/sdkwork-memory-pc/vite.config.ts` 改用 `defineConfig(({ mode }) => …)` + `createBrowserRuntimeEnvVitePlugin`，`outDir` 由 `resolveBrowserDistOutDir` 解析（原为 `undefined`）。新增门禁：根与每个 `apps/*/package.json` 中 `node <path>` 引用的文件必须存在（`tools/check_sdkwork_memory_architecture_alignment.mjs` 的 `assertScriptReferencesExist`）——这正是首轮该缺陷能逃过全部门禁的原因。

**未验证项**：本机 `node_modules` 与 `apps/sdkwork-memory-pc/node_modules` 仍不存在，`pnpm --dir apps/sdkwork-memory-pc check` 未实跑。因此「D 类：可交付」仍不能记满分，见 §8 / §9。

---

## 3. P1 — 严重

| # | 问题 | 状态 | 证据 / 当前位置 |
| --- | --- | --- | --- |
| P1-1 | k8s `SDKWORK_DATABASE_URL` 同名 3 次 | **已修** | `deployments/kubernetes/deployment.yaml`：唯一 `SDKWORK_DATABASE_URL`；退役的 `SDKWORK_MEMORY_DB_MAX/MIN_CONNECTIONS` 换成进程级 `SDKWORK_DATABASE_MAX_CONNECTIONS=32` + `SDKWORK_DATABASE_ACQUIRE_TIMEOUT=10`。规范依据 `DATABASE_SPEC_PROCESS_SHARED_POOL` §5/§6：`MAX_CONNECTIONS` 是**进程预算**，模块级形态是退役输入且「MUST NOT be read by runtime startup」。连带清理孤儿引用（secret 示例、k8s README、token 轮换 runbook、rollout runbook）并核对全 manifest 无残留同名键 |
| P1-2 | 并发上限 ≫ 连接池 | **部分** | 已对齐（64 ↔ 32×2 副本），见 §1.2 |
| P1-3 | 学习/评估任务无重试上限、无死信、无毒丸保护 | **未动** | `plugins/.../learning_jobs.rs:103-121` 无条件 requeue；`ai_learning_job` 无 `attempts` 列；`job_worker.rs` 执行 `Err` 直接置 `failed`。Outbox 已有指数退避+死信，能力不对等 |
| P1-4 | 优雅停机仅 3 秒 | **已修** | `job_worker.rs`：新增 `MemoryBackgroundWorkers`，把 `JoinHandle` 交回宿主（API_ASSEMBLY_SPEC §6.2.1 第 744-748 行明确允许）；`drain(budget)` 是**集合级**截止时间，过期即 abort 并返回 `false`。`main.rs` 顺序改为 HTTP 先排空再排后台 plane，最坏耗时=两个已知预算之和。预算由 `SDKWORK_MEMORY_SHUTDOWN_DRAIN_SECS`（默认 25）控制，k8s `terminationGracePeriodSeconds` 由 30 调整为 60。5 个契约测试 + 全量回归 |
| P1-5 | 生产 nginx 同块 `listen 443 ssl; listen 80;`，无 301、无 HSTS | **未动（venue 已纠正）** | `deployments/webserver/nginx.standalone.production.conf` 是**生成物**（头部注明 do not edit by hand），源是 `deployments/webserver/server.common.toml` + `server.production.toml`（16 个 `[[http.server]]` 各 `listen = ["443 ssl", "80"]`）。平台层**已支持** HSTS（`sdkwork-webserver-core` 的 `securityHeaders.strictTransportSecurity`，仅 HTTPS 下发）与 `returnStatus`/`returnLocation` 301——所以修复可在本仓 TOML 完成，无需改平台 |
| P1-6 | 生成物入库 + 就地 tsc 遮蔽 | **已修** | `git ls-files 'sdks/**/generated/server-openapi/src/**/*.js'` = 0、`*.d.ts` = 0、`sdks/**/src/**/*.js` = 0（首轮为 151 / 151 / 有） |
| P1-7 | forget `memory_ids` 无界 + `purged_events` | **已修** | 上限：`platform::MAX_FORGET_MEMORY_IDS`（= `MAX_LIST_PAGE_SIZE` = 200），契约三份均声明 `maxItems: 200`（顺带补上此前同样缺失的 `spaceIds` → `maxItems: 32`）；`purged_events` 澄清为结构性 0（§1.3） |
| P1-8 | 队列表 claim 排序与 trace 分页缺索引 | **未动** | 缺 `ai_retrieval_trace(tenant_id, space_id, created_at DESC, id DESC)`、`ai_outbox_event(publish_state, created_at, id)`、`ai_learning_job(state, priority DESC, created_at)`、`ai_eval_run(state, created_at)`；关键词/敏感词检索仍为 `LIKE '%kw%'` 前导通配 |
| P1-9 | 跨租户隔离无测试证据 | **未动** | `space_isolation_security_test.rs` 全部同一 `tenant_id = 100_001`；PRD 的质量目标是 zero，但越权风险最高的场景零覆盖 |

---

## 4. P2 — 一般

首轮清点的 36 项中，3 项经复核**不是缺陷**（P0-2 相关两条已在 §1.3，第三条为 `purged_events` 统计）。其余 33 项状态未变，按主题列于此节并标注与本轮改动的交集。

### 4.1 契约与规范偏离

| # | 问题 | 状态 |
| --- | --- | --- |
| P2-1 | `PageInfo.totalItems` 缺 `format: int64` 与 `x-sdkwork-int64-string`（3 份全中） | 未动 |
| P2-2 | 全仓 `"format":"int64"` 0 命中，偏离 `API_SPEC §13.6` MUST | 未动 |
| P2-3 | 29/35 个 `page_size` 未声明 `default: 20`（`tools/align-openapi-page-size-default.mjs` 存在但未执行） | 未动 |
| P2-4 | 自研 `serde_int64` 替代权威 `sdkwork_utils_rust::serde_int64`；本仓 Cargo.toml **未依赖** `sdkwork-utils-rust` | 未动（注：服务层实际已依赖 `sdkwork-utils-rust`，契约层仍未收敛） |
| P2-5 | SDK 生成输入与权威漂移（app-sdk 少 42 行 `x-swork-permission`） | **已消**：三份 SDK openapi ↔ `apis/` sha256 全 SAME |
| P2-6 | 路由 manifest 与权威路由表条数不一致 | 未动（本轮 materialize 重跑写出 open 26 / app 42 / backend 82） |
| P2-7 | 生成物内大量 `as any` | 未动 |

**已确认合规**：写命令 44 个均成对声明 `Idempotency-Key` + `x-sdkwork-idempotent: true`；9 个 DELETE 均无请求体；请求体无 `tenantId`；OpenAPI ↔ 路由 manifest ↔ axum handler 一一对应；无 `type: integer, format: int64` 违规。本轮新增合规：请求体数组上限在契约与运行时**同源**，并由新门禁断言（见附录 A）。

### 4.2 数据库与事务

| # | 问题 | 状态 |
| --- | --- | --- |
| P2-8 | `ai_record` INSERT 与 FTS 同步非原子（后者走 `self.pool()`，不在同一事务） | 未动 |
| P2-9 | supersede 为 3 次独立写，无事务 | 未动 |
| P2-10 | SQLite 分支直接返回「已获取」，无 PG 的 advisory lock 互斥 | 未动 |
| P2-11 | baseline 与 migration 漂移（`0001_memory_baseline.sql:9` 声明的源迁移不存在；`organization_id` 冗余重复） | 未动 |
| P2-12 | SQLite `ALTER TABLE ADD COLUMN` 无 `IF NOT EXISTS` | 未动（夹具路径：`tests/fixtures/database/sqlite/ddl/baseline/`） |
| P2-13 | 隔离级别未显式设置；SQLite 无 `BEGIN IMMEDIATE` | 未动 |
| P2-14 | 开发默认 Snowflake 节点号 = 0（`store.rs` 的 `unwrap_or(0)`） | 未动 |
| P2-15 | `sqlx_compat.rs` 查询形状缓存用 `assert!` 做上界，且 `RwLock` 在读路径 | 未动 |
| P2-16 | 布尔解码失败静默转 `false` | 未动 |
| P2-17 | 迁移初始化探测吞错（`Err(_) => Ok(false)`） | 未动 |
| P2-18 | 双栈语义靠哨兵对齐（PG `NULLS NOT DISTINCT` vs SQLite `user_id = -1`） | 未动 |

### 4.3 多副本进程内状态（高可用）

| # | 进程内状态 | 状态 |
| --- | --- | --- |
| P2-19 | `NORMALIZED_SQL` 查询形状缓存 | 未动（仅性能） |
| P2-20 | `COMPAT_OPERATION_SEQUENCE` 操作序号 | 未动 |
| P2-21 | `DOMAIN_METRICS` / HTTP metrics 每 Pod 计数 | 未动（监控侧聚合） |
| P2-22 | `shared_iam_postgres_pool`（第 2 个 DB 池，锁跨 await） | **已消**：锁已移除；单进程单池契约由 `specs/process-database-pool.spec.json` + `db:pool:validate` 约束 |
| P2-23 | 限流/幂等/准入：dev/demo 完全缺失，且 backend-api 下任意已认证主体获 `elevated_tenant_access` | 未动 |

### 4.4 客户端与前端

| # | 问题 | 状态 |
| --- | --- | --- |
| P2-24 | 53 处 `as unknown as` 强转 | 未动 |
| P2-25 | 懒加载 Admin 仅 `Suspense` fallback，无 ErrorBoundary / 超时 | 未动 |
| P2-26 | locale 仅 `zh-CN` / `en-US`，另 5 个语种目录为空 `.gitkeep` | 未动 |
| P2-27 | 缺 `sdkwork-memory-pc-shell` 包 | 未动 |
| P2-28 | i18n 英文 fallback 文案硬编码 | 未动 |

### 4.5 安全与部署配置

| # | 问题 | 状态 |
| --- | --- | --- |
| P2-29 | 示例 env 固定弱口令（`.env.postgres.example`） | 未动 |
| P2-30 | 图谱更新/删除只有 `tenant_id` 缺 `space_id` | 未动 |
| P2-31 | 敏感记录先出库再判敏感度 | 未动 |
| P2-32 | `elevated_tenant_access` 对 private/sensitive 无条件放行 | 未动 |
| P2-33 | retrieval trace 写入结果被丢弃（`let _ = …`） | 未动 |
| P2-34 | nginx 边缘 body 上限 1.1 GB（源：`server.common.toml` 的 `clientMaxBodySize = "1100m"`） | 未动；venue 已确认在 TOML 源 |
| P2-35 | Drive uploader `content_length` 恒为 0 | 未动 |
| P2-36 | `export_upload_profile_code` 两分支恒返回 `"document"` | 未动 |

---

## 5. 虚假实现 / 未完成实现 专项

**结论不变：核心能力未发现"假实现"；缺口是"显式拒绝"而非"静默假绿"。** 本轮另查出**两处** fail-open 并已修（数据库宿主未声明引擎、契约未声明而被运行时拒绝的数组上限），以及**一处**被误判为桩的结构性常量（`purged_events`，§1.3）。

**已确认为真实实现**（非桩）：检索融合的加权 RRF + 去重 + 归一与五路打分；上下文包的真实 token 预算裁剪 + Jaccard 去重；导出的真实采集 + 敏感度过滤 + 字节上限守护；forget 四条 scope 的真实物理删除；迁移的 dry_run/shadow/switch/promote；索引重建与保留期清理；评估的 `retrieval_quality` 真实召回/精确/NDCG/MRR/p95；Outbox 的租约续租/ack/指数退避/死信与 provider 真实 HTTP 健康探测；治理权限的 deny 优先与未知状态 fail-closed；SSRF 校验与配额原子准入。`sqlx_compat.rs` 只做 `?` → `$n` 词法编号（含字符串/注释/`$tag$`/`?|`/`?&` 保护），**不转换语法**，PG 专有语法均按方言显式分叉，未发现泄漏到 SQLite 分支。

**明确的能力缺口**（显式拒绝，非假绿）：

| 缺口 | 现状 |
| --- | --- |
| 评估引擎 | 仅 `retrieval_quality`；其它 `evalType` 一律 `skipped`（`"eval type is not implemented"`） |
| 评估数据集 | `datasetRef` 未接线，只支持内联 `config.cases` |
| 乐观并发（If-Match / ETag） | ADR 标注 `proposed, not implemented` |
| 上传 profile 映射 | 恒 `"document"` |

`plugins/sdkwork-memory-plugin-native-sql` 里的 `"not implemented; refusing … fallback"` 是 SPI 的**安全 fail-closed 默认实现**，native_sql 插件已逐项覆盖。这是良好设计，不是缺陷。

---

## 6. 已确认合规的安全项

- **无 SQL 注入**：`format!` 仅拼常量片段；LIKE 走 `escape_like_pattern` + `ESCAPE '\'`；FTS 走 `escape_fts5_query`；ORDER BY / 表名 / 列名全为常量
- **SSRF 防护完整**：解析后 IP 校验、拒私有/环回/链路本地/`169.254.0.0/16`/`fc00::/7`/IPv4-mapped、拒 mixed DNS、`resolve_to_addrs`、`redirect::Policy::none()`
- **错误不泄漏内部信息**：`ProblemDetail` 只暴露数字 `code` + 服务端 `traceId`；`sdkwork-utils` 对 `InternalError`/`ServiceUnavailable` 掩码 detail；PC `readSafeError` 只取 `code` + `traceId`。本轮补强：`MemoryServiceError::storage` 原先**丢弃**传入的 detail，已改为保留；并新增测试断言含口令的 store 错误文本不会外泄
- **无用户可达的缺 tenant 谓词业务查询**：命中的 8 处均为 worker / schema / 全局配置路径
- **凭据只存引用**：provider / S3 走 `credential_ref` / K8s Secret；`secret.example.yaml` 全为 `REPLACE_ME`；未发现真实密钥或硬编码 token 入库
- **PC 无 XSS 面**：无 `dangerouslySetInnerHTML`、无 `eval`、无 `new Function`
- **SDK 消费边界干净**：Console 树 → Backend SDK 0 命中；PC 全树无业务裸 HTTP；Backend SDK 唯一落点是懒加载的 `admin-core/src/sdk/index.tsx`

---

## 7. 行业对标差距（vs Mem0 / Zep / Letta(MemGPT) / LangMem / AWS AgentCore Memory）

架构取向是健康的：provider 中立 SPI、无强制向量库、证据/规范记忆/候选/习惯分层、租户隔离前置、审计与隐私工作流完备。差距集中在记忆产品上层语义与运营证据：

| 能力维度 | 行业头部做法 | 本仓现状 | 差距 |
| --- | --- | --- | --- |
| 记忆老化 / 强度衰减 | 遗忘曲线、访问强化、TTL 分层 | 有 `time_recency_score` 时序打分，**无策略化衰减/强化模型** | 中 |
| 矛盾检测与自动失效 | 抽出新事实即对比旧事实自动 supersede | 有 supersede 与候选/整合流程，但**非原子且无矛盾判定的显式算法证据** | 中 |
| 时序推理 | 自然语言时间区间解析 | 有事件时间戳与 UTC 规范，未见 NL 时间解析 | 中 |
| 记忆质量评估体系 | 召回/精确/NDCG/MRR + 冲突率 + 陈旧率 + 成本 | 仅 `retrieval_quality` 一种 | 高 |
| 多模态记忆 | 图像/音频/文档记忆 | 无（PRD 未列为目标） | 低（可接受） |
| 用量计量与计费 | per-tenant token/调用计量、配额扣减、账单导出 | 有 subjects/bindings/capability/quota 与原子准入，**未见用量计量与账单导出** | 高（商用必需） |
| 压测 / soak / 容量证据 | 公开 latency SLO 曲线与容量模型 | **完全缺失** | 高（商用必需） |
| 多区域 active-active | Zep Cloud 等 | PRD 明确 non-goal | 低 |
| 数据驻留 / 合规 | 区域隔离、DSR 自动化、GDPR 报告 | 有 region 概念、审计、export/forget 工作流；未见 DSR SLA 与合规报告产出 | 中 |

---

## 8. 商业化落地能力评估

**结论：仍不具备。** 评分（0–5）——括号内为首轮分值：

| 维度 | 评分 | 依据 |
| --- | --- | --- |
| 功能正确性 | **3.5**（3） | P0-1 契约断路已修，过滤能力恢复；P2-8/9 事务性与 P2-31 先取后判仍在 |
| 契约与规范一致性 | **3.5**（2.5） | query 命名、数组上限、SDK↔权威同源均已对齐并有门禁；P2-1/2/3/4 的 int64 元数据仍未收敛 |
| 并发与高可用 | **2.5**（2） | 准入↔池已对齐、停机改为有界真实 drain；但任务无重试/死信、缺 claim 索引、SQLite 仍是单写档位（契约已按规范表达） |
| 安全与合规 | **3.5**（3.5） | SSRF/注入/错误掩码/密钥引用扎实；跨租户无测试、生产明文 HTTP 未动 |
| 可交付与运营 | **3**（2） | 发布门禁转绿、构建链修复、生成物入库清除；但 `pnpm install` 未执行、无压测/容量证据、无用量计费 |

**综合：3.2 / 5**（首轮 2.6）。**距商用底线（约 4.0）仍有差距**，但性质已从「架构返工」变为「运营证据补齐」。对照 PRD 自述（publication remains blocked until …），本审计**独立复现该结论**。

---

## 9. 剩余改进方案

### 阶段 B 余项 — 达到商用底线

1. **任务可靠性（P1-3）**：`ai_learning_job` / `ai_eval_run` 增 `attempt_count` + `max_attempts` + 死信状态；过期重入带上限；瞬时失败走指数退避而非直接 `failed`；对齐 Outbox 已有的退避/死信。
2. **传输安全（P1-5）**：改 `deployments/webserver/server.production.toml`（**不是**生成的 `.conf`）——每个 `[[http.server]]` 拆为 443 块 + 80 块（80 走 `returnStatus`/`returnLocation` 301），并在 `server.common.toml`/production 的 `securityHeaders` 开启 `strictTransportSecurity`；同时把 `clientMaxBodySize` 从 `1100m` 收敛到与应用一致的量级（P2-34）。
3. **索引补齐（P1-8）**：补 4 张队列表的 claim 排序索引与 `ai_retrieval_trace(tenant_id, space_id, created_at DESC, id DESC)`；关键词/敏感词检索改 FTS/trigram，或显式接受全扫描并写容量模型。
4. **跨租户红队测试（P1-9）**：补租户 A 访问租户 B 的 space/record/entity/edge/policy/audit 负向用例，覆盖 open/app/backend 三面。
5. **容量与超时（P1-2 余项）**：外层独立 `TimeoutLayer` 兜底；worker 并发独立配额，禁止与请求路径争抢池。
6. **依赖与可复现**：执行 `pnpm install` 使 `pnpm check` / `pnpm verify` / `pnpm --dir apps/sdkwork-memory-pc check` 真正可复现，并把结果纳入发布证据。

### 阶段 C — 对齐头部产品

7. 事务化写入：`ai_record` + FTS + outbox 收敛到单一事务；supersede 三步合并为一个事务（P2-8/9）。
8. 多副本状态外置：SQL 形状缓存、operation 序号、指标聚合、并发准入全部外置或声明为可聚合；dev/demo 也装载最小安全策略（P2-19…P2-23）。
9. 契约元数据补齐：`totalItems` 的 `format: int64` + `x-sdkwork-int64-string`；29 处 `page_size` 补 `default: 20`；`serde_int64` 收敛到 `sdkwork_utils_rust::serde_int64`（P2-1…P2-4）。
10. 评估引擎扩展：数据集引用、多 eval 类型（陈旧率/冲突率/一致性）。
11. 前端质量：消除 `as unknown as`；lazy chunk 加 ErrorBoundary；补 `sdkwork-memory-pc-shell`；补齐 5 个缺失语种。
12. 记忆语义增强：记忆衰减/强化模型、矛盾检测与自动失效、NL 时间区间解析。
13. 文档措辞：`TECH_ARCHITECTURE.md:96` 补「不同并发档位」限定（P0-3 遗留）。

### 阶段 D — 商用发布前置（与 C 并行）

14. **运营证据**：load / soak 脚本 + 公开 latency SLO 曲线 + 容量模型；跨租户隔离红队报告。
15. **计量与计费**：per-tenant 用量计量（调用/token/存储）、配额扣减、账单导出。
16. **合规**：数据驻留区域校验、DSR SLA、合规报告产出。
17. **交付**：不可变 OCI / 浏览器制品、签名与 OIDC 证明、SBOM 与 provenance、部署冒烟、回滚演练记录（全部转为 CI 必过门禁）。

---

## 附录 A — 实跑证据（本轮复验）

```text
$ cargo test --workspace --no-fail-fast
passed=397 failed=0 ignored=1                      ← 首轮基线 390/0/1，本轮新增 7 个测试

$ 门禁（15 项全 PASS）
check:app-composition / check:architecture-alignment / check:release-readiness
check:pagination / check:api-envelope / check:api-operation-patterns / check:sdk-standard
check:cors-standard / check:pnpm-script-standard / check:agent-workflow-standard
check:browser-build-scripts
api:assembly:validate / db:validate / db:pool:validate / topology:validate

$ 契约测试（9 项全 PASS）
route_manifest_openapi_parity_test / openapi_phase1_contract_test
schema_registry_phase1_contract_test / native_sql_migration_contract_test
memory_database_migration_parity_test / runtime_plugin_layout_contract_test
open_api_prefix_contract_test / rust_spi_boundary_contract_test / spi_design_contract_test

$ node tools/check_sdkwork_memory_release_readiness.mjs
[release-readiness] passed (candidate mode)
[release-readiness] warning: release candidate is correctly blocked from production publication
EXIT=0                                              ← 首轮为 3 errors

$ 三份权威 OpenAPI 的 query 参数命名违规 = 0
$ SDK openapi ↔ 权威 apis/：sha256 三份全 SAME
$ git ls-files 'sdks/**/generated/server-openapi/src/**/*.js'    → 0（首轮 151）
$ git ls-files 'sdks/**/generated/server-openapi/src/**/*.d.ts'  → 0（首轮 151）
$ git ls-files 'sdks/**/src/**/*.js'                             → 0

$ 契约中的请求数组上限：三份文档均为 maxItems 32（spaceIds）与 200（memoryIds）
$ 生成器重跑幂等：apis/app-api/…openapi.json md5 前后一致

$ k8s env 键唯一性：deployment.yaml 中 SDKWORK_DATABASE_URL 出现 1 次，无任何重复 env 键
```

**变异自证**（证明新门禁与测试不是恒绿；每次变异后均还原并校验字节级一致）：

| 变异 | 期望 | 实测 |
| --- | --- | --- |
| SDK 权威副本 `maxItems` 200 → 201 | 门禁红 | 红：`…MemoryForgetRequest.memoryIds must declare maxItems 200 to match the enforced ceiling, found 201` |
| 生成器 `const MAX_SCOPE_SPACE_IDS = 32` → 33 | 门禁红 | 红：`the contract generator must declare MAX_SCOPE_SPACE_IDS = 32` |
| 移除 `if memory_ids.len() > max_memory_ids` | 测试红 | 红：`left: 201, right: 400`（201 CREATED vs 期望 400） |
| query 词汇分类门禁（`api-operation-patterns.mjs`） | 能识别 | 内联 `spaceId` + `$ref` `spaceId` 恰报 2 条；`page_size` 通过；path 参数跳过 |

## 附录 B — 已穷尽的检索结论

- Console 代码树 → Backend SDK：**0 命中**（含 static / dynamic / type-only import）
- PC 全树业务裸 HTTP：**0 命中**（唯一 `fetch` 取 `/runtime-env.json` 静态配置）
- 用户可达的「缺 tenant_id 谓词」业务查询：**0 处**
- SQL 注入：**0 处**
- 真实密钥 / 硬编码 token 入库：**0 处**
- k8s / nginx / env 资产中的退役数据库键（`SDKWORK_*_DATABASE_*`、`DATABASE_URL`、`DATABASE_SSLMODE`）：**0 处**

## 附录 C — 与本审计相关的权威规范

`sdkwork-specs/` 下：`PAGINATION_SPEC.md`（§14.1.1 query 命名、§16.6 分页模型）、`API_SPEC.md`（§13.6 int64 线格式、响应信封、错误模型）、`API_ASSEMBLY_SPEC.md`（§4 贡献中立、§6.2.1 第 744-748 行后台任务移交宿主）、`DATABASE_SPEC.md`、`DATABASE_FRAMEWORK_SPEC.md`（§40/§344/§421 单引擎权威）、`DATABASE_SPEC_PROCESS_SHARED_POOL.md`（§5 连接预算是进程预算、§6 统一身份）、`ENVIRONMENT_SPEC.md`（§7.2 server 角色不得静默接受 SQLite）、`COMPONENT_SPEC.md`（§3 组件清单 schema）、`SECURITY_SPEC.md`、`PRIVACY_SPEC.md`、`PERFORMANCE_SPEC.md`、`TEST_SPEC.md`、`DOCUMENTATION_SPEC.md`。
