# SDKWork Memory 商业化就绪审计

Status: review record (审计结论)

Owner: SDKWork Memory maintainers

Reviewed: 2026-09-23

Scope: `D:\sdkwork-space\sdkwork-memory` 全仓（Rust 服务 + SPI/插件 + 三份权威 OpenAPI + 生成 SDK + PC 客户端 + 数据库契约 + 部署资产）

Baselines: `docs/product/prd/PRD.md`、`docs/architecture/tech/TECH_ARCHITECTURE.md`、`D:\sdkwork-space\sdkwork-specs`（PAGINATION_SPEC / API_SPEC / DATABASE_SPEC / SECURITY_SPEC / PERFORMANCE_SPEC / PRIVACY_SPEC）

Method: 静态通读 + 全仓检索 + **实跑仓库门禁与校验器**（见附录 A）。所有结论均给出 `文件:行` 证据；无法实测的标注「疑似」。

---

## 0. 结论摘要

**判定：当前不具备商业化生产落地能力**，与仓库自述一致（PRD「Release State」：`internal release candidate, not an active production release`）。

这不是"质量差"，而是"**契约层有真实功能缺陷 + 双栈权威不同源 + 发布闸门自身为红**"三类硬阻断叠加。仓库里有大量真实、扎实的实现（见 §7），问题集中在**边界一致性**而非算法本体。

| 级别 | 数量 | 性质 |
| --- | --- | --- |
| **P0 阻断发布** | 5 | 契约与实现互斥导致官方 SDK 调用 400；SQLite 权威缺失；单连接架构；发布门禁为红；PC 构建链断裂 |
| **P1 严重** | 9 | 部署配置错误、容量失配、任务无重试/无死信、优雅停机不足、明文 HTTP、生成物一致性、无界请求、索引缺失、跨租户测试缺失 |
| **P2 一般** | 36 | 契约元数据缺失、事务边界、规范偏离、多副本进程内状态、前端类型安全与 i18n、部署配置 |

**一句话根因**：`sdkwork-specs` 的规范校验器（`check-pagination.mjs`、`check-api-response-envelope.mjs`、`check-api-operation-patterns.mjs`）**全部通过**，但它们没有覆盖「文档化的 query 参数名 ↔ Rust 反序列化字段名」这一层——于是 7 个列表操作的过滤能力在生产上是断的，而所有门禁显示绿灯。这是"**门禁假绿**"的典型案例。

---

## 1. 宣称 vs 实际 核对表

依据 PRD §Product Rules / §Quality And Operations Targets 与 TECH_ARCHITECTURE §Data And Job Model / §Security And Privacy 逐条核对：

| # | 文档宣称 | 判定 | 关键证据 |
| --- | --- | --- | --- |
| 1 | 请求体与 list 查询参数永不选择当前租户 | ✅ 成立 | 路由注入 `query.tenant_id = context.tenant_id`（`crates/sdkwork-routes-memory-app-api/src/commercial_routes.rs:72`）；契约请求体无 `tenantId`；`EnforcePrincipalTenantIsolationPolicy`（`crates/sdkwork-routes-memory-support/src/web_runtime.rs:54`） |
| 2 | Console 只消费 App SDK，Admin 只消费 Backend SDK，无裸 HTTP | ✅ 成立 | 全树检索 0 违规（附录 B） |
| 3 | 交互列表服务端分页；高量历史用 store 级 keyset，绝不下全量切片 | ⚠️ 部分 | keyset 与 200 上限已强制（`crates/sdkwork-intelligence-memory-service/src/platform.rs:245-253`，含 `0/-1/201` 拒绝测试）；但**7 个列表操作的过滤参数无法按契约使用**（见 P0-1） |
| 4 | 成功响应走标准信封；错误为 `application/problem+json` 带数字 code + traceId | ✅ 成立 | 实跑 `check-api-response-envelope.mjs` 通过；成功信封 100%、problem 100% |
| 5 | 嵌入可选，无外部 provider 时原生 SQL 检索仍可用 | ✅ 成立 | 关键词/词典/结构化/时序/事件五路打分真实（`crates/sdkwork-memory-retrieval/src/retrieval/mod.rs:249,266,285,323,338`）+ 加权 RRF 融合（`:151-247`） |
| 6 | 受限/敏感访问 fail-closed 且在 store 查询或 provider 调用**之前**约束 | ⚠️ 部分 | 列表与检索在 SQL 内过滤（`plugins/.../search_index.rs:313,370`；`store.rs:739,785`）；但单记录与 trace 是**先取后判**（`crates/sdkwork-intelligence-memory-service/src/open_api.rs:471-485,1590-1617`） |
| 7 | 导出内存有界：内联 4 MiB / Drive 64 MiB / 硬顶 256 MiB | ✅ 成立 | `plugins/.../privacy.rs:338-426`；`crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs:2184-2198` |
| 8 | 集群 worker 使用数据库围栏租约，过期 worker 不能在接管后 ack/complete | ✅ 成立 | PG `FOR UPDATE SKIP LOCKED`（`plugins/.../learning_jobs.rs:183,527`；`store.rs:2934`）；租约条件更新（`store.rs:3233-3234,3383-3384`）；heartbeat 续租（`crates/sdkwork-intelligence-memory-service/src/job_worker.rs:193-222`） |
| 9 | Outbox 写入属于变更边界 | ⚠️ 部分 | 仅规范数据路径在事务内（`plugins/.../canonical_data.rs:598-606`）；乐观并发 ADR 标注 `proposed, not implemented`（`docs/architecture/decisions/ADR-20260721-optimistic-concurrency.md:3`） |
| 10 | 出站 provider/Outbox 客户端校验解析地址、拒非公网与混合 DNS、固定地址、禁重定向 | ✅ 成立 | `crates/sdkwork-intelligence-memory-service/src/endpoint_validation.rs:11-132`（含 `redirect::Policy::none()`、`resolve_to_addrs`、拒 `169.254.0.0/16` / `fc00::/7` / IPv4-mapped） |
| 11 | PostgreSQL 与 SQLite 通过 `sqlx::Any` **共享同一逻辑存储模型** | ❌ 不成立 | 见 P0-2 / P0-3：SQLite 生产 DDL 来自 `tests/fixtures/`，权威契约 `engines: ["postgres"]`，且 SQLite 强制单连接 |
| 12 | 生产 PC 产物排除 sourcemap 与私有运行时状态 | ✅ 成立 | `apps/sdkwork-memory-pc/vite.config.ts:35` `sourcemap: false` |
| 13 | 可用性 99.9% / p99 读 <200ms / p99 写 <500ms | ❓ 未验证 | 仓库内无 load/soak 脚本与压测证据（`scripts/`、`tests/` 均无）；与 PRD 自述"release gates 未过"一致 |
| 14 | 跨租户访问为零 | ❓ 未验证 | 隔离测试**仅覆盖同租户跨 space**（`crates/sdkwork-memory-integration-tests/tests/space_isolation_security_test.rs:51-238`，tenant 恒为 `100_001`），无跨租户用例 |
| 15 | 生产 HTTP 面需共享 Redis（限流/幂等/并发准入）+ IAM 库就绪 | ✅ 成立（仅 production-like） | `web_runtime.rs:25-27,29-61`；Redis 缺失直接 panic（fail-fast）。**dev/demo 下全部缺失**（见 P2-13） |
| 16 | 网关施加有界请求 deadline / body limit / 本地并发上限 | ⚠️ 部分 | body 1 MiB + 并发 256 无条件（`crates/sdkwork-api-memory-assembly/src/bootstrap.rs:94,96,209-223`）；**请求 deadline 仅 production-like 生效**，默认 30s、上限 300s（`web_runtime.rs:16,61,89-96`）。并发 256 与连接池 16 失配（见 P1-2） |

---

## 2. P0 — 阻断商业化发布

### P0-1　7 个列表操作的过滤参数「契约与实现互斥」，官方 SDK 调用必然 400

**现象**：OpenAPI 把过滤参数声明为 camelCase，Rust 反序列化结构体是 snake_case 且开启 `deny_unknown_fields` → 客户端按文档发 `?spaceId=...` 会被拒绝；服务端真正接受 `?space_id=` 却**没有**在契约里声明。生成 SDK 忠实按契约发送 camelCase。

| 面 | operation | 契约声明的参数 | 结构体字段 | 证据 |
| --- | --- | --- | --- | --- |
| Open | `entities.list` | `spaceId`, `entityType` | `space_id`, `entity_type` | `apis/open-api/memory-open-api.openapi.json` |
| Open | `edges.list` | `spaceId`, `sourceEntityId`, `relationType` | `space_id`, `source_entity_id`, `relation_type` | 同上 |
| App | `entities.list` | `spaceId`, `entityType` | 同 Open | `apis/app-api/memory-app-api.openapi.json` |
| Backend | `subjects.list` | `subjectType` | `subject_type` | `apis/backend-api/memory-backend-api.openapi.json` |
| Backend | `entities.list` | `spaceId`, `entityType` | 同 Open | 同上 |
| Backend | `edges.list` | `spaceId`, `sourceEntityId`, `relationType` | 同 Open | 同上 |
| Backend | `policies.list` | `policyType` | `policy_type` | 同上 |

代码证据：

```rust
// crates/sdkwork-memory-contract/src/commercial.rs:517-540（ListEntitiesQuery）
#[serde(deny_unknown_fields)]          // ← 无 rename_all="camelCase"
pub struct ListEntitiesQuery { ... pub space_id: Option<u64>, ... }
```

```rust
// crates/sdkwork-routes-memory-app-api/src/commercial_routes.rs:65-68
async fn list_entities(..., Query(mut query): Query<ListEntitiesQuery>) // ← 直接反序列化
```

```ts
// sdks/sdkwork-memory-app-sdk/.../generated/server-openapi/src/api/memory.ts:74
{ name: 'spaceId', value: params?.spaceId, style: 'form', explode: true, ... }
// sdks/sdkwork-memory-backend-sdk/.../generated/server-openapi/src/api/memory.ts:164
{ name: 'spaceId', value: params?.spaceId, ... }
```

同文件 `:858` 的 `space_id` 是**正确**写法（TS 属性 camelCase、线格式 snake_case），证明这是局部疏漏而非约定。

**影响**：按空间/类型过滤实体、边、主体、策略在 App 面与 Backend 面全部不可用（400 参数错误）。这是**功能性错误**，不是风格问题。

**与规范的关系**：`sdkwork-specs/PAGINATION_SPEC.md` §14.1.1 要求 query 多词参数使用 `lower_snake_case`。**规范站得住，OpenAPI 违规**。

**为何门禁没抓到**：实跑 `check-pagination.mjs` / `check-api-response-envelope.mjs` / `check-api-operation-patterns.mjs` 三者**全部通过**（附录 A）——校验器未覆盖「契约参数名 ↔ serde 字段名」这一层。

**修复**：把 7 处 query 参数 `name` 改为 snake_case 并重新生成 SDK（TS 属性名会自然变成 camelCase），或在 DTO 上加 `#[serde(alias="spaceId")]` 并保持契约 camelCase——**二选一，但必须与 `PAGINATION_SPEC` 对齐，即改契约**。修完需补一条「OpenAPI query 参数名 ⊆ 契约结构体可接受字段名」的门禁，堵住这类假绿。

---

### P0-2　SQLite 的「生产 schema」来自测试夹具，双栈权威不同源

```rust
// plugins/sdkwork-memory-plugin-native-sql/src/store.rs:3926-3978
include_str!("../../../tests/fixtures/database/sqlite/migrations/0001_memory_schema.up.sql"),
include_str!("../../../tests/fixtures/database/sqlite/migrations/0002_memory_indexes.up.sql"),
// ... 共 10 段，全部来自 tests/fixtures/
```

对照 PostgreSQL：

```rust
// plugins/sdkwork-memory-plugin-native-sql/src/store.rs:230
include_str!("../../../database/ddl/baseline/postgres/0001_memory_baseline.sql"),  // 权威
```

事实清点：

| 路径 | 是否存在 |
| --- | --- |
| `database/ddl/baseline/postgres/` | ✅ 存在 |
| `database/ddl/baseline/sqlite/` | ❌ **不存在** |
| `database/migrations/postgres/` | ✅ 存在 |
| `database/migrations/sqlite/` | ❌ **不存在** |
| `database/database.manifest.json` → `engines` | `["postgres"]` |
| `database/contract/schema.yaml:8-9` → `engines` | `- postgres` |
| `crates/sdkwork-intelligence-memory-repository-sqlx/src/runtime.rs:165-176` | `native_sql implementation requires PostgreSQL; SQLite must use local_embedded` |

**影响**：
1. 违反 `DATABASE_SPEC`「baseline 为权威」。任何人修改测试夹具即等于修改 SQLite 生产 schema，测试资产被提升为生产权威。
2. `tests/contracts/memory_database_migration_parity_test.mjs:10-32` 会 `readdirSync("database/migrations/sqlite")` → **ENOENT 抛错**（无 try/catch）。即：**双引擎一致性门禁本身是坏的**，「SQLite 与 PG 一致」从未被真正校验过。
3. 权威契约把 SQLite 排除在 `engines` 之外 → 从契约视角看，SQLite 是"另一实现族"，与 TECH_ARCHITECTURE §"PostgreSQL and SQLite share one logical storage model" 的宣称直接矛盾。

**修复**：把 `tests/fixtures/database/sqlite/migrations/*.up.sql` 迁到 `database/ddl/baseline/sqlite/`，`database/manifest.json` 与 `schema.yaml` 的 `engines` 增列 `sqlite`，`database/migrations/sqlite/` 落地，parity 门禁改为「两引擎 baseline 的表/列/索引/约束集合等价」并加 ENOENT 断言（缺目录即红）。

---

### P0-3　SQLite 强制单连接且无 WAL/busy_timeout → 与「大规模集群高并发」根本冲突

```rust
// plugins/sdkwork-memory-plugin-native-sql/src/pool_backend.rs:30-53
if matches!(config.engine, DatabaseEngine::Sqlite) {
    config.max_connections = 1;      // ← 全部读写串行化
}
...
sqlx::query("PRAGMA foreign_keys = ON").execute(&pool).await?;   // ← 仅此一条 PRAGMA
```

未设置：`journal_mode=WAL`、`busy_timeout`、`synchronous`、`BEGIN IMMEDIATE`。

**影响**：
- 所有并发请求在**同一条 SQLite 连接**上排队；任何单条慢查询（或一次导出/批量写入）会让**全站请求**一起等待。这与 PRD「p99 读 <200ms」在 SQLite 档位下不可达。
- 一进程一单写者，水平扩容不提升写吞吐，多副本之间靠文件锁竞争，**没有任何集群高并发能力**。
- 无 `busy_timeout` 时，一旦出现第二个写者（例如同机的维护脚本、运维 SQLite CLI）立即 `SQLITE_BUSY`。

**结论**：SQLite 只可作为「单机开发 / 嵌入式档位」交付。TECH_ARCHITECTURE 中"SQLite 与 PG 共享同一逻辑存储模型"的表述，应改为「同一逻辑模型、不同并发档位」，并在部署规范里明确 **生产必须 PostgreSQL**（当前 k8s/nginx 资产确实是 PG 取向，但契约层没写死）。

**修复（择一）**：
- **推荐**：契约层明确 `sqlite` = `single_writer_local` 档位，生产 profile 启动时若检测到 SQLite 直接拒绝启动（fail-fast），并写入 `specs/component.spec.json`。
- 若确需 SQLite 承载并发：`max_connections` 提到 ≥4 并启用 WAL + `busy_timeout=5000`，写路径统一走 `BEGIN IMMEDIATE`，且必须补并发写压测证据。

---

### P0-4　发布就绪门禁自身为红（3 errors）

实跑仓库自家闸门：

```text
$ node tools/check_sdkwork_memory_release_readiness.mjs
[release-readiness] warning: process-shared database pool: specs/process-database-pool.spec.json must exist
[release-readiness] warning: release candidate is correctly blocked from production publication
[release-readiness] error: disabled candidate package container-x64-standalone-container-oci must not retain a stale immutable digest claim
[release-readiness] error: disabled candidate package container-x64-cloud-container-oci must not retain a stale immutable digest claim
[release-readiness] error: release-candidate manifests must keep publication packages disabled until immutable evidence exists
[release-readiness] failed (3 error(s))
```

事实核对：
- `specs/process-database-pool.spec.json` **不存在**（`specs/` 仅 4 个文件：README / component.spec.json / iam.module.manifest.json / topology.spec.json），而 PRD 明确把「gateway has a validated process-shared database pool contract」列为发布前置条件。
- 容器候选包残留"不可变摘要"声明 → 与"未产出不可变制品"矛盾。

**影响**：商业化发布链路的**第一道闸门就是红的**。任何"已就绪"的说法都缺证据。

**修复**：产出 `specs/process-database-pool.spec.json` 并让 gateway 真正按该契约装配（当前进程内存在第 2/3 个 DB 池：`crates/sdkwork-routes-memory-support/src/readiness.rs:9` 的 `IAM_POSTGRES_POOL`、`crates/sdkwork-memory-drive/src/bootstrap.rs:83`，且 `IAM_POSTGRES_POOL.lock().await` 跨 await 持锁）；清理候选包 manifest 的过期 digest 声明；把该检查接入 CI 必过项。

---

### P0-5　PC 客户端 check/build 必然失败：`prebuild` 引用不存在的脚本

```json
// apps/sdkwork-memory-pc/package.json
"prebuild": "node scripts/materialize-runtime-env.mjs --environment production",
"predev":   "node scripts/materialize-runtime-env.mjs --environment development",
```

```text
$ ls apps/sdkwork-memory-pc/scripts/
package-web-release.mjs        ← 只有这一个，materialize-runtime-env.mjs 不存在
```

**影响**：`pnpm build` / `pnpm check` 立即 `MODULE_NOT_FOUND`。PRD 的验收证据第 3 条 `pnpm --dir apps/sdkwork-memory-pc check` **无法通过**，整条客户端交付链断裂。

附带：根 `node_modules` 与 `apps/sdkwork-memory-pc/node_modules` **均不存在**，即当前工作区连 `pnpm install` 都没做（与迁盘后遗留问题一致），因此 `pnpm verify` / `pnpm check` 全链路目前不可复现。

**修复**：补齐 `materialize-runtime-env.mjs`（或删除 pre 钩子并把运行时环境物化改由 `sdkwork-specs/tools/build-browser-client.mjs` 统一负责），并执行 `pnpm install` 后跑通 `pnpm --dir apps/sdkwork-memory-pc check`。

---

## 3. P1 — 严重

| # | 问题 | 证据 | 影响 |
| --- | --- | --- | --- |
| P1-1 | k8s Deployment 中 `SDKWORK_DATABASE_URL` **同名定义 3 次** | `deployments/kubernetes/deployment.yaml:125,130,136` | k8s 只保留最后一个（drive），memory 与 IAM 的库 URL 丢失 → 多副本起不来 |
| P1-2 | 网关并发上限 256 ≫ PG 连接池 16，且外层 router 无独立 deadline | `crates/sdkwork-api-memory-assembly/src/bootstrap.rs:96`（`DEFAULT_MAX_CONCURRENCY = 256`）、`:209-223`（仅 body limit + concurrency） | 慢请求可长期占住并发槽与连接；256 并发对 16 连接 → 池排队 → p99 塌陷。且 `ConcurrencyLimitLayer` 是**每副本** 256，N 副本放大 N 倍（ADR 自认：`ADR-20260627-api-rate-limiting.md:23`） |
| P1-3 | 学习/评估任务**无重试上限、无死信、无毒丸保护** | `requeue_stale_running_learning_jobs` 无条件把过期 running 改回 queued（`plugins/.../learning_jobs.rs:103-121`）；`ai_learning_job` 表**无 attempts 列**（`database/ddl/baseline/postgres/0001_memory_baseline.sql` 中 `ai_learning_job` 定义）；`crates/sdkwork-intelligence-memory-service/src/job_worker.rs:226-261` 执行 `Err` 直接置 `failed` | 两个方向都错：**确定性失败**（毒丸输入让 worker 崩）→ 无限重入烧资源；**瞬时失败**（provider 抖动/连接超时）→ 永久 `failed` 无重试。Outbox 有指数退避（`1<<n` 封顶 32s）+ 死信（`store.rs:3301-3302,205-213`），学习/评估没有 → 能力不对等 |
| P1-4 | 优雅停机仅给后台 worker 3 秒 | `crates/sdkwork-api-memory-standalone-gateway/src/main.rs:87-91`（`sleep(Duration::from_secs(3))`）；worker 仅在批次间检查 shutdown（`job_worker.rs:67-76`） | 迁移/导出等长作业被强杀 → 可能留半成品数据 |
| P1-5 | 生产 nginx 同 server 块内 `listen 443 ssl; listen 80;`，无 301 跳转、无 HSTS | `deployments/webserver/nginx.standalone.production.conf:21-23` | 携带 Bearer 令牌的明文 HTTP 可访问 → 令牌可被旁路嗅探 |
| P1-6 | 被 `.gitignore` 显式声明「never committed」的生成物已入库 | `.gitignore:78-82` vs 实测 `git ls-files`：`sdks/**/generated/server-openapi/src/**/*.js` = **151** 个、`*.d.ts` = **151** 个、`sdks/**/generated/**` 合计 **1136** 个 | 生成物入库 + 就地 tsc 产物（`sdks/.../app-sdk-typescript/src/index.{js,d.ts}`）→ `.js` 遮蔽 `.ts`；改契约后若不重新生成，会**静默使用旧产物**（这正是"假实现"的经典产生方式） |
| P1-7 | forget `scope=memory` 的 `memory_ids` **无长度上限**，且 `purged_events` 恒为 0 | `crates/sdkwork-intelligence-memory-service/src/app_backend_api.rs:683-723`（逐条 `retrieve_record_detail` + `hard_delete_record_with_cleanup`，无 `MAX_*` 约束）、`:724-728`（`purged_events: 0` 硬编码） | 请求可控的 N+1 写路径（检索/导出都有 `MAX_SCOPE_SPACE_IDS=32`，此处缺失，不一致）；且即使真的清理了事件，API 统计仍报 0 → 返回给用户的统计**不真实** |
| P1-8 | 全局队列 claim 排序与 trace 分页**缺索引** | 缺 `ai_retrieval_trace(tenant_id, space_id, created_at DESC, id DESC)`；缺 `ai_outbox_event(publish_state, created_at, id)`（现有索引以 `next_attempt_at` 为次列）；缺 `ai_learning_job(state, priority DESC, created_at)`；缺 `ai_eval_run(state, created_at)` | worker 轮询与 trace 列表退化为排序扫描；且关键词/敏感词检索全部是 `LIKE '%kw%'` 前导通配（`store.rs:782-784,969-972,1015`；`privacy.rs:273-275`）→ 必然全分区扫描，无法走索引 |
| P1-9 | **跨租户隔离无测试证据** | `crates/sdkwork-memory-integration-tests/tests/space_isolation_security_test.rs:51-238` 全部使用同一 `tenant_id = 100_001` | PRD 把「Cross-tenant access: zero」列为质量目标，但仓库只有"同租户跨 space"用例。越权风险最高的场景**零覆盖** |

---

## 4. P2 — 一般（按主题分组）

### 4.1 契约与规范偏离

| # | 问题 | 证据 |
| --- | --- | --- |
| P2-1 | `PageInfo.totalItems` 缺 `format: int64` 与 `x-sdkwork-int64-string: true`（3 份契约全中） | open `:9181` / app `:11537` / backend `:17151` |
| P2-2 | 全仓 `"format":"int64"` **0 命中**，偏离 `API_SPEC §13.6` 的 MUST | 三份 OpenAPI 全量检索 |
| P2-3 | 29/35 个 `page_size` 契约未声明 `default: 20`（服务端 `unwrap_or(DEFAULT_PAGE_SIZE)` 实际生效，但契约未表达） | open 4/4、app 7/9、backed 18/22 缺 default；仓内 `tools/align-openapi-page-size-default.mjs` 存在但未执行 |
| P2-4 | 自研 `serde_int64` 替代规范要求的权威实现 | `crates/sdkwork-memory-contract/src/serde_int64.rs`（u64 版）vs 权威 `sdkwork_utils_rust::serde_int64`（`D:\sdkwork-space\sdkwork-utils\packages\sdkwork-utils-rust\src\serde_int64.rs`，其 `lib.rs:57` 明确要求 `#[serde(with = "sdkwork_utils_rust::serde_int64")]`）；本仓 Cargo.toml **未依赖** `sdkwork-utils-rust` |
| P2-5 | SDK 生成输入与权威漂移 | `sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json` 比权威 `apis/app-api/...` 少 42 行 `"x-swork-permission"` 元数据 |
| P2-6 | 路由 manifest 与权威路由表条数不一致 | `sdks/_route-manifests/*` 比对应 `crates/sdkwork-routes-*` 少 open 9 / app 9 / backend 41 条 |
| P2-7 | 生成物内大量 `as any` | backend `memory.ts` 82 处、app `memory.ts` 42 处（如 `method: 'GET' as any`） |

**已确认合规**：写命令 44 个均成对声明 `Idempotency-Key` + `x-sdkwork-idempotent: true`；9 个 DELETE 均无请求体；请求体无 `tenantId`；OpenAPI ↔ 路由 manifest ↔ axum handler **一一对应**（open 26 / app 42 / backend 82），无「声明未实现」或「实现未声明」；无 `type: integer, format: int64` 违规。

### 4.2 数据库与事务

| # | 问题 | 证据 |
| --- | --- | --- |
| P2-8 | `ai_record` INSERT 与 FTS 同步**非原子**（后者走 `self.pool()`，不在同一事务） | `plugins/.../store.rs:495-564`；`search_index.rs:80-101` → 崩溃窗口内 FTS 陈旧，检索静默丢结果 |
| P2-9 | supersede 为 **3 次独立写**，无事务 | `plugins/.../store.rs:567-647`（1 × create + 2 × UPDATE） |
| P2-10 | SQLite 分支直接返回「已获取」，无 PG 的 advisory lock 互斥 | `plugins/.../admin_tables.rs:844-859` |
| P2-11 | baseline 与 migration 漂移 | `database/ddl/baseline/postgres/0001_memory_baseline.sql:9` 声明的源迁移不存在；baseline 已含 `organization_id`，而 `database/migrations/postgres/0001_organization_id_not_null.up.sql:21-29` 冗余重复 |
| P2-12 | SQLite `ALTER TABLE ADD COLUMN` 无 `IF NOT EXISTS` | `tests/fixtures/database/sqlite/migrations/0009_memory_outbox_delivery_lease.up.sql:1-4` → 半应用后重跑永久失败 |
| P2-13 | 隔离级别未显式设置，SQLite 无 `BEGIN IMMEDIATE` | 两端均依赖默认（PG = read committed，SQLite = deferred）；需 `SELECT ... FOR UPDATE` 防丢更新的场景未见显式加锁 |
| P2-14 | 开发默认 Snowflake 节点号 = 0 | `plugins/.../store.rs:6107-6120`（`unwrap_or(0)`）→ 多副本同值碰撞（有主键约束兜底报错，但会报错） |
| P2-15 | `sqlx_compat.rs` 查询形状缓存用 `assert!` 做上界（上限 2048），且 `RwLock` 在读路径 | `plugins/.../sqlx_compat.rs:41-44` |
| P2-16 | 布尔解码失败静默转 `false` | `plugins/.../store.rs:6255-6267`（`unwrap_or(false)`）→ 掩盖类型/数据错误 |
| P2-17 | 迁移初始化探测吞错 | `plugins/.../store.rs:206-222`（`Err(_) => Ok(false)`）→ 瞬时错误触发重复迁移 |
| P2-18 | 双栈语义靠哨兵对齐而非同一约束 | PG 用 `NULLS NOT DISTINCT`（需 PG15+，`database/ddl/baseline/postgres/0001_memory_baseline.sql:481-483`），SQLite 用 `user_id = -1` 哨兵（`store.rs:6157-6168`） |

### 4.3 多副本进程内状态（高可用）

| # | 进程内状态 | 证据 | 多副本影响 |
| --- | --- | --- | --- |
| P2-19 | `NORMALIZED_SQL` 查询形状缓存 | `plugins/.../sqlx_compat.rs:9` | 仅性能，无害 |
| P2-20 | `COMPAT_OPERATION_SEQUENCE` 操作序号 | `plugins/.../consolidation.rs:17,38` | 极端下 operation_id 碰撞 |
| P2-21 | `DOMAIN_METRICS` / HTTP metrics 每 Pod 计数 | `domain_metrics.rs:4`、`metrics.rs:6` | 指标需在监控侧聚合 |
| P2-22 | `shared_iam_postgres_pool`（第 2 个 DB 池，锁跨 await） | `crates/sdkwork-routes-memory-support/src/readiness.rs:9`；`crates/sdkwork-memory-drive/src/bootstrap.rs:83` | 合并驱动预算不可控（正是 P0-4 的阻塞点） |
| P2-23 | 限流/幂等/准入：**dev/demo 完全缺失** | `crates/sdkwork-routes-memory-support/src/web_runtime.rs:25-27`（非 production-like 直接 `return layer`，连 request timeout 也不加） | 演示环境（POC/招标现场）无任何限流/幂等/审计，且 backend-api 下任意已认证主体获 `elevated_tenant_access=true`（`web_bootstrap.rs:44-47`）→ 对外演示有越权风险 |

### 4.4 客户端与前端

| # | 问题 | 证据 |
| --- | --- | --- |
| P2-24 | 53 处 `as unknown as` 强转，类型安全实质失效 | `apps/sdkwork-memory-pc/packages/*/src/sdk/index.tsx`（33 处）、`admin-core/src/sdk/index.tsx`（20 处）；`core/src/session/index.ts:30` |
| P2-25 | 懒加载 Admin 仅 `Suspense` fallback，**无 ErrorBoundary / 超时** | `apps/sdkwork-memory-pc/src/App.tsx:79,84` → chunk 加载失败整页崩 |
| P2-26 | locale 仅 `zh-CN` / `en-US`，与 PRD 七语种不符 | `apps/sdkwork-memory-pc/packages/*/config/runtime-config.ts:3,26-27`；`database/seeds/locales/` 下 de-DE/fr-FR/ja-JP/ko-KR/ru-RU 目录**全为空 `.gitkeep`** |
| P2-27 | 缺 `sdkwork-memory-pc-shell` 包，路由/AuthGate 组装散落在 `src/App.tsx` | 对照 `APP_PC_ARCHITECTURE_SPEC` 分层要求 |
| P2-28 | i18n 英文 fallback 文案硬编码 | `console-core`/`admin-core` 的 `action("create", "Create space", …)` |

### 4.5 安全与部署配置

| # | 问题 | 证据 |
| --- | --- | --- |
| P2-29 | 示例 env 固定弱口令 | `.env.postgres.example`：`SDKWORK_DATABASE_PASSWORD=sdkworkdev123`、`SDKWORK_DATABASE_ADMIN_USERNAME=postgres`、`SDKWORK_DATABASE_ADMIN_PASSWORD=postgres_admin_pass`、`SDKWORK_DATABASE_SSL_MODE=disable` |
| P2-30 | 图谱更新/删除只有 `tenant_id` 缺 `space_id` | `plugins/.../graph_store.rs:150,231,266`（依赖服务层前置检查，无纵深防御） |
| P2-31 | 敏感记录**先出库再判敏感度** | `crates/sdkwork-intelligence-memory-service/src/open_api.rs:471-485,1590-1617` → 违「查询前约束」，存在计数/时序侧信道 |
| P2-32 | `elevated_tenant_access` 对 private/sensitive **无条件放行** | `crates/sdkwork-intelligence-memory-service/src/access.rs:396-404` |
| P2-33 | retrieval trace 写入结果被丢弃 | `crates/sdkwork-intelligence-memory-service/src/open_api.rs:1483`（`let _ = ...append_retrieval_trace(...)`）→ 审计/可观测静默缺失 |
| P2-34 | nginx 边缘 body 上限 1.1 GB | `deployments/webserver/nginx.standalone.production.conf:13`（`client_max_body_size 1100m`）vs 应用侧 1 MiB → 边缘 DoS 面 |
| P2-35 | Drive uploader `content_length` 恒为 0 | `crates/sdkwork-memory-drive/src/uploader.rs:53,105`（`std::mem::take` 之后仍读 `request.body.len()`） |
| P2-36 | `export_upload_profile_code` 两分支都返回 `"document"`，恒常量 | `crates/sdkwork-memory-drive/src/uploader.rs:119-124` |

---

## 5. 虚假实现 / 未完成实现 专项核查结论

这是本次最关注的一项。**结论：核心能力未发现"假实现"；缺口是"显式拒绝"而非"静默假绿"。**

**已确认为真实实现**（非桩、非硬编码）：

- 检索融合：加权 RRF + 去重 + `[0,1)` 归一（`crates/sdkwork-memory-retrieval/src/retrieval/mod.rs:151-247`），五路打分（keyword / dictionary / sql-structured / time-recency / event）全部真实计算
- 上下文包：真实 token 预算裁剪 + 近重复 Jaccard 去重（`retrieval/context_pack.rs`）
- 导出：真实采集 + 敏感度过滤 + 字节上限守护写入；Drive 上传真实调用 uploader
- forget：`hard_delete_record_with_cleanup` / `forget_all_records_in_space` / `forget_records_for_user` / `forget_records_matching_query` 真实物理删除
- 迁移：`dry_run` / `shadow` 真实返回报告，`switch` / `promote` 真实切主并 `rebuild_all_record_search_indexes`
- 索引重建、保留期清理（含 dry_run）、合并去重：真实落库
- 评估：`retrieval_quality` 真实跑在线检索并计算 recall / precision / ndcg / MRR / p95
- Outbox：真实租约续租、ack、指数退避、死信；provider 健康探针真实 HTTP 探测
- 治理/权限：capability deny 优先、时间有效性校验、未知状态 fail-closed；SSRF 校验 + 私有 IP 拒绝 + 重定向禁用；配额原子准入
- SQLite/PG 的 `sqlx_compat.rs:50-156` 只做 `?` → `$n` 词法编号（含字符串/注释/`$tag$`/`?|`/`?&` 保护），**不转换语法**；因此所有 PG 专有语法都已按方言显式分叉（`RETURNING`、`FOR UPDATE SKIP LOCKED`、`to_tsvector`、`pg_try_advisory_xact_lock`），未发现泄漏到 SQLite 分支

**明确的能力缺口**（显式拒绝，不是假绿，但影响商用完整度）：

| 缺口 | 现状 | 证据 |
| --- | --- | --- |
| 评估引擎 | 仅 `retrieval_quality` 一种；其它 `evalType` 一律 `skipped` | `crates/sdkwork-intelligence-memory-service/src/job_worker.rs:571-597`（`"reason": "eval type is not implemented"`） |
| 评估数据集 | `datasetRef` 未接线，只支持内联 `config.cases` | 同上 `:605-611` |
| 乐观并发（If-Match / ETag） | ADR 标注 `proposed, not implemented` | `docs/architecture/decisions/ADR-20260721-optimistic-concurrency.md:3` |
| forget 统计 | `purgedEvents` 恒 0 | `app_backend_api.rs:724-728` |
| 上传 profile 映射 | 恒 `"document"` | `crates/sdkwork-memory-drive/src/uploader.rs:119-124` |

`plugins/sdkwork-memory-plugin-native-sql` 里确实存在一批 `"not implemented; refusing ... fallback"` 的错误返回，但那是 **SPI 的安全 fail-closed 默认实现**，native_sql 插件已逐项覆盖（`store.rs` 6577 行真实 SQL）。这是良好设计，不是缺陷。

---

## 6. 已确认合规的安全项（避免误伤）

- **无 SQL 注入**：`format!` 仅拼常量片段与宿主占位符（`store.rs:711,754` 插值 `RECORD_SENSITIVITY_FILTER_SQL` 常量）；LIKE 走 `escape_like_pattern` + `ESCAPE '\'`（`privacy.rs:13-29`）；FTS 走 `escape_fts5_query`（`search_index.rs:398-407`）；ORDER BY / 表名 / 列名全为常量
- **SSRF 防护完整**：解析后 IP 校验、拒私有/环回/链路本地/`169.254.0.0/16`/`fc00::/7`/IPv4-mapped、拒 mixed DNS、`resolve_to_addrs` 固定地址、`redirect::Policy::none()`
- **错误不泄漏内部信息**：`ProblemDetail` 只暴露数字 `code` + 服务端 `traceId`；`sdkwork-utils/packages/sdkwork-utils-rust/src/http_api.rs:1064-1073` 对 `InternalError`/`ServiceUnavailable` 掩码 detail；PC `readSafeError` 只取 `code` + `traceId`
- **无用户可达的缺 tenant 谓词查询**：穷尽检索命中的 8 处（outbox claim/恢复、learning/eval 恢复、FTS rowid 删除、schema bookkeeping、Drive 全局 provider 配置）均为 worker / schema 元数据 / 全局配置路径，非业务数据读
- **凭据只存引用**：provider / S3 走 `credential_ref` / K8s Secret；`deployments/kubernetes/secret.example.yaml` 全为 `REPLACE_ME` 占位（5 处）；未发现真实密钥或硬编码 token 入库
- **PC 无 XSS 面**：无 `dangerouslySetInnerHTML`、无 `eval`、无 `new Function`
- **SDK 消费边界干净**：Console 树 → Backend SDK **0 命中**（含 type-only / 动态 import）；PC 全树无 `fetch(`/`axios`/`XMLHttpRequest` 业务调用；Backend SDK 唯一落点是被懒加载引用的 `admin-core/src/sdk/index.tsx:2`

---

## 7. 行业对标差距（vs Mem0 / Zep / Letta(MemGPT) / LangMem / AWS AgentCore Memory）

本仓的**架构取向是健康的**：provider 中立 SPI、无强制向量库、证据/规范记忆/候选/习惯分层、租户隔离前置、审计与隐私工作流完备。差距集中在"记忆产品的上层语义能力"与"运营证据"：

| 能力维度 | 行业头部做法 | 本仓现状 | 差距 |
| --- | --- | --- | --- |
| 记忆老化 / 强度衰减 | 遗忘曲线、访问强化、TTL 分层（Zep/Mem0 均有） | 有 `time_recency_score` 时序打分（`retrieval/mod.rs:323`），但**无策略化的衰减/强化模型** | 中 |
| 矛盾检测与自动失效 | 抽出新事实时对比旧事实、自动 supersede/invalidate | 有 supersede 机制（`store.rs:567-647`）与候选/整合流程，但**非原子且无矛盾判定的显式算法证据** | 中 |
| 时序推理 | "上周三之后""上个月提到的" 语义时间解析 | 有事件时间戳与 UTC 规范（`datetime.rs`），未见自然语言时间区间解析 | 中 |
| 记忆质量评估体系 | 多维度（召回/精确/NDCG/MRR + 冲突率 + 陈旧率 + 成本） | 仅 `retrieval_quality` 一种，无陈旧率/冲突率/成本维度 | 高 |
| 多模态记忆 | 图像/音频/文档记忆 | 无（PRD 未列为目标） | 低（可接受） |
| 用量计量与计费 | per-tenant token/调用计量、配额扣减、账单导出 | 有 subjects/bindings/capability/quota 与原子准入，但**未见用量计量与账单导出实现** | 高（商用必需） |
| 压测 / soak / 容量证据 | 公开 latency SLO 曲线、容量模型 | **完全缺失** | 高（商用必需） |
| 多区域 active-active | Zep Cloud 等 | PRD 明确 non-goal（合理） | 低 |
| 数据驻留 / 合规 | 区域隔离、DSR 自动化、GDPR 报告 | 有 region 概念、审计、export/forget 工作流；未见 DSR SLA 与合规报告产出 | 中 |

---

## 8. 商业化落地能力评估

**结论：不具备。** 按 5 个维度打分（0–5）：

| 维度 | 评分 | 依据 |
| --- | --- | --- |
| 功能正确性 | **3** | 核心算法真实可用；但 P0-1 让 7 个列表过滤能力在生产断路 |
| 契约与规范一致性 | **2.5** | 信封/错误码/幂等/分页形态高度规范；但参数命名与 int64 元数据偏离，且门禁覆盖不到 |
| 并发与高可用 | **2** | PG 路径有租约围栏 + SKIP LOCKED（良好）；但 SQLite 单连接、并发 256 ≫ 池 16、任务无重试/死信、优雅停机 3s |
| 安全与合规 | **3.5** | SSRF/注入/错误掩码/密钥引用都很扎实；扣分在跨租户无测试、敏感数据先取后判、生产明文 HTTP |
| 可交付与运营 | **2** | 发布闸门为红、PC 构建链断裂、无压测/容量证据、无用量计费 |

**综合：2.6 / 5 —— 内部 RC 水位，距商用底线（约 4.0）有明显差距。**

对照 PRD 自述（"internal release candidate... publication remains blocked until..."），本审计**独立复现了该结论**，并额外发现 PRD 未列出的 4 项阻断（P0-1 契约断路、P0-2 SQLite 权威缺失、P0-3 单连接、P0-5 PC 构建链断裂）。

---

## 9. 改进方案

### 阶段 A — 解阻断（建议 1–2 周，必须全绿才可谈发布）

1. **修 P0-1 契约断路**：7 处 query 参数改 `lower_snake_case` → 重跑 `pnpm sdk:generate` → 补一条门禁「OpenAPI query 参数名 ⊆ 契约结构体字段（含 alias）」，并回填到 `sdkwork-specs/tools/`，否则同类问题会复发。
2. **修 P0-2 SQLite 权威**：`tests/fixtures/database/sqlite/migrations/*` → `database/ddl/baseline/sqlite/`；`database.manifest.json` 与 `contract/schema.yaml` 的 `engines` 增列 `sqlite`；`database/migrations/sqlite/` 落地；parity 门禁加 ENOENT 断言 + 表/列/索引/约束集合等价断言。
3. **修 P0-3 档位声明**：`specs/component.spec.json` 声明 `sqlite = single_writer_local`；生产 profile 检测到 SQLite 直接 fail-fast；若确需并发则 WAL + `busy_timeout` + `BEGIN IMMEDIATE` + 并发写压测证据。
4. **修 P0-4 发布闸门**：产出 `specs/process-database-pool.spec.json`；合并进程内三个 DB 池为单一受预算约束的宿主池（`readiness.rs:9`、`drive/bootstrap.rs:83`）；清理候选包过期 digest 声明；把 `check_sdkwork_memory_release_readiness.mjs` 接入 CI 必过项。
5. **修 P0-5 构建链**：补齐或移除 `materialize-runtime-env.mjs`；`pnpm install` 后跑通 `pnpm --dir apps/sdkwork-memory-pc check` 与根 `pnpm check` / `pnpm verify`。

### 阶段 B — 达到商用底线（建议 3–5 周）

6. **容量对齐**：`DEFAULT_MAX_CONCURRENCY` 与连接池联动（建议并发 ≤ 池 × 2，并设 `acquire_timeout`）；worker 并发独立配额，禁止与请求路径争抢池；外层加独立 `TimeoutLayer` 兜底。
7. **任务可靠性**：`ai_learning_job` / `ai_eval_run` 增 `attempt_count` + `max_attempts` + `dead_letter` 状态；过期重入带次数上限；瞬时失败走指数退避重试而非直接 `failed`；对齐 Outbox 已有的退避/死信能力。
8. **优雅停机**：worker drain 超时提到 ≥ 单批最长执行时间（或让长作业支持 checkpoint 续跑），HTTP 侧先停新请求再 drain。
9. **传输安全**：nginx 拆 443/80 两个 server 块（80 → 301），加 HSTS + CSP + `X-Content-Type-Options`；`client_max_body_size` 收敛到与应用一致的量级。
10. **仓库卫生**：`git rm --cached` 清理 302 个被 gitignore 声明的生成物与就地 tsc 产物，改为构建期生成 + CI 校验一致性（解决"改契约不重生成 → 静默用旧产物"）。
11. **无界请求**：forget `memory_ids` 加 `MAX_FORGET_MEMORY_IDS`（对齐 `MAX_SCOPE_SPACE_IDS=32`）；`purged_events` 改为真实计数。
12. **索引补齐**：4 张队列表的 claim 排序索引 + `ai_retrieval_trace(tenant_id, space_id, created_at DESC, id DESC)`；关键词/敏感词检索改 FTS/trigram 或显式接受全扫描并写容量模型。
13. **跨租户红队测试**：补一组租户 A 访问租户 B 的 space/record/entity/edge/policy/audit 的负向用例，覆盖 open/app/backend 三面。

### 阶段 C — 对齐头部产品（建议 6–10 周）

14. 事务化写入：`ai_record` + FTS + outbox 收敛到单一事务；supersede 三步合并为一个事务。
15. 多副本状态外置：SQL 形状缓存、operation 序号、指标聚合、并发准入全部外置或声明为可聚合；dev/demo 也装载最小安全策略（至少限流与审计）。
16. 契约元数据补齐：`totalItems` 的 `format: int64` + `x-sdkwork-int64-string`；29 处 `page_size` 补 `default: 20`；`serde_int64` 收敛到 `sdkwork_utils_rust::serde_int64`；重新生成 `sdks/sdkwork-memory-app-sdk/openapi/*` 消除漂移。
17. 评估引擎扩展：数据集引用、多 eval 类型（陈旧率/冲突率/一致性）。
18. 前端质量：消除 53 处 `as unknown as`；lazy chunk 加 ErrorBoundary；补 `sdkwork-memory-pc-shell`；补齐 5 个缺失语种。
19. 记忆语义增强：记忆衰减/强化模型、矛盾检测与自动失效、自然语言时间区间解析。

### 阶段 D — 商用发布前置（与 C 并行）

20. **运营证据**：load / soak 脚本 + 公开 latency SLO 曲线 + 容量模型；跨租户隔离红队报告。
21. **计量与计费**：per-tenant 用量计量（调用/token/存储）、配额扣减、账单导出。
22. **合规**：数据驻留区域校验、DSR SLA、合规报告产出。
23. **交付**：不可变 OCI / 浏览器制品、签名与 OIDC 证明、SBOM 与 provenance、部署冒烟、回滚演练记录（全部转为 CI 必过门禁）。

---

## 附录 A — 本次实跑的命令与输出

```text
$ node ../sdkwork-specs/tools/check-pagination.mjs --workspace .
pagination check passed (1 repo root(s))                       EXIT=0

$ node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
api response envelope check passed                             EXIT=0

$ node ../sdkwork-specs/tools/check-api-operation-patterns.mjs --workspace .
api operation patterns check passed                            EXIT=0

$ node ../sdkwork-specs/tools/check-app-sdk-consumer-imports.mjs --workspace .
app SDK consumer import checks passed                          EXIT=0

$ node tools/check_sdkwork_memory_release_readiness.mjs
[release-readiness] warning: process-shared database pool: specs/process-database-pool.spec.json must exist
[release-readiness] warning: release candidate is correctly blocked from production publication
[release-readiness] error: disabled candidate package container-x64-standalone-container-oci must not retain a stale immutable digest claim
[release-readiness] error: disabled candidate package container-x64-cloud-container-oci must not retain a stale immutable digest claim
[release-readiness] error: release-candidate manifests must keep publication packages disabled until immutable evidence exists
[release-readiness] failed (3 error(s))                        ← 自家闸门为红

$ ls node_modules apps/sdkwork-memory-pc/node_modules
ls: cannot access 'node_modules': No such file or directory
ls: cannot access 'apps/sdkwork-memory-pc/node_modules': No such file or directory
                                                               ← pnpm install 未执行，check/verify 不可复现

$ ls apps/sdkwork-memory-pc/scripts/
package-web-release.mjs
                                                               ← prebuild 引用的脚本不存在

$ git ls-files "sdks/**/generated/server-openapi/src/**/*.js" | wc -l     → 151
$ git ls-files "sdks/**/generated/server-openapi/src/**/*.d.ts" | wc -l   → 151
$ git ls-files "sdks/**/generated/**" | wc -l                             → 1136
                                                               ← 与 .gitignore:78-82「never committed」矛盾

计数器（自写脚本，Python 解析三份 OpenAPI）：
  camelCase query 参数：open 5 / app 2 / backend 7   → 共 14 个，涉 7 个 operation
  page_size 参数总数 35，缺 default=20 者 29，maximum!=200 者 0
```

## 附录 B — 已穷尽的检索结论

- Console 代码树 → Backend SDK：**0 命中**（含 static / dynamic / type-only import）
- PC 全树业务裸 HTTP（`fetch(` / `axios` / `XMLHttpRequest`）：**0 命中**（唯一 `fetch` 在 `core/src/config/runtime-config.ts:16-17`，取 `/runtime-env.json` 静态配置）
- 用户可达的「缺 tenant_id 谓词」业务查询：**0 处**（命中 8 处均为 worker / schema / 全局配置路径）
- SQL 注入：**0 处**（`format!` 拼 SQL 全为常量片段）
- 真实密钥 / 硬编码 token 入库：**0 处**

## 附录 C — 与本审计相关的权威规范

`sdkwork-specs/` 下：`PAGINATION_SPEC.md`（§14.1.1 query 命名、§16.6 分页模型）、`API_SPEC.md`（§13.6 int64 线格式、响应信封、错误模型）、`DATABASE_SPEC.md`（baseline 权威、keyset 分页）、`SECURITY_SPEC.md`、`PRIVACY_SPEC.md`、`PERFORMANCE_SPEC.md`、`TEST_SPEC.md`、`DOCUMENTATION_SPEC.md`。

---

**新增文件说明**：本文档为审计新增，**尚未登记到 `docs/INDEX.yaml`**。若要入库为正式评审记录，需按 `DOCUMENTATION_SPEC.md` §2 补登记，并同步 `docs/engineering/reviews/README.md`。
