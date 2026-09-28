# mem0 线面（API/SDK）兼容：回归结论与落地工单

- 日期：2026-09-28
- 范围：`sdkwork-memory` 仓（HEAD `b799b49`，`main`，工作树干净）
- 上游参照：`external/mem0` @ `47a69e1`（v2.2.1 附近）
- 前置文档：[`REVIEW-20260923-mem0-capability-parity-matrix.md`](./REVIEW-20260923-mem0-capability-parity-matrix.md)

---

## 0. 两份结论，必须分开读

用户诉求里有两条独立的线，此前的工作只覆盖了第一条：

| 线 | 含义 | 现状 |
| --- | --- | --- |
| **A. 能力对齐** | 把 mem0 的算法与语义搬进本仓自有架构（BM25 / 实体 / 加法归一 / ADD-only 写入 / filter 语言 / 过期语义…） | ✅ 矩阵口径「缺失 0 / 部分 0」（批次 1–19），本轮抽检成立 |
| **B. 线面兼容** | 让 **mem0 官方 SDK 不改代码** 连上本仓 | 🔴 **完全不存在**。本轮首次取证 |

**A 不等于 B。** 矩阵 §1 的「公开 API 面对照」是**能力映射表**（mem0 的 `add/search/...` 语义 ↔ 本仓的 SPI/HTTP 语义），它从未主张一条 mem0 形状的 HTTP 线面。B 是本轮新提出的、可证伪的要求。

---

## 1. 回归实况（本机实测，非文档自述）

### 1.1 Rust 全量测试

```
cargo test --workspace -j 2
EXIT=0
779 passed / 0 failed / 0 ignored
```

> 取证纪律：首轮我把输出接了 `| tail -120`，导致 **管道退出码 = `tail` 的退出码**、且日志只剩尾部。
> 该做法**不足以定论**，已重跑并完整落盘、单独取 `cargo` 的真实退出码。上表是重跑结果。

### 1.2 门禁套件（`_sdkwork:check` 的 15 条，逐条取退出码）

| 门禁 | 退出码 |
| --- | --- |
| `check:app-composition` | 0 |
| `check:architecture-alignment` | 0 |
| `check:crate-inventory-standard` | 0 |
| `check:release-readiness` | 0 |
| `check:pnpm-script-standard` | 0 |
| `check:agent-workflow-standard` | 0 |
| `check:repository-docs` | 0 |
| `check:pagination` | 0 |
| `check:api-envelope` | 0 |
| `check:api-operation-patterns` | 0 |
| `check:sdk-standard` | 0 |
| `topology:validate` | 0 |
| `db:validate` | 0 |
| `db:pool:validate` | 0 |
| `check:db-dialect-parity` | 0 |

**假绿排除**：`check:crate-inventory-standard` 实测读数 `20 first-level crate directories scanned / 20 Cargo.toml / 20 declared -> 20 resolved / 40 units examined`，非 `scanned: 0`。

### 1.3 能力对齐抽检（不信文档，独立复核）

| 矩阵声明 | 复核方式 | 结果 |
| --- | --- | --- |
| §13 P0「插件不可达」已解 | `grep sdkwork-memory-plugin-search-first-vector --include=Cargo.toml` | ✅ `crates/sdkwork-intelligence-memory-service/Cargo.toml:24` 依赖它 |
| 批次 9「加法归一栈可达」 | `grep score_candidates_additive` | ✅ 生产调用点 `crates/sdkwork-intelligence-memory-service/src/open_api.rs:1857` |
| 批次 15/17「provider 已装配」 | `grep with_embedder\|with_llm` | ✅ `crates/sdkwork-api-memory-assembly/src/bootstrap.rs:297,299` |
| 批次 12「metadata_json 写入路径关闭」 | `grep metadata_json plugins/.../canonical_data.rs` | ✅ INSERT 段（`:162`）含该列，supersede 幂等比对含它（`:557`） |

---

## 2. 缺口取证：mem0 官方 SDK 连不上本仓

### 2.1 官方客户端说的是平台 REST

从 `external/mem0/mem0/client/main.py`（Python）与 `mem0-ts/src/client/mem0.ts`（TS）逐条抄出的端点全集，**两个 SDK 完全一致**：

| mem0 SDK 调用 | 方法 + 路径 | 请求体要点 | 响应体要点 |
| --- | --- | --- | --- |
| `ping` | `GET /v1/ping/` | — | `{...}` |
| `add` | `POST /v3/memories/add/` | `{messages:[{role,content}], ...kwargs}` | v1.1 `{results:[...]}` |
| `search` | `POST /v3/memories/search/` | `{query, output_format:"v1.1", filters, top_k, threshold, explain, rerank, show_expired}` | `{results:[...]}` |
| `get_all` | `POST /v3/memories/?page&page_size` | `{filters, ...}` | `{count,next,previous,results:[...]}` |
| `get` | `GET /v1/memories/{id}/` | — | memory 对象 |
| `update` | `PUT /v1/memories/{id}/` | `{text?, metadata?, timestamp?, expiration_date?}` | memory 对象 |
| `delete` | `DELETE /v1/memories/{id}/?delete_linked` | — | 任意 dict |
| `delete_all` | `DELETE /v1/memories/?user_id|agent_id|run_id|app_id` | — | `{message}` |
| `history` | `GET /v1/memories/{id}/history/` | — | `[{id,memory_id,input,old_memory,new_memory,event,...}]` |
| `users` | `GET /v1/entities/` | — | `{count,results,next,previous}` |

**鉴权头**（`mem0/client/main.py:189`）：`Authorization: Token <api_key>`，另带 `Mem0-User-ID`、`X-Mem0-Client`。

**响应类型**（`mem0-ts/src/client/mem0.types.ts`）：`Memory{id,messages?,event?,memory?,userId?,hash?,createdAt?,updatedAt?,score?,metadata?,expirationDate?,...}`、`MemoryHistory{oldMemory,newMemory,event,...}`、`PaginatedMemories{count,next,previous,results}`。

> 注意：`MemoryClient` 走的是**平台面**。mem0 自托管 server（`external/mem0/server/main.py` 的 `POST /memories`、`POST /search`、`PUT /memories/{id}`）是**另一套形状**，官方 SDK 并不用它。

### 2.2 本仓提供的是完全不同的面

| 面 | 前缀 | 鉴权 |
| --- | --- | --- |
| open-api | `/mem/v3/api/memory/*` | `X-API-Key`（`apis/open-api/*.openapi.json` 的 `securitySchemes.ApiKey`） |
| app-api | `/app/v3/api/memory/*` | 双令牌 |
| backend-api | `/backend/v3/api/memory/*` | 双令牌 |

**零命中证据**（全仓 `crates/ plugins/ apis/ apps/ specs/ tools/ scripts/`）：

```
grep -rn "v1/memories|v3/memories|/v1/ping|mem0-compat|mem0_compat" \
  --include=*.rs --include=*.json --include=*.mjs --include=*.ts
→ 唯一命中全部是 plugins/sdkwork-memory-plugin-native-sql 的 `sqlx_compat`（无关）
```

⇒ 无 mem0 路径、无 `Token` 鉴权、无 mem0 响应形状。**官方 SDK 一条调用都跑不通。**

---

## 3. 落地方式：走 SDKWork 规范已有的「Vendor Compatibility open-api」通道

**这不是自创旁路。** `sdkwork-specs/API_SPEC.md` §4.5.2 定义了一等公民形态：

- 允许 `/v1/`、`/anthropic/v1/` 这类**上游协议前缀**（"`SHOULD` group exempt operations under dedicated path prefixes such as `/v1/`"）；
- `MAY` 绕过 `SdkWorkApiResponse` 信封、section 14.1 列表词表、`ProblemDetail`；
- `MUST` 逐操作声明 `x-sdkwork-wire-protocol: external` + `x-sdkwork-external-protocol-id`（小写 kebab-case，如 `openai-v1`）；
- 门禁侧已实现豁免：`check-api-response-envelope` 的 `isExternalProtocolOpenApi()` 在**全部操作都是 external** 时整个文档跳过；`classifyStructuredOpenApiEnvelope()` 对混装文档只校验非 external 的操作。

**工作区已有成熟先例**：`sdkwork-cloudrouter` 的 `apis/open-api/cloudrouter/cloudrouter-open-api.openapi.json` —— 167 条操作、11 个协议 id（`openai-v1`、`anthropic-messages`、`google-gemini-v1beta`…），路由清单 `prefix: "/v1"`，路径就是裸 `/v1/chat/completions`。本仓照此办理。

拟用协议 id：**`mem0-platform`**。

---

## 4. 实施工单（按序，每步各自可验证）

### 步骤 1 · 契约（先定线面，再写实现）

| # | 文件 | 改动 |
| --- | --- | --- |
| 1.1 | `tools/materialize_phase1_contracts.mjs` | 在 `writeOpenApi()` 内新增一组 `mem0Operation({...})`：`authMode: "api-key"`（**不用** `compatibility`——后者会要求 `HttpRoute::Compatibility` 必须携带 `compatibility_auth` + `compatibility_openapi_operation` 精确上游 JSON，见 `sdkwork-web-contract/src/lib.rs:241` `validate_compatibility_contract()`），并注入 `x-sdkwork-wire-protocol: external` + `x-sdkwork-external-protocol-id: mem0-platform` |
| 1.2 | 同上 | `extractRoutesFromOpenApi()` 透传这两个标记；`writeRouteManifestJson()` 写进路由清单条目（cloudrouter 的清单条目就是直接带这两个键） |
| 1.3 | 同上 | 新增 `mem0Paths()`：`/v1/ping/`、`/v3/memories/add/`、`/v3/memories/search/`、`/v3/memories/`、`/v1/memories/{memory_id}/`、`/v1/memories/{memory_id}/history/`、`/v1/memories/` |
| 1.4 | 同上 | 新增 `mem0Schemas()`：上游形状的 `Memory`、`PaginatedMemories`、`MemoryHistory`、`PingResponse`（`memory`/`created_at`/`updated_at` 等**逐字段照上游**，不掺 SDKWork 信封） |
| 1.5 | `tests/contracts/open_api_prefix_contract_test.mjs` | 路径断言放行 external 操作（`/v1`、`/v3` 前缀）。**这是本工单里唯一需要动既有测试的地方**，理由写进测试注释并引 API_SPEC §4.5.2 |

验证：`node tools/materialize_phase1_contracts.mjs` → `pnpm check` 全绿 + `node --test tests/contracts/route_manifest_openapi_parity_test.mjs`。

### 步骤 2 · 鉴权桥（`Token` → 框架认得的形状）

框架只认 `X-Api-Key` / `Authorization: Bearer` / `Access-Token`（`sdkwork-web-core/src/constants.rs:13-14`），且**显式拒绝**在 api-key 面上出现凭据头（`open_api_auth.rs` 的 `credential-profile-contamination`，已有测试钉住）。

做法：一个最外层 axum 中间件，**仅对 mem0 路径**做 header 改写 ——
`Authorization: Token <k>` → 删除 `Authorization`，写入 `X-Api-Key: <k>`。
改写后完全复用既有鉴权栈（dev-inline / IAM database / production fail-closed），**不改 sdkwork-web-framework**（跨仓，且非本仓所有权）。

### 步骤 3 · 处理器（uuid 寻址）

**关键事实**：本仓 HTTP 面此前按内部数字 id 寻址，而 mem0 的 `id` 由本仓决定 —— 应返回记录的 **`uuid`**，而不是把内部行 id 泄漏到公开面（批次 14 已把图谱面收敛到 uuid 级：「内部行 id 不再跨越边界」）。

存储层已具备 uuid 寻址：`plugins/sdkwork-memory-plugin-native-sql/src/canonical_data.rs` 按 `(tenant_id, space_id, uuid)` 查询（`:116`、`:136`、`:164`）；SPI 端口 `RetrieveMemoryRecordQuery{scope, memory_id:String}`（`crates/sdkwork-memory-spi/src/ports.rs:187`）的 `memory_id` 即 uuid。

| mem0 端点 | 落到本仓 |
| --- | --- |
| `POST /v3/memories/add/` | `MemoryOpenApi::create_memory`（`infer` 走既有抽取管线，无 provider 时确定性模式） |
| `POST /v3/memories/search/` | `create_retrieval` → 映射 `{id,memory,score,metadata,createdAt,updatedAt}` |
| `POST /v3/memories/` | `list_memories` → 映射 `{count,next,previous,results}` |
| `GET/PUT/DELETE /v1/memories/{id}/` | `retrieve_memory` / `update_memory` / `delete_memory` |
| `DELETE /v1/memories/` | `delete_all_memories` → `{message}` |
| `GET /v1/memories/{id}/history/` | `MemoryEventStorePort` 版本链 → `{old_memory,new_memory,event}` |
| `GET /v1/ping/` | 常量 |

响应 `Content-Type: application/json` + **裸对象**（`axum::Json`），不得经 `ok_resource_json`。

### 步骤 4 · 挂载与面资格

- `crates/sdkwork-routes-memory-open-api/src/mem0/`（新模块）+ `routes.rs` 里 `.merge(...)`；
- `web_bootstrap.rs` 的 `memory_open_api_prefixes()` 增加 `/v1`、`/v3`（否则框架不把这两族当 open-api 处理）。

### 步骤 5 · 验收

Rust 契约测试（路径、鉴权头、请求体、响应体逐字段对拍，零网络）+ **官方 SDK 真跑通**。

⚠️ **本机约束（已实测）**：`netstat` 无 `:5432` 监听、无 `psql`/`pg_ctl`，`.env.postgres` 不存在 ⇒ **真 server 起不来**（standalone.development.env 明确要求 server 角色走 PostgreSQL）。因此 E2E 走：
新增 `crates/sdkwork-memory-integration-tests/tests/mem0_compat_e2e.rs` —— 用既有 `new_seeded_in_memory_store()` 组装 router，`TcpListener` 绑 `127.0.0.1:0`，`axum::serve` 后台跑，把实际端口打到 stdout；shell 侧读端口后驱动官方 `mem0ai`（Python，`MemoryClient(host=...)`）打 add/search/get/get_all/update/delete/history。

```sh
pip install mem0ai        # 或 uv/pipx；走 Clash 代理
```

---

## 5. 风险与边界（诚实登记）

1. **`prefix` 语义**：open-api 路由清单的 `prefix` 现为 `/mem/v3/api`，加入 `/v1`、`/v3` 后不再覆盖全部路径。本轮**未找到**强制「清单前缀必须覆盖每条路径」的门禁（`check-component-api-surface-prefixes.mjs` 只校验 `apiSpec.apiSurfaces` 的合成前缀与依赖前缀），但这属于约定性缺口，落地时需实测确认并按 cloudrouter 的先例（`prefix: "/v1"`）决定取值。
2. **混合线协议的权威文档**：工作区现有各仓 open-api 权威**都是单一协议**（cloudrouter 167/167 external；本仓 100% owned）。本仓将成为**第一个混装**的。规范侧支持（`classifyStructuredOpenApiEnvelope` 显式处理混装），但无先例，需以门禁实测为准。
3. **平台面独有族**（`/v2/profiles/*`、`/v1/exports/*`、`/v1/batch/*`、`/api/v1/webhooks|orgs/*`）属 mem0 平台账号能力，本仓无对应域。本轮先把**记忆 CRUD + search + history + entities + ping** 打通；其余按「显式 501/明确不支持」处理，不做静默假成功。
4. **不做未声明旁路**：不改 `sdkwork-web-framework`（跨仓）；不在网关直挂未进路由清单的路由（那正是本仓反复打击的「不可达/未声明能力」反模式）。
5. 本工单**尚未开工**。工作树是**多会话共享面**（另一会话在改 `apps/**`），故不在未完成状态下动 `main`。

---

## 6. 与既有结论的关系

- 本工单**不改变**矩阵「能力对齐 缺失 0 / 部分 0」的判定 —— 那一条仍然成立。
- 本工单关闭的是矩阵 §1 **没有覆盖的新维度**：线面（wire）兼容。
- 落地后，矩阵应新增一节记录 `mem0-platform` 协议面，并把 §1 的「本仓对应」列补上「官方 SDK 可直接接入」的证据链。

---

## 7. 施工回归（2026-09-28 落地实况）

§1–§6 是开工前的取证与工单；本节记录**已落地**的部分，以及落地过程中实测出来、§1–§6 没有预见的三处硬缺陷。

### 7.1 三处硬缺陷（都已修，每处都有可复核证据）

#### 7.1.1 🔴 框架把本层的 mem0 错误体整个换掉

**症状**（E2E 实测响应体）：删掉一条 memory 后再读，应回 `{"detail":"memory not found"}`，实测回
`{"code":40401,"detail":"Not found",…}`；本层的**具名拒绝**（501）实测回 `{"code":40002,"detail":"Malformed request",…}`。

**根因**在框架侧 `sdkwork-web-core/src/problem.rs::normalize_problem_response`，它对所有 4xx/5xx 做二分：

```rust
if is_problem_json(response) { enrich_problem_response(..) }   // body 保留
else { replace_with_standard_problem(status, ..) }             // body 丢弃
```

被丢的正是 `detail`；且 `result_code_from_status` 把 501 这类未映射状态一律折叠成 `InternalError`，所以一个**刻意**的 501 边界会被改写成 `code 50001` / "An internal error occurred" —— 对调用方和运维都是假信息。

**修法**（不动框架）：`Mem0Error` 的响应体在**进程内**声明为 `application/problem+json`（`mem0::error::MEM0_ERROR_CONTENT_TYPE`），让框架走 "enrich" 分支——`enrich_problem_response` 只补 `instance`/`operationId`/`traceId`，**从不覆盖 `detail`**——再由最外层的 `mem0_problem_document_bridge` 把 media type 改回 `application/json`（mem0 Python 客户端只在 `application/json` 下解析 body，`mem0/client/utils.py::_handle_http_error`）。**载荷一字未改。**

**证据**：`tests/mem0_wire_flow.rs::mem0_platform_wire_serves_the_official_client_flow` 第 9 步现在取到 `memory not found`；新增的 `Response::assert_json_media_type` 在每处失败上钉住 media type（媒体类型是契约的一部分，不是实现细节——mem0 客户端据它决定解不解析）。

#### 7.1.2 🔴 `ai_space` 唯一索引：兼容空间不能用 `space_type='personal'`

`uk_ai_space_owner_type ON (tenant_id, owner_subject_type, owner_subject_id, space_type)` ⇒ **一个主体每种 `space_type` 只能有一个空间**。原实现用 `personal` 建兼容空间，与主体自己的个人空间直接撞索引：

```
error returned from database: (code: 2067) UNIQUE constraint failed:
ai_space.tenant_id, ai_space.owner_subject_id, ai_space.owner_subject_type, ai_space.space_type
```

**修法**：`MEM0_SPACE_TYPE = "mem0"`（`space_type` 是开放列、无 `CHECK`、无授权决策读它）。这是**常态路径**而非边界：任何已拥有个人空间的主体第一次调 mem0 就会撞。同时把 `create_space_atomic_with_quota` 的 `QuotaExceeded` 从 `?` 吞掉改成具名上报，并处理首次并发建空间的唯一冲突（重查后判定）。

#### 7.1.3 🔴 B8 上下文选择器守卫把整条 mem0 线面拒掉（跨仓裁定）

见 §7.2。

### 7.2 B8 冲突与框架豁免（本轮唯一跨仓改动）

**实测**（真实 HTTP 语义，带 `content-length`）：

```
POST /v3/memories/add/  {"messages":[…],"user_id":"alice","agent_id":"planner"}
→ 400 {"detail":"client must not supply context selector body field `user_id`",
       "failedStage":"request-size-limit"}          ← 框架，处理器根本没跑
DELETE /v1/memories/?user_id=bob
→ 400 {"detail":"client must not supply context selector query parameter `user_id`",
       "failedStage":"surface-classification"}
```

守卫 = `client_context_guard`，键表 `{tenant_id, organization_id, app_id, user_id, session_id, …}`，作用于 `AppApi | OpenApi | InternalApi | GatewayApi` 四种面。

**为什么必须解决**：`add(messages, user_id=…)` / `app_id=…` 是官方 SDK 最基本的一次调用，`delete_all(user_id=…)` 同样是标准用法。`agent_id`/`run_id` 不在键表内，所以碰撞面恰好是 `user_id` + `app_id`，但它们是**必然**碰撞。上游客户端固定，改名不是选项。

**为什么本仓绕不过**：`Unknown` 面 fail-closed（`interceptors.rs` 直接 401）；`GatewayApi`/`BackendApi` 要求 dual-token 路由档案，与 `api-key` 清单冲突；`classify_api_surface` 在框架内。工作区也**没有**既有的 B8 豁免机制（`RouteAuth::Compatibility` 只豁免响应归一化）。

**裁定（用户批准）**：框架侧加**显式、声明式、默认关闭**的豁免。

- 新增 `WebRequestContextProfile.external_protocol_prefixes: Vec<String>` 与 `is_external_protocol_path()`；匹配规则与 profile 内其余前缀一致（声明 `/v1` 覆盖 `/v1` 与 `/v1/...`，**不**覆盖 `/v1beta`）。
- 两处运行时守卫各自跳过：`SurfaceClassification` 的查询守卫、`RequestSizeLimit` 的 body 守卫。**其余一律不动**——身份投影头（`x-sdkwork-*`）仍拒、鉴权/授权/租户隔离/凭据档案白名单原样；跳过 body 检查不 buffer body，所以请求体原样留给 extractor。
- 本仓 `web_bootstrap` 声明 `external_protocol_prefixes = memory_open_api_external_protocol_prefixes()`（即 `/v1`、`/v3`）。

框架侧测试（`sdkwork-web-core`，全部通过）：

| 测试 | 钉住什么 |
| --- | --- |
| `pipeline_contract_tests::declared_external_protocol_prefix_suspends_the_context_selector_guard` | **未声明**时 query / body 两种形状仍各自在原来的 stage 被拒（含 `user_id` 断言）；**声明**后同样两个请求正常鉴权，且 body **未被消费** |
| `request_context::…::nothing_is_exempt_until_a_prefix_is_declared` | 默认 profile 不豁免任何路径 |
| `request_context::…::a_declared_prefix_covers_itself_and_its_subtree_only` | `/v1beta`、`/v10`、SDKWork 自有前缀**不**被误豁免 |

框架改动清单（**未提交**）：`crates/sdkwork-web-core/src/request_context.rs`、`interceptors.rs`、`pipeline_contract_tests.rs`（+223 / −11）。

### 7.3 ⚠️ 教训：`oneshot` 夹具必须自带 `content-length`

上一轮的 E2E 是**假绿**。`Body::from(String)` 造的进程内请求不带 `content-length`，而框架的 body 守卫用 `content_length_from_headers(..) > 0` 判断「这个请求有没有 body」⇒ 守卫整段跳过。真实 HTTP/1.1 客户端必带该头，于是**测试全绿而线上每个 `add`/`search`/`get_all` 都会 400**。

已修：`tests/mem0_wire_flow.rs::send` 显式设置 `content-length` 并注明理由。

> 判据：任何用 `oneshot` 驱动框架的测试，若断言依赖「框架是否读到请求体」，都必须自己带上 `content-length`（或 `transfer-encoding`）。否则测的不是线上的那条路径。

### 7.4 验收实况（本轮）

| 范围 | 命令 | 结果 |
| --- | --- | --- |
| mem0 线面 crate | `cargo test -p sdkwork-routes-memory-open-api` | 27 passed / 0 failed（13 单测 + 5 线面 E2E + 9 既有）+ 1 ignored（官方 SDK 验收，见 §7.6） |
| memory 全工作区 | `cargo test --workspace` | **exit 0**，**100 个 suite 全 ok，797 passed / 0 failed**；用例总数 799 = 797 执行 + **2 条显式 `#[ignore]`**（官方 SDK 验收 + 既有的 PostgreSQL 迁移用例） |
| 框架受影响 crate | `cargo test -p sdkwork-web-axum -p sdkwork-web-bootstrap -p sdkwork-web-contract` | 92 passed / 0 failed |
| 框架 core | `cargo test -p sdkwork-web-core` | 219 passed / 0 failed |
| **官方 SDK 真跑通** | `cargo test -p ... --test mem0_official_sdk_flow -- --ignored` | **1 passed / 0 failed**（mem0ai 2.2.0，真 socket）——见 §7.6 |
| 契约测试 | `openapi_phase1_contract`、`openapi_body_schema_parity`、`route_manifest_openapi_parity`、`schema_registry_phase1_contract`、`open_api_prefix_contract` | **5/5 exit 0**（其中 `openapi_phase1_contract` 起先是红的，见 §7.7.1） |
| 门禁（`_sdkwork:check` 的 15 条 + `check:cors-standard`，逐条取退出码） | `verify-repo` / `architecture-alignment` / `crate-inventory-standard` / `release-readiness` / `pnpm-script-standard` / `agent-workflow-standard` / `repository-docs` / `pagination` / `api-envelope` / `api-operation-patterns` / `sdk-standard` / `topology:validate` / `db:validate` / `db:pool:validate` / `db-dialect-parity` / `cors-standard` | **16/16 exit 0** |

线面 E2E 全程**只**用 `Authorization: Token <dev key>` 鉴权，从不发 `X-Api-Key`——所以通过即证明凭据桥、面分类、路由清单鉴权档案、media-type 桥、body/query 拒绝语义与处理器是**一起**工作的。

### 7.5 仍未做（诚实登记）

1. ~~**官方 `mem0ai` 真跑通**未做。本机无 PostgreSQL……要做需先提供 PostgreSQL。~~
   **⚠️ 此条判断是错的，已作废并已执行——见 §7.6。**
   错在前提：我把「真 socket」与「PostgreSQL」绑在了一起。实际上验收用的存档是
   `space_fixtures::new_seeded_in_memory_store()`（**内存态**），`examples/mem0_platform_server.rs`
   把它绑到真实 `TcpListener` 即可；PostgreSQL 只在真实部署装配里才是必需。
   **教训：判断「某验收被环境阻塞」之前，先确认该验收到底需要哪个最小依赖，而不是需要生产装配的全部依赖。**
2. **`RouteAuth::Compatibility` 这条更「正统」的通道本轮未采用。** 依据：工作区唯一先例（cloudrouter 167 条 external 操作）用的是普通 `api-key-or-dual-token` + `x-sdkwork-wire-protocol: external`，而非 `compatibility`；后者还要求逐操作内嵌**上游 operation JSON**（`validate_compatibility_contract`）并实现 `resolve_compatibility` 适配器，materializer 也要改成生成 `HttpRoute::compatibility`。当前设计不需要它：响应侧由 media-type 桥解决，参数侧由 `external_protocol_prefixes` 解决。**若后续要以「内嵌上游契约」为强证据，这是独立工单。**
3. ~~**材料化权威与本轮修正后的 DTO 尚未对齐**……~~ **已修，见 §7.7.2。** 材料化器 `tools/materialize_phase1_contracts.mjs` 的 `mem0Schemas()` 是权威来源；改生成器后重跑材料化，`apis/open-api/…` 与 `sdks/sdkwork-memory-sdk/openapi/…` 两份同步更新。
4. ~~**`sdkwork-web-contract` 的材料化期校验器不认识新字段**：`validate_openapi_routes_context_selectors` / `validate_openapi_document_context_selectors` 不接受 profile，因此仍会拒绝 `user_id` 参数。本仓不跑它们（门禁实测 0），但这是「运行时允许 / 材料化期拒绝」的不对称，应补一个带前缀参数的变体，或让调用方传入。~~
   **已收口，见 §7.8。** 实测确认它**不只是"不跑"**——本仓权威文档喂进去是**真的红**（`OpenAPI path /v1/memories/ declares forbidden context selector query parameter user_id`）。本条为此前唯一的跨仓不对称，现已补上带前缀参数的变体，并在本仓新增契约测试把两侧钉在同一个前缀列表上。
5. §5 风险 1（路由清单 `prefix` 语义）仍未实测收口；mem0 的 10 个操作也尚未做**逐字段**契约对拍（现有 E2E 覆盖行为，不覆盖字段全集）。
6. **官方 SDK 仍未覆盖的端点**（`get_memory_export`/`get_summary`/`get_profile`/`get_profile_settings`/`delete_users`，落在 `/v1/exports/`、`/v1/summary/`、`/v2/entities/…`、`/v2/profiles/…`）。本仓未路由这些路径，客户端会拿到 404。**这是尚未实现的兼容面，不是回归出的缺陷**；本轮已按裁定补了**映射到既有能力**的三条（见 §7.8），其余仍取决于产品是否需要。
7. **框架仓 `sdkwork-routes-web-framework-backend-api` 有一条预存红灯，与本轮改动无关**：`openapi_authority::committed_openapi_authority_matches_runtime_contract` 失败，因为 `apis/backend-api/web-framework/openapi.json` 相对 `build_openapi_document` 已漂移（构建器多出 `deprecated: true` 的 `limit` 查询参数，79 insertions / 7 deletions）。**已用 `git stash` 摘掉本轮 5 个改动文件后复现，证明是 HEAD 既有状态**。修法是重跑 `cargo test -p sdkwork-routes-web-framework-backend-api materialize_openapi_authority_file -- --ignored`；本轮**未擅自改**（属另一条线面的制品，且会污染本轮的提交面）。

### 7.6 官方 SDK 真跑通（本轮新增；含一处自我更正）

#### 7.6.1 结论

官方 PyPI 包 **`mem0ai==2.2.0`**，经真实 `TcpListener`（`axum::serve`，`127.0.0.1:<ephemeral>`）驱动本仓生产装配，**端到端通过**：

```
official mem0ai 2.2.0 drove the platform wire end to end (5 refusal(s) classified)
test official_mem0_sdk_drives_the_platform_wire ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

覆盖的调用链（全部由 `MemoryClient` 自己构造请求、自己解析响应、自己抛异常）：

| # | SDK 调用 | 真实端点 | 断言要点 |
| --- | --- | --- | --- |
| 1 | `MemoryClient(...)` 构造 | `GET /v1/ping/` | 非 2xx 即抛；`org_id`/`project_id` = 租户 |
| 2 | `add(..., user_id="alice", agent_id="planner", metadata=…)` | `POST /v3/memories/add/` | `event=ADD`、`memory` 等于原文、`user_id`/`agent_id` 回显、`hash` 为 64 位摘要、调用方 metadata 保留 |
| 3 | `get(id)` | `GET /v1/memories/{id}/` | id 与文本一致 |
| 4 | `search(q, filters={"user_id": …}, top_k=5)` | `POST /v3/memories/search/` | 命中刚写入的 id；`filters` **嵌套**故不被守卫拦 |
| 5 | `update(id, text=…)` | `PUT /v1/memories/{id}/` | 文本被改写 |
| 6 | `history(id)` | `GET /v1/memories/{id}/history/` | 返回**裸 JSON 数组**（SDK 的返回类型就是一个 list）；事件含 `ADD`+`UPDATE` 且落在封闭枚举内 |
| 7 | `users()` | `GET /v1/entities/` | 含 `user:alice`、`owner` = 租户 |
| 8 | `delete(id)` → `get(id)` | `DELETE` + `GET` | 404 被映射成 SDK 自己的 **`MemoryNotFoundError`**，且 `detail` 就是本层的 `"memory not found"` |
| 9 | 5 次**应被拒绝**的调用 | 见下 | 全部 501，且是 SDK 自己的异常类型 + 非空理由 |
| 10 | `delete_all()` → `get_all()` | `DELETE /v1/memories/` → `POST /v3/memories/` | 清空后 `count=0` |

被分类记录的 5 个拒绝（每个都必须是可读的**具名**失败，而不是 2xx）：

| 调用 | 端点 | 现实状态 |
| --- | --- | --- |
| `get_all(filters={"user_id": …})` | `POST /v3/memories/` | 501：本层列表无 metadata 过滤，指引改用 `search` |
| `get_all(page=2)` | `POST /v3/memories/?page=2` | 501：canonical 列表是游标序，只有第一页可寻址 |
| `delete_all(user_id=…)` | `DELETE /v1/memories/?user_id=…` | 501：canonical 批量删除是空间作用域 |
| `update(id, timestamp=…)` | `PUT /v1/memories/{id}/` | 501：canonical 更新不接受调用方时间戳 |
| `delete(id, delete_linked=True)` | `DELETE …?delete_linked=true` | 501：本层只删被寻址的那条 |

**这 5 条不是缺陷，是有意的「拒绝而非降级」**：上游 mem0 会把它们答成某种结果，本层若照答
就会给出与真实状态不符的成功。但它们**确实是兼容缺口**——尤其 `get_all(filters=…)` 是 mem0
最常用的读写路径之一。是否补（例如让列表也走 `search` 的元数据过滤）属产品取舍，未擅自改。

#### 7.6.2 验收件（可复现）

| 文件 | 作用 |
| --- | --- |
| `crates/sdkwork-routes-memory-open-api/tests/mem0_official_sdk_flow.rs` | 绑真 socket、起生产装配、子进程跑官方 SDK；`#[ignore]`（需 Python+mem0ai，不能进 hermetic 全量） |
| `crates/sdkwork-routes-memory-open-api/tests/mem0_official_sdk/acceptance.py` | 驱动脚本。**不自建请求**：每个端点、每个请求头、每次响应解析都由 `MemoryClient` 完成 |
| `crates/sdkwork-routes-memory-open-api/examples/mem0_platform_server.rs` | 可手动起线面（`cargo run --example mem0_platform_server`），便于用 `curl` 或任意语言绑定直接打 |

复现命令：

```bash
MEM0_E2E_PYTHON=<有 mem0ai 的解释器> cargo test -p sdkwork-routes-memory-open-api \
  --test mem0_official_sdk_flow -- --ignored --nocapture
```

`#[ignore]` 的理由要看清：**不是软门禁**。默认全量 `cargo test --workspace` 不背 Python 依赖；
显式 `--ignored` 跑时，缺 SDK 会**直接失败并给出装法**，不会静默跳过。
两层断言同时成立：脚本自报 `ok`，Rust 侧再从脚本回传的**原始观测值**重新推导每一条声明并复核
（不信脚本自己的结论，也不信 SDK 版本号——`sdk.module` 若落在仓库内会直接判失败）。

#### 7.6.3 ⚠️ 本轮踩到并修掉的坑：`api_key` 不能自带 scheme

首次运行 401，报文是 `detail: "api_key_id claim is required"`。定位过程值得留档：

1. 用 `curl` 只带 `Authorization` 打同一个服务 → **200**；
2. 把官方客户端的每一个额外请求头（`Mem0-User-ID` / `X-Mem0-Client` / `Accept` /
   `Accept-Encoding` / `Connection` / `User-Agent`）**逐个**加上去打 → **全部 200**；
3. 于是问题不在头，在**值**：驱动脚本把 `MEM0_API_KEY` 传成了已经带 `Token ` 前缀的整串，
   而 `MemoryClient` 自己会拼 `Authorization: f"Token {api_key}"`
   （`mem0/client/main.py::_client_headers`）⇒ 线上实际发出的是 `Token Token <claims>`。
   凭据桥按**第一个空格**切分，于是 claim 解析器看到的键名是 `Token api_key_id`，
   报「缺 api_key_id」。

修法：`MEM0_API_KEY` 只传**裸凭据**（`api_key_id=…;tenant_id=…;…`），scheme 交给客户端。
**判据：把一个第三方 SDK 接进来时，「它替你拼了哪一段」必须逐字从它源码里读出来，
不能按我们自己的测试夹具的习惯去传参**——本仓的进程内夹具是直接 `headers.insert("authorization", "Token …")`，
习惯不同，直接照搬就错。

顺带一条诊断质量观察（**不修**，仅登记）：双重 scheme 这种畸形凭据，最终报的是
「缺 claim」而不是「凭据格式非法」。上游从不会发这种形状，且报错仍指向了可修的位置，
因此保持现状；但如果将来要在凭据解析上加诊断，这是可改进点。

### 7.7 官方 SDK 跑通之后，又把两处契约缺陷翻了出来

这两处都不是「跑通 SDK」直接报的错，而是**在把权威与实现对齐的过程中**暴露出来的。
记录它们是因为成因都有一个共同形状：**新线面第一次用了某个此前用例没覆盖到的取值。**

#### 7.7.1 🔴 `PUT` 没被自动分配限流档位（门禁本来就是红的）

- **症状**：`node --test tests/contracts/openapi_phase1_contract_test.mjs` 红：
  `PUT /v1/memories/{memory_id}/ must declare openApiDefault rate limit tier`。
- **根因**：`tools/materialize_phase1_contracts.mjs::resolveRateLimitTier` 自动补档位的动词表是
  `["post", "patch", "delete"]`——**漏了 `put`**。而同一个仓库的契约测试断言的是
  「`post`/`put`/`patch`/`delete` 四个动词都必须有档位」。两者从写下那天起就不一致，
  只是**在这条 mem0 线面之前，本仓三份权威里一个 `PUT` 都没有**（canonical 的更新一律写成 `PATCH`），
  所以这个洞一直是死的。
- **修法**：把 `put` 加进动词表（而不是只给 mem0 的 update 单独补一行）。理由是让解析器与既有契约测试
  一致，且**爆炸半径实测为 1**：三份权威合计 `PUT` 数量 = 1（就是 mem0 的 update）。
- **证据**：修前 `openapi_phase1_contract_test` exit 1；修后 exit 0（4 个断言全过）。
  统计命令见下，「1」既是爆炸半径也是修好它的理由。
- **判据（可复用）**：**同一个仓库里，「生成器默认值」与「契约测试期望值」是两份独立陈述，
  必须有一条用例同时踩到它们。** 某个取值长期没人用，这个洞就一直不会红——所以「全绿」不能
  证明生成器与契约一致，只能证明**当前用到的取值**上一致。

```bash
# 爆炸半径：三份权威里 PUT 各有多少条
node -e "for (const f of ['apis/open-api/memory-open-api.openapi.json','apis/app-api/memory-app-api.openapi.json','apis/backend-api/memory-backend-api.openapi.json']) { const d=require('./'+f); const n=Object.values(d.paths).filter(i=>i.put).length; console.log(f, n); }"
```

#### 7.7.2 🔴 材料化权威里的 mem0 schema 与实现不符（`is_deleted` 等）

- **症状（读权威即可见，不必跑任何东西）**：`Mem0HistoryEntry` 声明了 `is_deleted`
  （上游**没有**这个字段）、缺 `user_id`/`categories`、把 `created_at`/`updated_at` 标成 nullable；
  `Mem0EntityList.results` 是 `additionalProperties: true` 的**不透明袋**，
  生成的客户端拿不到任何字段。
- **根因**：`mem0Schemas()` 是在 DTO 定型**之前**写的（当时按「上游大 schema」粗描），
  之后 DTO 收敛到「只发真实存在的字段」，schema 没跟着走。注意 `is_deleted` 上游确实存在，
  但挂在**实体响应** `UserResponse`/`AgentResponse`/`AppResponse`/`RunResponse` 上，
  **从不在 history item 上**——所以它不是「多抄了上游字段」，而是**串了上游的另一个 schema**。
- **修法（对齐到「本层真实发出的字段」，并逐条注明与上游的差异）**：
  - `Mem0HistoryEntry.required` → `[id, memory_id, old_memory, new_memory, user_id, event, created_at, updated_at]`；
    `input` 降为可选并注明原因（审计轨迹**有意不存**记忆正文：在另一套留存/删除路径下再存一份个人数据，
    `PRIVACY_SPEC` 不允许）；删掉 `is_deleted`；`event` 补封闭枚举 `[ADD, UPDATE, DELETE]`。
  - `Mem0EntityList.results.items` → 展开为具体 schema，`required = [id, name, created_at, updated_at, owner, type]`，
    `type` 补枚举 `[user, agent, app, run]`，并把信封 `required` 补成 `[count, next, previous, results]`
    （与上游一致）。另**不声明**上游那四个可选 `total_*`：上游定义它们是**项目级**总数，
      而本层列表是**兼容空间级**，声明了会诱导调用方把空间数当项目数读。
- **证据**：改生成器 → 重跑 `node tools/materialize_phase1_contracts.mjs` → 两份权威
  （`apis/open-api/…` 与 `sdks/sdkwork-memory-sdk/openapi/…`）同步生效，`is_deleted` 消失、
  `event` 带枚举、实体 item 有 `required`；随后 5 个契约测试 5/5 exit 0。

#### 7.7.3 ⚠️ 沉淀成一条纪律

**「兼容面」的权威必须描述「本层真实发出的字段」，而不是「上游 schema 长什么样」。**
上游 schema 是**允许的上界**；本层的按名拒绝（501）、按名缺失（如 history 不存正文）会**主动收窄**它。
把上界抄成本层契约，生成出来的客户端就会去读永远不存在的字段——而这类错误**在跑通 SDK 时不会报**，
因为 Python/TS 客户端读 `dict.get(...)`，缺字段只是 `None`。所以：**权威对齐必须独立于 SDK 跑通来验收。**

---

## 8. 第二轮：补齐映射到既有能力的端点 + 材料化期校验器收口（2026-09-28 续）

第一节把兼容面按「官方 SDK 引用了但本仓没路由」的端点做了分类登记。本轮按裁定执行两条：
**只补映射到既有能力的端点**，以及**把材料化期校验器补成认识 external 前缀**。

### 8.1 新增的三条操作（路由 37 → 40）

| 新增 | 方法 | 映射到的 canonical 能力 | 契约要点 |
| --- | --- | --- | --- |
| `/v1/feedback/` | `POST` | `create_feedback`（`targetType="memory"`） | `feedback` 取值为**封闭枚举** `POSITIVE\|NEGATIVE\|VERY_NEGATIVE`；`id` 回读自落库行；`feedback_reason` **回 null**（canonical 只写不读，回显即声称了无法证明的往返） |
| `/v1/batch/` | `PUT` | `update_memory` × N | 每条必须带 `text` 或 `metadata`；重复 `memory_id` **折叠为首次出现** |
| `/v1/batch/` | `DELETE` | `delete_memory` × N | 同上折叠规则 |

两条**形状约束上的拒绝**（都是具名 501/400，不是降级）：

- `POST /v1/feedback/` 不带 `feedback`（上游语义是**撤回**）→ **501**。canonical 的 feedback 记录只有"写入"没有"清空"，
  答 200 就是报告了一次没有发生的变更。
- 上游 200 只有 `{"message": "Successfully updated N memories"}`，**没有任何逐条结果通道**。所以一个"部分成功"的
  batch 根本无法被如实汇报 ⇒ 校验必须**先做完再写**：条数上限（上游 `maxItems: 1000`）、每条 payload 合法性、
  每条 `memory_id` 可寻址性，**全部前置**。前置之后仍失败的只可能是"检查与写入之间记录消失"的竞态，
  这类失败返 **409**，并在消息里写明已写入几条 / 共几条 / **batch 非原子且未回滚**。

### 8.2 官方 SDK 验收已覆盖新增端点

`tests/mem0_official_sdk/acceptance.py` 扩展后，官方客户端真跑通的内容增加：

| 调用 | 端点 | 断言 |
| --- | --- | --- |
| `feedback(id, feedback="POSITIVE", feedback_reason=…)` | `POST /v1/feedback/` | 有 `id`；`feedback` 回读为 `POSITIVE`；`feedback_reason` 为 `null` |
| `feedback(id)`（不带值） | 同上 | **`MemoryError` / `HTTP_501`**，消息含 `feedback withdrawal` |
| `feedback(id, feedback="MAYBE")` | **未发出** | 客户端自己的 `ValueError`。**单独记录为 local guard**——把这条算作服务端枚举校验的证据就是假的（服务端那条由 `mem0_wire_flow.rs` 覆盖，它真的走线） |
| `batch_update([...2 条...])` | `PUT /v1/batch/` | `"Successfully updated 2 memories"`；随后 `get` 逐条读回新文本与合并后的 metadata |
| `batch_update([已知 id, 未知 id])` | 同上 | **`MemoryNotFoundError` / `HTTP_404`**，且 detail 精确等于 `memory 999999999999999999 not found`；**已知那条仍保持原文本**（前置校验真的拦住了整批） |
| `batch_update([{memory_id}])`（无 payload） | 同上 | **`ValidationError` / `HTTP_400`** |
| `batch_update(1001 条)` | 同上 | **`ValidationError` / `HTTP_400`**（上游 `maxItems`） |
| `batch_delete([...2 条...])` | `DELETE /v1/batch/` | `"Successfully deleted 2 memories"`；两条 `get` 均 **404 `MemoryNotFoundError`** |

验收入口不变（`#[ignore]`，需真实 socket + 装了 `mem0ai` 的解释器）：

```
MEM0_E2E_PYTHON=<装着 mem0ai 的解释器> cargo test -p sdkwork-routes-memory-open-api \
  --test mem0_official_sdk_flow -- --ignored --nocapture
```

**期望值由 Rust 侧持有**：batch 的两组文本、未知 id 都通过环境变量下传（`MEM0_E2E_BATCH_*`、`MEM0_E2E_UNKNOWN_ID`），
否则「断言」只是在拿驱动脚本跟自己比。

### 8.3 🔴 材料化期校验器与运行时豁免不对称（本轮已收口）

**这不是"本仓不跑所以无影响"，而是"本仓权威文档喂进去是真的红"。** 实测（临时探针调框架真函数）：

```
PROBE_RESULT: REJECTED: OpenAPI path `/v1/memories/` declares forbidden context selector query parameter `user_id`
```

根因：`DELETE /v1/memories/` 在**上游契约里**就把 `user_id`/`agent_id`/`run_id`/`app_id` 声明为 **query 参数**
（这是 mem0 自己 API 的一部分，官方客户端这么发），而 `user_id`/`app_id` 正在框架的禁用选择器名单里。
运行时有豁免（`external_protocol_prefixes`），材料化期没有 ⇒ 同一个契约，一侧放行一侧拒绝。

**收口方式（框架仓）**：新增带前缀参数的变体，零参数版保留并委托空列表，行为不变：

| 新增 API | 用途 |
| --- | --- |
| `validate_openapi_routes_context_selectors_with_external_prefixes(routes, prefixes)` | 路由清单侧 |
| `validate_openapi_document_context_selectors_with_external_prefixes(document, prefixes)` | 材料化文档侧 |
| `build_openapi_document_with_external_prefixes(title, routes, prefixes)` | 直接材料化的宿主入口 |

匹配语义与运行时**逐字对齐**（声明 `/v1` 覆盖 `/v1` 与 `/v1/...`，**不**覆盖 `/v1beta`/`/v10`；
空声明或 `"/"` 这种退化声明不豁免任何东西）。

**收口方式（本仓）**：新增 `tests/mem0_wire_context_selector_contract.rs`，把两侧钉在
**同一个** `memory_open_api_external_protocol_prefixes()` 上：

1. 权威文档里**每条**非 `/mem/v3/api` 路径都必须落在某个已声明前缀下（反之每个已声明前缀都必须真有路径用它）
   —— 防止"新增一条上游路径却忘了声明前缀"；
2. 权威文档**带声明**通过材料化期门禁；**不带声明**必须红，且错误必须落在 `/v1/memories/` + `user_id` 上；
3. 每条落在已声明前缀下的操作都必须带 `x-sdkwork-wire-protocol: external`，**且反向**也不许有例外
   —— 防止前缀声明与逐操作声明互相漂移；
4. 路由清单带声明通过路由侧门禁。

**变异控制（证明测试不是恒绿）**：把 `paths::MEM0_PATH_PREFIXES` 的 `/v3/` 改成 `/v9/`，第 1、3 条立刻变红：

```
`/v3/memories/` is neither SDKWork-owned (`/mem/v3/api`) nor under a declared upstream prefix ["/v1", "/v9"].
```

还原后 4/4 绿。`paths.rs` 用 `read_bytes`/`write_bytes` 变异（Python 文本模式会把 LF 改成 CRLF），
变异后从备份**逐字节**还原并复核内容。

> 诚实登记：路由侧那条门禁**没有**可用的变异控制——它只拦 `FORBIDDEN_AMBIENT_CONTEXT_PATH_MARKERS`
> （`/tenants/`、`/organizations/`），而 mem0 没有这样的路径。也就是说本轮的豁免在路由侧**观察不到**，
> 真正起作用的是文档侧。区分"通过"和"这条检查对本例是否有鉴别力"是两件事，测试注释里写明了。

### 8.4 本轮全量回归

| 项 | 命令 | 结果 |
| --- | --- | --- |
| memory 工作区测试 | `cargo test --workspace` | **`CARGO_EXIT=0`**；含 `mem0_wire_flow` 6/6、`mem0_wire_context_selector_contract` 4/4、`mem0_official_sdk_flow` 1 ignored（设计如此），全仓 0 failed |
| 官方 SDK 验收 | `--ignored` 单跑 | **1 passed**，10 条调用链 + 5 条 501 拒绝 + 3 条 4xx 拒绝全绿 |
| 契约测试 | `openapi_phase1_contract` / `openapi_body_schema_parity` / `route_manifest_openapi_parity` / `open_api_prefix_contract` / `runtime_plugin_layout_contract` | **5/5 exit 0** |
| 门禁 | `_sdkwork:check` 的 15 条 + `check:cors-standard` | **16/16 exit 0**（`crate-inventory` 实测扫描 20 个一级目录 / 20 manifest / 20 member，**不是假零**） |
| 框架工作区测试 | `cargo test --workspace`（框架仓） | 仅 1 条红：见 §7.5 第 7 条（**预存**，已用 stash 归因） |

框架仓 `sdkwork-web-contract` 侧新增 3 个测试：`external_prefix_covers_itself_and_its_subtree_only`、
`external_prefix_suspends_the_route_context_selector_rule`、`external_prefix_suspends_the_document_context_selector_rule`
（36 passed / 0 failed）。后者同时是**变异控制**：同一份文档不带声明必须红，且混装文档里 SDKWork 自有路径上的
同名参数**仍然**是违规。
