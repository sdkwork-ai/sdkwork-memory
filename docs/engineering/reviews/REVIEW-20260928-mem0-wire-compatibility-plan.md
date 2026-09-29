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
5. §5 风险 1（路由清单 `prefix` 语义）仍未实测收口；
   ~~mem0 的 10 个操作也尚未做**逐字段**契约对拍（现有 E2E 覆盖行为，不覆盖字段全集）。~~
   **已做，见 §13**（现为 15 条路径 / 19 条 verb-edge）。对拍方式是两个官方客户端**满字段真送达到捕获桩**，
   翻出 **22 个「客户端可送达而我方静默丢弃」的字段**，其中一条是破坏性的
   （`delete_all(filters=…)` 清空整个空间，§13.4，已修）；其余 21 条待裁定（§13.5）。
6. **官方 SDK 仍未覆盖的端点**（`get_memory_export`/`get_summary`/`get_profile`/`get_profile_settings`/`delete_users`，落在 `/v1/exports/`、`/v1/summary/`、`/v2/entities/…`、`/v2/profiles/…`）。本仓未路由这些路径，客户端会拿到 404。**这是尚未实现的兼容面，不是回归出的缺陷**；本轮已按裁定补了**映射到既有能力**的三条（见 §7.8），其余仍取决于产品是否需要。**精确计数（2026-09-28 续二补）：Python 客户端 28 个线上方法中已服务 12 个、未路由 16 个；TS 客户端不新增未路由风险——见 §9.3 / §9.4。**
7. **框架仓 `sdkwork-routes-web-framework-backend-api` 有一条预存红灯，与本轮改动无关**：`openapi_authority::committed_openapi_authority_matches_runtime_contract` 失败，因为 `apis/backend-api/web-framework/openapi.json` 相对 `build_openapi_document` 已漂移（构建器多出 `deprecated: true` 的 `limit` 查询参数，79 insertions / 7 deletions）。**已用 `git stash` 摘掉本轮 5 个改动文件后复现，证明是 HEAD 既有状态**。修法是重跑 `cargo test -p sdkwork-routes-web-framework-backend-api materialize_openapi_authority_file -- --ignored`；本轮**未擅自改**（属另一条线面的制品，且会污染本轮的提交面）。

### 7.6 官方 SDK 真跑通（本轮新增；含一处自我更正）

#### 7.6.1 结论

官方 PyPI 包 **`mem0ai==2.2.0`**，经真实 `TcpListener`（`axum::serve`，`127.0.0.1:<ephemeral>`）驱动本仓生产装配，**端到端通过**：

```
official mem0ai 2.2.0 drove the platform wire end to end (6 refusal(s), 5 rejection(s) classified)
test official_mem0_sdk_drives_the_platform_wire ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

> **本节初写时该行是 `(5 refusal(s) classified)`**——当时驱动只覆盖 5 条拒绝、还没有 batch 那三条拒绝。
> 现在驱动覆盖 6 条 501 拒绝 + 5 条 4xx 拒绝，下面这个数字是**当前实测**。
> 顺带修掉一处**会误导后续回归**的计数缺陷：原来的汇总行取的是两个"具名列表"的长度
> （`observed["refusals"]`=5 / `observed["rejections"]`=3），而 feedback 撤回那条拒绝与
> 两条 batch 删除后的回读 404 各自记在**别的块**里，于是汇总行**系统性地少报**。
> 已改为在 `expect_refusal`/`expect_error` **观察点**累加 `refusals_total`/`rejections_total`
> 并写进驱动 JSON，Rust 侧对 `Some(6)`/`Some(5)` 断言。**变异控制**：把 `refusals_total += 1`
> 改成 `+= 0` 后该断言立刻变红并打印
> `six classified 501 refusals: five in the filter/paging list plus the feedback withdrawal`；
> 已用 `cp` 字节备份还原。**教训：验收汇总不能用"某个列表的长度"当总数——列表是按原因分组的，
> 分组数不等于总数。**

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
| 官方 SDK 验收 | `--ignored` 单跑 | **1 passed**，13 条 verb-edge 调用链 + 6 条 501 拒绝 + 5 条 4xx 拒绝全绿 |
| 契约测试 | `openapi_phase1_contract` / `openapi_body_schema_parity` / `route_manifest_openapi_parity` / `open_api_prefix_contract` / `runtime_plugin_layout_contract` | **5/5 exit 0** |
| 门禁 | `_sdkwork:check` 的 15 条 + `check:cors-standard` | **16/16 exit 0**（`crate-inventory` 实测扫描 20 个一级目录 / 20 manifest / 20 member，**不是假零**） |
| 框架工作区测试 | `cargo test --workspace`（框架仓） | 仅 1 条红：见 §7.5 第 7 条（**预存**，已用 stash 归因） |

框架仓 `sdkwork-web-contract` 侧新增 3 个测试：`external_prefix_covers_itself_and_its_subtree_only`、
`external_prefix_suspends_the_route_context_selector_rule`、`external_prefix_suspends_the_document_context_selector_rule`
（36 passed / 0 failed）。后者同时是**变异控制**：同一份文档不带声明必须红，且混装文档里 SDKWork 自有路径上的
同名参数**仍然**是违规。

---

## 9. 官方 SDK 双客户端面覆盖对拍（2026-09-28 续二）

§7.6/§8.2 的验收只跑通了 **Python** 官方客户端。但裁定原文是「完整兼容 mem0 的 **api 和 sdk**」，
所以本节回答剩下的那个问题：**JS/TS 官方 SDK 有没有 Python 覆盖不到的线上差异？**

**结论先行：没有。** 两个客户端对所有共有操作使用**完全相同的路径与动词**；TS 只多出一条上游自己标了
`@deprecated` 的路径（`deleteUser`），并且 TS **缺** `get_summary` 与 `chat`（后者本来就是本地抛错）。
因此 §7.6 那次 Python 真跑通对**共享线面**具有代表性，不是「只验了一半」。

### 9.1 枚举命令（可复跑，不依赖网络）

上游两份客户端源码都在 gitignore 的 `external/mem0/`，所以这是纯静态取证：

```sh
# TS 客户端路径
grep -oE "/v[0-9][a-zA-Z0-9/_?=&{}.\$]*" \
  external/mem0/mem0-ts/src/client/mem0.ts | sed 's/\${[^}]*}/{}/g' | sort -u
# Python 客户端路径
grep -rhoE "/v[0-9][a-zA-Z0-9/_?=&{}.\$\"\`']*" \
  external/mem0/mem0/client/ | sed 's/["`'"'"']//g; s/\${[^}]*}/{}/g' | sort -u
# 公开方法（归一成 snake_case 后逐名对拍）
grep -oE "async [a-zA-Z_]+\(" external/mem0/mem0-ts/src/client/mem0.ts
grep -oE "^    def [a-zA-Z_]+\(" external/mem0/mem0/client/main.py
# 我方真源是 paths.rs + mem0_routes()，不是本文档
sed -n '/pub fn mem0_routes/,/^}/p' crates/sdkwork-routes-memory-open-api/src/mem0/mod.rs
```

⚠️ **坑**：`grep -oE "[\"'\`]/v[0-9]…"` 只捞到 **1** 条。因为 TS 客户端的路径写在
`` `${this.host}/v3/…` `` 模板串里，`/v` 前面不是引号。**枚举客户端路径时不能假设路径紧跟在引号后**——
这个错误会让人误判「TS 只用一个版本化路径」。

### 9.2 我方实际**实现**的 13 条 verb-edge（真源 `mem0_routes()`）

| 路径 | 动词 | mem0 语义 |
| --- | --- | --- |
| `/v1/ping/` | GET | `ping`（**TS 独有**方法；Python 只在构造函数里隐式打） |
| `/v1/entities/` | GET | `users` |
| `/v1/memories/` | DELETE | `delete_all` |
| `/v1/memories/{memory_id}/` | GET / PUT / DELETE | `get` / `update` / `delete` |
| `/v1/memories/{memory_id}/history/` | GET | `history` |
| `/v1/feedback/` | POST | `feedback` |
| `/v1/batch/` | PUT / DELETE | `batch_update` / `batch_delete` |
| `/v3/memories/` | POST | `get_all`（**POST**，不是 GET） |
| `/v3/memories/add/` | POST | `add` |
| `/v3/memories/search/` | POST | `search` |

10 条路径、13 条 verb-edge。验收驱动**逐条都打到了**：`ping` 靠 `MemoryClient(api_key=…, host=…)`
构造函数发起（构造函数不是 2xx 就抛），其余 12 条是显式方法调用——
`grep -oE "client\.[a-z_]+\(" acceptance.py` 正好给出 12 个不同方法 + 隐式 ping = 13，与 verb-edge 数一致。

#### 9.2.1 `/v2/` 的 6 条 verb-edge：**只拒绝，不实现**（§12 落地）

| 路径（上游形状逐字） | 动词 | 客户端方法 | 拒绝理由（handler 内的原话，已实测回传） |
| --- | --- | --- | --- |
| `/v2/entities/{entity_type}/{entity_id}/` | DELETE | `delete_users` | 实体作用域擦除；本服务**没有**实体删除（canonical 实体路由是 read/patch） |
| `/v2/entities/{entity_type}/{entity_id}/profile/` | GET | `get_profile` | profile 是 mem0 平台**派生**并存储的文本，本服务不生成也不存 |
| `/v2/profiles/jobs/` | POST | `generate_profile` `sample_profiles` | 不生成 profile，作业无事可做 |
| `/v2/profiles/jobs/{job_id}/` | GET | `get_profile_job` | 作业从未创建，id 无状态可报 |
| `/v2/profiles/settings/` | GET | `get_profile_settings` | 项目级配置属 mem0 平台账号面，不归本服务 |
| `/v2/profiles/settings/` | POST | `update_profile_settings` | 同上；接受写入等于报告一个**哪都没生效**的配置变更 |

**为何是这 6 条、不是上游 `/v2/` 全空间**：本面的设计原则是「镜像官方客户端真正发起的调用」，
不是「镜像上游 OpenAPI 的每一条」。这 6 条正是**两个官方客户端可达**的 `/v2/` 形状。
`/v2/memories/`、`/v2/memories/search/`（上游声明的 v2 版列表/检索）官方客户端**不发**，
故不注册——它们仍在已声明的 `/v2/` 前缀之下，因此依旧得到 mem0 形状的 `404`，
只是没有具名理由（实测见 §12.5）。

**运行期总账**：`mem0_routes()` 现在 15 条路径 / **19 条 verb-edge** = 13 实现 + 6 拒绝。

### 9.3 覆盖账（Python 客户端 28 个线上方法）

| 归类 | 数 | 明细 |
| --- | --- | --- |
| **已服务**（真实现） | **12** | `add` `get` `get_all` `update` `delete` `delete_all` `history` `search` `users` `feedback` `batch_update` `batch_delete` |
| **显式拒绝**（具名 501） | **7** | profiles(6) `generate_profile` `get_profile` `get_profile_job` `get_profile_settings` `update_profile_settings` `sample_profiles`；`delete_users`。**§12 落地** |
| 未路由 | **9** | exports(2) `create_memory_export` `get_memory_export`；`get_summary`（此 3 条在 `/v1/` 之下 ⇒ mem0 形状 404）；webhooks(4) `create_webhook` `get_webhooks` `update_webhook` `delete_webhook`；projects(2) `get_project` `update_project`（此 6 条在 `/api/v1/` ⇒ **未框化 401**，§12.5(c) 已按裁定登记） |

12 + 7 + 9 = 28。

另有两个 Python 方法**根本不产生线上调用**，因此**不算缺口**（用源码举证，不是推测）：

- `chat()` → `raise NotImplementedError("Chat is not implemented yet")`。上游自己就没实现，属**客户端本地守卫**。
- `reset()` → 方法体是 `self.delete_users()` + 遥测。它是**组合**而非线上操作，覆盖度跟随 `delete_users`。

**本条表的沿革（避免与 §9.3 旧读数冲突）**：§9.3 最初记的是「已服务 12 / 未路由 16」。
§12 把其中 **7 条从「未路由」提升为「显式拒绝」** —— 它们此前会拿到 **401 + 整段 JSON**（连 404 都不是），
现在拿到具名 501。这不是「多实现了 7 个功能」，而是**把一个误导性的失败换成一个诚实的失败**：
「未路由」这一栏里混着两种完全不同的东西（mem0 形状 404 / 未框化 401），本表按后者重新切开。
剩余 9 条属产品裁定范围（§7.5 第 6 条），且分成两族（本服务尚未实现的 3 条 / 不归本服务的账号面 6 条）。

### 9.4 TS 与 Python 的差异只有两处，都不构成新增风险

| 差异 | 方向 | 判定 |
| --- | --- | --- |
| `deleteUser` → `DELETE /v1/entities/{entity_type}/{entity_id}/` | **TS 独有** | 上游源码自标 `@deprecated`（改用 `deleteUsers`），且属 §7.5 第 6 条已登记未路由面。**不为它单独开面。** |
| `get_summary` / `chat` | **Python 独有** | `chat` 是本地抛错；`get_summary` 与 TS 无关。TS 侧不存在这条缺口。 |

对每条共有方法，TS 的 `host` 与动词都逐条核对过（`add`/`getAll`/`search`/`history`/`deleteAll`/`deleteUsers`）：
`/v3/memories/add/` POST、`/v3/memories/` POST、`/v3/memories/search/` POST、
`/v1/memories/{id}/history/` GET、`/v1/memories/?…` DELETE 与 §9.2 真源**逐字一致**。

### 9.5 本轮附带修复：验收驱动的解释器发现（否则「反复回归」不可复跑）

`--ignored` 的官方 SDK 验收在本轮**第一次跑直接失败**，但它失败的原因不是被验收的东西：

```text
no interpreter could `import mem0`; tried python (exit exit code: 1), python3 (exit …), py (program not found)
```

根因：`find_python_with_mem0()` 只按 **PATH 上的裸名字**探测，而 `mem0ai` 按工作区
运行环境隔离规则装在**隔离环境**里。于是「上一轮验证通过的环境」与「这一轮 PATH 上的环境」一旦不同，
门禁就**静默失去可复跑性**——测试还在、`--ignored` 还在，但再也跑不起来。

修法（只改 `tests/mem0_official_sdk_flow.rs`，+46/−8）：发现顺序改为
`MEM0_E2E_PYTHON` → 已激活的 `VIRTUAL_ENV` → 仓库根 `.venv`（Windows `Scripts/python.exe` /
POSIX `bin/python` 两种布局）→ 裸 `python`/`python3`/`py`，**仍然逐条靠 `import mem0` 实测**，
不是按名字猜。仓库根用 `Path::ancestors().nth(2)` 而非 `join("..")`，让未命中时报出的候选路径可读
（否则打印 `crates\x\..\..\.venv\Scripts\python.exe`）。

验证（三条，都不是"看着对"）：

| 断言 | 命令/条件 | 结果 |
| --- | --- | --- |
| 走**新增的 venv 路径**能发现 | `env -u MEM0_E2E_PYTHON VIRTUAL_ENV=…/envs/default cargo test … -- --ignored` | **1 passed**（4.06s，6 拒绝 + 5 4xx 分类） |
| 无可用解释器时**仍然大声失败且列全候选** | `env -u MEM0_E2E_PYTHON -u VIRTUAL_ENV …` | exit **101**，消息列出 `.venv\Scripts\python.exe`、`.venv\bin\python`、`python`、`python3`、`py` 五个候选 |
| 候选路径**不带 `..` 残留** | 同上，看消息文本 | `D:\sdkwork-space\sdkwork-memory\.venv\Scripts\python.exe` |

**教训（可复用）**：「按可执行文件名探测依赖」在依赖装在隔离环境时必然退化；凡是跨会话复跑的验收，
解释器/运行时发现必须**包含隔离环境约定位置**，并且失败时的候选清单要能直接告诉人或 agent 该设哪个环境变量。

---

## 10. 第二条腿：JavaScript 官方客户端**真跑通**（2026-09-28 续三）

### 10.1 为什么静态对拍不够

§9 用源码对拍论证了 TS 客户端「不新增未路由风险」——那证明的是**路径与动词一致**，
证明不了**客户端会接受我们的响应形状**。把 `mem0ai`（npm，实测 **3.3.1**）真接上去跑，
立刻翻出静态对拍看不见的东西：一处**产品缺陷**和三处**必须声明、不能抹平的客户端差异**。

### 10.2 🔴 A. `GET /v1/ping/` 缺 `status: "ok"`（产品缺陷，已修）

- **实测证据**：装包 `dist/index.mjs` 可 grep 到 `status !== "ok"`；`memory.ts` 的 `ping()`：
  ```
  if (response.status !== "ok") { throw new APIError(response.message || "API Key is invalid"); }
  ```
- **为什么此前没发现**：Python 客户端的 `_validate_api_key` 只要求 `data.get("org_id") and data.get("project_id")`
  ——**对 `status` 完全不关心**。所以「只跑 Python」这条线面**永远**测不出这个缺口。
- **⚠️ 机制更正（2026-09-28 续四核源码后推翻先前措辞）**：先前文档与本仓注释写作
  「JS 客户端**连构造都过不去**」——**不准确**。实测 `mem0.ts` L249-277：`_initializeClient()`
  把 `ping()` 的异常 **`catch` 掉只 `console.error`、不 rethrow**；`constructor` 只在
  **空 apiKey** 时抛。真正的后果是**三层**，逐层都有不同可见度：

  | 层 | 行为 | 可见度 |
  | --- | --- | --- |
  | `new MemoryClient(...)` | **不抛**，只打一条 init 错误日志 | 最隐蔽：构造"成功" |
  | `await client.ping()` | **抛** `APIError("API Key is invalid")` | 直接调它才看得见 |
  | 身份解析 | `organizationId`/`projectId` **留空**（`_resolveIdentity` 因此不缓存这次结果） | 使 `getProject`/`updateProject` 报 *"organizationId and projectId must be set"* |
  | 记忆类方法 | **照常发出**（源码注释：*"Memory requests never wait on this"*） | 所以只跑 add/search 的驱动**也不会失败** |

  ⇒ 教训：**"客户端拒了"这句话必须落到具体是哪一层拒**；把"构造成功但身份没解析"写成
  "构造不出来"，会让读者以为任何调用都会挂，从而**低估**这个字段到底影响什么。
  已把该判断**做成断言**而不是注释：JS 驱动新增
  `check("the client resolved its identity from the ping body", client.organizationId != null && client.projectId != null)`
  ——字段的效力现在由**客户端自身状态**证明。
- **修法**：`Mem0PingResponse` 增加 `status: &'static str`（值 `"ok"`）→ handlers 赋 `"ok"` →
  生成器 `mem0Schemas()` 补上该字段 → **重新材料化**。
- **爆炸半径已证明**：`git diff --stat` 显示两份权威**各 +4 行**，只有这一个字段：
  ```
  apis/open-api/memory-open-api.openapi.json                   | 4 ++++
  sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json | 4 ++++
  ```
- **三处钉住**（不是只改实现）：
  1. `mem0_wire_flow.rs` 加线面断言（不经过任何客户端）；
  2. **两个驱动都额外裸读取** `GET /v1/ping/` 原始 body——Python 客户端不读该字段，
     只靠它自己的观测**无法**证明契约满足，必须直接看响应；
  3. 共享断言加 `ping.status == "ok"`，两个客户端一起受约束。
- **变异控制（两条，第二条是用来隔离的）**：
  1. `"ok"` → `"banana"` ⇒ 线面测试红（打印 `status: String("banana")`）+ JS 验收红（0.49s 内失败）。
     ⚠️ 但先变红的是**raw-body 那条断言**，不是 identity 断言——所以这一条**证明不了**
     identity 断言有效，它只证明"有人在守这个字面量"。
  2. **隔离变异**：让 ping 仍回 `status:"ok"`、但把 `org_id`/`project_id` 置空
     （= "status 对而身份没解析"这个真实场景）⇒ **唯一**失败的是
     `FAIL the client resolved its identity from the ping body {"organizationId":null,"projectId":null}`。
     ⇒ 这才是 identity 断言**非空**的证明，也精确对应了这个字段**真实影响什么**。
  两条均 `cp` 字节备份还原，`sha256` 与改前一致（`634ada4eabbc94f0`）。

### 10.3 三处客户端差异（**声明**而不是宽容）

| # | 差异 | 实测 | 处理 |
| --- | --- | --- | --- |
| B | **响应键被 camelCase** | `_fetchWithErrorHandling` = `snakeToCamelKeys(raw)` | JS 驱动映射**回** snake_case，保持**一份观测契约** |
| C | **错误信息是原始 body** | JS `message` = `{"detail":"memory not found"}`；Python `message` = `memory not found` | 共享断言由 `==` 改为**包含**本线面措辞；**精确形式钉在各自客户端测试**（Python 钉 bare 形式） |
| D | **`batchUpdate` 打不了 metadata** | JS 把每条映射成 `{memory_id, text}`，**丢弃其它字段** | metadata 断言从共享函数**移入 Python 测试**；JS 测试断言该字段**不存在**，作为已记录的能力上限 |
| E | **feedback 枚举守卫方向相反** | Python 本地 `ValueError`（不发请求）；JS **没有守卫** ⇒ 非法值**到达线上** | 各客户端断言各自的行为：Python 断言本地守卫，JS 断言**服务端 400 拒绝** |

- **type 与 `HTTP_<status>` 两边完全一致**（实测：404→`MemoryNotFoundError`、400→`ValidationError`、
  501→`MemoryError`；`errorCode`/`error_code` 仅命名风格不同）⇒ 这一层是**真正的共享契约**。
- E 是**意外收获**：JS 没有本地守卫，于是这次真跑通覆盖了 **服务端闭集校验**——
  而 §7.6 的 Python 跑通**明确声明不覆盖**它（当时只在手写请求测试里钉过）。两个客户端互补。

### 10.4 验收侧结构改动：一份期望，两个客户端

新增 `tests/mem0_official_sdk_support/mod.rs`，把原先散在 Python 测试里的约 20 条断言收敛为
`assert_wire_observations(result, stderr, label, expect)`。各客户端用 `ClientExpectations` **声明**
自己的拒绝/拒绝集合与总数，因此：

- 客户端差异是**声明出来的**，不是被"宽容断言"抹平的；
- 「两个客户端满足同一组期望」从**口头承诺**变成**结构事实**（同一个函数、同一份期望表）；
- 新增客户端只需写「驱动 + 差异声明」，不需要复制断言逻辑。

Python 测试瘦身为「驱动 + 客户端特有精度断言」，新的 `mem0_official_js_sdk_flow.rs` 同构。

### 10.5 顺带修掉一处**预存 flake**（与本轮无关，但会让「反复回归」不可信）

`backend_router_web_framework_rejects_unauthenticated_requests` **依赖兄弟测试的全局副作用**：
它自己**没有**调用 `lock_integration_test_env()`，而那是唯一设置 `SDKWORK_MEMORY_ENVIRONMENT`
的地方（该变量不设就会 panic 拒绝启动）。于是结果取决于 libtest 先调度哪个用例。取证：

| 实验 | 结果 |
| --- | --- |
| 单跑该用例（`--exact`） | **FAILED** —— `SDKWORK_MEMORY_ENVIRONMENT must be set explicitly…` |
| 整二进制单线程（兄弟用例先跑，先 `set_var`） | **passed** |

⇒ 典型**顺序依赖 flake**。**它比红灯更坏**：反复回归无法区分「真回归」和「输了一次竞态」。
修法：该用例自己取 `lock_integration_test_env()` 守卫——既**声明前置**又**串行化**两者。
修后：单跑过、并行过、`--test-threads=4` 连跑 3 次过。

#### 10.5.1 修复后补的**直接归因**（不是"应该好了"，是"单独跑现在真的过"）

| 证据 | 结果 |
| --- | --- |
| 该用例**单独跑**（`-p sdkwork-routes-memory-backend-api --test backend_web_framework_routes <name>`） | **`1 passed`**（修复前此项必 FAILED） |
| 该用例在全量日志中的状态 | `test backend_router_web_framework_rejects_unauthenticated_requests ... ok`（`ws-final2.log` L1567） |
| 全量 | `CARGO_TEST_WORKSPACE_EXIT=0`、`FAILED` 计数 **0**、**802 passed / 0 failed** |

#### 10.5.2 ⚠️ 那次红灯的读数「710 passed / 1 failed」**不能**用来和 802 对比

同一份日志做全局累计重建后：该用例所在二进制是**第 71 / 102 个**，**截至它结束全局累计恰好 711**
（= 710 passed + 1 failed）——与红灯读数**逐字吻合**。结论：

> 那不是"范围更小的套件"，而是 **`cargo test` 默认 fail-fast**：第一个失败的测试二进制之后，
> **其后 31 个二进制、91 个测试从未执行**。

⇒ 教训升级：**红灯的 `n passed` 是两个变量的乘积（真失败 + fail-fast 截断），不能当作覆盖率读数。**
把"红轮的 710"与"绿轮的 802"并列会得到"补了 92 个测试"的错觉——实际它们一直在，只是红轮根本没跑到。
**要范围可比，就一律 `--no-fail-fast`**；否则红轮的数字只配用来定位，不配用来对比。

### 10.6 本轮回归（改动后）

| 项 | 命令 | 结果 |
| --- | --- | --- |
| 全工作区测试 | `cargo test --workspace` | **`CARGO_TEST_WORKSPACE_EXIT=0`**、**802 passed / 0 failed**、`FAILED` 计数 0（日志 `%TEMP%/ws-final2.log`；83 个测试二进制 + 19 个 doc-test） |
| 契约测试 | `_sdkwork:test` 里全部 **15** 个 `node --test` | **15/15 exit 0**（含 `parity`／`prefix`／`SPI`／插件布局／`database-framework`／SDK ownership／crate-inventory 自测；上一版此处误记 11，实际条数按 `package.json` 的 `_sdkwork:test` 计） |
| 门禁 | `_sdkwork:check` 全 15 条 + `check:cors-standard` | **16/16 exit 0**（直调同一批检查器，覆盖等价；见 §10.6.1） |
| Python 官方 SDK 验收 | `MEM0_E2E_PYTHON=<有 mem0ai 的解释器> cargo test -p sdkwork-routes-memory-open-api --test mem0_official_sdk_flow -- --ignored --nocapture` | **1 passed**（6 拒绝 + 5 4xx 分类）⚠️ **必须带该环境变量**，见 §10.6.2 |
| **JS 官方 SDK 验收** | `cargo test -p sdkwork-routes-memory-open-api --test mem0_official_js_sdk_flow -- --ignored` | **1 passed**（6 拒绝 + **6** 4xx 分类：多出的那条正是 E 的服务端枚举拒绝；无需环境变量，自解析） |
| 线面流 | `mem0_wire_flow` | **6/6** |
| 豁免两半 | `mem0_wire_context_selector_contract` | **4/4** |

#### 10.6.1 门禁为何是"直调"而不是 `pnpm check`

本工作树 `node_modules` 不存在 ⇒ `pnpm check` / `pnpm verify` 第一步即
`'sdkwork-app' 不是内部或外部命令`。故**逐条直接调用同一批检查器**（`package.json` 的
`_sdkwork:check` 展开），覆盖面等价：

- 用 **`D:/sdkwork-space/sdkwork-memory` 正斜杠 Windows 路径**传 `--root` / `--workspace`。
  用 Git Bash 的 `/d/sdkwork-space/...` 会被 node 解析成 `D:\d\sdkwork-space\...`，
  **门禁照样 exit 0 但报 `crates scanned: 0` 的假读数**——绿得毫无意义。
- `topology:validate` 需兄弟仓 `sdkwork-app-topology`；`check:*` 里的
  `db:validate` / `db:pool:validate` 分别是 `check-database-framework-standard.mjs` /
  `check-process-shared-database-pool.mjs`。
- 恢复完整 `pnpm check` / `pnpm verify` 链，需先 `pnpm install`。

#### 10.6.2 ⚠️ 两条验收的**调用契约不对称**（Python 要环境变量，JS 自解析）

本机实测（裸 shell，未激活任何 venv）：

| 客户端 | 需要环境变量 | 裸跑结果 |
| --- | --- | --- |
| JS | **否**（`require.resolve("mem0ai")` + `process.execPath` 自推导 node workspace） | `1 passed` |
| **Python** | **`MEM0_E2E_PYTHON`** | **exit 101**：`no interpreter could \`import mem0\`` |

Python 侧候选全落空的原因不是探测写坏了，而是**依赖本来就装在隔离环境里**：

| 候选 | 实际情况 |
| --- | --- |
| `python` / `python3`（PATH） | 命中 managed **base** 解释器 `…/binaries/python/versions/3.13.12/python.exe` —— 按本机隔离规则**不该**有包 ⇒ `ModuleNotFoundError: No module named 'mem0'` |
| `VIRTUAL_ENV` | 未设置（仓外无激活的 venv） |
| 仓库根 `.venv` | **不存在** ⇒ `os error 3` |

真正装了 `mem0ai==2.2.0` 的是 **`C:\Users\Charlesluo\.workbuddy\binaries\python\envs\default`**
（隔离 venv）。`MEM0_E2E_PYTHON` 指向它之后：

```
PY_SDK_EXIT=0
ping: {'org_id': '100001', 'project_id': '100001', 'user_email': None, 'status': 'ok'}
  ok   ping carries the status literal the JavaScript client requires
official mem0ai 2.2.0 (Python) drove the platform wire end to end (6 refusal(s), 5 rejection(s) classified)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.39s
```

**这不改代码**（把 agent 侧二进制目录硬编进产品仓的测试是错的），**改的是记录方式**：

> 上一版这里只写了「1 passed」，**没写它依赖 `MEM0_E2E_PYTHON`** ⇒ 一条**不可复现的读数**。
> 凡是验收读数，**必须连同调用契约一起记**（哪个环境变量、指向什么、为什么需要）。
> 否则下一个人照文档敲命令得到 exit 101，会误判成"兼容性坏了"。

⇒ 通用判据：验收脚本的依赖若允许装在**隔离环境**，那么
① 探测要覆盖隔离环境的**约定位置**（`VIRTUAL_ENV` / 仓根 `.venv`）；
② 兜底闸门是**显式环境变量**，且失败信息必须**直接点名该设哪个变量**（本仓已做到）；
③ 文档里的读数要带**完整调用命令行**，不能只写"passed"。

#### 10.6.3 顺带查出的**无门禁项**：`docs/INDEX.yaml` 登记缺口（**未改，待裁定**）

顺 §10.6.2 的流程核对文档登记时发现：

| 事实 | 证据 |
| --- | --- |
| `docs/engineering/reviews/` 下有 **4** 份 `REVIEW-*` | `ls` 回显 |
| `docs/INDEX.yaml` 只登记了 **1** 份 | 仅 `REVIEW-20260923-memory-commercial-readiness-audit.md`（L43-47） |
| `reviews/README.md` 的 `## Records` 也只列 **1** 份 | 该 README L11-14 |
| **零门禁读它** | 全仓 `grep -rln "INDEX.yaml"`（除 `target/`、`node_modules/`）**零命中**；15/15 契约测试在此状态下全绿 |

规格侧口径是 **SHOULD 不是 MUST**：`DOCUMENTATION_SPEC.md:57`「`docs/INDEX.yaml` `SHOULD` register Canon paths,
Working document ids …」、`:305`「exists **or** the repository documents why it is not yet adopted」。
本地 `reviews/README.md:5-7` 则把这条读成硬要求：「a superseding review is a new `REVIEW-*` document
**registered in `docs/INDEX.yaml`**」。

⚠️ **同处还有一处分歧要一并说清**：该 README 明写 review 记录是 **point-in-time、
must not be rewritten**，而本文档已按 §8 / §9 / §10 三轮在同一份文档里续写。
按该 README 的字面口径，§9/§10 本应是**新文档**；本仓实际沿用的「一份滚动 REVIEW + 编号续篇」
是此前会话既成的事实做法。

⇒ **本轮不改**（`INDEX.yaml` 属登记类文件，按本仓约定**先问再改**；且其中 2 份是此前会话的产物）。
待裁定项：① 是否把缺的 3 份补进 `INDEX.yaml` + `README.md`；② 是否给这条 SHOULD 配一条门禁
（现状是**没人守**）；③ 滚动续写要不要拆成独立文档。

**教训（可复用）**：
1. **「另一个官方客户端」不是形式主义**——它是唯一能发现「某客户端特有的响应形状要求」的手段。
   静态对拍能证明路径一致，**证明不了**客户端会接受响应。
2. **客户端差异要声明，不要宽容**：宽容断言会把真分歧藏起来；声明 + 各自断言能保精度又能表达差异。
3. **顺序依赖的测试比红灯更坏**：它让反复回归无法分辨真回归与竞态。见到「单跑红、整跑绿（或反之）」，
   先查它是否依赖了别的用例的全局副作用。
4. **红轮的 `n passed` 不是覆盖率读数**：`cargo test` 默认 fail-fast，第一个失败的目标之后全部不跑。
   拿红轮的 710 与绿轮的 802 并列，会误判为"补了 92 个测试"。要横向对比就加 `--no-fail-fast`。
5. **门禁绿的前提是它真的扫到了东西**：`exit 0` 搭配 `crates scanned: 0` 是**假绿**。
   计数为 0 时，先去对照仓验证计数逻辑，再下结论。
6. **验收读数必须连同调用契约一起记**：只写"1 passed"而不写"要设 `MEM0_E2E_PYTHON`"，
   等于交付了一条**不可复现**的读数。下一个人裸跑得到 exit 101，会误判成"兼容性退化了"。
   依赖允许装进隔离环境时，环境变量是**契约的一部分**，不是可选参数。

### 10.7 覆盖率声明**从源码重算**（§9.3/§9.4 的"12 已服务 / 16 未路由"不再靠记忆）

§9 的两个数字此前只存在于散文里：**没有任何东西重算它们**。改客户端或改路由都不会让它变红，
而它却是评审里回答「mem0 兼容到什么程度」时被引用的那个数。本轮补上重算，结论是**数字成立**。

**工具**：`tools/audit-mem0-client-coverage.mjs`

```sh
node tools/audit-mem0-client-coverage.mjs              # 采样并比对记录值
node tools/audit-mem0-client-coverage.mjs --self-test  # 抽取规则的正反例
```

**为什么是采样器而不是门禁**：客户端 vendored 在 **gitignore 的 `external/mem0/`**。
门禁必须能在全新签出上通过，而这条不能——参照树可能合法缺失。故它**报告漂移而不改写基线**，
且退出码把"干净"与"根本没采到"分开：

| 退出码 | 含义 |
| --- | --- |
| `0` | 账目与记录值一致 |
| `1` | 漂移（客户端调用 / 路由 / 分类变了） |
| `2` | 用法错 |
| **`3`** | **未 vendoring，什么都没采到** —— **必须不等于 `0`** |

**重算结果（本机实测，与本文件 §9.3/§9.4 一致）**：

| 客户端 | 已服务 | 未路由 | 不可静态判定 | 无自身请求 |
| --- | --- | --- | --- | --- |
| Python（`MemoryClient`） | **12** | **16** | 0 | `chat`、`reset` |
| TypeScript | **13** | **15** | `getProfileJob` | `constructor` |

**重算顺带确认/新增的四条事实**（此前散文没写全）：

1. **Python 包里是**两个**客户端类**，`MemoryClient` 与 `AsyncMemoryClient`，**28 端点 1:1 镜像**
   （逐个 `method + verb + path` 相等，已做成采样器的不变量 `async-mirrors-sync`）。
   ⇒ 记"Python 28 个方法"时应当同时说明这是**两个类共用同一线面**，否则读者会以为只有一套。
2. **TS 服务 13 而 Python 服务 12，差的那条是 `ping`**：Python 的 ping 在**私有**
   `_validate_api_key` 里（构造期调用），不落在公开方法计数上。⇒ 两者**并不矛盾**，
   但必须写明，否则会被读成"Python 不支持 ping"。
3. **TS 的未路由面 = Python 的 16 − `get_summary` + 上游已 `@deprecated` 的 `deleteUser`**，
   共 15 + 1（`getProfileJob` 不可静态判定）。这与 §9.4 的结论一致。
4. **`update_project` 用的是 `PATCH`**（两侧都是）。本仓线面未声明 `PATCH`，
   因此在分类上属未路由；但它也提醒：**客户端并不只用四种动词**，
   只按 GET/POST/PUT/DELETE 记账会把它误记成 `GET`。

**两类客户端内部响应读取，全仓只有 2 处 —— `ping` 那类缺陷的搜索空间到此封闭**：

| 处 | 读取 | 本仓是否满足 |
| --- | --- | --- |
| `ping()`（仅 JS） | `status === "ok"`、`orgId`/`projectId`/`userEmail` | ✅（§10.2） |
| `deleteUsers()`（**两侧都有**） | `entities.results[].type`、`.name` | ✅ `Mem0EntityList.results[].{type,name}` |

其余方法一律 `return response` 透传（由驱动决定断言），**不再有第三个隐藏读取点**。
⇒ 这比"再跑一遍流程"更有价值：它把"还有没有 `ping` 那种缺陷"从**抽样**变成了**穷举**。

**变异控制（证明它会红，而不是恒绿）**：

| 变异 | 期望 | 实测 |
| --- | --- | --- |
| 改 `paths.rs` 的 `MEM0_SEARCH` 路径 | exit 1 且点名 `search` | **exit 1**，两侧各报 `no longer served: search` / `newly unrouted: search` |
| 去掉 `MEM0_MEMORY` 的 `PUT` 声明 | exit 1 且点名 `update` | **exit 1**，两侧报 `update` |
| 藏掉 `main.py`（未 vendoring） | **exit 3**（≠0） | **exit 3**，并打印缺哪个文件 |

两个被变异文件均按 `cp` 字节备份还原，`sha256` 与改前**逐一致**。

**自测（20 条正反例）** —— 抽取口径本身是这套东西最容易说谎的地方，**本轮已三次说谎**：

| 抽错形态 | 后果 | 现在的守卫 |
| --- | --- | --- |
| 用"含小写字母"判模板 | `/v1/ping/` 也是小写 ⇒ **所有路由被跳过** | 只认残留的 `${` |
| 关键字表里放了 `delete` | TS 的 `delete()` 是**方法名** ⇒ 丢掉一条已服务路由 | 只列真能出现在 `name(` 形态的关键字 |
| `isServed` 对"路径对但动词未声明"返回真值 | 调用方一律当真 ⇒ 动词不符也算 served | 改结构化返回 `{served, route, detail}` |

第三条是**自测抓出来的**（`--self-test` 首跑即红两条），不是事后补的说明——
正是"抽取规则必须先自检再报数"这条纪律的价值所在。

**未接线（待裁定）**：本工具**不在** `_sdkwork:check` 链里（参照树 gitignore，进去会假红）。
若希望它被 CI 守住，正确形态是**采样器 + 一条契约测试**（锁抽取规则与退出码语义）；
但新增契约测试文件会牵动 `REAL_CONTRACT_FILES` / `REAL_CONTRACT_TESTS` / 基线 `testInventory`
等**六处**声明值（本仓既有绊线），属结构改动，**本轮未做**。

## 11. 第四条腿：客户端可达路径的**框（framing）**覆盖审计（2026-09-28 续四）

> ⚠️ **本节记录的是「续四」当时的缺陷现场，`/v2/` 那一列的读数已被 §12 修掉。**
> 保留原样是有意的：§12 的修法只有在看懂本节「同一个未注册状态、前缀内外形状不同」之后才成立。
> 要引用当前状态请看 **§12.5**（线面探针 10/10）与 **§12.4**（采样器读数）。

§10.7 查清了「哪些方法被服务」，但没问一个更底层的问题：**客户端打到一条未声明的路径前缀时，
收到的报文是什么形状？** 这一节用「curl 探针 + 官方客户端真跑」回答它，并翻出一条新缺陷。

### 11.1 `/v2/…` 与 `/api/v1/…` 两个族**逃出 mem0 桥**

| 族 | 客户端方法（两侧） | 在声明前缀内？ | 实测响应 |
| --- | --- | --- | --- |
| `/v1/…`、`/v3/…` | 13 条已服务 verb-edge | ✅ | mem0 形状 |
| `/v2/entities/…`、`/v2/profiles/…` | `delete_users` / `get_profile`（Python）、`deleteUsers` / `getProfile`（JS） | ❌ | **401 + `application/problem+json`** |
| `/api/v1/orgs/…`、`/api/v1/webhooks/…` | `get_project` / `update_project` / 四个 webhook 方法 | ❌ | 同上（mem0 平台账号面） |

curl 探针（直打线面，带**有效**凭据）：

| 请求 | 状态 | content-type | 报文（截断） |
| --- | --- | --- | --- |
| `GET /v1/ping/` | 200 | `application/json` | `{"org_id":"100001",…,"status":"ok"}` |
| `GET /v1/summary/`（未注册，**前缀内**） | 404 | `application/json` | `{"code":40401,"detail":"Not found",…}` |
| `POST /v1/exports/`（未注册，前缀内） | 404 | `application/json` | 同上 |
| **`DELETE /v2/entities/user/probe-u/`** | **401** | **`application/problem+json`** | `{"code":40101,"detail":"requests on unclassified API surfaces require explicit public_path registration","failedStage":"request-context-resolution",…}` |

⇒ **同一个「未注册」状态，前缀内是干净的 404、前缀外是带内部词汇的 401。**

### 11.2 真客户端实测：错误消息**降级成整段 JSON**

装好的官方 Python 客户端（`mem0ai` 2.2.0）打进这两条路径：

| 调用 | 路径状态 | 异常 | 消息 |
| --- | --- | --- | --- |
| `delete_users(user_id=…)` | 未注册 | `AuthenticationError` | **整段问题文档 JSON** |
| `get_profile("probe-u")` | 未注册 | `AuthenticationError` | 同上 |
| `get_all(page=2)` ← **对照（已注册）** | 注册 | `MemoryError` | 干净散文：`page-based pagination is not supported on this surface: …` |
| `get("nosuchid")` ← **对照（已注册）** | 注册 | `MemoryNotFoundError` | `memory not found` |

根因是两层叠加，**不在** handler：

1. `mem0/client/utils.py:39` 用 `content-type.startswith("application/json")` 判断要不要把消息换成
   `detail`，而 `application/problem+json` **不满足该前缀** ⇒ 返回原始体（第 38 行无条件取 `response.text`）
   ⇒ 异常消息 = 整段 JSON。
   ⚠️ **顺带更正一处我们先前的推断**：本仓 `mem0/error.rs` 的注释写过「content type 不对 ⇒
   Python 那些 handler 会报 `Error: None`」。实测**不成立** —— `_validate_api_key`（`main.py:279`）
   走 `e.response.json()`，**不看** content-type；`utils.py` 那条也保留原始报文。
   正确后果是「**消息变成整段 JSON**」，不是 None。
2. 该路径不在 `MEM0_PATH_PREFIXES = ["/v1/", "/v3/"]` 里 ⇒
   - 框架分类器判 `WebApiSurface::Unknown`（`sdkwork-web-core/src/surface.rs::classify_api_surface`）；
   - `interceptors.rs` 对 Unknown 直接 `missing_credentials(...)` ⇒ **401 Authentication required**，
     而调用方**是带凭据的**（同会话里 `ping` 是 200）⇒ **语义错位**，把人引去查密钥；
   - 该拒绝发生在两道 mem0 桥**之前**（`mem0_credential_bridge` 与 `mem0_problem_document_bridge`
     都按 `paths::is_mem0_compat_path()` 判定，见 `mem0/mod.rs`）⇒ 内部媒体类型原样出线。
     `mem0/mod.rs` 的模块文档其实**已经预言**了这个后果（"a problem document reaches the caller as raw
     text and its `detail` is never read"），只是没意识到 `/v2/` 根本不在桥的覆盖范围内。

### 11.3 为什么**不能**只加一行前缀（本轮最有价值的结论）

直觉修法是把 `/v2/` 加进 `MEM0_PATH_PREFIXES`。**它会被本仓自己的契约测试挡住** ——
`tests/mem0_wire_context_selector_contract.rs::the_declared_prefixes_cover_every_upstream_path_in_the_authority`
的最后一段断言：

> Every declared prefix must actually carry something. A stale declaration would
> silently exempt a path space nobody serves.

即**「声明了却不服务的路径空间」被显式禁止**。

**这一条是实测的，不是只读断言**：把 `/v2` 加进常量后跑
`cargo test -p sdkwork-routes-memory-open-api --test mem0_wire_context_selector_contract --
the_declared_prefixes_cover_every_upstream_path_in_the_authority` ⇒ `FAILED`（exit 101），
诊断逐字为 `` `/v2` is declared as an external protocol prefix but no path in the authority uses it ``；
还原后 sha256 逐字节一致（`78950307e95fc0cd`）、`git diff` 为空。

合法修法只能是在 `/v2/` 下**服务点什么**
（最低限度：按既有 501 拒绝模式注册为显式拒绝）—— **属契约面 / 产品范围决定**。

两条候选：

| 方案 | 动作 | 代价 | 收益 |
| --- | --- | --- | --- |
| **A 注册为显式拒绝** | 把客户端可达的 `/v2/` 形状注册为路由，回 `Mem0Error::unsupported`（501） | 操作数 13 → 19；两个 `openapi.json` 重新材料化；覆盖率基线与 §9.2 的 verb-edge 表同步 | 前缀声明变合法 ⇒ 401→501、媒体类型回到 `application/json`、消息重新可读，且失败原因是诚实的 |
| **B 维持现状并登记** | 不改契约，只把差距做成机器可见的不变量（本轮已落地，见 11.4） | 无 | 差距不会静默扩大；但调用方仍拿到 401 + 整段 JSON |

> **裁定（2026-09-28）**：取 **方案 A**，且**只针对 `/v2/`**。
> `/api/v1/…` 判定为 **mem0 平台账号面（项目/组织/webhook 管理），不归本服务**，
> 因此**不声明该前缀**、维持现状并登记（实测对照见 §12.5）。
> 方案 A 的落地与全部证据见 **§12**。
> 采样器**暂不接入 CI**（同一次裁定）：它是采样器不是门禁，理由见文件头（`external/mem0` 被 gitignore）。

### 11.4 本轮已落地的那半：把「框覆盖」做成不变量

> ⚠️ **本节数字是「续四」当时的读数**。§12 已把方案 A 落地，采样器与读数都变了
> （自测 25 → **30** 条、`outside` **12 → 6**）。要引用当前值请看 **§12.4**。

`tools/audit-mem0-client-coverage.mjs` 新增 **framing** 维度（同一批调用上的第二轴）：

- **前缀从 `paths.rs` 派生**（新增 `readDeclaredPrefixes()`）。此前文件里那份手抄的
  `DECLARED_PREFIXES = ["/v1/", "/v3/"]` 是**死常量**——只出现在定义处，无人消费；
  而它偏偏是决定"桥跑不跑"的那个集合 ⇒ 手抄一份等于把读数押在会过期的东西上。
- 每个调用按 `isUnderAnyPrefix(path, prefixes)` 判 `bridged` / `outside`，**`outside` 集合被登记**
  （Python **12** 条、TS **11** 条；Python 多出的那条是 `GET /v2/profiles/jobs/{id}/`）。
  **新增** any outside 路径 ⇒ exit 1。
- 自测 **20 → 25 条**，新增 5 条前缀用例，含两条关键反例：
  `/api/v1/webhooks/…` 不得被当成 `/v1/` 之下（否则平台账号面会被误报成已桥接），
  `/v10/…` 不得被当成 `/v1/`（前缀必须锚在开头）。
- **变异自证**：把 `/v2/` 加进 `MEM0_PATH_PREFIXES`
  ⇒ `outside` **12 → 6**、采样器 **exit 1** 并点名 6 条 Python / 5 条 TS 路径；
  还原后 sha256 逐字节一致（`78950307e95fc0cd`）、`git diff` 为空、采样器复绿。
- 附：本线面**缺一个 413 分支**的观察 —— 11 MiB 报文并未触发体积限制，而是走到 JSON 反序列化
  报 `400 … missing field \`messages\``（dev 环境下如此）。客户端把 413 映射成
  `MemoryQuotaExceededError`，但本harness 下该分支不可达，故未登记为缺口。

⚠️ **同一个动作会被两处同时拦**：契约测试拦的是「声明了不服务的路径」（结构性），
采样器拦的是「框覆盖变了」（读数性）。理由不同、都必要 —— 与 §8.3「两半」是同一条道理。

## 12. 第五条腿：方案 A 落地 —— `/v2/` 注册为**显式 501 拒绝**（2026-09-28 续五）

### 12.1 改了什么（runtime 3 处 / 契约 1 处 / 采样器 1 处）

| 文件 | 改动 |
| --- | --- |
| `crates/.../src/paths.rs` | `MEM0_PATH_PREFIXES` **2 → 3**（加 `/v2/`）；新增 5 个 `MEM0_V2_*` 常量 + `MEM0_REFUSED_PATHS`（**以常量标识符而非字符串字面量成列**，避免两份真源）；+2 条单元测试（前缀按值钉死、每条拒绝路径必须在前缀之下且以 `/` 结尾） |
| `crates/.../src/mem0/handlers.rs` | +6 个拒绝 handler，各自 `Mem0Error::unsupported(operation, detail)`；**每个都不读 body、不读参数**——答案与请求无关，解析它反而会暗示存在「部分兑现」 |
| `crates/.../src/mem0/mod.rs` | `mem0_routes()` 挂载 6 条；模块文档说明 `/v2/` 是「声明但不实现」的那半 |
| `tools/materialize_phase1_contracts.mjs` | `mem0Operation` 新增 `refusal: true`；`MEM0_OPERATION_METADATA` +6 条；`mem0Paths()` +6 条 `addPath`（含 path 参数声明）；顶部线面清单补 `/v2/` 段 |
| `tools/audit-mem0-client-coverage.mjs` | 新增 `refused` 桶与 `pathMatchesRoute`（见 §12.4），前缀集成为**被登记的期望**，新增 `/v2/` 覆盖不变量 |

**物化读数**：open-api 路由 **40 → 46**；`apis/open-api/memory-open-api.openapi.json`
sha256 `af316fea2b15651a…` → `aadc47770bebb283…`；**连续两次物化逐字节一致**（`sha256sum -c` 三条产物全 OK）。

### 12.2 六条拒绝的语义，与一处元数据裁定

六条理由都能在 canonical 面找到支撑，且**没有一条是「本可以但懒得做」**：

- `DELETE /v2/entities/…` → canonical 实体路由**只有 read/patch，没有 delete**（`commercial_routes.rs:18`）。
  客户端语义是「擦除作用域连同其记忆」，本面无法诚实兑现 —— 删了记忆留作用域、或什么都没删却报成功，
  都与「擦除发生了」不可区分。
- `GET /v2/entities/user/{id}/profile/` → profile 是 mem0 平台**派生并存储**的文本；本服务存 record / retrieval / context pack，
  把原始记录挂在 `profile` 键下会把**源记录presented成派生文本**。
- profiles 三条 + settings 两条 → 项目级配置属 mem0 平台账号面（与 `/api/v1/orgs|webhooks` 同族），本服务不是它的所有者。

⚠️ **元数据裁定（须复核）**：这 6 条的 `permission`/`auditEvent` 一律复用**已存在的**
`memory.open.capabilities.read`（`mem0.ping` 用的同一对），`resource: capabilities`。
两条理由：**不复刻不存在的词表**（API key 拿不到的权限会导致永远 403，而不是 501），
且**审计事件必须为真** —— 若按「它们本会产生的效果」登记（如实体删除记 `memory.open.entity.updated`），
审计流里会留下一个本面**明确拒绝做出的**变更。代价是 DELETE 挂着读权限：
这条路由什么都不会删，它的语义是「探测本面能力」。**若评审认为应按资源族授权（`entities.write`/`read`），这是一处可改的裁定，不是缺陷。**

### 12.3 契约面形状：拒绝型操作**没有 2xx**

`refusal: true` 让该操作的 `responses` **只含 `mem0ErrorResponses()`**（400/401/404/409/429/500/501），
不发布任何成功响应 —— 发布 `200` 等于承诺一个永远不会发生的响应。生成结果实测：

```text
DELETE /v2/entities/{entity_type}/{entity_id}/   operationId: mem0.entity.delete
GET    /v2/entities/{entity_type}/{entity_id}/profile/  mem0.entity.profile
POST   /v2/profiles/jobs/                        mem0.profile.job.create
GET    /v2/profiles/jobs/{job_id}/               mem0.profile.job.retrieve
GET    /v2/profiles/settings/                    mem0.profile.settings.retrieve
POST   /v2/profiles/settings/                    mem0.profile.settings.update
→ responses: 400,401,404,409,429,500,501 | wire: external | proto: mem0-platform
```

不写 `requestBody`：处理器不读 body，建模一个永不检查的请求体等于暗示存在部分兑现。
（`verify_openapi_operation_ids.ps1` 里「mutating operation has no requestBody」「400/404 必须带 `application/problem+json`」
两条规则**对 external 协议操作显式 `continue`**，故不冲突 —— 是实测跑过之后确认的，不是读代码推的。）

### 12.4 采样器：新增 `refused` 桶，并**修掉一个潜伏的路径匹配缺陷**

1. **`refused` 是独立桶**（`served`/`refused`/`unrouted`/`unparsed`/`inert`）。理由：拒绝型路由**也是本仓声明的路由**，
   只问「路由存不存在」的分类器会把 501 边界归进「已实现」栏 —— 这是本轴能产出的最误导的读数。
   方法级归属带**带外报告**：一个方法的多次调用若落到不同处置，记入 `mixed` 并**上报**（不是静默按优先级吞掉）。
2. 🔴 **`pathMatchesRoute` 修掉一处潜伏缺陷**：原实现是**归一化后的字符串相等**
   （路由 `{...}` → `{id}`）。Python 客户端把实体类型**硬编码**成 `/v2/entities/user/{id}/profile/`，
   而上游声明的是 `/v2/entities/{entity_type}/{entity_id}/profile/` —— 同一条路由，**永远不同一个字符串**。
   改前该调用被读成「落在任何路由之外」（即「本面什么都没声明」，而它其实回的是具名 501）。
   现按**路由器的段/参数语义**比较（`{param}` 匹配任意单段，其余必须相等）。这个缺陷是**这次改动暴露的**，
   与 `/v2/` 无关却影响所有读数 —— 属于「顺带修掉的既有不准确」。
3. **前缀集成为被登记的期望**（`EXPECTED.prefixes`）：它决定每个调用的框，改了就是漂移，即便没有调用移动过。
4. **新增 `/v2/` 覆盖不变量**（主流程 + 自测两处）：`/v2/` 下的每条路由**必须**出现在 `MEM0_REFUSED_PATHS` 里。
   缺一条，分类器就会把它算作 `served` —— 这个不变量就是为那个读数准备的。

**当前读数（真跑）**：

```text
routes served (from paths.rs): 15        ← 10 实现 + 5 拒绝路径
python      served=12 refused=7 unrouted=9
typescript  served=13 refused=6 unrouted=9
declared prefixes (from paths.rs): /v1/ /v2/ /v3/
refusal routes (from paths.rs): 5
refused shapes reached by a client: 6
outside every prefix (unbridged framing): 6      ← 改前 12
→ account matches the recorded expectation (exit 0)
```

`outside` 从 **12 收敛到 6**，且**剩下 6 条全部是 `/api/v1/…` 平台账号面**（§12.5 已按裁定登记为不归本服务）。
自测 **25 → 30 条**（新增 6 条拒绝分类用例 + 5 条路径匹配用例），另含 2 条结构性不变量。

**变异控制（3 项，全部 exit 1 且逐字诊断，还原均字节一致）**：

| 变异 | 结果 |
| --- | --- |
| M1 `MEM0_REFUSED_PATHS` 去掉一条 | `newly served: get_profile_settings, update_profile_settings` + `/v2/ routes not declared as refusals: MEM0_V2_PROFILE_SETTINGS` + 自测 `MEM0_REFUSED_PATHS holds 4 paths but 5 /v2/ routes exist` |
| M2 `MEM0_PATH_PREFIXES` 去掉 `/v2/` | `declared prefixes dropped: /v2/` + `newly outside every declared prefix: …`（6 条 Python / 5 条 TS 逐条点名） |
| M3 拒绝路由的动词表收窄 | `no longer refused by name: update_profile_settings` + `newly unrouted: update_profile_settings` |

M1 的诊断尤其值得记：**去掉一条拒绝声明后，那两条方法立刻被读成 `newly served`** ——
这正是「只问路由存不存在」会犯的错，也正是新增不变量存在的理由。

### 12.5 实测证据（三个独立层面）

**(a) 线面探针（真 socket，10/10）** —— 覆盖 6 条拒绝 + 未注册 `/v2/` 路径 + 无凭证 + 对照：

| 探针 | 结果 |
| --- | --- |
| 6 条 `/v2/` 拒绝形状 | **501** + `application/json` + 各自具名 `detail` |
| `GET /v2/nothing/here/`（已声明前缀下未注册） | **404** + `application/json`（mem0 形状，无具名理由） |
| `GET /v2/profiles/settings/` 无凭证 | **401** + `application/json`（mem0 形状，不再是 problem document） |
| 对照 `GET /api/v1/webhooks/projects/p1/` | **401** + `application/problem+json`（**维持原样**，见下） |
| 对照 `GET /v1/ping/` | 200，`{"org_id":…,"project_id":…,"status":"ok"}`（§10.2 的 TS 修复未回退） |

**(b) 官方客户端真跑（Python `mem0ai` 2.2.0，7/7）** —— 改前 vs 改后的调用方可见差异：

| 方法 | 改前（§11.2 实测） | 改后（本轮实测） |
| --- | --- | --- |
| `get_profile` / `get_profile_settings` / `update_profile_settings` / `generate_profile` / `get_profile_job` / `delete_users` | `AuthenticationError`，消息是**整段 problem-document JSON** | **`MemoryError`**，消息就是 handler 的理由（如 `entity profile retrieval is not supported on this surface: …`） |

6 条全部由**官方客户端自己**发出（`client.get_profile(...)` 等），断言的是**异常类型 + 消息内容**，不是状态码。

**(c) `/api/v1/…` 对照（诚实登记，不修）**：`client.get_project()` 仍抛
`AuthenticationError`，消息仍是整段 JSON（`{"code":40101,"detail":"requests on unclassified API surfaces require explicit public_path registration",…}`）。
这是**裁定的结果**：该族是 mem0 平台账号面（项目/组织/webhook 管理），不归本服务，
不为它声明 `/api/v1/` 前缀（声明了就得在下面服务点什么，而本面对该族没有任何操作）。
所以这一族**仍是未框化的 401**，且被采样器**逐条登记**（`EXPECTED.*.unbridged`）——差距不会静默扩大，
但它是一条**已知且有意保留**的边界。

### 12.6 回归总账（改动后）

| 层面 | 结果 |
| --- | --- |
| 契约/路由 Rust 测试 | `mem0_wire_context_selector_contract` 4 + `open_api_routes` 3 + `open_openapi_routes` 1 + `route_manifest_contract` 1 = **9 passed, 0 failed** |
| 官方 SDK 验收（Python） | `mem0ai 2.2.0` drove the platform wire end to end（6 refusal / 5 rejection）**ok** |
| 官方 SDK 验收（JS） | `mem0ai 3.3.1` drove the platform wire end to end（6 refusal / 6 rejection）**ok** |
| 门禁（逐条取退出码） | **17/17 exit 0**：app-composition、architecture-alignment、crate-inventory-standard、repository-docs、pagination、api-envelope、api-operation-patterns、sdk-standard、topology:validate、db:validate、6 个 contracts、`verify_openapi_operation_ids.ps1` |
| 采样器 | 自测 30 条 pass；主流程 exit 0；3 项变异全红且逐字诊断 |
| 物化幂等 | 连续两次逐字节一致（3 个产物 `sha256sum -c` 全 OK） |

⚠️ 关键点：改前**被本仓契约测试明确拒绝**的那个动作（`/v2` 声明了却不服务），
改后 `the_declared_prefixes_cover_every_upstream_path_in_the_authority` **通过** ——
即「合法修法」确实被这一次改动满足了，而不是被绕过。

### 12.7 本轮未做

1. **代码未提交**（改动面见 §13.6 的门禁行与本仓 `git status`）；工作树里还有另一会话改的 2 个文件，不能 `git add -A`。
2. 采样器**未接 CI**（本次裁定）：理由与文件头一致，`external/mem0` 被 gitignore，
   门禁必须能在干净检出上通过；接法若是「CI 里先 vendor 再跑」则可行，属独立决定。
3. `/api/v1/…` 账号面**仍不服务、不声明**（裁定）。
4. 六条拒绝的**审计/权限元数据裁定**（§12.2 末）等待评审确认。
5. `/v2/memories/`、`/v2/memories/search/` 未注册为拒绝（官方客户端不发它们），
   因此它们回已声明前缀下的 mem0 形状 404 —— 如果将来有客户端版本开始发 v2 列表/检索，
   采样器会以「newly outside/refused 变化」暴露它，而不是静默 404。

## 13. 第六条腿：逐字段契约对拍 —— 客户端**真送达到**的字段 vs 我方声明（2026-09-28 续六）

§7.5 第 5 条从第二轮挂到现在：「mem0 的操作尚未做**逐字段**契约对拍（现有 E2E 覆盖行为，不覆盖字段全集）」。
这一段把它做完 —— 而且**不是**靠读源码推断，是让两个官方客户端把**每个声明过的 option 字段都填上**，
打到捕获桩上，再读原始报文。

### 13.1 方法：捕获桩 + 双客户端满字段驱动

静态解析客户端不可靠：TS 侧路径写在 `${this.host}/…` 模板串里且响应经 `snakeToCamelKeys` 改写，
Python 侧 `_prepare_payload` 把 `**kwargs` 整体灌进 body。所以：

- 起一个**捕获桩**（`ThreadingHTTPServer`，记录 `(method, path, query, body)`，回一个宽松的 2xx）；
- 两个官方客户端指过去（`MemoryClient(api_key=…, host=…)` / `new MemoryClient({apiKey, host})`）；
- **把每个声明字段都填上哨兵值**；
- 桩对 `GET /v1/ping/` 必须回 `{"status":"ok"}` —— JS 客户端 `ping()` 硬要求它（§10.2）。

两端各 13 次调用，覆盖所有带请求体的已服务 verb-edge。**记录值由驱动产出，不手抄。**

三个可复跑件就放在官方 SDK 验收旁边（不接 CI，理由同 §12.7 第 2 条：`external/mem0` 被 gitignore）：

```sh
# 1. 捕获桩
python crates/sdkwork-routes-memory-open-api/tests/mem0_official_sdk/field_capture_stub.py 6321 /tmp/capture.jsonl
# 2. Python 客户端满字段驱动（需装了 mem0ai 的解释器，同 acceptance.py 的发现契约）
MEM0_BASE_URL=http://127.0.0.1:6321 MEM0_API_KEY=probe \
  python crates/sdkwork-routes-memory-open-api/tests/mem0_official_sdk/field_coverage.py
# 3. JS 客户端满字段驱动（ESM 不吃 NODE_PATH，故用 MEM0_E2E_SDK_ENTRY 指到已安装包）
MEM0_BASE_URL=http://127.0.0.1:6321 MEM0_API_KEY=probe \
MEM0_E2E_SDK_ENTRY=file:///<...>/node_modules/mem0ai/dist/index.mjs \
  node crates/sdkwork-routes-memory-open-api/tests/mem0_official_sdk/field_coverage.mjs
```

本轮实测：两端各 13 条报文落盘（合计 26 条）。

### 13.2 实测：两端实际送达的字段（原样）

| 调用 | 到达的报文（Python `mem0ai 2.2.0` / JS `mem0ai 3.3.1`） |
| --- | --- |
| `add` | `POST /v3/memories/add/`，body：`messages, user_id, agent_id, app_id, run_id, `**`filters`（仅 Py）**`, metadata, infer, custom_categories, custom_instructions, agent_custom_instructions, timestamp, expiration_date, structured_data_schema` |
| `get_all` | `POST /v3/memories/?page=2&page_size=5`，body：`filters, start_date, end_date, categories, show_expired, latest_only` |
| `search` | `POST /v3/memories/search/`，body：`query, `**`output_format:"v1.1"`（仅 JS，恒定送）**`, filters, metadata, top_k, rerank, threshold, fields, categories, show_expired, reference_date, latest_only, keyword_search, `**`source`（仅 JS）** |
| `update` | `PUT /v1/memories/{id}/`，body：`text, metadata, timestamp, expiration_date` |
| `delete` | `DELETE /v1/memories/{id}/?delete_linked=true` |
| `delete_all` | `DELETE /v1/memories/?user_id=…`（JS）/ `?filters={'user_id': …}`（**Python，见 §13.4**） |
| `feedback` | body：`memory_id, feedback, feedback_reason` |
| `batch_update` | `PUT /v1/batch/`，条目 `{memory_id, text}`（JS）/ **原样透传**（Py：可带 `metadata`/`timestamp`/`expiration_date`） |
| `batch_delete` | `DELETE /v1/batch/`，条目 `{memory_id}` |
| `users` | `GET /v1/entities/?page=&page_size=`（仅 JS） |

两端差异**声明而不抹平**：

1. JS `search` **恒定**追加 `output_format: "v1.1"`（Python 不送）—— 我方已声明并接受（§12.3 同款处理）。
2. JS 多送 `source`（`SearchMemoryOptions.source`，上游注释说未知值会被后端归到 `OTHERS`）。
3. JS `getAll` 走 `camelToSnakeKeys(rest)`，但 **`filters` 是单独展开的、键原样透传**
   —— 调用方写 `{userId: …}` 送上来的就真是 `userId`。**上游同样是透传，所以这不是我方的缺口**，
   只记入「过滤器键的拼写由调用方决定，文档拼写是 `user_id`，我方按 `user_id` 读」。
4. JS 的 `DeleteAllMemoryOptions` 只有 `EntityOptions`，**没有 `filters`** ⇒ §13.4 那条它够不到。
5. Python 的 `delete_all(filters=…)` 把 dict 整个交给 `httpx`，wire 值是 **`str()` 出来的 Python dict**
   （`{'user_id': 'alice'}`），**不是 JSON**。

### 13.3 对账：我方声明 vs 客户端送达

| 操作 | 客户端送达 | 我方声明 | 静默丢弃 |
| --- | --- | --- | --- |
| `POST /v3/memories/add/` | 14 | 7 | **7**：`filters, custom_categories, custom_instructions, agent_custom_instructions, timestamp, expiration_date, structured_data_schema` |
| `POST /v3/memories/search/` | 14 | 12 | **7**：`metadata, fields, categories, reference_date, latest_only, keyword_search, source` |
| `POST /v3/memories/` | 6 + 2 query | 5 + 2 query | **5**：`start_date, end_date, categories, show_expired, latest_only` |
| `PUT /v1/memories/{id}/` | 4 | 4 | — ✅ |
| `DELETE /v1/memories/{id}/` | `delete_linked` | 同 | — ✅ |
| `DELETE /v1/memories/` | 4 名 + `filters` | 4 名 | **1 —— 破坏性，见 §13.4** |
| `GET /v1/entities/` | `page, page_size` | **无** | **2**（且 `page_size` 运行时其实认，契约没声明） |
| `POST /v1/feedback/` | 3 | 3 | — ✅ |
| `PUT /v1/batch/` 条目 | 2（JS）/ 任意（Py 透传） | 3 | 条目多余键（Py 可塞任意键） |
| `DELETE /v1/batch/` 条目 | 1 | 1 | — ✅ |

**合计 22 个「客户端可送达而我方静默丢弃」的字段**（另加 batch 条目的透传类）。
根因不是个别疏漏：`serde` 默认**不拒绝未知字段**，而三个请求 DTO 只声明了「我方已实现」的那部分 ——
于是一个**未被声明的**参数既不会报错、也不会生效。

### 13.4 🔴 本轮最重的发现：`delete_all(filters=…)` 把**整个空间**删掉，还回 200

客户端文档（`DeleteAllMemoryOptions`）写明过滤放进 `filters`；`delete_all` 把它整个交给 `httpx`，
wire 上是**一个名叫 `filters` 的查询参数**。而我方 `Mem0DeleteAllParams` 只读
`user_id`/`agent_id`/`run_id`/`app_id` 四个**名字**的查询参数 —— `filters` **根本不在读取范围内**，
请求于是被当作「无过滤」⇒ 走**空间级全删**。

修前实测（真 socket，两条分属 alice / bob 的数据）：

| 请求 | 结果 |
| --- | --- |
| `DELETE /v1/memories/?user_id=alice`（**声明拼写**，对照） | **501** 具名拒绝 —— 守卫本身是对的 |
| `DELETE /v1/memories/?filters={'user_id': 'alice'}`（**客户端真实拼写**） | **200** `{"message":"2 memories deleted successfully"}` |
| 之后 listing | `count = 0` —— **alice 与 bob 的记忆都没了** |

即 `client.delete_all(filters={"user_id": "alice"})` —— 官方 Python SDK 里带类型、文档化的那个调用 ——
会清空整个空间并报成功。处理器自己的注释写的正是相反意图
（“Ignoring an entity filter would delete far more than the caller asked for, so any filter is refused”），
守卫只是**没覆盖第五种拼写**。

**修法（本轮已落地）**

1. `Mem0DeleteAllParams` 新增 `filters: Option<String>` 与 `requested_filter()`（返回「哪个参数、什么值」）。
   `filters` 按**存在性**判定：它可能是 Python repr 而非 JSON，所以只问「有没有东西」。
2. `is_empty_filter_literal` 把 `{}` / `{ }` / `[]` / 空串读作**没有过滤** —— 那正是调用方提的要求，
   拒绝它才是**假拒绝**（实测 `?filters={}` 仍 200 并正常全删）。
3. 拒绝消息现在**回显收到的值**（截断 200 字符），运维能看见被丢弃的是什么。
4. 生成器为该操作声明 `filters` 查询参数（`type: string`）—— **声明才让拒绝可达**；
   `verify_openapi_operation_ids.ps1` 对外部协议操作先 `continue`，所以无 body 的形状合法。

修后实测：

| 请求 | 结果 |
| --- | --- |
| `?filters={'user_id': 'alice'}` | **501**，消息回显 `` received `{'user_id': 'alice'}` `` |
| 之后 listing | `count = 2` —— **两条都还在** |
| `?filters={}` | **200**，正常全删（无假拒绝） |

> 「是否改为真正按过滤器逐条删」是产品决定；本轮按既有口径**拒绝**而非实现（见 §13.5 第 6 项）。

### 13.5 其余 21 个字段：需要裁定（本轮未动）

判据：**canonical 面里有没有对应能力**。有 ⇒ 应当**实现**；没有 ⇒ 应当**具名拒绝**
（与 `Mem0UpdateRequest` 对 `timestamp`/`expiration_date` 的既有做法一致，`dto.rs:101`）。
**不论哪一选，都不应继续静默丢弃。**

| # | 字段 | 建议 |
| --- | --- | --- |
| 1 | `add.filters` | **实现**。Python 文档明说 v3 的身份要放 `filters`（`types.py:6`）；只读顶层会让按文档写的调用**写进无归属的记录**，之后 `search(filters=…)` 再也找不到它 |
| 2 | `add.expiration_date` | **实现**或拒绝（canonical 已有 `expiration_date`，`expires_at` 也已端到端接线） |
| 3 | `add.timestamp` | 同 #2；canonical 能否接受调用方时间需确认 |
| 4 | `add.custom_instructions` / `agent_custom_instructions` | **拒绝**：提取提示词属插件/配置面，不是逐请求开关 |
| 5 | `add.custom_categories` / `structured_data_schema` | **拒绝**：同上，属项目级配置 |
| 6 | `search.metadata` | **实现**（元数据前置过滤）或拒绝 |
| 7 | `search.categories` / `list.categories` | **实现**（canonical 有 categories）或拒绝 |
| 8 | `search.latest_only` / `list.latest_only` | **实现**（canonical 有版本/链接语义）或拒绝 |
| 9 | `search.keyword_search` | **实现**：canonical 已有 BM25 关键词分支 |
| 10 | `search.reference_date` | **拒绝**（相对时间语义未定义）或实现 |
| 11 | `search.fields` | **拒绝**：投影会改变响应契约 |
| 12 | `search.source` | **接受并忽略**（上游也只当遥测提示）—— 但要在契约里写明 |
| 13 | `list.start_date` / `end_date` | **拒绝**：与已拒绝的 `filters` 同因（canonical listing 无元数据过滤） |
| 14 | `list.show_expired` | **实现**，或与 `search.show_expired`（已实现）口径对齐 |
| 15 | `entities.page` | **拒绝**：与 `list.page` 已拒绝同因（游标式列表） |
| 16 | `entities.page_size` | **补声明** —— 运行时其实认（`handlers.rs:637`），契约没写，属纯契约补齐 |
| 17 | `batch` 条目任意键（Python 透传） | **拒绝**未知条目键，或在契约里显式声明 `additionalProperties` |

### 13.6 本轮回归（全部本机实测）

| 层面 | 结果 |
| --- | --- |
| Rust（该 crate 全目标） | **20 passed / 0 failed**（lib 单测 20，含新增 5 条 `mem0::dto::tests`）+ 集成 4 / 6 / 2 / 3 / 1 / 2 / 1 全绿 |
| 官方 SDK 验收（Python `mem0ai 2.2.0`） | **1 passed**；**7 refusals** / 5 rejections；新增「拒绝必须点名 `filters`」断言通过 |
| 官方 SDK 验收（JS `mem0ai 3.3.1`） | **1 passed**；6 refusals / 6 rejections（不变 —— 它的 `DeleteAllMemoryOptions` 没有 `filters`，够不到这条） |
| 覆盖率采样器 | 自测 30 条 pass；主流程 exit 0；读数不变（12/7/9、13/6/9） |
| 门禁（逐条取退出码） | **20/20 exit 0**：14 个 `pnpm run` 检查器 + 5 个契约 node 测试 + `verify_openapi_operation_ids.ps1`（比 §12.6 记的 17 条更全：本轮把 `check:release-readiness`、`check:pnpm-script-standard`、`check:agent-workflow-standard`、`check:cors-standard` 也逐条跑了） |
| 物化幂等 | 只改动 open-api 三件产物；`app-api` / `backend-api` 产物逐字节未变 |

> 本节按 §12.4 的方式留了**可复跑**的驱动：捕获桩 + 双客户端满字段驱动。
> 与 §12.4 的采样器一样，它是**审计工具而非 CI 门禁**（`external/mem0` 被 gitignore）。
