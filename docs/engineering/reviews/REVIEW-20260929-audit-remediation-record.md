# 2026-09-28/29 全仓审计修复记录

- 日期:2026-09-29
- 范围:`sdkwork-memory` 全仓(工作树基线:mem0 wire 兼容落地后)+ 兄弟仓最小改动(`sdkwork-web-framework`、`sdkwork-drive`)
- 前置:[`REVIEW-20260928-mem0-wire-compatibility-plan.md`](./REVIEW-20260928-mem0-wire-compatibility-plan.md)(mem0 线面六轮取证)、2026-09-28 第三轮全仓审计(六面并行)
- 性质:point-in-time 修复台账。每项列出缺陷、修复、与验证方式;后续状态变化写新文档,不改写本文。

## P1(全部修复,逐项实测)

| # | 缺陷 | 修复 | 验证 |
| --- | --- | --- | --- |
| 1 | `forget_all_records_in_space` 分页查询无 +1 哨兵,`has_more` 恒 false → 空间级遗忘只删前 200 条即报成功(隐私合规级漏删) | `privacy.rs` 补哨兵行;同型三处(`forget_records_for_user`/`forget_records_matching`)核对为"循环到空"形态、无需修改 | 新增 `sqlite_forget_all_records_in_space_sweeps_past_the_page_boundary`(205 条跨页);变异自证:还原旧绑定测试即红 |
| 2 | SQLite 内嵌迁移守卫钉 `"0013"` 而迁移列表已到 `0015` → 存量库永不应用 0014/0015,运行时 `no such column` | 守卫键改为**从迁移列表末项推导**(单一真源,不可能再漂移);PG 侧核实基线为全折叠快照、本就完整 | 新增 `embedded_migration_guard_tests` 三断言(末项版本钉死、版本严格递增);守卫测试在加 0016 时如约拦截旧期望 |
| 3 | k8s 清单注入 `SDKWORK_MEMORY_CORS_ALLOWED_ORIGINS`,代码只读 `SDKWORK_CORS_ALLOWED_ORIGINS` → 生产 CORS 白名单恒空 | `deployment.yaml` 改为规范键名;全清单 28 个环境变量逐一交叉核对(仅此一处漂移) | 清单-代码逐变量 grep 核对 |
| 4 | 检索重水化每空间对每个候选拉一次点查(join_all 无上限)→ 单请求最多 32×200 次点查/驻留 | SPI 加带默认实现的 `retrieve_canonical_batch`;native-sql 以分块(100/批)参数化 IN 查询覆写;服务端每空间一次批量读,授权/范围/敏感度过滤逐字保留 | service/integration 全测试绿;行为过滤逻辑逐行等价改写 |
| 5 | LLM 抽取 prompt 随事件内容无界(1000 事件 × 各自 body 上限 → 单作业 GB 级内存) | 前缀字节预算(`SDKWORK_MEMORY_EXTRACTION_MAX_INPUT_BYTES`,默认 2 MiB);超出部分如实计入响应 `skippedEventCount`,两种抽取模式一致 | 编译+service 测试;预算语义写入 `platform.rs` 文档 |
| 6 | Drive 导出全量驻留内存约 3 份 × 无进程级并发闸 → 并发 64MiB 导出可瞬态数 GB RSS | 进程级 `tokio::sync::Semaphore`(`SDKWORK_MEMORY_EXPORT_MAX_CONCURRENCY`,默认 2);不支持的导出格式在采集前即拒;本地 Drive `put_object` 的阻塞 `fs::write` 移入 `spawn_blocking`(sdkwork-drive 仓) | 驱动仓 7 测试绿;本仓 export 相关测试绿 |
| 7 | mem0 `GET /v1/memories/{id}/history/` 经活记录鉴权 → 删除后 404,官方客户端"删后查历史"主流程不可用 | 鉴权改经空间(空间存续);`user_id` 在记录删除后如实缺省 | 线面流测试新增"删除后 history 200 且含 DELETE 事件"断言 |

## 关键 P2(修复)

- **mem0 §13.5 的 21 个静默丢弃字段全部收口**:实现 `add.filters`(v3 身份拼写,与顶层拼写合并、顶层优先、未知键具名 400)、`add.expiration_date`(落 `expires_at`)、`search.metadata`(合并进过滤合取,语法保留字具名 400)、`list.show_expired`、`entities.page`(具名 501);具名拒绝 `add.timestamp/custom_categories/custom_instructions/agent_custom_instructions/structured_data_schema`、`search.fields/categories/reference_date/latest_only/keyword_search`、`list.start_date/end_date/categories/latest_only`;`search.source` 声明为"接受并忽略"(上游同义)。权威 OpenAPI 同步重新材料化,DTO↔OpenAPI parity 门禁绿。
- `ProductionFailClosed` 鉴权分支补 `DefaultBodyLimit`(三模式统一执行 `SDKWORK_MEMORY_MAX_BODY_BYTES`)。
- mem0 兼容空间查找改 `uk_ai_space_owner_type` 精确索引查找(`find_space_id_by_owner_and_type`)——>200 空间主体不再永久 500;分页扫描常量与其错误注释一并删除。
- `PUT/DELETE /v1/batch/` 预检区分 NotFound 与其他错误:存储故障不再谎报"memory N not found"。
- `append_event`(SPI)与 `insert_learning_job` 补唯一冲突恢复(重推导幂等裁决/幂等键语义成功),与 `append_open_api_event` 对齐。
- 三处 stale-requeue(learning/eval/outbox)由全局无界 UPDATE 改为 500 行批(先枚举 id、再按表+谓词更新,进度有保证);共享 `requeue_stale_running_on_table` 助手消除重复。
- 硬删清理三列加复合索引(新迁移 PG `0003` + SQLite `0016`,成对 down,基线投影重折叠;parity 门禁 110/110 索引双方言对齐)。
- retention sweep 加集群租约(会话级 advisory lock,独立连接,Drop 安全网)——N 副本每窗口只跑一份;provider-health 原有租约核实未回退。
- `web_module()` 无后台 worker 的装配路径加一次性显式告警(行为不变,API_ASSEMBLY_SPEC §6.2.1 保持;消除静默)。
- Cursor:MAC 比较改常时;生产启动拒绝 <32 字节的签名 key;新增伪造/截断/未知前缀/多余段的拒绝测试(全部 400 `validation_error`,非 5xx)。
- 预鉴权限流:框架 `RateLimitPolicy.pre_auth_aggregate_multiplier`(默认 `None` 零行为变化),聚合桶按 path+tier 共享;测试钉住"跨类型轮换被聚合桶截断"与"默认关闭时各类型桶独立"两个行为。审计中发现既指纹只含**类型掩码**(非凭据值)——同类型轮换本就共享桶,真正可碎片化的是跨类型轮换,聚合桶精确封住该洞;本仓生产策略启用 ×20。`X-Api-Key` 首版测试失败暴露了这一事实,测试按真实语义重写。
- 契约补齐:`GET /v1/entities/` 声明 `page`/`page_size` 查询参数(运行时行为与权威文档一致)。

## 文档/门禁

- `docs/INDEX.yaml` 补登 3 份 REVIEW;`reviews/README.md` Records 补全。
- `sdkwork.workflow.json` validate 阶段补 `pnpm check` + PC check(发布提交重跑契约/分页/信封门禁)。
- PRD(保留租约)、TECH_ARCHITECTURE(租约化 sweep、常时比较、key 强度、聚合桶、抽取预算、导出信号量、批量重水化)、database/README(修正 baseline-plus-migrations 语义描述)、`.env.example`(新变量)同步。

## 第二轮对齐复查追加(同日落盘)

- **降级 LIKE 搜索的大小写语义双方言统一**:FTS 不可用时的关键词回退、事件 payload 搜索、habit 列表过滤、forget 按文本匹配,列与参数两侧统一 `lower()` 包裹——修复"PG 大小写敏感 / SQLite 不敏感"的同查询不同结果分歧(各引擎内部两侧用同一 `lower()`,引擎内一致)。
- **冒烟测试适配 key 强度门槛**:`api_server_smoke_test` 按"仅查非空"的旧语义用的 21 字节测试 key 在新的 ≥32 字节生产门槛下先触发;测试 key 改为 39 字节并在注释中写明三条拒绝路径。cursor 准入抽成纯函数 `validate_cursor_signing_key` 并新增无环境竞争的单元测试(缺失/空白/31 字节拒,32 字节过)。
- **框架仓预存红灯清零**:`sdkwork-routes-web-framework-backend-api` 的 `committed_openapi_authority_matches_runtime_contract`(2026-09-28 评审 §7.5 第 7 条登记的 HEAD 既有漂移)通过重跑权威材料化修复,差异即已登记的 `deprecated limit` 参数;框架全工作区 587 测试 0 失败。

## 第三轮对齐循环(同日落盘)

- **分页 mode 逐操作文档化**(PAGINATION_SPEC §3 "MUST be documented per operation"):两个共享参数构建器(`listParams`/`cursorListParams`,覆盖全部 37 处 owned 列表操作声明)为 `cursor`/`page_size` 加规范级 description——游标不透明、伪造即 400、默认 20/上限 200、稳定 keyset 位次非序数页。材料化后 pagination/envelope/契约门禁全绿。
- **无界语句清零收尾**:`delete_events_in_space`/`delete_events_for_user_all_spaces`/`delete_events_for_user_in_space` 与 `purge_expired_records_for_scope` 由全范围单语句 DELETE/UPDATE 改为 500 行批(先枚举 id,子表 `ai_record_source` 先于父表逐批删除,外键序保持);`delete_sources_referencing_space_events` 由此失去存在必要,删除。forget 大范围场景不再有锁整段持有。
- **database-host 补 PG15 版本准入**:根生命周期引导在引擎准入后、`init()` 前断言 `server_version_num >= 150000`,PG14 得到具名诊断而非 DDL 中段的 `NULLS NOT DISTINCT` 语法错误;镜像数据面既有断言(常量因依赖方向在两仓各自声明,注释写明)。sqlx 以 `default-features = false`(仅 runtime-tokio+postgres)加入生产依赖,不引入 SQLite 驱动链接。
- **P3 清仓**:`decode_bool` 由"记错误日志后静默回 false"改为传播存储错误(4 个调用点同步);`mark_outbox_published/failed` 补 `rows_affected` 围栏判定——过期 worker 的 ack/fail 现在如实得 `None`,与 `ack_outbox_delivery_success` 契约一致(SPI 端口本就返回 Option,无调用方破坏);删除全仓零调用且无界读取的 `pub list_entity_memory_links`(有界变体 `..._for_memories` 是在用真源);mem0 `add.messages` 加 1000 条具名上限(字节由 body limit 管住,条数由面自己管住)。
- **依赖库最新化**:慎重 `cargo update` 把全部 semver 兼容依赖推到最新(锁文件 25+ 包更新);无通配/漂浮主版本。主版本级落后项经逐项评估:**rand 0.8→0.10 尝试后被依赖链阻塞**(opentelemetry 0.27 钉 rand ^0.8,级联需 otel 0.27→0.33 + reqwest 0.12→0.13 + sha2 0.11 + hmac 0.13 + base64 0.23 的跨仓联合升级),按 RUST_CODE_SPEC §14"主版本升级是慎重决策"回退并在下方登记为钉住项;PC 应用 semver 兼容更新后 `pnpm check` 绿。试升级期间对兄弟仓 sdkwork-utils 的临时改动已按 CODE_STYLE_SPEC §7.3 枚举路径自愈模式恢复,兄弟仓回到 HEAD。
- **主版本钉住清单(慎重决策,待专项升级窗口)**:`opentelemetry*` 0.27→0.33、`reqwest` 0.12→0.13、`sha2` 0.10→0.11、`hmac` 0.12→0.13、`base64` 0.22→0.23、`rand` 0.8→0.10(被 otel 链阻塞)、`generic-array` 0.14.7→0.14.9。每项都需按依赖管理规范单独评审,不建议与本仓功能修复混批。

## 第四轮对齐循环:主版本级联升级落地(2026-09-29 续)

第三轮登记的主版本钉住清单在本轮全部落地,依赖栈整体到达 2026-09 的最新发布:

- **`reqwest` 0.12 → 0.13**:TLS feature 词汇更替(`rustls-tls` → `rustls` + `webpki-roots`);出口客户端(SSRF 钉扎、16 MiB 响应上限、重定向禁用)用法零改动,provider/service 测试全绿。
- **加密家族 `sha2` 0.11 / `hmac` 0.13 / `base64` 0.23 / `hkdf` 0.13 / `aes-gcm` 0.11**(本仓 + `sdkwork-utils-rust`):digest 0.11 把密钥初始化移到 `KeyInit` trait(hmac 重导出);cursor MAC 与兄弟仓 HMAC/SHA256/AES-GCM 往返测试全部保持通过——密码学语义逐位不变(测试向量即证)。`KeyInit` 与 `aes_gcm` 的二义性由共享工作树的另一会话以别名导入收敛。
- **观测家族 `opentelemetry*` 0.27 → 0.33 / `tracing-opentelemetry` 0.28 → 0.34**:`TracerProvider` 更名 `SdkTracerProvider`,`with_batch_exporter` 不再收 runtime 参数(`rt-tokio` 特性绑定);gateway 的 `otel` 特性带特性编译零错。
- **`rand` 0.8 → 0.10**:otel 0.33 解锁后落地;`thread_rng().gen_range` → `rng().random_range`(`RngExt` blanket trait)。
- **诚实性收尾**:`entity_filter` 的合并语义注释改为如实陈述优先级(`filters` 同轴值胜出,官方客户端不会触发);删除全仓零调用且无 keyset 的死代码 `list_admin_config_entities`(与已删的 `list_entity_memory_links` 同类)。

主版本钉住清单至此清空。剩余的版本工作只有 `cargo update` 的例行跟进(semver 兼容层)。

**第四轮验证总账(逐项退出码)**:`cargo test --workspace --no-fail-fast` **821 passed / 0 failed suites / WS_EXIT=0**——在完整最新依赖栈(reqwest 0.13、sha2 0.11、hmac 0.13、base64 0.23、otel 0.33 家族、tracing-opentelemetry 0.34、rand 0.10)之上;`pnpm check` exit 0;DTO↔OpenAPI parity 与路由清单 parity exit 0;`sdkwork-utils-rust` 在其自身锁下 126 passed / 0 failed。期间一次全量红灯(`runtime_bootstrap` 的 sqlite 准入)根因判定为**验证方法缺陷而非代码缺陷**:同目录并发运行两个 cargo 调用使 `sqlite` feature 变体(由 integration-tests 经 assembly 启用)在两套 feature 图间换入换出;严格串行重跑即 7/7 与 821/0 双绿——教训与 §10.5 同源:同一 target 目录严禁并发 cargo 调用。

## 第五轮对齐循环:重复能力去重与依赖图卫生测绘(2026-09-29 续二)

- **规范去重(RUST_CODE_SPEC §14 "不得重复兄弟仓既有能力")**:cursor MAC 验证的手写 `constant_time_eq` 删除,改用 `sdkwork_utils_rust::secure_compare`(同源的常时折叠比较,crate 根导出);伪造/往返测试改钉"委托契约"——本仓不得再悄悄长出本地常时比较副本。
- **依赖图卫生测绘**:解析 `Cargo.lock`(426 crate)发现 29 个 crate 携带重复主版本;`cargo tree -i` 逐一溯源后确认陈旧要求方全部来自三个兄弟仓的安全敏感代码——`sdkwork-drive`(reqwest 0.12、sha2 0.10、hmac 0.12、tower-http 0.6)、`sdkwork-database`(base64 0.22)、`sdkwork-iam`(rand 0.8、aes-gcm 0.10);`axum` 单一 0.8.9、框架仓已对齐。三个兄弟仓当前各自 `cargo check` 零错误(健康起点)。
- **裁定(登记为专项,不在本仓批次强推)**:对齐上述三仓需升级其 HMAC/加密/认证路径并按仓补各自测试证据;`syn` 2/3、`getrandom` 0.2/0.3/0.4、`windows-sys` 等来自未持有第三方发布物,属不可消除的过渡态。跨仓版本对齐作为独立工作流执行,迁移映射(哪个仓、哪个 crate、哪个目标版本)以本节为起点记录。

## 第六轮对齐循环:跨仓版本对齐执行(2026-09-29 续三)

第五轮测绘的迁移映射在本轮直接执行,逐仓独立锁、独立测试:

- **`sdkwork-drive`**:`opendal` 0.57 → 0.59.3(上游新版本把 reqwest/sha2/hmac/tower-http 栈上移;两处 API 适配——`Operator::new` 直返、`UserMetadata` 改 `IntoIterator`);observability 的 `opentelemetry*` 0.30 → 0.33、`tracing-opentelemetry` 0.31 → 0.34(代码已用新命名,零适配);根与 `drive-security` 的 reqwest 0.12 → 0.13(TLS feature 迁移 + `blocking` 保留)。drive 全仓编译零错,依赖测试通过。
- **`sdkwork-iam`**:`iam-web-adapter` reqwest 0.12 → 0.13(0.13 把 `query`/`form` 变为可选特性,按适配器实际用量补齐);**107 测试全绿**。`rand`/`aes-gcm` 经上游核实**被 RustCrypto `password-hash` 0.5 阻塞**(钉 rand_core 0.6)——非本仓可解,登记上游等待项。
- **`sdkwork-database`**:核实 `base64 0.22` 由 **sqlx-core 0.9 自身**持有——上游所有,无需也无法在本侧消除。
- **本仓回锁**:`reqwest 0.12` 从锁中消失(重复主版本 29 → 28);对剩余 28 个逐一溯源——全部**上游所有**(RustCrypto 0.10 代差经 aws-sdk-s3 SigV4 与 password-hash、reqwest 0.13.5 内部依赖 tower-http 0.6、syn 2/3、getrandom 三代),跨仓/生态内对齐已尽。

**drive 仓两处预存红灯(与本轮无关,取证在案)**:`database_schema_parity` 的两用例在 HEAD 即红——测试引用 `0002_drive_outbox_pending_dispatch_index.up.sql` 但该迁移文件从未提交、schema registry 缺 `dr_drive_storage_provider_kind` 表;属 drive 仓 schema 工作流(疑似另一会话 TDD 半成品),已取证不属依赖升级回归。

**本轮验证总账**:drive `cargo check --workspace` 零错 + 依赖测试通过(唯一红灯为上述 HEAD 预存);iam `cargo test -p sdkwork-iam-web-adapter` 107 passed / 0 failed;本仓串行终验 **821 passed / 0 failed / WS_EXIT=0**、`pnpm check` exit 0。

## 第七轮对齐循环:官方 SDK 端到端复验(2026-09-29 续四)

第三轮对 mem0 请求 DTO 做了大面积具名拒绝改造,而官方 SDK 验收是 `#[ignore]` 用例——全量测试从未覆盖它。本轮把两条验收在改造后的服务器上**真实重跑**:

- **Python `mem0ai==2.2.0`**:按验收发现契约在仓根重建 `.venv`(发现顺序原生支持)并安装,`official_mem0_sdk_flow` **1 passed**——完整调用链(ping/add/get/search/update/history/users/delete/delete_all + 11 条具名拒绝)端到端通过。
- **JS `mem0ai`(npm)**:node_modules 按隔离目录安装(pnpm 工作区与 npm `--no-save` 互斥,故用 `MEM0_E2E_SDK_ROOT` 指向 `.sdkwork/mem0-js-sdk`,零仓库污染),`official_mem0_js_sdk_flow` **1 passed**。

**结论**:第三轮的 21 字段收口(实现 + 具名拒绝 + 声明忽略)对两个官方客户端**向后真实兼容**——被拒绝的字段恰好是驱动未依赖的字段,被实现的字段(`add.filters`、`expiration_date`、`search.metadata`)按新语义工作。双客户端验收从此有了本机可复现的夹具约定(仓根 `.venv` + `MEM0_E2E_SDK_ROOT` 隔离目录),复跑命令记录于本节。

抽取路径残余的按事件串行读取(N+1)经评估保持现状:字节预算使获取次数以预算填充数为上界,属有界吞吐细节而非正确性问题;进一步批量化需 SPI 面变更,登记为后续可选优化。

## 验证总账(逐项退出码)

- `pnpm check`:exit 0(15+ 门禁含 parity/pagination/envelope/db-dialect-parity/docs;全部修复落盘后复跑仍 0)。
- `cargo test --workspace --no-fail-fast`(本仓):**821 passed / 0 failed,WS_EXIT=0**(基线 809 → 821,净增 12 条回归测试)。
- 框架仓 `cargo test --workspace --no-fail-fast`:**587 passed / 0 failed,FW_EXIT=0**(含修复后的预存红灯)。
- 15 个 node 契约测试全 OK;材料化器连续两次运行产物逐字节一致(sha256sum -c 三件全 OK);`verify_openapi_operation_ids.ps1`、architecture alignment、SDK ownership、database-framework 契约、cors-standard 均 exit 0。
- `sdkwork-drive-storage-local`:7 passed / 0 failed。
- 双方言 parity:28 表 / 110 索引两侧一致。

## 遗留(有意决策,非遗留债)

- 商业化发布门中的 load/soak 证据、回滚演练记录、容器 smoke 记录仍缺(运营工作流,见 PRD Release State)。
- 嵌入向量无持久化、feedback 只写不读、`/api/v1/` mem0 平台账号面不服务不声明(既有裁定不变)。
- 框架 `credentials_fingerprint` 的类型掩码语义保持不变(值级指纹需与聚合桶默认开启联动,属框架侧独立评审项;当前聚合桶已在本仓生产配置启用)。
