# mem0 ↔ SDKWork Memory 能力对齐矩阵

| 项 | 值 |
| --- | --- |
| mem0 参考版本 | `f8082a7345dadd9e042ebbc40b57b1498c8f6d63`（2026-09-22，`pyproject.toml` 声明 `2.1.0`） |
| mem0 本地位置 | `external/mem0/`（已入 `.gitignore:102`，不参与构建与交付） |
| 本仓 | `D:\sdkwork-space\sdkwork-memory` |
| 编制日期 | 2026-09-23 |
| 方法 | 逐文件读上游源码取证 → 与本仓 SPI / contract / DB baseline / retrieval / 插件逐项对照 |

> 判定口径：**对齐** = 能力存在且语义等价；**部分** = 存在但语义或覆盖度不足；**缺失** = 无对应实现；
> **不适用** = 架构上不应在本仓出现（如 provider 凭据、多租户边界外的托管能力）；
> **超越** = 本仓比 mem0 更强。

---

## 0. 五条必须先说清的上游事实（会改变对齐方向）

### 0.1 上游 HEAD 的自动写入路径是 **ADD-only + linking**，不是四操作仲裁

`mem0/memory/main.py:916` 标注 `# === V3 PHASED BATCH PIPELINE ===`，Phase 2 单次 LLM 调用使用
`ADDITIVE_EXTRACTION_PROMPT`（`main.py:942`），落库事件恒为 `"ADD"`（`main.py:1070`、`main.py:1196`）。

`"UPDATE"` 与 `"DELETE"` 事件**只出现在显式 API 路径**：
`_update_memory`（`main.py:2084`）与 `_delete_memory`（`main.py:2116`）。

经典的四操作仲裁提示词 `DEFAULT_UPDATE_MEMORY_PROMPT`（`configs/prompts.py:176`）在本 commit 是**死代码**
—— 全仓仅被 `configs/prompts.py:406` 的 `get_update_memory_messages()` 引用，而后者无调用方。

> **对齐含义**：自动写入应当对齐 **ADD-only + `linked_memory_ids`**，而不是 LLM 自主 UPDATE/DELETE。
> 这同时也是**更安全**的语义：LLM 不再有机会自行删除或覆写既有记忆。
> 本仓 `plugins/sdkwork-memory-plugin-search-first-vector` 原先实现的是经典四操作仲裁
> （`src/arbitration.rs` 的 `MemoryArbitrationDecision::{Add,Update,Delete,Noop}`）。
> **批次 2 已完成转向**：`arbitration.rs` 已删除，代之以 `additive.rs`（内容哈希去重 + 链接校验）；
> 证据见 §12。

### 0.1.1 上游把模型回传的 `linked_memory_ids` **构建后丢弃**

这条决定了对齐的写法，必须单列。三处硬证据：

1. Phase 1 建立序号→UUID 映射，`uuid_mapping[str(idx)] = mem.id`（`main.py:935-937`），
   但 `grep -c uuid_mapping mem0/memory/main.py` 全仓仅 4 处命中：2 处初始化、2 处赋值，
   **无任何读取**。
2. 提示词明确要求「`linked_memory_ids` uses the UUIDs from this list」（`prompts.py:513`）与
   「Use the exact IDs from the Existing Memories list」（`prompts.py:936`），
   而 Phase 1 递给模型的列表里 `id` 就是序号字符串（`main.py:938`）——即**回传的只能是序号**，
   而把序号翻译回 UUID 的 `uuid_mapping` 又从不被读。
3. `grep -n linked_memory_ids mem0/memory/main.py` 共 26 处命中，**全部属于实体记录**
   （624-693 / 1157-1175 / 1797-1805 / 2292-2370 / 2814-2832 / 3452-3460：
   实体 upsert 的链接集合、实体删除时的反查、实体增强打分时的读取）。
   提取输出里的该字段**没有消费者**。

> **对齐含义**：本仓**不复刻这个丢弃**。`resolve_linked_memory_ids` 保留链接，并逐条对照
> 调用方实际给出的证据校验；未被提供过的 id 被**记录**在 `plan.unverified_links` 里而不是静默吞掉。
> 这既不发明边（不能指向调用方从未授权的记忆），也不因为一条坏链接丢掉一条有效记忆。

### 0.2 上游 OSS 没有图记忆

全仓 `graph_store` / `enable_graph` / `MemoryGraph` 仅命中 `mem0/exceptions.py:396`，
而那是 `DependencyError` 的 docstring 示例。无 `mem0/graphs/` 目录。
官方文档一致承认 OSS 只有「实体增强排序」（`docs/core-concepts/how-it-works.mdx:64,78`），
图记忆为 Platform-only。

> **对齐含义**：**不存在需要对齐的 mem0 图能力**。本仓的 `ai_entity` / `ai_edge`（含时间有效性）
> 反而**超越**上游 —— 见 §5。

### 0.3 `memory_type` 只有 `procedural_memory` 一条真实路径

`configs/enums.py:4-7` 枚举三值，但 `add()` 只接受 `procedural_memory`，其它非 None 值直接抛错
（`main.py:831-837`）；`semantic_memory` / `episodic_memory` 从未被读取。
无 `categories` 字段，无 custom categories 实现。

### 0.4 **BM25 的原生打分不在 mem0 仓库里** —— 它被下推给向量库后端

这条是 2026-09-23 补取证时发现的，它**改写了"对齐 BM25"这句话的含义**，必须放在最前面。

`mem0/utils/scoring.py` 只做**归一层**：`get_bm25_params`（按 query 词数选 sigmoid 中点/陡度）与
`normalize_bm25`。真正的 raw keyword score 由各后端自己的 `keyword_search()` 产出，**量纲互不相同**：

| 后端 | raw score 来源 | 取证位置 |
| --- | --- | --- |
| PostgreSQL（pgvector） | **`ts_rank_cd`**，不是 BM25：`ts_rank_cd(to_tsvector('simple', payload->>'text_lemmatized'), plainto_tsquery('simple', %s))` | `vector_stores/pgvector.py:389` |
| Elasticsearch | `multi_match` 查询 → **Lucene BM25**（k1=1.2, b=0.75，含 Lucene 版 IDF） | `vector_stores/elasticsearch.py:200-203` |
| qdrant / weaviate / redis / mongodb / milvus / pinecone … | 各自实现，共 16 个后端各有一份 `keyword_search` | `vector_stores/*.py` |

> **对齐含义（重要）**：本仓的存储是 PostgreSQL（`tsvector`）。如果逐字对齐**上游在 PG 上的行为**，
> 应当用 `ts_rank_cd`；如果对齐**BM25 语义**，应当实现 Okapi BM25，此时对齐的是上游的
> **Elasticsearch 分支**。
>
> 本仓选择后者（`crates/sdkwork-memory-retrieval/src/bm25.rs`：k1=1.2 / b=0.75 / Lucene 版
> 恒正 IDF），理由有二：① `ts_rank_cd` 的取值区间与
> `get_bm25_params` 那套按 BM25 量级（典型 0–20+）标定的 sigmoid 中点（5–12）**不匹配**，
> 见附录「上游自身缺陷」；② 上游 16 个后端里唯一语义明确叫 "BM25" 的就是 ES 分支。
> 该选择在代码里以文档注释显式标注，不是默认继承。

### 0.5 上游的实体抽取**完全依赖 spaCy**，无模型即返回空

`extract_entities(text)` 先取 `get_nlp_full()`，为 `None` 直接 `return []`
（`utils/entity_extraction.py:751-758`）。而四个候选类里**只有 `QUOTED` 是纯正则**：

| 候选类 | 上游判定依据 | 无 spaCy 可否复现 |
| --- | --- | --- |
| `PROPER`（`spacy_ner`） | `doc.ents` + 11 个 NER label 白名单 | 否 |
| `PROPER`（`proper_name_span`） | `pos_` / `tag_` / `dep_` / `is_stop` | 否 |
| `TOPIC`（`topic_phrase`） | `doc.noun_chunks` + `pos_` / `dep_` | 否 |
| `IDENTIFIER`（`technical_identifier`） | 纯正则 `[A-Za-z_][\w-]*(?:\.[A-Za-z_][\w-]*)+` | **是**（需先有分词） |
| `QUOTED`（`quoted`） | 纯正则，且 start/end 记为 `-1,-1` | **是（精确）** |

> **对齐含义**：本仓不引入 500MB 模型依赖，因此**不可能**逐字复刻"带 spaCy 的上游"。
> 对齐口径改为：① 与 spaCy 无关的部分（`QUOTED`、`IDENTIFIER`、`_clean_text`、`_has_artifacts`、
> 去重/重叠/排序规则、boost 算术）**精确移植**；② POS/NER 依赖的部分为**确定性近似**，
> 且每个近似都在代码里带独立 `source` 标签（如 `identifier_heuristic`），使评审能一眼区分
> "精确对齐"与"近似"。详见 §3 与 §5。

---

## 1. 公开 API 面对照

| mem0 API | mem0 语义要点 | 本仓对应 | 判定 |
| --- | --- | --- | --- |
| `add(messages, *, user_id, agent_id, run_id, metadata, timestamp, expiration_date, infer, memory_type, prompt)` | `infer=True` 走 V3 ADD-only；`infer=False` 逐条原样落库；`timestamp` 抛 `ValueError` | `MemoryRecordStorePort` / `CreateMemoryRecordCommand`（`spi/ports.rs:34`）+ `sdkwork-memory-contract` 的 `CreateMemoryCommand`；HTTP 面在 `crates/sdkwork-routes-memory-open-api` | **部分**（缺 `infer` 开关、`expiration_date`、`memory_type`、`prompt` 覆盖） |
| `search(query, *, top_k, filters, threshold, rerank, explain, reference_date, show_expired)` | 混合检索 + 阈值 + rerank + explain | `MemoryRetrieverPort::search_scoped`（`spi/ports.rs:1156`）+ `sdkwork-memory-retrieval` | **部分**（缺 `threshold`、`explain`、`show_expired`、`reference_date`；**`filters` 已对齐**，见 §4 / §14） |
| `get(memory_id)` | 返回平铺 dict；查不到返回 `None` | `RetrieveMemoryRecordQuery`（`spi/ports.rs:152`） | **对齐** |
| `get_all(*, filters, top_k, show_expired)` | `filters` 必须含实体 id；`fetch_limit = max(limit*4, 60)` | 列表接口（`ListMemoriesQuery`） | **部分**（无 over-fetch 语义） |
| `update(memory_id, text, metadata, expiration_date)` | 文本变更才重建实体链接 | `UpdateCanonicalMemoryCommand`（`spi/ports.rs:136`） | **部分**（无实体重链接） |
| `delete(memory_id)` | 先校验存在，不存在抛错 | `DeleteMemoryRecordCommand`（`spi/ports.rs:158`） | **对齐** |
| `delete_all(user_id, agent_id, run_id)` | 顶层参数（非 `filters`）；批量 1000 | 无等价批量删 | **缺失** |
| `history(memory_id)` | 版本链 `old_memory`/`new_memory`/`event` | `MemoryEventStorePort`（`spi/ports.rs:868`）+ `MemoryMutationJournal`（`spi/ports.rs:70`） | **超越**（journal 语义更严） |
| `reset()` | 清 SQLite + 向量库 + 实体库 | 无等价（本仓按 scope 结构隔离，无环境级 reset） | **不适用**（设计上更安全） |
| `AsyncMemory` | 全异步 | Rust 全异步（`async_trait`） | **对齐** |

---

## 2. 作用域与身份模型

| 维度 | mem0 | 本仓 | 判定 |
| --- | --- | --- | --- |
| 身份轴 | `user_id` / `agent_id` / `run_id` 三者独立可选、AND 收窄；至少一个 | `MemoryScopeContext { tenant_id, space_id, organization_id, user_id }`（`spi/ports.rs:8`） | **超越**（多了租户与空间两层） |
| 租户 | **OSS 无**（`app_id` 为 Platform-only） | `tenant_id` 强制 | **超越** |
| 第四身份轴 | `actor_id`（不写入存储 metadata） | `MemoryGovernanceActor`（`spi/ports.rs:536`）+ `MemoryActorSpaceBindingFact` | **超越** |
| 作用域进入 filter | 写入时只保留三 id，刻意丢弃其它键（`main.py:924`） | `read_scope: MemorySensitivityReadScope`（`Public/Elevated/Owner`）+ 结构上不接受环境作用域 | **超越** |
| 空作用域守卫 | `VALIDATION_001` | 结构性（无作用域即无法构造查询） | **超越** |

> 本仓在这一层**全面强于上游**：mem0 的 `reset()` 全库清空与跨作用域读写（见
> `REVIEW-20260923-memory-mem0-implementation-audit.md` §C-1/§C-2）在本仓架构上无法表达。

---

## 3. 检索与排序（**上游领先的核心区**）

> **本轮（批次 1）已落地**部分以 **✅ 已对齐** 标注，落点为
> `crates/sdkwork-memory-retrieval/src/{bm25,lemmatization,scoring,entities}.rs`。
> 验证证据见本文 §11。

> ## 🔴 可达性更正（2026-09-23，批次 6 复核）—— 本节此前**高估**了
>
> 本节所有 ✅ 的落点都在 `bm25.rs` / `lemmatization.rs` / `scoring.rs` / `entities.rs` 四个模块里。
> 复核结论：**这四个模块从部署入口完全不可达，本节 ✅ 描述的是「已实现」而不是「已生效」。**
>
> **硬证据（两条，互相独立）：**
>
> 1. **符号级**（`.workbuddy/tools/audit-symbol-reachability.py`）：14 个符号判定为 `DEAD` ——
>    `score_and_rank`、`HybridSignals`、`max_possible_score`、`validate_threshold`、
>    `internal_fetch_limit`、`ScoreDetails`（`scoring`）；`Bm25Index`、`normalize_bm25`、
>    `get_bm25_params`（`bm25`）；`lemmatize_for_bm25`（`lemmatization`）；`entity_boosts`、
>    `select_query_entities`、`extract_entities`、`memory_count_weight`（`entities`）。
>    同时 `keyword_match_score` / `orchestrate_retrieval_candidates` /
>    `fuse_retrieval_candidates_with_policy` / `build_context_pack_from_hits` / `time_recency_score`
>    判定为 `REACHED`。
> 2. **图级**：`retrieval/mod.rs` 与 `context_pack.rs` —— 服务唯一消费的两个模块 ——
>    对 `scoring` / `bm25` / `entities` / `lemmatization` 的引用**为零**
>    （`grep -n "scoring\|bm25\|entities\|lemmatiz\|Bm25\|entity"` 两文件合计零命中）。
>
> **根因**：`sdkwork-memory-retrieval` 里存在**两套互不引用的打分栈**。
>
> | 栈 | 模块 | 算法 | 状态 |
> | --- | --- | --- | --- |
> | **可达栈** | `retrieval` + `context_pack` | 5 路信号（`keyword_match_score` 子串+词元重合、dictionary、sql_structured、time_recency、event）+ **RRF 融合** | 服务与 3 个 routes crate 在用 |
> | **死栈** | `scoring` + `bm25` + `entities` + `lemmatization` | **mem0 对齐**：真 BM25 + 自适应 sigmoid + 加法归一 + `max_possible` + 阈值 gate + `explain` 分解 + 实体加权 | **零调用点**，只有 `lib.rs` 再导出 |
>
> `MemoryRetrievalStrategy` 只有 `Balanced` / `SearchFirst` / `EventAware` 三个变体，全部是
> 可达栈的信号权重组合，**没有**指向加法归一栈的档位 —— 即这套实现不但不可达，连
> 「可选的另一条策略」都没接。
>
> **因此本节的 ✅ 应当读作「已实现、已被测试钉住、但运行时零贡献」**，与 §13 的插件属同一缺陷类
> （`QUALITY_GATE_SPEC` §31 的口径：不可达的能力贡献为零）。关闭方式见 §10 批次 8。
> 本节**不删除**这些行，因为「实现已存在且有逐条对照证据」这一事实仍是真实资产 ——
> 需要更正的是它的**效力判定**，不是它的存在。

| 能力 | mem0 实现 | 本仓现状 | 判定 |
| --- | --- | --- | --- |
| **BM25 原生打分** | 不在 mem0 内，下推给后端（PG=`ts_rank_cd`、ES=Lucene BM25，见 §0.4） | ✅ `bm25.rs`：Okapi BM25，k1=1.2 / b=0.75 / `Bm25Index::fit`+`score`。**对齐 ES 分支**（见 §0.4 的选择理由） | **对齐**（有意的分支选择，已书面标注） |
| **自适应 sigmoid 归一** | `get_bm25_params` 5 档 `(midpoint, steepness)` + `normalize_bm25`（`utils/scoring.py:16-54`） | ✅ `bm25.rs::get_bm25_params` 五档逐一对照（`0..=3 → (5.0,0.7)` … `>15 → (12.0,0.5)`）；`normalize_bm25` 同式，额外把非有限值/非正陡度收敛为 `0.0` | **对齐**（+硬化） |
| **词形还原** | `lemmatize_for_bm25(text)` → payload `text_lemmatized`；保留 `-ing` 原形解决名/动词歧义；"避免过度词干化"（`organization != organize`） | ✅ `lemmatization.rs`：确定性实现，含 `IRREGULARS` 表、`SILENT_E_STEMS` 白名单、`MIN_STEM_LEN=3`、`-ing` 双写；**无 spaCy，无过度词干化** | **对齐意图**（实现方式不同，见 §0.5） |
| **实体抽取** | spaCy 四类候选 + 优先级(1/2/3/4) + 置信度(0.90/0.80/0.75/0.45) + 重叠消解（`utils/entity_extraction.py`） | ✅ `entities.rs`：`QUOTED`/`IDENTIFIER` **精确移植**；`PROPER`/`TOPIC` 为确定性近似，带 `source` 标签区分；`clean_text`/`has_artifacts`/去重/重叠/位置排序逐条对齐 | **对齐（分层）**：2 类精确、2 类近似、NER 类缺失 |
| **实体增强** | `_compute_entity_boosts`：query 实体 `[:8]` 去重 → 每实体 `top_k=500` → `similarity>=0.5` → `boost = similarity*0.5*1/(1+0.001*(n-1)²)` → 每记忆取 **max**（`main.py:1733-1813`） | ✅ `entities.rs::entity_boosts` + `select_query_entities`（`[:8]` 去重按位置在查询侧）+ `memory_count_weight`。**修正**：上限作用于**查询实体**而非匹配结果（上一版曾误把上限放在匹配侧） | **对齐** |
| **加法融合 + `max_possible` 归一** | `(semantic + bm25 + entity_boost) / max_possible`，`max_possible ∈ {1.0,1.5,2.0,2.5}`，clamp ≤1（`utils/scoring.py:94-139`） | ✅ `scoring.rs::score_and_rank` + `max_possible_score`。**两个易错语义都保住**：`max_possible` 取决于信号 map 是否为空（而非该候选是否有值）；平局用 `memory_id` 升序求全序 | **对齐**（+确定性排序） |
| **阈值只 gate 语义分** | `semantic_score < threshold` 即淘汰，BM25/实体救不回（`scoring.py:111-112`） | ✅ `scoring.rs` 同一顺序：先 gate 再融合；`validate_threshold` 拒绝 `[0,1]` 之外与非有限值（对齐上游 `ValueError`） | **对齐** |
| **`explain` / `score_details`** | `semantic_score`/`bm25_score`/`entity_boost`/`raw_score`/`max_possible_score`/`final_score`/`threshold` 七项 | ✅ `scoring.rs::ScoreDetails` 七项字段一一对应 | **对齐** |
| **over-fetch** | `internal_limit = max(limit*4, 60)`（`main.py:1641`） | ✅ `scoring.rs::internal_fetch_limit`（饱和乘法防溢出） | **对齐** |
| **rerank** | 5 provider；失败静默回退原顺序（`main.py:1500-1505`） | `RerankModelPort`（`spi/ports.rs:1193`）+ 插件内 `reorder_hits`（不丢候选） | **超越**（不丢候选优于静默回退） |
| **多路召回** | 语义 + keyword 两路；**候选池受限于语义结果**（keyword-only 命中被丢弃，`main.py:1666-1677`） | 5 路独立信号 + RRF 真融合（`orchestrate_retrieval_candidates`） | **超越** |
| **过期过滤（检索路径侧）** | `expiration_date` + `show_expired` 默认 `False` | **可达栈不读该字段**：`retrieval` / `context_pack` 里没有 `expiration_date` 的消费点，请求面也没有 `show_expired`。写入期投影与门控在插件侧已有（见 §6 行），但插件不可达（§13） | **缺失**（批次 6/8） |
| **时间锚点** | `Observation Date` 提示词约束；但 `timestamp` 被禁 → 实际恒等于今天（`prompts.py:1007-1013`） | `time_recency_score` 7 天半衰期（`retrieval/mod.rs:332`） | **部分**（本仓有真实时间信号，但无"观察日期"锚定） |

**结论（批次 1 后）**：上游"真 BM25 + 词形还原 + 实体增强 + 加法归一打分 + 阈值 gate +
explain 分解"整条链路**已在 `sdkwork-memory-retrieval` 落地并逐条对照**，且本仓原有的
"多路召回 + RRF + 不丢候选 rerank"优势保持不变。
两条主动偏离已书面标注：① BM25 对齐的是上游 ES 分支（PG 分支上游用 `ts_rank_cd`，见 §0.4）；
② 实体抽取的 POS/NER 部分为确定性近似（见 §0.5）。
`keyword_match_score`（子串+词元重合率）保留为一路独立信号，未被替换。

---

## 4. Filter 表达式语法

> **批次 3 已落地。** 语言本体落在共享层 `crates/sdkwork-memory-spi/src/filter.rs`（**语义**），
> SQL 翻译落在 `plugins/sdkwork-memory-plugin-native-sql/src/filter_pushdown.rs`（**方言**），
> 两者以「差分对拍」互钉 —— oracle 在 SPI、被测实现是真实 SQLite 下推。验证证据见 §14。

| 维度 | mem0 | 本仓现状 | 判定 |
| --- | --- | --- | --- |
| 运算符全集 | 隐式相等 + `eq`/`ne`/`gt`/`gte`/`lt`/`lte`/`in`/`nin`/`contains`/`icontains`/`*`；未知 op **抛 `ValueError`**（`main.py:1524-1599`） | ✅ `INFIX_OPERATORS`（10 个）+ 隐式相等 + 裸 `*`；未知 op 抛 `MetadataFilterError::UnsupportedOperator`。`{"field":{"eq":"*"}}` **不**被读作通配（照上游：否则会把字面比较悄悄变成存在性判断） | **对齐** |
| 逻辑组合 | `AND` 压平为隐式与；`OR`→`$or`；`NOT`→`$not`；空 list 抛错 | ✅ 同一语义：`AND` 压平进外层合取、`OR`→`Any`、`NOT`→对其各项析取取反；空逻辑表抛错。**逻辑键在任意深度递归识别**，`{"OR":[{"AND":[…]}]}` 不会被当成名为 `AND` 的字段 | **对齐** |
| 求值语义 | 各后端各自翻译，无独立语义层 | ✅ `MetadataFilterExpression::matches` 实现 Kleene 三值逻辑（`True`/`False`/`Unknown`），刻意保留 SQL 语义：`{"owner":{"ne":"alice"}}` **不**匹配没有 `owner` 键的记录（`NULL <> 'alice'` 为 unknown）。正是这一点使它可作 SQL 翻译的 oracle | **对齐** |
| 下推一致性 | **`get_all()` 不翻译逻辑键 → pgvector 上静默空结果**（`upstream docs metadata-filtering.mdx:139-141`） | ✅ `translate_metadata_filter` 为 **Postgres / SQLite 双方言**生成谓词与绑定参数，紧接原有关键词分支注入；FTS 与 LIKE 两条回退路径**共用同一次翻译** | **上游缺陷，已消除** |
| 生效位置 | 上游在 `get_all()` / `search()` 内部应用 | ✅ API 边界（`open_api.rs`）解析 → `SearchMemoryCandidatesQuery.metadata_filter` → native-sql 下推到 SQL。**修掉「`filters` 被接收却从不生效」的静默降级**：原实现全仓 0 处引用该字段，调用方拿到未过滤结果且无任何信号 | **对齐** |
| 后端静默降级 | 上游自身 8 类静默降级（Chroma `contains`→相等、Qdrant `*`→丢弃、Weaviate 除三 id 外全丢、Redis 除相等全丢…） | ✅ 不复刻。结构性无法表达的运算**报错**：PG 走 `jsonb_exists(...)` 而非裸 `?` 运算符（sqlx 会把 `?` 当占位符吞掉）；SQLite 的非 ASCII `icontains` 返回 `DialectCannotExpressOperator`；`search-first-vector` 因投影不含元数据文档而**拒绝**带 filter 的请求 | **不适用**（照 spec 走，不复刻降级） |

**8 项已书面登记的主动偏离**（逐条写在 `filter.rs` 模块文档内；取向一致：**宁可报错，不可悄悄少做**）：

1. 未知 op 报错（与上游一致，作为其余各条的原则基线）；
2. 空 `AND`/`OR`/`NOT` 列表报错（上游视为 no-op，**静默放宽**结果集）；
3. 空条件分组报错（上游跳过该组，若各分组皆空则整条子句被丢弃）；
4. `null` 字面量报错（上游接受并与字符串 `"None"` 比较 —— 同样永不匹配，但只有一种会说出来）；
5. `eq`/`ne`/`contains`/`icontains` 拒绝列表字面量（上游把列表字符串化成单个操作数）；
6. 排序运算符要求存储侧为 JSON **数字**（上游允许 PG 把 `"3.5"` cast 成 numeric，使 PG 与 SQLite 语义分叉）；
7. 存储侧非数字值**排除该行**而非让查询失败（上游 `::numeric` 会因一行坏数据中止整个请求）；
8. `icontains` 是 Unicode 大小写折叠；SQLite 的 `lower()` 只折叠 ASCII，故含非 ASCII 的 needle 直接报错。


---

## 5. 实体 / 关系 / 图

| 维度 | mem0 | 本仓 | 判定 |
| --- | --- | --- | --- |
| 实体存储 | 独立向量 collection `{collection}_entities`；payload = `data` + `entity_type` + `linked_memory_ids` | `ai_entity` 表：`entity_type` / `canonical_name` / `aliases_json` / `attributes_json` / `sensitivity_level` / `status` / `version`（`database/ddl/baseline/postgres/0001_memory_baseline.sql:576`） | **超越** |
| 关系模型 | **无**（`linked_memory_ids` 是唯一关联表达） | `ai_edge` 表：`source_entity_id` / `target_entity_id` / `relation_type` / `weight` / `valid_from` / `valid_to`（`*.sql:602`） | **超越** |
| 时间有效性 | 无 | `valid_from` / `valid_to` + `idx_ai_edge_validity` | **超越** |
| 重建作业 | 无 | `ai_relation_rebuild_job`（`*.sql:815`） | **超越** |
| **存储层实现** | 内联在 `Memory` 类 | **存在**：`plugins/sdkwork-memory-plugin-native-sql/src/graph_store.rs`（742 行）——`NativeSqlEntityRow` / `NativeSqlEdgeRow` / `InsertEntityCommand` / `UpdateEntityCommand` / `InsertEdgeCommand` / `UpdateEdgeCommand`，以及 `impl NativeSqlMemoryStore` 的 17 个方法（`insert_entity_with_journal` / `update_entity_with_journal` / `insert_edge_with_journal` / `update_edge_with_journal` / `delete_edge_with_journal` / `resolve_entity_internal_id(_in_space)` / `insert_entity` / `retrieve_entity` / `list_entities` / `update_entity` / `insert_edge` / `retrieve_edge` / `list_edges` / `update_edge` / `delete_edge` / `count_entities_for_tenant` / `count_edges_for_tenant`）；由该 crate `lib.rs` 的 `pub use graph_store::*` 导出 | **对齐**（2026-09-23 更正） |
| **服务层 CRUD** | 内联在 `Memory` 类 | **存在**：`crates/sdkwork-intelligence-memory-service/src/commercial_api.rs` 消费上述方法——建实体（`.insert_entity_with_journal(`）、列实体、建边（`.insert_edge_with_journal(`）、列边，另有 `map_entity_row_to_dto` / `map_edge_row_to_dto` | **对齐**（2026-09-23 更正） |
| **实体加权算术** | `_compute_entity_boosts` 注入打分 | **存在**：`crates/sdkwork-memory-retrieval/src/entities.rs` 的 `entity_boosts`（逐行移植 `main.py:1791-1808`）、`memory_count_weight`、分层抽取（`EntityKind` / `EntitySpan` / `ExtractedEntity`） | **对齐**（2026-09-23 更正） |
| **加权喂给打分** | 组装后注入 `score_and_rank` | **断链**：`HybridSignals.entity_boosts` 与 `score_and_rank` 都在，但 `HybridSignals` 在**全仓唯一的生产构造点是零** —— 仅 `scoring.rs` 自己的 `#[cfg(test)] mod tests` 与文档示例构造它 ⇒ 实体权重恒为空 map，算术写了却从不生效 | **缺失（已实现未接线）** |
| **查询侧实体选择** | `select_query_entities` 在检索**之前**施加 `[:8]`（`main.py:1747`） | 函数存在（`entities.rs:509`），但**无生产调用点**（`extract_entities` / `extract_entity_texts` / `select_query_entities` 仅在 `retrieval/src/lib.rs` 被再导出） | **缺失（已实现未接线）** |
| **记忆 → 实体归属** | `linked_memory_ids` 写在实体 payload 上（且上游自己丢弃，§0.1.1） | **列在、命令不暴露**：`ai_edge.source_memory_id BIGINT REFERENCES ai_record(id)` 存在于 DDL（`*.sql:611`），但 `InsertEdgeCommand` 无该字段，两处 `INSERT INTO ai_edge`（`graph_store.rs:181`、`:466`）均不绑定它，该列仅被 `store.rs:1638/1650` 在**删除记忆时置 NULL` ⇒ **从未被写入** | **缺失** |
| SPI 端口 | n/a（内部直接调向量库） | `crates/sdkwork-memory-spi/src/` 内无任何实体/边端口（`grep -i "port\|trait"` 零命中） | **缺失** |
| HTTP 端点 | `client` 层有实体接口 | **三个面全部已实现**：`crates/sdkwork-routes-memory-{open,app,backend}-api/src/commercial_routes.rs` 各自注册 `paths::ENTITIES` / `paths::ENTITY` / `paths::EDGES` —— `get(list_entities).post(create_entity)`、`get(retrieve_entity).patch(update_entity)`、`get(list_edges).post(create_edge)`，处理器实调服务的同名方法；`apis/open-api/memory-open-api.openapi.json` 声明了这些路径；后端面还有真实集成测试 `backend_commercial_management_flow.rs` 对 `/backend/v3/api/memory/entities` 跑过 POST+GET | **对齐**（2026-09-23 更正） |

> **本仓模式领先于代码，但「领先」的程度曾被两次高估。** 2026-09-23 的两轮复核推翻了
> 本文档此前的两条判断：①「服务/插件实现：未见实现」为错 —— 存储层（`graph_store.rs`，742 行）、
> 服务层 CRUD（`commercial_api.rs`）**都已落地且可编译**；②「无 entity/edge HTTP 端点」亦为错 ——
> **三个 API 面全部实现了端点**（见上表末行）。
>
> 真实缺口是**三处断点**：
> ① `ai_edge.source_memory_id` 从未被写入 ⇒「这个实体出现在哪些记忆里」在库中不可答；
> ② `select_query_entities` / `extract_entities` 无生产调用点；
> ③ `HybridSignals` 无生产构造点 ⇒ 加权算术恒空。
> 三者串联起来才是上游的 `_compute_entity_boosts`：**先查查询实体 → 匹配候选记忆 → 按命中数加权**。
> 本仓已有第 3 段的算术，缺的是第 1、2 段的数据通路与第 0 段的归属列填充。
>
> ⚠️ **但第 ③ 条已被批次 6 的发现吸收并放大**：`HybridSignals` 之所以没有生产构造点，是因为
> 承载它的整个 `scoring` 模块**从部署入口不可达**（§3 的可达性更正）。也就是说，即便补齐
> ①②，加权也无处可去 —— 除非先把可达栈接起来。**实体接线的真正前置条件是 §10 批次 8。**

---

## 6. 写入路径的记忆语义

| 能力 | mem0 | 本仓 | 判定 |
| --- | --- | --- | --- |
| 自动写入语义 | **ADD-only**（§0.1） | 插件 `extraction` + `additive`，无 UPDATE/DELETE/NOOP | **对齐**（批次 2） |
| 记忆链接 | 提示词要求回传，但**上游自己丢弃**（§0.1.1） | `resolve_linked_memory_ids` 保留并逐条校验，未证实的记入 `unverified_links` | **超越** |
| 归属标记 | `attributed_to ∈ {user, assistant}`；缺失可容忍，写入 payload 时判真（`main.py:1036`） | `AttributedTo` 闭集 + `ExtractedMemory.attributed_to` | **对齐**（批次 2） |
| 消息角色 | `role` 写入 payload（`infer=False` 路径） | `ConversationTurn.role` 参与提示词；落库属 canonical payload | **部分**（落库在组合根） |
| 内容哈希去重 | `hash = md5(text)`；**去重范围仅 top-10 候选 + 批内**（`main.py:1007-1024`） | `additive::content_hash`（sha256）+ 调用方给出的全部既有记忆 + 批内 | **超越**（sha256，且去重范围不受 top-k 限制） |
| 词形还原字段 | `text_lemmatized` | 本插件不落该字段 | **归属他处**（见下） |
| 时间戳 | `created_at` / `updated_at`（更新时 `created_at` 继承） | 有 | **对齐** |
| 过期 | `expiration_date`（`YYYY-MM-DD`）+ `_payload_is_expired`；`show_expired` 默认 `False` | 投影 `expiration_date` + 写入期校验 + `rank` 内过期门控 + `config.include_expired` | **对齐**（请求级开关见批次 6） |
| procedural memory | 独立提示词 + `memory_type` 落库 | `procedural.rs`：`resolve_write_path` 路由（只认 `procedural_memory`，其余报错）+ 专用提示词 + `plan_procedural_memory`（空答复、缺 metadata 两处拒写）+ `runtime.plan_procedural_memory` | **对齐**（批次 5） |
| 自定义抽取指令 | `MemoryConfig.custom_instructions` / `add(prompt=...)` | `AdditiveExtractionRequest.custom_instructions` → 提示词 `## Custom Instructions` 段 | **对齐**（批次 2） |

> **`text_lemmatized` 的归属裁决。** 上游把它写进**向量库 payload**，因为上游的向量库自己也要做关键词检索。
> 本仓的关键词（BM25）打分活在服务层 `sdkwork-memory-retrieval`（批次 1），而本插件的
> `specs/component.spec.json` 只声明 `requiredPorts: [sdkwork-memory-spi]`。
> 因此该字段属**服务层/canonical payload**，而不是本插件的派生投影：
> 若强行加进 `VectorRecordProjection`，它在插件内没有消费者，就是第二个「算了却从不读的哈希」。
> **判定由「缺失」改为「归属他处」**，批次 4 接线实体 SPI 时一并在服务层落地。

---

## 7. 提示词资产

| mem0 提示词 | 状态 | 本仓对应 | 判定 |
| --- | --- | --- | --- |
| `ADDITIVE_EXTRACTION_PROMPT`（`prompts.py:468-944`，含 12 个 few-shot） | **live** | 插件 `extraction::ADDITIVE_EXTRACTION_PROMPT` | **部分**（契约段已逐段移植；12 个 few-shot **有意不内联**，理由见下） |
| `generate_additive_extraction_prompt`（`prompts.py:1016-1062`） | live | `build_additive_extraction_prompt` + `AdditiveExtractionRequest` | **对齐**（Summary / Last k / Recently Extracted / Existing / New / Observation Date / Current Date / Custom Instructions 八段齐备） |
| `AGENT_CONTEXT_SUFFIX`（`prompts.py:947-957`） | live | `procedural::AGENT_CONTEXT_SUFFIX`；由 `build_additive_extraction_prompt` 在 `is_agent_scoped(agent_id, user_id)` 成立时**追加在契约提示词之后、输入段之前** | **对齐**（批次 5；**按义重述**，见下） |
| `PROCEDURAL_MEMORY_SYSTEM_PROMPT`（`prompts.py:326-403`） | live | `procedural::PROCEDURAL_MEMORY_SYSTEM_PROMPT`，由 `build_procedural_prompt` 使用；可用 `system_prompt_override` 整段替换 | **对齐**（批次 5；**按义重述**，见下） |
| `DEFAULT_UPDATE_MEMORY_PROMPT` | **死代码** | 原 `FACT_ARBITRATION_PROMPT` 已删除 | **已消除**（批次 2） |
| `MEMORY_ANSWER_PROMPT` | live（仅 proxy） | 无（proxy 不在本仓范围） | **不适用** |
| `FACT_RETRIEVAL_PROMPT` / `USER_/AGENT_MEMORY_EXTRACTION_PROMPT` | **死代码** | — | **不适用** |

> **为什么 12 个 few-shot 不内联。** 实测 `sed -n '468,944p' prompts.py | wc -c` = **33 875 字节**，
> 其中绝大部分是 few-shot 示例。它们不是接口契约而是调参产物：解析器只依赖 `## OUTPUT FORMAT` 段，
> 而那一段已逐字移植。把 33 KB 第三方文本（mem0 为 Apache-2.0）整段复制进本仓会带来两个问题：
> ① 在 AGPL-3.0 源文件里形成一份需要单独署名（NOTICE）的逐字副本；
> ② 参考实现在 `external/` 下已被 gitignore，这份副本会成为仓内唯一拷贝并**静默漂移**。
> 质量取向的行为规则（双角色抽取、不抽什么、上下文丰富而非原子、相对时间锚定、附带事实）
> 已按语义移植。若某部署需要更强的示例引导，走 `custom_instructions` 注入即可。

> **「按义重述」的适用边界（批次 5）。** 上面两项在批次 5 落地时**没有逐字复制**，理由与
> few-shot 同一节：一是许可证（mem0 Apache-2.0，本仓 AGPL-3.0，逐字副本需单独署名），
> 二是 `external/` 已被 gitignore ⇒ 逐字副本会成为仓内唯一拷贝并静默漂移。
> 但两者的**风险等级不同，必须区别对待**：
> - `AGENT_CONTEXT_SUFFIX` 是**接口契约**：它的三条框定规则与「`attributed_to` 仍记原始来源」
>   这句话共同决定 `attributed_to` 取值，而 `attributed_to` 是**被解析器读取的结构化字段**。
>   故其**规范性内容逐句保留**，只有措辞重述。
> - `PROCEDURAL_MEMORY_SYSTEM_PROMPT` 是**质量引导**：procedural 路径的产物是一段自由文本，
>   上游**不对它做任何结构解析**（`main.py:2021` 之后只做非空检查与 `remove_code_blocks`）。
>   故其结构性义务（动作结果逐字记录、不省略步骤、不掩盖失败）保留，具体措辞可重述。
>
> 判定依据不是「我们没抄全」，而是**该资产是否被解析器读取**。凡被解析器读取的提示词段
> （如 `ADDITIVE_EXTRACTION_PROMPT` 的 `## OUTPUT FORMAT`）一律逐字移植，不受本条豁免。

---

## 8. Provider 与集成面

| 维度 | mem0 | 本仓 | 判定 |
| --- | --- | --- | --- |
| LLM provider | 18 个（openai/anthropic/gemini/ollama/…） | `LanguageModelPort`（`spi/ports.rs:1177`）—— **端口而非内置 provider** | **不适用**（本仓按 spec 走端口 + 绑定表 `ai_provider_binding`） |
| Embedding provider | 11 个 | `EmbeddingModelPort`（`spi/ports.rs:1184`） | **不适用** |
| Reranker provider | 5 个 | `RerankModelPort`（`spi/ports.rs:1193`） | **不适用** |
| 向量库 | 28 个适配 | 无（本仓 `ai_*` 表 + 派生索引可重建） | **不适用**（架构选择不同，非缺陷） |
| 凭据管理 | 从环境变量读 API key | `secretRefs` 只允许引用；插件 `secretRefs: []` | **超越** |
| 遥测 | `capture_event` 上报能力维度，含**未哈希的 `memory_id`** | 无内置遥测 | **不适用**（且上游该点有隐私问题） |

---

## 9. 汇总

> **批次 5 后口径。** 批次 1 把 §3 的 9 项从「缺失/部分」推到「对齐」；
> 批次 2 把 §6 的写入路径 6 项推到位（4 项对齐、2 项超越），并消除 §7 的 1 项偏离；
> 批次 3 把 §4 的 filter 表达式语言从「无」推到「对齐」（运算符全集 + 逻辑组合 + 三值求值语义 +
> PG/SQLite 双方言 SQL 下推），并修掉 API 边界「`filters` 被接收却从不生效」的静默降级缺陷；
> 批次 5 把 §6 的 procedural memory 与 §7 的两项提示词资产推到「对齐」；
> 批次 6（可达性审计）**不改能力判定，改效力判定** —— 它更正了 §5 的端点误报，
> 并发现 §3 的整套 mem0 对齐打分栈从部署入口不可达。
> 累计：缺失 18→**6**、部分 10→**7**、偏离 2→0、对齐 5→**24**；另立**可达性阻断 2 项**、
> **归属他处 1 项**。
>
> 计数口径（**机械可核，不再手写**）：
> - 「缺失」「部分」= **§1 / §3 / §5 / §6 / §7 表中该判定的行数**。
>   复核命令（读到 §9 即停，避免命中 §15/§16 正文里的同形文字）：
>   ```sh
>   F=docs/engineering/reviews/ALIGN-20260923-mem0-capability-parity-matrix.md
>   awk '/^## 9\. /{exit} /^\| .*\| \*\*缺失/{n++} END{print n}' "$F"   # 实测 6
>   awk '/^## 9\. /{exit} /^\| .*\| \*\*部分/{n++} END{print n}' "$F"   # 实测 7
>   ```
> - 「对齐」「超越」为累计口径，不逐条复述。
> - **可达性阻断独立计数**，不并入「缺失」：它不是在描述「能力没实现」，而是在描述
>   「实现已存在但运行时到不了」。混在一起会让「缺失」失真。

**§9 上一版的手写清单与表内条目已脱节，本轮改为机械推导。** 上一版列了 9 项「缺失」，
其中 3 项在表里根本不是缺失（后端端点已实现、`text_lemmatized` 为「归属他处」、`role` 与
12 个 few-shot 为「部分」）。下表为**按表枚举**的 6 项缺失与 7 项部分，逐条可 grep 复核：

| # | 缺失条目 | 出处 |
| --- | --- | --- |
| 1 | 实体 SPI 端口 | §5 表 |
| 2 | 记忆 → 实体归属写入（`ai_edge.source_memory_id` 列在而命令不暴露） | §5 表 |
| 3 | 查询侧实体选择接线（`select_query_entities` 无生产调用点） | §5 表 |
| 4 | 实体加权喂给打分（`HybridSignals` 无生产构造点） | §5 表 |
| 5 | `delete_all` 等价批量删 | §1 表 |
| 6 | **检索路径**侧过期过滤（`expiration_date` + `show_expired`） | §3 表 |

> 🔎 **第 6 项的取证更正（§21）**：本行原写作「`expiration_date`」，但本仓 DDL 里**没有**这个列名 ——
> 真实列是 **`ai_record.expires_at TEXT`**（PG 基线 `:89`，SQLite fixture `:85`），
> 且配有一条**已契约化**的 `idx_ai_record_validity (tenant_id, valid_from, valid_to, expires_at)`
> （`tools/materialize_phase1_contracts.mjs:1248`）。同时 `expiresAt` **已被三面 OpenAPI 声明**
> （`MemoryRecord` / `MemoryRecordRequest`），但 Rust DTO 没有它 ⇒ **契约承诺的入参被 serde 静默丢弃**。
> ⇒ 该项**不需要新 DDL**，性质是「实现追上已有契约」。详情与推进顺序见 **§21**。

| # | 部分条目 | 出处 |
| --- | --- | --- |
| 1 | `add` 参数面（缺 `infer` / `expiration_date` / `memory_type` / `prompt` 覆盖） | §1 表 |
| 2 | `search` 参数面（缺 `threshold` / `explain` / `show_expired` / `reference_date`） | §1 表 |
| 3 | `get_all` over-fetch 语义 | §1 表 |
| 4 | `update` 实体重链接 | §1 表 |
| 5 | 时间锚点（无「观察日期」锚定） | §3 表 |
| 6 | 消息角色 `role` 落库约定（落库在组合根） | §6 表 |
| 7 | `ADDITIVE_EXTRACTION_PROMPT` 的 12 个 few-shot 示例未内联 | §7 表 |

> 另有一项判定为 **「归属他处」**（不计入上表）：`text_lemmatized` 属服务层 canonical payload，
> 不属本插件的派生投影（裁决见 §6 脚注）。

| 判定 | 数量 | 主要条目 |
| --- | --- | --- |
| **缺失（需补齐）** | **6** | 见上表（§5 四项 + §1 一项 + §3 一项） |
| **部分（需增强）** | **7** | 见上表（§1 四项 + §3 一项 + §6 一项 + §7 一项） |
| **偏离（需转向）** | **0** | — （批次 1 清掉 9 项检索偏离，批次 2 清掉 2 项写入偏离） |
| **对齐** | **24** | `get`、`delete`、异步、时间戳、`MemoryRetrieverPort` 边界契约、**BM25 打分、自适应 sigmoid、词形还原、实体抽取（分层：2 类精确 + 2 类近似）、实体增强算术、加法归一融合、阈值 gate、`explain` 分解、over-fetch**、**ADD-only 写入语义、`attributed_to` 闭集、`expiration_date` 与过期门控、`custom_instructions`、提示词输入段结构化**、**filter 表达式语言与三值求值语义、API 边界 filter 解析与双方言 SQL 下推**、**procedural memory 写入路径、`AGENT_CONTEXT_SUFFIX`、实体存储层 / 服务层 CRUD / 三面 HTTP 端点** |
| **🔴 可达性阻断（独立计数）** | **3** | ① `sdkwork-memory-plugin-search-first-vector` 整 crate 不在部署入口的依赖闭包内（§13）；② `sdkwork-memory-retrieval` 的 `scoring` / `bm25` / `entities` / `lemmatization` 四个模块零生产调用点（§3 可达性更正、§16.2）；③ **可达系统没有向量/语义检索通道** —— 能力声明 `embedding: false`、DDL 无向量列、可达信号集无向量、`SemanticCandidate` 无人生产（§16.7）。**三者同源：③ 是 ① 与 ② 的共同原因**（插件提供的正是 `retrieverKinds: ["vector"]`）。「对齐」列中标注 ✅ 的检索能力，其效力以本条为准 |

| **超越（本仓更强，保持）** | **15** | 租户/空间双层作用域、`actor_id`、读作用域、journal 事件链、多路召回 RRF、rerank 不丢候选、sha256 去重、`ai_edge` 关系与时间有效性、关系重建作业、凭据引用、provider 端口化、结构性空作用域守卫、无全库 reset、能力缺口报错而非静默降级、**链接校验而非丢弃** |
| **不适用** | 8 | provider 内置实现、向量库适配、图记忆（上游无）、`reset`、`timestamp`/`reference_date`、proxy、遥测、上游静默降级矩阵 |

---

## 10. 执行顺序（按价值/风险排序）

| # | 批次 | 内容 | 风险 | 状态 |
| --- | --- | --- | --- | --- |
| 1 | **检索质量对齐** | BM25 原生打分 + 自适应 sigmoid 归一 + 词形还原 + 实体抽取 + 实体增强 + 加法归一融合 + 阈值 gate + `explain` 分解 | 低（纯新增，不改既有 RRF 路径） | **已完成**（§11 证据：137 测试绿 / clippy 0 警告 / 变异 7-7 被抓） |
| 2 | **写入语义转向** | 插件由四操作仲裁转向 ADD-only + `linked_memory_ids`；补 `attributed_to` / `expiration_date` / `custom_instructions` | 中（改动插件全部既有测试） | **已完成**（§12 证据：131 测试绿 / clippy 0 警告 / 变异 10-10 被抓） |
| 3 | **filter 表达式语言** | 运算符全集 + 逻辑组合 + 三值求值语义（共享层）；API 边界解析 + PG/SQLite 双方言 SQL 下推，禁止静默降级 | 中 | **已完成**（§14 证据：四 crate 433 测试绿 / clippy 0 警告 / 变异 17-17 被抓 / fmt 干净） |
| 4 | **实体链路接线** | ① 在 `InsertEdgeCommand` 上暴露 `source_memory_id` 并写入（**列已存在于 DDL，无需迁移**）；② 新增实体 SPI 端口 + 服务实现；③ 把 `select_query_entities` → 候选实体匹配 → `entity_boosts` 接进检索，使 `HybridSignals` 有生产构造点；④ 暴露 entity/edge HTTP 端点；同批落地 `text_lemmatized` 与 `role` 落库 | 中高（跨 SPI + 服务 + API），**无 schema 迁移** | **部分可做**：**①②④ provider-free，可立即做**；**③ 不可**——`SemanticCandidate.semantic_score` 是必需 `f64` 且**先过门**，`HybridSignals` 在门之后才参与，故无真实语义分则实体加权无处可去（§16.5 原判正确；§20.4 已自更正初版的错分类）。阻断项见 §15.4 / §15.5 |
| 5 | **procedural memory** | `memory_type` 路径 + 专用提示词 + `AGENT_CONTEXT_SUFFIX` | 低 | **已完成**（§15 证据：159 测试绿 / clippy 0 警告 / 变异 7-7 被抓 / fmt 干净）· ⚠️ 已实现但**不可激活**（需 `LanguageModelPort`，仓内无生产实现，§20.1） |
| 6 | **API 面补齐** | `threshold` / `expire` / `explain` 透出到 HTTP 契约 | 中 | **部分可做**：**`show_expired` provider-free，可立即做**；**`threshold` / `explain` 不可**——`threshold` 只门语义分（§16.8），无向量分则恒为空结果（§20.4） |
| **7** | **插件可达性接线** | 把 `SearchFirstVectorRuntime` 接进 `sdkwork-intelligence-memory-service` 的组合根（比照 `reference-profiles` 的 `primary_plugin_id` 与 provider 绑定方式） | **高**（触及服务组合根、provider 绑定、部署资格） | **被 provider 阻断**（§13 + §20）：接线本身可写，但**接了也不可激活**——无 `EmbeddingModelPort` 实现，且 `retriever` 端口是**单槽**（§20.6），向量插件只能"替换"而非"并列" |
| **8** | **向量/语义通道打通**（🔑 **其余一切的解锁项**） | **8a 接收端（✅ 已完成，§17）**：可达编排器接收「每记忆一条相似度」，`"vector"` profile 键默认 0.0、默认行为字节不变，8/8 变异捕获。**8b 供应端**：① 服务层绑 `EmbeddingModelPort` 并算相似度 —— 需放开 `SUPPORTED_RETRIEVERS`（**公开词表**，且须反转既有拒绝测试）；② 数据面按 `retriever_kinds: ["vector"]` 返回向量候选 —— 需 §13 的插件路径 | **中高**（8b 触及公开词表 + **一处 provider 绑定，属配置面**） | **被 provider 阻断**（§20，本节为**更正后的评级**）：8a 已落地但**运行时产出为 0**，不计为能力完成；8b 的**两套备选（落列 / 走插件）在仓内都不可激活**，因为根因是**仓内与整个工作区都没有 provider 适配器**（§20.1），与 §19 的阶段门禁无关。🔴 **不要再在「存储归属」上做决策**——A/B 的分歧位于激活之后的下一层 |
| **9** | **mem0 打分栈可达性接线** | 语义源就位后，给 `MemoryRetrievalStrategy` 增加一个指向 `score_and_rank` 的档位（或让可达栈直接采用加法归一），使 `HybridSignals` 有生产构造点 | 中（**默认策略不变**） | **被 provider 阻断**（§16.8 原判 + §20.4 更正）：`SemanticCandidate.semantic_score` 是**必需 `f64`** 且 `score_and_rank` **先过门**（`scoring.rs:236`），故「先接打分栈、等语义源」**不通**——只能靠伪造语义分或篡改 threshold，两者都是禁止行为 |

> 🔴 **§20 的核心更正（读 §10 前必读）**：上表原先把批次 8 当作「其余一切的解锁项」。
> 这个读法**过于乐观**：解锁批次 8 的**不是接线，而是 provider 适配器**。
> 依赖链的**根在 provider，不在批次 8**。凡标注「被 provider 阻断」的行，
> 继续加代码只会扩大「已实现未接线」的面积（`QUALITY_GATE_SPEC §31`：不可达的能力贡献为 0）。
> 真正**在本仓可产出价值**的剩余项只有 §20.4 更正表上半区的 5 项。

---

## 11. 批次 1 验证证据

| 项 | 结果 |
| --- | --- |
| `cargo test -p sdkwork-memory-retrieval` | **137 passed / 0 failed**（lib 97 + 集成 3 + 13 + doctest 24），exit 0 —— §17 后为 **148**（lib 98 + 集成 3 + 13 + 10 + doctest 24） |
| `cargo clippy -p sdkwork-memory-retrieval --all-targets` | **0 warning**（`grep -cE "^warning: [a-z]"` = 0） |
| 变异对照 | `.workbuddy/tools/mutate-retrieval-alignment.py`：**7/7 变异全部被抓红**，每个文件按字节还原（SHA256 一致），对照组保持绿 |
| 变异覆盖的断言 | 开场动词守卫、阈值先 gate 语义分、BM25 短查询 sigmoid 档位、候选 2 字符下界、末次按位置重排、去重平局取先者、TOPIC/PROPER 不重叠不变式 |
| 行尾 | `crates/sdkwork-memory-retrieval/src/**` 全部纯 LF（字节级核对 `crlf=0 / bare_cr=0`） |

**本批次自查出的两处自身缺陷（已修）**：

1. 变异脚本首版用 `Path.write_text()`，在 Windows 上把 `\n` 转成 `\r\n`，把 `bm25.rs` /
   `entities.rs` / `scoring.rs` 三个 LF 文件悄悄改成了 CRLF。修法：脚本改为**二进制读写**；
   三个文件已规范化回 LF 并用字节级核对确认。
   （附带教训：`grep -c $'\r'` 的计数在本环境**不可信** —— 它曾让我误判 `lib.rs` 也是 CRLF，
   而字节测量显示 `lib.rs` 是纯 LF。判行尾必须做字节计数。）
2. `entities.rs` 首版的 `entity_boosts(matches, max_entities)` 把 `[:8]` 上限错误地施加在
   **匹配结果**上，而上游施加在**查询实体**上（`main.py:1747` 在检索之前）。已拆成
   `select_query_entities`（查询侧）与无上限的 `entity_boosts`（匹配侧）。

---

## 12. 批次 2 验证证据（写入语义转向）

### 12.1 落地改动

| 文件 | 动作 | 说明 |
| --- | --- | --- |
| `src/arbitration.rs` | **删除** | 四操作仲裁（549 行）整体退役 |
| `src/fact_extraction.rs` | **删除** | `facts` 信封 + 槽位式 fact（350 行）整体退役 |
| `src/extraction.rs` | **新增** | `ADDITIVE_EXTRACTION_PROMPT`（按契约移植）、`AttributedTo` 闭集、`ConversationTurn`、`ExistingMemoryView`、`AdditiveExtractionRequest`（8 段）、`ExtractedMemory`、`ExtractionReport`、`EmissionRejection` / `RefusedEmission`、`parse_additive_response`、`extract_memories`、`build_additive_extraction_prompt`、`today_utc_date` |
| `src/additive.rs` | **新增** | `content_hash`、`PlannedAddition`、`DuplicateReason` / `SuppressedDuplicate`、`UnverifiedLink`、`LinkResolution`、`resolve_linked_memory_ids`、`AdditivePlan`、`plan_additions` |
| `src/vector_index.rs` | 改 | `VectorProjectionCommand` / `VectorRecordProjection` 增加 `expiration_date`；新增 `normalize_expiration_date`（含闰年与月长的日历校验）、`VectorRecordProjection::is_expired_on`；`VectorRankFilters` 增加 `include_expired` 与 `as_of_date`；`upsert` 写入期校验；`rank` 内过期门控 |
| `src/config.rs` | 改 | 删除已无消费者的 `duplicate_similarity` / `related_similarity`（仲裁退役后它们不再影响任何行为）；新增 `include_expired` + `DEFAULT_INCLUDE_EXPIRED` |
| `src/runtime.rs` | 改 | `extract_facts`→`extract_memories`；`arbitrate_fact` / `arbitrate_fact_lexically` 退役；新增 `plan_additions`、`plan_write`、`AdditiveWritePlan`；`search_scoped_inner` 传 `include_expired` 与单次读取的 `as_of_date` |
| `src/error.rs` | 改 | `FactExtractionUnparseable`→`MemoryExtractionUnparseable`；`ArbitrationRejected` 删除；新增 `ExpirationDateInvalid`（归 `INDEX_PORT`） |
| `src/lib.rs` | 改 | 模块表与写路径说明重写 |
| `tests/fact_extraction_contract.rs` | 删除 | 由 `tests/additive_extraction_contract.rs`（15 项）替代 |
| `README.md` | 改 | 补「写入路径是加法式的」章节、配置表、不变量目录 8-13 条 |

### 12.2 验收命令与结果

| 命令 | 结果 |
| --- | --- |
| `cargo test -p sdkwork-memory-plugin-search-first-vector` | **131 passed / 0 failed**（lib 67 + additive_extraction_contract 15 + manifest_matches_json 8 + retrieval_scope_contract 25 + doctest 16） |
| `cargo clippy -p … --all-targets` | **0 warning** |
| `cargo fmt -p … -- --check` | 干净 |
| `cargo check --workspace --all-targets` | **exit 0**，无 `error` 行 |
| `python .workbuddy/tools/mutate-additive-write-path.py` | **10/10 mutations caught**，每个文件还原后 SHA256 字节一致，对照跑绿 |

### 12.3 自我更正（本批次内推翻的两处判断）

1. **最初计划给 `VectorRecordProjection` 加 `text_lemmatized`。** 写码前核对分层：本插件
   `component.spec.json` 的 `requiredPorts` 只声明 `sdkwork-memory-spi`，而
   `lemmatize_for_bm25` 住在 `sdkwork-memory-retrieval`（被 `sdkwork-intelligence-memory-service`
   与 `-repository-sqlx` 消费的**服务层**库）。若为此加依赖即越层；若在本插件重写一份词形化
   即违反 `AGENTS.md`「不要本地重复共享工具逻辑」。**更正为：该字段归服务层/canonical payload**，
   §6 的判定由「缺失」改为「归属他处」，批次 4 落地。避免了造出「算了却从不读」的第二个字段。
2. **最初把「信封只接受 `{"memory":[...]}`」写成 `#[serde(default)]`。** 随即发现 serde 默认忽略
   未知键，`{"facts":[…]}` 会被解析成**空的成功**——正是本仓最反对的静默降级。
   **更正为：`memory` 键必需（不加 `default`）**，并加测试 `the_facts_envelope_is_refused`
   与 `a_bare_array_is_refused` 双向钉住；变异 M4 证明该断言确实会红。

### 12.4 本批次新登记的待办

- `AdditiveExtractionRequest` 目前由调用方构造；**上游 Phase 1 的「取最近 10 条消息」与
  「top-10 既有记忆」取数逻辑**属服务层（本插件不持有会话历史），批次 4/7 一并接线。
- `expiration_date` 的**请求级开关**（上游 `show_expired`）尚无请求字段可承载：
  `SearchMemoryCandidatesQuery` 需新增字段（SPI 契约变更），归批次 6。

---

## 13. 阻断项：本插件的记忆能力当前**不可达**（P0）

`QUALITY_GATE_SPEC` §30 的立场是：扫到 0 个单元的门禁比没有门禁更糟。同一条道理适用于能力——
一个写得再对的插件，若没有任何运行时路径能到达它，就对产品零贡献。

**硬证据（对照法）：**

```text
$ grep -rn "sdkwork-memory-plugin-search-first-vector" --include=Cargo.toml .
./Cargo.toml:22:  "plugins/sdkwork-memory-plugin-search-first-vector",     # 仅 workspace member
./Cargo.toml:79:sdkwork-memory-plugin-search-first-vector = { path = … }  # 仅 workspace.dependencies
# 无任何 crates/*/Cargo.toml 声明依赖它

$ grep -rn "sdkwork-memory-plugin-native-sql|sdkwork-memory-plugin-reference-profiles" --include=Cargo.toml .
crates/sdkwork-intelligence-memory-repository-sqlx/Cargo.toml:22: …native-sql.workspace = true
crates/sdkwork-intelligence-memory-service/Cargo.toml:20:        …native-sql.workspace = true
crates/sdkwork-intelligence-memory-service/Cargo.toml:32:        …reference-profiles.workspace = true
```

即：**另外两个插件都被服务层消费，本插件没有。** 全仓对它的引用只剩 `Cargo.lock`
与 `cargo test` 的测试产物。

**判定：这是批次 2 之后排序第一的阻断项**，因为批次 2/3/5 的全部成果都卡在它后面。接线方式有
两处需要人工决策，故列入 §10 批次 7 并在此显式挂起：

> **2026-09-23 两次复跑确认（批次 5 收尾时 + 批次 6 审计时）。** 上列证据仍逐字成立，且批次 6 用
> `cargo metadata` 的依赖闭包**独立复现**了同一结论：以部署入口
> `sdkwork-api-memory-standalone-gateway` + `sdkwork-api-memory-assembly` 为根，本插件是唯一
> 不在闭包内的业务 crate（详见 §16.2）。
>
> ⚠️ **本节此前把「批次 1 的成果也卡在这里」写进了同一句，批次 6 更正：批次 1 卡在**另一处**。**
> 批次 1 的落点是 `sdkwork-memory-retrieval` 的 `scoring` / `bm25` / `entities` / `lemmatization`
> 四个模块，它们与插件无关；它们不可达的原因是**可达检索路径不调用它们**（§3 可达性更正、§16）。
> 两处阻断因不同的接法而解锁，不可混为一谈：
>
> | 被阻断的成果 | 阻断原因 | 解锁批次 |
> | --- | --- | --- |
> | 批次 2 / 3 / 5（插件内的 mem0 写入与检索语义） | 插件不在部署依赖闭包内 | **批次 7** |
> | 批次 1（`sdkwork-memory-retrieval` 的加法归一对齐栈） | 可达栈 `retrieval`/`context_pack` 零调用它 | **批次 8** |

1. **组合根选择**：`sdkwork-intelligence-memory-service` 还是 `-repository-sqlx`，以及它是
   作为 `primary_plugin_id` 还是并列 retriever（比照 `runtime_profile_contract.rs:62` 的写法）。
2. **provider 绑定**：本插件 `deploymentQualification.state` 为 `provider-binding-required`，
   必须有一份已评审的 `EmbeddingModelPort` 绑定；`LanguageModelPort` 则是写路径必需。
   绑定落哪个部署 profile 属配置面改动，按既有约定**先问再改**。

---

## 14. 批次 3 验证证据（filter 表达式语言）

### 14.1 落地改动

| 文件 | 动作 | 说明 |
| --- | --- | --- |
| `crates/sdkwork-memory-spi/src/filter.rs` | **新增** | 语言本体：`LOGICAL_AND/OR/NOT`、`WILDCARD`、`INFIX_OPERATORS`（10）、三个类型化运算符枚举（`Scalar`/`Ordering`/`Set`）、`MetadataFilterScalar`、`MetadataFilterCondition`（4 变体，**把「运算符与取值形态不匹配」变为不可表达**）、`MetadataFilterExpression`（`Condition`/`All`/`Any`/`Not`）、`MetadataFilterError`（15 变体）、`parse_metadata_filter`、`matches`（Kleene 三值求值，兼作差分 oracle）。**不持有任何 SQL** |
| `crates/sdkwork-memory-spi/src/lib.rs` | 改 | 导出 `filter` 模块 |
| `crates/sdkwork-memory-spi/src/ports.rs` | 改 | `SearchMemoryCandidatesQuery` 新增 `metadata_filter: Option<MetadataFilterExpression>`；文档注释写明**必须在查询内应用**，缺口宁可报错也不得静默收窄 |
| `crates/sdkwork-memory-spi/tests/filter_contract.rs` | **新增** | 49 项契约测试：接受的 wire 形态、三值语义、每条拒绝路径、嵌套回归、结构不变式 |
| `plugins/sdkwork-memory-plugin-native-sql/src/filter_pushdown.rs` | **新增** | 方言翻译：`SqlPredicate`、`translate_metadata_filter`、`existence_predicate`、`metadata_text_expr`；PG 用 `jsonb_exists(...)`、SQLite 用 `json_type` 判型 CASE、`instr` 做 `contains` |
| `plugins/sdkwork-memory-plugin-native-sql/src/store.rs` | 改 | 新增 `MetadataFilterUnsupported` 错误变体；把 `metadata_filter` 贯穿三个检索入口，谓词与绑定在**同一处**追加 |
| `plugins/sdkwork-memory-plugin-native-sql/src/search_index.rs` | 改 | FTS 路径新增 `metadata_filter`；谓词**翻译一次**、同时用于 PG 与 SQLite 两个分支 |
| `plugins/sdkwork-memory-plugin-native-sql/tests/metadata_filter_pushdown_contract.rs` | **新增** | **差分门禁**：9 夹具 × 30 过滤器，把真实 SQLite 下推结果与 SPI oracle 逐条对拍；覆盖 FTS 与 LIKE 回退两条路径，另含收窄语义与拒绝用例 |
| `crates/sdkwork-intelligence-memory-service/src/open_api.rs` | 改 | **缺陷修复**：触达任何存储之前解析 `filters`，解析失败即 `Validation` 失败；把结果注入 `SearchMemoryCandidatesQuery` |
| `plugins/sdkwork-memory-plugin-search-first-vector/src/{error,runtime}.rs` | 改 | 新增 `MetadataFilterUnsupported`（归 `RETRIEVER_PORT`）；该插件投影不含元数据文档，故在**任何 provider 调用之前**拒绝带 filter 的请求 |
| `crates/sdkwork-intelligence-memory-service/tests/retrieval_workflow_contract.rs` | 改 | 新增 2 项服务级回归：过滤器生效（带控制组）与不可解析过滤器 fail-closed |

### 14.2 验收命令与结果

| 命令 | 结果 |
| --- | --- |
| `cargo test -p sdkwork-memory-spi` | **68 passed / 0 failed**（`filter_contract` 49 + manifest 7 + ports 5 + registry 4 + composition 3），exit 0 |
| `cargo test -p sdkwork-memory-plugin-native-sql` | **129 passed / 0 failed**（含 `metadata_filter_pushdown_contract` 4 项差分；`sqlite_store_contract` 72 项） |
| `cargo test -p sdkwork-intelligence-memory-service` | **100 passed / 0 failed**（含新增的 2 项服务级 filter 回归） |
| `cargo test -p sdkwork-memory-plugin-search-first-vector` | **116 passed / 0 failed**（含新增拒绝用例） |
| 四 crate 合计 | **433 passed / 0 failed**，另 `Doc-tests` 20 项全绿 |
| `cargo check --workspace --all-targets` | **exit 0** |
| `cargo clippy -p … --all-targets`（spi / native-sql / service / search-first-vector） | **0 warning** |
| `cargo fmt -p … -- --check`（native-sql / search-first-vector） | 干净 |
| 变异对照 | 三支脚本共 **17/17 被抓**：`mutate-metadata-filter-pushdown.py` 12/12、`mutate-search-first-vector-filter-refusal.py` 3/3、`mutate-metadata-filter-api-boundary.py` 2/2；每次按字节还原并以 SHA256 校验一致 |

**变异控制的可信度前提**：脚本只在「被点名的那条测试自己报 FAILED」时计为抓获。
首版判据写成 `"test {name} ... FAILED"`，而单元测试的实际名字带模块前缀
（`error::tests::…`），导致一条**确实被抓获**的变异被误记为 SKIP。判据已改为按后缀匹配，
该轮结论也随之更正为 3/3。

### 14.3 本批次自查出的自身缺陷（已修）

1. **解析器把嵌套逻辑键当成字段名。** 差分门禁首跑即抓到：`{"OR":[{"AND":[…]}]}` 被判为对名为
   `AND` 的元数据字段做比较（`InvalidValueShape { field: "AND", … }`）。根因是逻辑键只在顶层识别。
   修法：抽出递归的 `parse_object_conditions`，逻辑键在**任意深度**识别；补 4 项嵌套回归测试。
2. **`grep -c $'\r$'` 在本环境会匹配每一行**，不能用来判行尾 —— 它一度让我以为刚编辑过的文件全变成 CRLF。
   改用 Python **字节计数**后确认全部为纯 LF（`crlf=0 / bare_cr=0`）。**判行尾只认字节计数。**
3. **native-sql 引入了 `clippy::useless_format`**（`let path = format!("('$.' || ?)")`）。批次 3 当时
   只对 spi 与 search-first-vector 跑了 clippy，未覆盖 native-sql，故该 lint 漏到本轮全量 clippy 才被抓到。
   已改为字面量 `&str`，并**重跑差分门禁**证明 SQL 语义未变。
4. **native-sql 从未跑过 fmt**，留有 3 处漂移（本批次新增的 2 个文件 + 1 个被改文件的既有 hunk）。已 `cargo fmt` 收敛。

### 14.4 残余缺口（登记，未关闭）

- **`ai_record.metadata_json` 目前没有写路径。** 该列在 baseline DDL 中存在，但没有任何 SPI 命令
  （`CreateCanonicalMemoryCommand` / `UpdateCanonicalMemoryCommand`）承载 metadata，也没有 `INSERT`/`UPDATE` 写过它。
  后果：filter 的**语义**与**下推**都已验证（差分对拍 + 服务级控制组），但**尚无法通过公开 API 用真实存储的元数据
  端到端演练**。因此本批次的服务级测试采用「同一请求加/不加过滤器的差分」证明其生效，
  而不是「命中/不命中某条带元数据的记忆」。关闭方式：批次 4 让 canonical 写入承载 metadata 并落到该列。
- **工作区提交状态。** `plugins/sdkwork-memory-plugin-search-first-vector/` 整个目录，以及本批次多个新文件
  （`filter.rs`、`filter_contract.rs`、`filter_pushdown.rs`、`metadata_filter_pushdown_contract.rs`、本矩阵）
  仍为**未提交**状态。`.gitattributes` 为 `* text=auto eol=lf`，提交时行尾会被规范化，故不构成仓库污染；
  但在提交之前，这批成果只存在于工作区。

---

## 15. 批次 5 验证证据（procedural memory 与 agent 上下文）

### 15.1 落地改动

| 文件 | 动作 | 说明 |
| --- | --- | --- |
| `plugins/sdkwork-memory-plugin-search-first-vector/src/procedural.rs` | **新增**（393 行） | `MEMORY_TYPE_PROCEDURAL` / `_SEMANTIC` / `_EPISODIC`、`PROCEDURAL_MEMORY_REQUEST`（逐字）、`AGENT_CONTEXT_SUFFIX`、`PROCEDURAL_MEMORY_SYSTEM_PROMPT`、`MemoryWritePath` / `resolve_write_path`、`is_agent_scoped`、`remove_code_blocks`、`ProceduralMemoryPlan` / `plan_procedural_memory`、`build_procedural_prompt`；每个公开项带可运行 doctest |
| `.../src/error.rs` | 改 | 新增三变体：`UnsupportedMemoryType` / `ProceduralSummaryEmpty` / `ProceduralMetadataRequired`；`port_name()` 归类（摘要空 → `LANGUAGE_MODEL_PORT`；路由与 metadata 拒写 → `INDEX_PORT`）；**并补上此前缺失的端口断言**（见 §15.3） |
| `.../src/extraction.rs` | 改 | `AdditiveExtractionRequest` 新增 `agent_id` / `user_id`；`build_additive_extraction_prompt` 在契约提示词之后、`## Summary` 之前追加 `AGENT_CONTEXT_SUFFIX`（谓词成立时）；`ConversationTurn::is_renderable` / `render` 提升为 `pub(crate)` 供 procedural 复用 |
| `.../src/runtime.rs` | 改 | 新增 `SearchFirstVectorRuntime::plan_procedural_memory(&self, turns, instruction, has_metadata)`：无语言模型端口即 `RequiredPortMissing`；provider 失败包装为 `ProviderCallFailed`；成功后交给自由函数规划 |
| `.../src/lib.rs` | 改 | 注册 `pub mod procedural;` + `pub use procedural::*;`，并在行为表登记 procedural 是**唯一不走 additive 抽取的写入路径** |
| `.../tests/procedural_memory_contract.rs` | **新增**（378 行，21 测试） | 路由、agent 作用域、围栏/推理块剥离、规划拒写次序、提示词装配、runtime 接线 |

**与上游的逐点对应**（均为本轮在 `external/mem0` @ `f8082a7` 上复核）：

| 上游行为 | 位置 | 本仓 |
| --- | --- | --- |
| `is_agent_scoped = bool(filters.get("agent_id")) and not filters.get("user_id")` | `main.py:941` | `is_agent_scoped(agent_id, user_id)`；空串按 Python `bool("")` 视为**不存在**（`is_present`） |
| 追加 `AGENT_CONTEXT_SUFFIX` | `main.py:944`、`:2606` | `build_additive_extraction_prompt` 内同一位置（契约段后、输入段前） |
| `add()` 只接受 `procedural_memory`，其余抛 `VALIDATION_002` | `main.py:831-837` | `resolve_write_path` → `UnsupportedMemoryType { requested }`，且**回显被比对的原值** |
| 独立 procedural 路径（不走 additive 抽取） | `main.py:1993-2037`（异步孪生 `:3671`），自 `:854` / `:2506` 调用 | `plan_procedural_memory` + `runtime.plan_procedural_memory` |
| 提示词 = 系统段 + 对话 + 收尾请求 | `main.py:2002-2007` | `build_procedural_prompt`：三段按序合成单一 prompt，收尾请求**恒在末位** |
| `remove_code_blocks`：先剥围栏（锚定整串），后剥 `<think>`，最后 `strip` | `utils.py:115-132` | `remove_code_blocks`：`strip_enclosing_fence` → `strip_reasoning_blocks` → `trim`，**次序被钉住** |
| 摘要为空 ⇒ `ValueError`；`metadata is None` ⇒ `ValueError` | `main.py:2021`、`:2027` | `ProceduralSummaryEmpty`、`ProceduralMetadataRequired`；**空摘要优先判定**（同上游行序） |

### 15.2 验收命令与结果

| 项 | 结果 |
| --- | --- |
| `cargo test -p sdkwork-memory-plugin-search-first-vector` | **138 passed / 0 failed**（lib 68 + 集成 15 + 8 + 21 + 26）+ **doctest 21 passed**，exit 0 |
| `cargo check --all-targets`（全工作区） | exit 0（无关 crate `sdkwork-web-core` 的 3 条既有警告不受影响） |
| `cargo fmt -p sdkwork-memory-plugin-search-first-vector -- --check` | **exit 0，零 diff** |
| `cargo clippy -p sdkwork-memory-plugin-search-first-vector --all-targets -- -D warnings` | **exit 0，0 warning** |
| 变异对照 | `.workbuddy/tools/mutate-procedural-memory-contract.py`：**7/7 变异全部被抓红**，还原后 SHA256 与原文一致 |
| 变异覆盖的断言 | 路由删除 procedural 分支、`user` 不再取消 agent 框定、空串按存在处理、**两次剥离的次序反转**、**两处拒写的判定次序反转**、摘要空错报 `INDEX_PORT`、**agent 块挪到输入段之后** |
| 行尾 | 8 个被改/新增文件字节级核对 `crlf=0`（纯 LF） |

### 15.3 本批次自查出的自身缺陷（已修）

1. **端口断言凭空缺了一个。** 新增的三个错误变体（`ProceduralSummaryEmpty` / `UnsupportedMemoryType` /
   `ProceduralMetadataRequired`）在 `error.rs` 里**没有任何测试钉住 `port_name()`**。
   这是我在**为变异电池找锚点时**才发现的：能给 `ProceduralSummaryEmpty` 写变异，却没有测试会因它变红。
   修法：把 `ProceduralSummaryEmpty` 并入 `extraction_failures_report_the_language_model_port`，
   新增 `write_path_refusals_report_the_index_port` 覆盖另两个。
   **教训：写变异电池的过程本身会暴露覆盖空洞 —— 找不到「会变红的测试」就是空洞的信号。**
2. **两处测试基线与实现本身都错不得，先错的是基线。**
   `the_agent_block_is_appended_only_for_agent_scoped_writes` 首跑失败：我用 `prompt.find("## Summary")`
   找位置，但 `ADDITIVE_EXTRACTION_PROMPT` **常量自身带一份章节图例**（`extraction.rs:96` 起就有一个
   `## Summary`），`find` 返回的是图例那一份，于是「后缀在前」被判反。
   修法：比照同文件既有的 `the_prompt_renders_every_section_in_order`，先切掉常量再在**附加区**内比较，
   并把断言加固为「后缀在附加区**恰好出现一次**」。
   **这是本会话第二次出现「实现是对的、我的基线是错的」——判据必须是切片后的区域，不是全文 `find`。**
3. **变异 M4 首跑 MISSED，暴露出我测试注释里的过度断言。** 我在注释里写「两种剥离次序在该输入上会产生
   不同结果」，并把 `"<think>a</think>\n```\nbody\n```"` 当作判别输入。实测**两种次序结果相同**：
   变异版在两次 pass 之间**没有重新 trim**，推理块剥掉后留下的前导换行让围栏根本无法匹配。
   修法：换成**推理块紧贴围栏**的输入 `"<think>a</think>```\nbody\n```"`（无换行），此时两种次序确实分叉
   （正确序保留围栏标记、变异序剥掉），并把注释改为如实描述；原先那个换行输入降级为「非判别用例，仅钉参考实现答案」。
   **教训：凡是宣称「顺序可观测」的测试，必须用一条真的能走红的变异来证明 —— 否则注释就是空头支票。**
   本条已写入 skill `gate-mutation-verification`。

### 15.4 批次 4 的阻断项（**上一轮的判断已更正**）

上一轮我记下「批次 4 卡在 schema 决策：`ai_edge` 需要新列」。**该判断是错的。**

复核后的事实：`ai_edge.source_memory_id BIGINT REFERENCES ai_record(id)` **早已存在于 baseline DDL**
（`database/ddl/baseline/postgres/0001_memory_baseline.sql:611`）。所谓「缺列」并不成立。

真实的阻断项只有两个，且都**不需要 schema 迁移**：

1. **命令面未暴露该列。** `graph_store.rs` 的 `InsertEdgeCommand` 没有 `source_memory_id` 字段，
   两处 `INSERT INTO ai_edge`（`:181`、`:466`）也就不绑定它；该列在 `store.rs:1638` / `:1650`
   只被**删除记忆时置 NULL**。结论：**列在、从未被写入**。补法是给命令加字段并绑定，纯代码改动。
2. **`ai_memory_binding` 是否才是归属的正确居所，需要一个设计裁决。** 该表（`*.sql:691`）
   带 `binding_kind` / `source_entity_id` / `source_memory_id` / `target_entity_id` 等，且已被
   `commercial_store.rs`（两处 `INSERT INTO ai_memory_binding`）真实写入，语义是「通用可审计绑定」。
   于是「实体 ↔ 记忆」的归属有两条候选通路：`ai_edge.source_memory_id`（语义窄、就是"这条边由哪条记忆产生"）
   与 `ai_memory_binding`（语义宽、通用绑定）。**这属于 public API 边界的取舍，按 `AGENTS.md`
   的人审规则需要用户裁决，我不自行选定。** 这次我不把它写成「阻断」，而是写成**待裁决的选项**。

### 15.5 残余缺口（登记，未关闭）

- **procedural 路径在服务层不可达。** `runtime.plan_procedural_memory` 已具备并被测试覆盖，
  但 `search-first-vector` 插件**仍未被任何 crate 消费**（§13）。因此本批次的 procedural 能力
  与 §13 的 P0 同因：**已实现、已被测试钉住、但运行时不可达。** 批次 7 是它的解锁项。
- **服务层没有 `agent_id` 概念。** `is_agent_scoped` 的谓词已在插件内落地并被 4 项测试覆盖，
  但服务层的写入命令里没有 `agent_id` / `user_id` 字段，所以该框定在当前组合根下**也拿不到输入**。
  与上一条同因（待批次 7 接线时一并决定这两个字段由谁承载）。
- **`ai_record.metadata_json` 仍无写路径**（§14.4 未变）：procedural 的 `has_metadata` 判定因此
  只能由调用方传布尔，尚不能由真实落库的 metadata 驱动。
- **工作区提交状态**（§14.4 未变）：`plugins/sdkwork-memory-plugin-search-first-vector/` 整目录
  及本批次新增的 `procedural.rs` / `procedural_memory_contract.rs` / 变异脚本仍**未提交**。

---

## 16. 批次 6 验证证据（可达性审计：能力已实现但到不了）

批次 6 不改任何能力代码，只回答一个问题：**本仓已经写好的记忆能力，有多少是运行时到不了的。**
结论是 §3 的整套 mem0 对齐打分栈与 §13 的插件同属一个缺陷类。

### 16.1 方法与工具

| 工具 | 回答的问题 | 判据 |
| --- | --- | --- |
| `.workbuddy/tools/audit-workspace-reachability.py` | 哪些 workspace member **不在部署入口的依赖闭包内** | `cargo metadata` 的 `resolve.nodes[].deps[].pkg`（**不是** `packages[].dependencies[]`）；入口 = 带 `bin` 的 crate + 显式登记的组合根 |
| `.workbuddy/tools/audit-symbol-reachability.py` | 关注清单里的符号**是否有生产引用** | 排除 `lib.rs`（只再导出）、排除 `#[cfg(test)]` 之后的一切、排除注释行（doctest 会点名符号）；跨 crate 引用才是有效种子 |

**为什么不用 grep 下结论**：`grep -rn "score_and_rank"` 在 `lib.rs` 的 `pub use` 行上命中，
看起来就像“有人在用”。`pub use` 是**暴露**，不是**使用**。

### 16.2 结果

**crate 级（入口 `sdkwork-api-memory-standalone-gateway` + `sdkwork-api-memory-assembly`）：**

```text
unreachable workspace members (2):
    sdkwork-memory-integration-tests          # 纯测试 crate，符合预期
    sdkwork-memory-plugin-search-first-vector # 即 §13 的 P0
reachable (17): …（含 sdkwork-memory-retrieval、sdkwork-intelligence-memory-service、三个 routes crate 等）
```

**符号级（`sdkwork-memory-retrieval`，5 reached / 14 dead）：**

```text
score_and_rank                         DEAD     sdkwork-memory-retrieval::scoring
HybridSignals                          DEAD     sdkwork-memory-retrieval::scoring
max_possible_score                     DEAD     sdkwork-memory-retrieval::scoring
validate_threshold                     DEAD     sdkwork-memory-retrieval::scoring
internal_fetch_limit                   DEAD     sdkwork-memory-retrieval::scoring
ScoreDetails                           DEAD     sdkwork-memory-retrieval::scoring
Bm25Index                              DEAD     sdkwork-memory-retrieval::bm25
normalize_bm25                         DEAD     sdkwork-memory-retrieval::bm25
get_bm25_params                        DEAD     sdkwork-memory-retrieval::bm25
lemmatize_for_bm25                     DEAD     sdkwork-memory-retrieval::lemmatization
entity_boosts                          DEAD     sdkwork-memory-retrieval::entities
select_query_entities                  DEAD     sdkwork-memory-retrieval::entities
extract_entities                       DEAD     sdkwork-memory-retrieval::entities
memory_count_weight                    DEAD     sdkwork-memory-retrieval::entities
keyword_match_score                    REACHED  sdkwork-memory-retrieval::retrieval
orchestrate_retrieval_candidates       REACHED  sdkwork-memory-retrieval::retrieval
fuse_retrieval_candidates_with_policy  REACHED  sdkwork-memory-retrieval::retrieval
build_context_pack_from_hits           REACHED  sdkwork-memory-retrieval::context_pack
time_recency_score                     REACHED  sdkwork-memory-retrieval::retrieval
```

**交叉印证（不依赖上面的工具，纯 grep 可复核）：** 服务唯一消费的两个模块
`retrieval/mod.rs` 与 `context_pack.rs`，对 `scoring` / `bm25` / `entities` / `lemmatization`
的引用**合计零命中**。两条独立证据给出同一结论。

**代价量化（`cargo test -p sdkwork-memory-retrieval`，§17 后复跑 `148 passed / 0 failed`）：**

| 模块 | 规模（文件总行数） | lib 单测数 | 生产调用方 |
| --- | --- | --- | --- |
| `scoring.rs` | 496 行 | 16 | **0** |
| `bm25.rs` | 521 行 | 14 | **0** |
| `entities.rs` | 2 254 行 | 42 | **0** |
| `lemmatization.rs` | 487 行 | 16 | **0** |
| **死栈小计** | **3 758 行** | **88** | **0** |
| `retrieval/mod.rs` | 764 行 | 7 | 服务 + 3 个 routes crate |
| `context_pack.rs` | 190 行 | 3 | 服务 |
| **可达栈小计** | **954 行** | **10** | — |

> 该 crate 的 98 条 lib 单测里，**88 条（89.8%）在测没有任何生产调用方的代码**，只有 10 条
> 覆盖真正在跑的检索路径。这正是「测试全绿」与「能力生效」之间那道缝的量化形状。
>
> **计数口径与复核命令**（本表为**文件总行数**口径，含各文件自己的 `#[cfg(test)]` 模块）：
> ```sh
> wc -l crates/sdkwork-memory-retrieval/src/{scoring,bm25,entities,lemmatization}.rs \
>       crates/sdkwork-memory-retrieval/src/retrieval/mod.rs \
>       crates/sdkwork-memory-retrieval/src/context_pack.rs
> grep -c '^\s*#\[test\]' crates/sdkwork-memory-retrieval/src/{scoring,bm25,entities,lemmatization}.rs
> ```
> **行数已按 §17 的重测结果更正**（上一版记 487 / 520 / 1 886 / 479）：`cargo fmt -p
> sdkwork-memory-retrieval` **会重排整个 crate**，其中 `scoring.rs` / `bm25.rs` /
> `entities.rs` / `lemmatization.rs` 此前与当前 rustfmt 的输出不一致，被一并重排
> （+9 / +1 / +368 / +8 行，mtime 印证为同一次 fmt）；上一版的行数是**重排前**测的。
> 单元测数（88 / 97→98）与结论不受影响。**教训**：只想格式化本轮改动的文件时用
> `rustfmt <file…>`，不要用 `cargo fmt -p <crate>` —— 后者会改到你没打算动的文件。

### 16.3 根因：一个 crate 里两套互不引用的打分栈

见 §3 的可达性更正表。一句话：**mem0 对齐栈写完了、测绿了、连 `lib.rs` 都导出了，
但既没有调用方，也没有策略档位能选中它。**

### 16.4 本轮审计工具自身错了 6 次（每一次都把结论翻转）

这一节比结论更值得记 —— 审计工具的**假阳性与假阴性同样危险**，而「误报 DEAD」尤其昂贵：
它会诱导人去删**正在用**的代码。

| # | 错误 | 症状 | 修法 |
| --- | --- | --- | --- |
| 1 | 用 `packages[].dependencies[].pkg` 建图 —— 该字段**不存在** | 边集恒空 ⇒ 所有 crate 都是叶子 ⇒ 反而报「全部可达」 | 改用 `resolve.nodes[].deps[].pkg` |
| 2 | 把 `lib.rs` 当调用方 | `pub mod` / `pub use` 被当成使用 ⇒ 整个 crate 全绿 | `lib.rs` 永不作为调用方 |
| 3 | 按**模块名**在别的 crate 里找引用作为种子 | 服务是通过**再导出**导入的，从不写模块名 ⇒ `context_pack` 被误报 DEAD（假阴性方向） | 种子改为**符号驱动** |
| 4 | 用**全部公开符号**做种子 | `pub fn new` 之类泛名几乎每个模块都定义、每个 crate 都调用 ⇒ 全绿 | 种子限定为关心清单 |
| 5 | 退一步改用「只被一个模块定义」过滤泛名 | `pub fn fit` 只在 `bm25.rs` 定义，而 `access.rs` 恰好写了无关的 `.fit(` ⇒ BM25 模块被误种 | 泛名不能只靠「唯一归属」排除，必须靠**人工选定清单** |
| 6 | 把模块级推断也打印出来 | `app_backend_api` 等**正在使用**的服务模块被报 DEAD —— 跨 crate 通过**方法调用**到达的模块，调用方根本不写模块名 | **删掉该输出**，只保留逐条可选核的符号判定 |

> **一般化：可达性结论必须逐条可指名到「引用它的那个文件」。**
> 报 `DEAD` 这件事的门槛要**高于**报 `REACHED`：误报 REACHED 只是让结论偏乐观，
> 误报 DEAD 会让人删掉在跑的代码。凡是不能列出具体引用点的「不可达」，都不算结论。

### 16.5 这对优先级的影响（**本轮最重要的产出**）

在本轮之前，§10 的排序是「先补能力（批次 4/6），最后接线（批次 7）」。
审计结果不支持这个排序：**已实现但不可达的能力，补得越多、沉没成本越大。**

| 批次 | 补完之后是否可被用户观察到？ |
| --- | --- |
| 批次 4（实体链路） | ❌ 即便补齐，实体加权也到不了 —— `scoring` 模块本身不可达（§5 脚注） |
| 批次 6（API 面 `threshold`/`explain`） | ❌ 这三个参数由不可达的 `scoring` 提供语义 |
| 批次 1 / 2 / 3 / 5 的既有成果 | ❌ 除 filter（服务面已接）外，其余均不可达 |

⇒ **批次 8 与批次 7 是其余一切的解锁项。** 在它们之前继续补能力，只会扩大「已实现未接线」的面积。

### 16.6 待决策（本小节在 16.7 被收敛为一条）

1. **批次 8 的接法**：给 `MemoryRetrievalStrategy` 增加一个指向 `score_and_rank` 的档位
   （**加性、默认策略不变**，推荐），还是把可达栈的 keyword 信号整体换成 `bm25`
   （**改变既有默认行为**，需要回归全部检索测试）？
2. **批次 7 的接法**（§13 原文两问未变）：组合根选 `sdkwork-intelligence-memory-service`
   还是 `-repository-sqlx`；插件作 `primary_plugin_id` 还是并列 retriever；
   以及 `provider-binding-required` 的绑定落哪个部署 profile（配置面改动，按约定**先问再改**）。

> ⚠️ 上面两条是在**下钻语义通道之前**写下的。16.6 的追加与 16.7 的收敛表明：
> 两条的**共同前置**是「有没有向量/语义通道」，因此本文最终只留**一条**决策。

### 16.7 🔴 更深一层：可达系统**没有向量/语义检索通道**（本轮追加）

上一轮的批次 8 把问题写成「打分链没接线」。继续下钻后发现**前提就不成立**：
`score_and_rank` 吃的是 `SemanticCandidate { memory_id, semantic_score }`，
而**可达路径里没有任何语义分**。查到底：

**可达系统按声明与 schema 都是 embedding-free 的：**

| 证据 | 位置 |
| --- | --- |
| 能力声明 `{ "keyword": true, "embedding": false }` | `crates/sdkwork-intelligence-memory-service/src/backend_admin_api.rs:1388`（另有 `implementation_migration.rs:172` 的期望串同值） |
| `"embeddingRequired": false` | `crates/sdkwork-intelligence-memory-service/src/open_api.rs:682` |
| `embedding_optional: true` | `open_api.rs:798`、`crates/sdkwork-memory-contract/src/dto.rs:80` |
| DDL 无向量列，只有全文检索 | `database/ddl/baseline/postgres/0001_memory_baseline.sql:526,538` 的 `to_tsvector`（`grep -iE "vector\(|embedding|pgvector"` 零命中） |
| 该 crate 的检索测试文件名 | `crates/sdkwork-memory-retrieval/tests/no_embedding_retrieval.rs` |
| 可达检索输入不含语义字段 | `RetrievalRecordInput { memory_id, subject, predicate, object_text, canonical_text, created_at }` |
| 可达信号集无向量 | keyword（子串+词元重合）/ dictionary / sql_structured / time_recency / event + RRF |

**向量能力只存在于不可达的两处：**

| 位置 | 规模 | 状态 |
| --- | --- | --- |
| `plugins/sdkwork-memory-plugin-search-first-vector/src/vector_index.rs` | 1 347 行 | 所属 crate 不在部署依赖闭包内（§13） |
| `crates/sdkwork-memory-retrieval/src/scoring.rs` | 487 行 | 零生产调用点（§16.2） |

**上游的检索模型是向量优先的**：`add()` 先 embed 再入向量库，`search()` 先向量相似度召回，
再叠 BM25 与实体增强，最后加法归一。**本仓可达系统缺了这条通道的整条主干。**

### 16.8 收敛：三个「待决策」其实是同一个决策

| 表面上的独立问题 | 实际依赖 |
| --- | --- |
| §13 插件可达性（组合根 + `provider-binding-required`） | 提供 `retrieverKinds: ["vector"]` 与 `indexKind: "vector"` —— 就是那条缺失的通道 |
| §3 打分栈不可达（批次 8） | 打分栈**消费** `SemanticCandidate`，而无人生产 ⇒ 没有语义源就无解 |
| §5 实体加权喂不进去 | 上游把 boost **加到语义分上**，无语义分则加权无锚点 |
| §1 `search` 的 `threshold` | 对齐语义是「**只 gate 语义分**」（§3 行）⇒ 无语义分时该门控没有意义 |

⇒ **唯一的前置决策是：是否打通向量/语义通道**（绑定 `EmbeddingModelPort` + 接入向量检索/索引）。
其余三项在它之后都变成机械落地。**这也把上一轮那份「待决策」清单从 3 条收敛到 1 条。**

**建议的最小落地路径**（按影响面从小到大，可分别停在任一步）：

1. **只声明不动行为**：给 `MemoryRetrievalStrategy` 无需改动、而是给服务的能力声明一个可切换开关
   （`embedding: false` → 由 profile 决定），使「有没有向量通道」成为可观测的部署事实。
   零行为变化，但会让 §13 的阻断在能力面上显式化。❌ 仍然只是声明，不产生可观察价值 ——
   **按 §16.4 的纪律，这种「只落声明」的步骤不应单独交付。**
2. **接入向量召回为一路额外信号**（推荐）：`orchestrate_retrieval_candidates` 已是多路 RRF，
   再添一路「向量相似度」是最小侵入 —— 默认权重 0（**默认行为不变**），
   绑定 `EmbeddingModelPort` 后按 profile 启用。这一步同时让 `SemanticCandidate` 有生产者。
   ⇒ **接收端已于 §17 落地（8a 完成）；本条实际拆成 8a/8b 两半，供应端待决策，见 §17.7。**
3. **接入加法归一栈**（批次 8）：语义源就位后，`score_and_rank` 才可被喂满，
   实体加权 / `threshold` / `explain` 随之可接。
4. **接入插件写入语义**（批次 7）：`search-first-vector` 承担 mem0 的 `add` 语义
   （ADD-only + 链接 + procedural），锁在同一个 provider 绑定之后。

---

## 17. 批次 8 步骤 1 验证证据（向量信号**接收端**落地，默认关闭）

### 17.1 本轮边界：做了什么，**没**做什么

`§16.8` 的最小落地路径第 2 步写成「再添一路向量相似度」。本轮只交付这句话的**一半**，
并且把另一半的阻塞写死在下面 —— 因为这一半足以独立验证，而另一半需要两个**配置面决策**。

| 半 | 内容 | 状态 |
| --- | --- | --- |
| **接收端（排序）** | 可达编排器能接收「每记忆一条相似度」，按 profile 决定是否参与 | ✅ 本轮落地 |
| **供应端（召回）** | 谁算这条相似度：嵌入端口绑定 + 数据面返回向量候选 | ❌ 需决策，见 §17.7 |

### 17.2 落地改动

| 文件 | 改动 |
| --- | --- |
| `crates/sdkwork-memory-retrieval/src/retrieval/mod.rs` | 新增 `VectorSimilarityInput`；新增私有 `vector_similarity_score`；`orchestrate_retrieval_candidates` 改为**委托** `orchestrate_retrieval_candidates_with_vector(…, &[], …)`；新增 `"vector"` profile 键（**默认 0.0**） |
| `crates/sdkwork-memory-retrieval/src/lib.rs` | 导出 `orchestrate_retrieval_candidates_with_vector`、`VectorSimilarityInput` |
| `crates/sdkwork-memory-retrieval/tests/vector_signal_contract.rs` | 新增 10 条契约测试 |
| `.workbuddy/tools/mutate-vector-signal-contract.py` | 新增 8 条变异的控制脚本 |

三条不可让步的设计约束，全部写进了 doc 注释并被测试钉住：

1. **默认关闭且不是普通默认值。** 其余信号在 `profile == None` 时走各自的默认权重
   （keyword 1.0 / dictionary 0.85 …），即「无 profile = 全开」。`vector` **刻意例外**：
   `retriever_weight(profile, "vector", 0.0)` ⇒ 无 profile 也是 0，且任何未提及 `vector`
   的历史 profile 都不会突然开始用它打分。理由是向量信号需要**已绑定的嵌入端口**，
   而本 crate 看不到端口状态 —— 只有供应方明确要求时才允许参与。
2. **上游越界的相似度**被 clamp 到 `[0, 1]` 而不是重标定。`fused` 的 `raw_score` 在 RRF
   贡献打平时**跨检索器比较**，未归一化的点积会碾过全部词法信号。这复刻的是上游
   `min(raw_combined / max_possible, 1.0)` 的意图（上游 clamp 的是合成分，本仓 clamp 的是
   输入分 —— 这是**按义重述**，不是等价变换，见 §15.5 的边界规则）。
3. **不引入调用方未读的记忆。** 相似度对应的 `memory_id` 若不在 `records` 里就丢弃：
   本模块只对调用方**已解密、已授权、已还原**的候选集重排。
   ⇒ **这条同时决定了本步骤打不通「召回端」**：向量只能重排，不能扩大召回。

### 17.3 验收证据

```text
cargo fmt -p sdkwork-memory-retrieval                                              -> exit 0
cargo clippy -p sdkwork-memory-retrieval --all-targets                             -> 0 warnings
cargo check --all-targets                                                          -> exit 0 (8.20s)
cargo test -p sdkwork-memory-retrieval
    lib                                    98 passed (原 97，+1 边界单测)
    tests/no_embedding_retrieval.rs         3 passed
    tests/context_pack.rs                  13 passed
    tests/vector_signal_contract.rs        10 passed (新增)
    Doc-tests                              24 passed
```

字节核验（`crlf=0`，纯 LF）：

```text
crates/sdkwork-memory-retrieval/src/retrieval/mod.rs          bytes=25625 crlf=0 sha256=a203724c23ad4d42
crates/sdkwork-memory-retrieval/src/lib.rs                    bytes=1648  crlf=0 sha256=4c807df51b751cc5
crates/sdkwork-memory-retrieval/tests/vector_signal_contract.rs bytes=8516 crlf=0 sha256=cd8249d9f27f9a2b
.workbuddy/tools/mutate-vector-signal-contract.py             bytes=8043  crlf=0 sha256=b7d90719d606eec1
```

### 17.4 变异电池：8/8 全部被捕获

```text
CAUGHT M1: vector recall turns on even without a profile
CAUGHT M2: the profile key no longer matches the emitted retriever name
CAUGHT M3: an unbounded similarity escapes the upper clamp
CAUGHT M4: a non-finite similarity is clamped into a perfect score
CAUGHT M5: a similarity for an unread memory is attributed to another record
CAUGHT M6: the vector signal is never contributed at all
CAUGHT M7: vector recall is emitted before the lexical signals
CAUGHT M8: a commercial strategy profile silently enables vector recall

8/8 mutations caught
restore byte-identical: True
```

`M4` 值得单记：我原本以为 `!similarity.is_finite()` 是防御性冗余（下游 `append_ranked_signal`
也会丢非有限值），但**它并不冗余** —— `f64::INFINITY.clamp(0.0, 1.0) == 1.0`，
去掉守卫后 `+inf` 会变成一个**看起来完美的 1.0 分**并参与排序。这是本轮唯一靠变异才发现的真缺陷。

**未纳入变异的项（显式登记为等价变异）**：下界 clamp（`clamp(0.0, …)` 的 `0.0`）单独移除
是**不可观测**的 —— `append_ranked_signal` 已经丢弃所有非正分。它保留为 `[0, 1]` 契约的
唯一执行点，而不是第二道防线。按 skill `gate-mutation-verification` §6.24，**不可观测**
与「没被覆盖」必须区分开写。

### 17.5 可达性复核：**代码可达，能力贡献仍为 0**

```text
.workbuddy/tools/audit-symbol-reachability.py .
== symbol verdicts ==
keyword_match_score                            REACHED  sdkwork-memory-retrieval::retrieval
orchestrate_retrieval_candidates               REACHED  sdkwork-memory-retrieval::retrieval
orchestrate_retrieval_candidates_with_vector   REACHED  sdkwork-memory-retrieval::retrieval
VectorSimilarityInput                          REACHED  sdkwork-memory-retrieval::retrieval
fuse_retrieval_candidates_with_policy          REACHED  sdkwork-memory-retrieval::retrieval
build_context_pack_from_hits                   REACHED  sdkwork-memory-retrieval::context_pack
time_recency_score                             REACHED  sdkwork-memory-retrieval::retrieval
(其余 14 项 DEAD，同 §16.2)
7 reached, 14 dead          # 上一轮 5 reached / 14 dead
```

新代码**没有**变成又一个 §16 缺陷，但要精确表述「为什么没有」，否则就是自我安慰：

| 层面 | 结论 | 证据 |
| --- | --- | --- |
| 代码可达 | ✅ | 委托链 `open_api.rs:1381` → `orchestrate_retrieval_candidates` → `_with_vector`，审计为 REACHED |
| 数据供应 | ❌ 空 | 唯一生产调用点传 `&[]`（全仓只有 1 处调用，见 §17.7） |
| 权重开启 | ❌ 恒 0 | 可达的 profile 词汇表 `SUPPORTED_RETRIEVERS` 不含 `vector`，且**有测试断言必须被拒**（`retrieval_profile.rs:144-147`） |

⇒ **本轮的运行时产出为 0，与 §16 的缺陷同类**；区别在于**它是被登记的、且下一步只需要一个决策**，
而不是被当成"已实现完整能力"记账。因此在 §9 的计数里，本轮**不新增任何 ✅**，
只把「可达性阻断 3 项」中的第 ① 项从「无接收端」推进到「接收端就绪、供应端待决」。

### 17.6 与上游的语义差异（诚实登记，不复刻）

| 维度 | 上游 mem0 | 本仓现状 |
| --- | --- | --- |
| 融合方式 | 加法：`(semantic + bm25 + entity) / max_possible`，**语义为主干** | RRF：各路独立排名后融合，向量是**并列的一路** |
| 向量在链路里的角色 | 召回**来源**（`search` 先向量召回，BM25/实体只做增强） | 召回**重排输入**（§17.2 约束 3）⇒ 换不出新记忆 |
| 门控 | `threshold` 先 gate 语义分再合成 | 无 threshold（批次 6，依赖 §17.7） |

⇒ 即使供应端就位，本仓也不会自动获得「语义为主干」的召回形状；那要求**召回端**一起打开
（数据面返回 `MemoryRetrieverKind::Vector` 候选）—— 见 §17.7。

### 17.7 阻塞在前的决策（配置面 / 组合根，按约定先问再改）

供应端拆成两半，各自撞在不同的面上：

| 子步 | 内容 | 撞到的面 | 属于 |
| --- | --- | --- | --- |
| 8a **排序端供应** | 服务层绑 `EmbeddingModelPort`，对已还原候选算相似度并传入 | `SUPPORTED_RETRIEVERS` 需接受 `"vector"`，且必须同时**反转**既有测试 `rejects_unsupported_vector_retriever` | **配置面 + 公开词表** |
| 8b **召回端供应** | 数据面按 `retriever_kinds: ["vector"]` 返回向量候选 | 需要 `retrieverKinds: ["vector"]` 的提供者 = §13 的插件路径 | **批次 7（组合根）** |

两处都不是「改代码」能定的：8a 会改变一个**公开契约的词汇表**（并有测试专门守护它被拒），
8b 会改变**部署 profile 的 provider 绑定**。按 `USER.md` 的配置面约定，先把证据摆出来再动。

> **决策已定（用户选择）**：8b 走「**写路径落嵌入（向量列）**」。
> 完整侦察结论、逐文件变更清单、物理类型选择、不变量、验收清单与回滚见 **§18**。

`MemoryRetrievalStrategy` 的三个档位**故意都没有** `vector` 权重 —— 测试
`commercial_retrieval_strategies_do_not_silently_enable_vector_recall` 与变异 `M8` 共同守护这一点：
三档 profile 是可达系统里 `default_retriever_profile()` 的直接来源，它们一旦带上权重，
就等于在无嵌入端口时**默认打开**向量打分。

### 17.8 本轮副作用披露：`cargo fmt -p` 的改动面**超出**本轮改动

`cargo fmt -p sdkwork-memory-retrieval` 会重排整个 crate。除本轮改动的三个文件外，
`scoring.rs` / `bm25.rs` / `entities.rs` / `lemmatization.rs` 也被重排
（+9 / +1 / +368 / +8 行；mtime 全为同一次 fmt）。这四个文件**本轮没有功能改动**，
行数变化纯属格式化，并导致 §16.2 的行数需要更正（已改）。

- **保留还是回滚**：无法回滚（该 crate 多数文件在 HEAD 中不存在，没有可还原的基线），
  且格式化方向与仓库 `rustfmt` 约定一致，故保留。
- **代价**：这四行文件在本次 review 里的 diff 变大，评审时**只需看 §17.2 列出的三个文件**。
- **若需功能级回滚**：本轮的语义改动只落在 `retrieval/mod.rs` 的 `_with_vector` 委托与
  向量块、`lib.rs` 的两行导出、以及新增的测试文件；把 `orchestrate_retrieval_candidates`
  的委托换回原实现体即可恢复到「无向量信号」的行为（三个 profile 权重本就默认 0）。
- **下次纠正**：只格式化本轮改动的文件时用 `rustfmt <file…>`，不要用 `cargo fmt -p <crate>`。

---

## 18. 批次 8b 设计记录：写路径落嵌入（向量列）

> ## 🔴 本节方案已被 §19 阻断（2026-09-23 晚追加）
>
> 本节的结论在**未看到 `native_sql` phase1 阶段门禁**的前提下写成。追加侦察后发现：
> 两套基线里**禁止出现 `vector`/`embedding`/`pgvector` 这些词**，且该断言**在活的 fail-closed
> 门禁链上**（实测加列即 exit 1）。⇒ 本节按原方案落地会**直接让门禁变红**。
> **以 §19 为准**：方案 A（写路径落嵌入）需要先决定是否移动该阶段门禁；
> 方案 B（向量存储归插件）不需要。本节其余侦察事实（§18.1/§18.2/§18.5/§18.6）仍然有效。

> **状态：待落地**（决策已定：写路径落嵌入；本轮只完成侦察与变更清单，未动 DDL）。
> 本节的每一行都是**可复核的硬事实**，因为重新推导一遍代价很高。

### 18.1 侦察结论：本仓有**两套引擎原生 DDL 树**，且各自都是权威

| 引擎 | 位置 | 权威来源 | 运行时如何使用 |
| --- | --- | --- | --- |
| PostgreSQL | `database/ddl/baseline/postgres/0001_memory_baseline.sql`（899 行）+ `database/migrations/postgres/*.up.sql` | `database/database.manifest.json#baselineStrategy = baseline-plus-migrations` | 生产生命周期（`sdkwork-database` CLI） |
| SQLite | `tests/fixtures/database/sqlite/migrations/0001..0010_*.up.sql`（各带 `.down.sql`） | 客户端本地契约 | **`include_str!` 硬编码进可达代码** |

SQLite 那一套被 `plugins/sdkwork-memory-plugin-native-sql/src/store.rs:3952-4010` 的
`apply_sqlite_phase1_migration()` 用 `include_str!("../../../tests/fixtures/database/sqlite/migrations/00NN_*.up.sql")`
**逐条内联**，版本号 `0010` 是当前 SQLite 期望版本（`store.rs:208`；PG 是 `0009`，由基线承担）。
PG 那一套则被 `store.rs:230` 用 `include_str!("../../../database/ddl/baseline/postgres/0001_memory_baseline.sql")`
内联为 `"baseline"` 版本。

**⇒ 两套必须同时改，且不能互相拷贝**：§7.1 明文「PostgreSQL 与 SQLite 迁移
`MUST NOT` 被拷贝、机械转写、或在同一个 SQL 文件里按运行时分支选择」。

### 18.2 三条容易踩错的工具事实（均已实测）

| 事实 | 证据 |
| --- | --- |
| `baseline-plus-migrations` 下 `db:materialize:baseline` **只检查、不写盘**，且**禁止**由迁移折叠出基线 | `scripts/materialize-memory-database-baseline.mjs:150-161`；脚本自述「folding … would replace the authoritative schema DDL with the handful of post-baseline deltas … a fresh install would create no tables at all」 |
| 契约**只由 PG 基线派生**（不读迁移） | `package.json:34` 的 `db:materialize:contract` 只传 `--baseline …0001_memory_baseline.sql`；且 `database/contract/*` 里含 `search_document`（该列只存在于已折叠的 0005）⇒ 契约 = f(基线) |
| 迁移目录的编号**在初始化态合并后重开** | 基线内含 `-- source: …0001_memory_schema.up.sql` … `0009_memory_job_execution_lease.up.sql` 九条来源；而 `database/migrations/postgres/` 现只有 `0001_organization_id_not_null.up.sql` ⇒ 新迁移应编号 `0002` |

⇒ **推论（决定了变更清单）**：新列若要被契约工具与 SQLite 同时看见，
**必须进 PG 基线**（它是契约与 `include_str!` 的共同来源），**同时**补一条 post-baseline 迁移
给已存在的部署。这不是「改写基线以吸收迁移」，而是让基线保持它是权威 DDL 的职责；
`§7.5` 也明文允许这一状态：「A migration whose effect the baseline happens to already contain
remains valid: it is idempotent by construction, not redundant debt.」

### 18.3 变更清单（逐文件）

| # | 文件 | 改动 | 性质 |
| --- | --- | --- | --- |
| 1 | `database/ddl/baseline/postgres/0001_memory_baseline.sql` | `ai_record` 增 4 列（见 §18.4）+ 列注 + CHECK | 权威 DDL（同时驱动契约与 SQLite fixture 的对齐基准） |
| 2 | `database/migrations/postgres/0002_memory_record_embedding.up.sql` | **新增**，`ADD COLUMN IF NOT EXISTS` ×4，幂等；头部含 `engine/module/purpose/reversible/rollback/transactional/lock/lock_timeout/statement_timeout` | post-baseline 增量 |
| 3 | `tests/fixtures/database/sqlite/migrations/0011_memory_record_embedding.{up,down}.sql` | **新增**，SQLite 原生写法（`BLOB`，无 `IF NOT EXISTS` 依赖） | 客户端本地增量 |
| 4 | `database/contract/{schema.yaml,table-registry.json}` | 由 `pnpm db:materialize:contract` **重新生成**（不手改） | 生成物 |
| 5 | `plugins/sdkwork-memory-plugin-native-sql/src/canonical_data.rs` | 写路径：`INSERT/UPDATE ai_record` 增列 + 读路径：`SELECT` 取出 embedding 与 `embedding_dimensions`/`embedding_model` | 可达数据面 |
| 6 | `crates/sdkwork-memory-contract/src/dto.rs` | 新增嵌入读写 DTO（**不经 HTTP 契约暴露向量本身**） | 契约层 |
| 7 | `crates/sdkwork-intelligence-memory-service/src/open_api.rs` | 检索时：`embedding_model()` 有绑定时嵌入 query 一次 → 对已还原候选算余弦 → 传入 `orchestrate_retrieval_candidates_with_vector` | 服务层 |
| 8 | `crates/sdkwork-intelligence-memory-service/src/retrieval_profile.rs` | `SUPPORTED_RETRIEVERS` 增 `"vector"`，并**反转**既有测试 `rejects_unsupported_vector_retriever`（改为「接受 `vector`，仍拒绝未知名」） | **公开词表** |
| 9 | `crates/sdkwork-memory-retrieval/src/lib.rs` | 无需改动（批次 8a 已导出） | — |

### 18.4 物理类型与列集（含规范依据）

**逻辑类型选 `binary`，不是 `json`。** 依据 `DATABASE_SPEC.md` §8.1 类型映射表：
`binary` → PG `BYTEA` / SQLite `BLOB`，两引擎原生支持；而 `json` 在本模块的 native SQL profile 下
是 TEXT（`database/README.md` 明写），1536 维 f32 存成 JSON 文本约 20KB 且无法用长度做不变量。
**不用 pgvector**：§8.2 要求扩展「versioned, schema-qualified, allow-listed, and verified in
bootstrap, backup, restore and managed-service profiles」，代价远超本期需要；
且向量相似度在 Rust 侧算（与 §17.6 已登记的形状一致）。

| 列 | PG | SQLite | 作用 / 不变量 |
| --- | --- | --- | --- |
| `embedding` | `BYTEA` | `BLOB` | little-endian `f32` 序列，长度 = `4 × embedding_dimensions` |
| `embedding_dimensions` | `INTEGER` | `INTEGER` | 维度；与 provider 绑定不一致即可检出 |
| `embedding_model` | `TEXT` | `TEXT` | 产出该向量的 provider/model 标识；同维不同模型**不可比** |
| `embedding_source_hash` | `TEXT` | `TEXT` | 生成向量时 `canonical_text` 的稳定哈希；文本变更后据此判定**过期** |

CHECK（两引擎都可表达）：

```sql
-- PostgreSQL
CONSTRAINT ck_ai_record_embedding_shape
  CHECK (embedding IS NULL OR (embedding_dimensions IS NOT NULL
                               AND octet_length(embedding) = 4 * embedding_dimensions))
-- SQLite
CHECK (embedding IS NULL OR (embedding_dimensions IS NOT NULL
                             AND length(embedding) = 4 * embedding_dimensions))
```

**为什么需要 `embedding_source_hash`**：PG 基线里 `search_document` 由触发器在
`UPDATE OF canonical_text, …` 时自动重算（基线 541-566 行），但**向量无法在 SQL 里重算**
（需要 provider）。所以文本变更后向量必然过期，只能由写路径重算 + 读路径校验哈希。
这条不写下来，未来必然出现「改了记忆但召回还按旧向量排序」的静默错误。

### 18.5 放 `ai_record` 列上，还是单独一张表？

| 方案 | 优 | 劣 |
| --- | --- | --- |
| **`ai_record` 增列（推荐，最小侵入）** | 一记忆一向量，与当前「单嵌入候选」能力声明一致；无需 FK/身份机制；回滚=删列 | 多 provider / 多 index_kind 时会出现第二个向量无处放 |
| 单独 `ai_record_embedding(index_id, memory_id, …)` 子表 | 天然支持多 provider / 多索引；与既有 `ai_index`（含 `index_kind`/`provider_binding_id`/`rebuild_cursor`）配套 | 需要身份与级联规则；本期没有第二个消费者的实证需求 |

⇒ 取**列方案**。若将来出现第二个 provider，再以「expand/backfill/contract」流程迁到子表，
届时 `embedding_source_hash` 与 CHECK 可直接搬到子表。

### 18.6 验收清单（落地时必须全绿）

```sh
pnpm db:validate                                             # DATABASE_FRAMEWORK_SPEC 合规
node scripts/materialize-memory-database-baseline.mjs --check # 基线存在且非空，迁移头部/可逆性合规
pnpm db:materialize:contract && git diff --exit-code database/contract  # 契约已重生成
cargo test -p sdkwork-memory-plugin-native-sql               # SQLite fixture 能建出新 schema
cargo test --all-targets                                     # 全量
```

另需新增的**功能级**证据（不是"能建表"就算）：
- 写路径：绑定桩嵌入 provider 时，`ai_record.embedding` 非空且 `embedding_source_hash` 与
  `canonical_text` 一致；未绑定时该列为 NULL 且行为与今天**字节不变**。
- 读路径：同一批候选，向量信号开启后排序与 §17 的契约测试一致；关闭时与关闭前一致。
- 过期：改 `canonical_text` 后，未重算的向量**不被**当作有效信号。

### 18.7 顺带发现的两处仓内不一致（登记，未修）

1. **`database/README.md` 的折叠说法对本模块不成立**：它写「`pnpm db:materialize:baseline` 会由迁移
   折叠出基线，`pnpm verify` 检查基线是最新的」；但该说法**只对 `baselineStrategy = migrations-only` 成立**，
   本模块是 `baseline-plus-migrations`，脚本在此模式下**从不写盘**（见 §18.2）。照 README 操作会得到
   「什么都没发生」。
2. **§7.5「基线不可改写以吸收迁移」与本仓基线的实际角色有张力**：脚本自述基线是
   「the authoritative DDL source」且契约工具只读基线，所以新列不进基线就会造成
   契约与 SQLite 双向漂移。本期按 §18.2 的推论处理（基线 + 幂等迁移并存），
   并把这条张力登记在此，供后续与 specs 侧对齐。

### 18.8 回滚

DDL 侧：`0002` 为 `ADD COLUMN IF NOT EXISTS`，回滚策略 `forward-fix`（删列是**有损**的
——向量无法从数据重建，只能重新嵌入），故**不提供 `.down.sql`**，符合 §7.1 的双向约束。
应用侧：批次 8a 的向量信号默认权重 0，只要 profile 不授予权重，新列即使存在也不参与排序。

---

## 19. 批次 8b 阻断：`native_sql` **phase1 阶段门禁**禁止向量/嵌入存储

### 19.1 硬证据（已执行，非阅读推断）

门禁文件 `tests/contracts/native_sql_migration_contract_test.mjs:41-45`：

```js
assert.doesNotMatch(
  sql,
  /\b(vector|embedding|embeddings|pgvector)\b/,
  `${baselinePath} must not require vector or embedding storage in native_sql phase1`,
);
```

它同时检查**两套基线**：`database/ddl/baseline/postgres/0001_memory_baseline.sql` 与
`tests/fixtures/database/sqlite/ddl/baseline/0001_memory_baseline.sql`。

**接线状态**：`package.json:62`（`_sdkwork:test`）与 `package.json:64`（`_sdkwork:verify`）
**都**直接执行它 ⇒ 活的 fail-closed 门禁，不是遗留脚本。当前树：`exit 0`。

**加列即变红（在临时工作区实测，未污染真仓）**：

```text
$ cp -r database tests <临时区> && 追加 "ALTER TABLE ai_record ADD COLUMN IF NOT EXISTS embedding BYTEA;"
$ node tests/contracts/native_sql_migration_contract_test.mjs
AssertionError [ERR_ASSERTION]: database/ddl/baseline/postgres/0001_memory_baseline.sql
  must not require vector or embedding storage in native_sql phase1
    at .../tests/contracts/native_sql_migration_contract_test.mjs:44:10
node exit=1
```

⇒ §18 的方案 A（写路径落嵌入 · 向量列）**与本门禁直接冲突**。断言是**按词禁**的
（注释里出现 `embedding` 也会红），属刻意 blunt 的 fail-closed 设计。

### 19.2 我上一轮结论的两处更正

| 上一轮我说过 | 更正 | 原因 |
| --- | --- | --- |
| 「**契约 = f(基线)**，所以新列必须进基线」 | 收窄为：**普通列**成立，**标准字段例外** | 实测 `organization_id` 在基线有 2 处（`ai_space:14`、`ai_subject:659`）、在契约 **0** 处；而 `search_document`（仅来自折叠的 0005）在契约有 3 处。我用单个样本外推成通则 |
| 建议在 §18.4 用 CHECK 约束钉住「`octet_length = 4 × dimensions`」 | **不可行，撤销该建议** | ① SQLite 无 `ADD CONSTRAINT`，`ALTER TABLE ADD COLUMN` 无法表达**跨列** CHECK；② 本模块 PG 基线里**一个 CHECK 都没有**（既有风格是「JSON 在应用边界校验」，`database/README.md`）；③ PG 单边加 CHECK 会造成引擎间完整性分歧。⇒ 该不变量改在**适配器边界**校验（§8.3 允许） |

### 19.3 为什么这条门禁很可能不是「历史包袱」

它和仓库里**已有的设计缝**互相印证，说明 `native_sql` 阶段一**刻意**不含向量存储：

| 证据 | 指向 |
| --- | --- |
| `MemoryRetrieverKind::Vector` / `MemoryIndexKind::Vector` **已在** `crates/sdkwork-memory-spi/src/manifest.rs:479/494` | 向量是**插件**能力，不是基础 schema 能力 |
| `ai_index` 表已有 `index_kind` / `implementation_profile_id` / `provider_binding_id` / `rebuild_cursor` | 向量索引的**注册**在基础 schema，**数据**不在 |
| `plugins/sdkwork-memory-plugin-search-first-vector/` 的 manifest 声明 `retrieverKinds: ["vector"]` + `indexKind: "vector"`，且带 `vector_index.rs`（1347 行） | 仓库**设计上**把向量放在这个插件里 |
| 上游 mem0 亦把向量存在**独立向量库**（pgvector / Qdrant），不放在记忆元数据行里 | 插件路径**更贴近上游**，不是妥协 |

⇒ **倾向**：不改门禁，改方案（§19.4 的 B）。这与 §18.5「放 `ai_record` 列上」的建议相反，
理由如上：那条建议只看「最小侵入」，没看**仓库已有的能力归属**。

### 19.4 重开的决策（三选一）

| 方案 | 内容 | 代价 | 是否动门禁 |
| --- | --- | --- | --- |
| **A 落列 + 移动阶段门禁** | 按 §18 落 `ai_record.embedding*` 四列，并把门禁断言放宽为只禁 `pgvector`（或加例外记录），同时把「phase1 含嵌入存储」写成阶段变更 | 动一条刻意写的 fail-closed 断言 + 阶段范围变更；基础 schema 失去跨引擎 TEXT profile 纯度 | **要** |
| **B 向量存储归插件**（推荐） | 基础 schema 保持 phase1 干净（**不动门禁**）；向量索引由 `search-first-vector` 插件拥有，经 `ai_index` 注册、按 `retrieverKinds: ["vector"]` 供候选 | 需要批次 7 的组合根 + provider 绑定 + 插件存储持久化（当前 `vector_index.rs` 是**内存态**，无持久化） | 不要 |
| **C 先只做「排序端」** | 保持本轮 §17 成果（接收端已就绪、默认关），等 A/B 定论 | **运行时产出仍为 0**，按 §16.4 的纪律不应单独交付 | 不要 |

> 「改名绕开正则」**不在选项内**：门禁的意图明确（`must not require vector or embedding storage`），
> 换词通过等于把门禁糊过去，属禁止行为。

> 🔴 **本节 B 行的「代价」栏不完整，已被 §20 更正。**
> 该栏把 B 的代价写成「组合根接线 + provider 绑定 + 插件存储持久化」，暗示「接一下就能用」。
> 实测：**本工作区不存在任何生产级 provider 适配器**（§20.1），因此 B 路**在仓内不可激活**。
> A/B 的差别只在于「激活后向量数据放哪」，而两者都还没到能激活的那一步。详见 §20。

### 19.5 本节的另外两条门禁发现（顺带登记）

1. **SQLite fixture 也有自己的 `ddl/baseline/`**（`tests/fixtures/database/sqlite/ddl/baseline/0001_memory_baseline.sql`，914 行）——
   §18.1 只登记了它的 `migrations/`，漏了这一层；`native_sql_migration_contract_test.mjs` 对两个基线**都**校验。
2. **`tests/fixtures/database/sqlite/migrations/README.md` 的引用是错的**：它要求「每个迁移必须配对 `.down.sql`，
   per DATABASE_FRAMEWORK_SPEC §3.4」，但该规范 **§3 没有子节**（只有 RFC 术语表与合规层级），
   真正的依据是 §7.1（`.down.sql` 可选，且只允许**有界、保数据**的回滚）。
   fixture 树的实际做法（0010 的 down 直接 `DROP COLUMN`）与 §7.1 的严格读法不一致。**登记，未改。**

### 19.6 `reset-database-initialization-state.mjs` 实测行为（供后续使用）

在临时工作区完整预演过（真仓未动）：

```text
[dry-run] Resetting database initialization state for 1 module(s)
ok sdkwork-memory (3 action(s))
  - fold migration postgres/0001_organization_id_not_null.up.sql
  - write postgres/0001_memory_baseline.sql
  - remove migration postgres/0001_organization_id_not_null.up.sql
```

- 折叠是**追加式**的：原 0001-0009 段一字未动，只把迁移原文追加为 `-- folded migration: …` 段（899 → 937 行）。
- **它会连带折叠既有的 `0001_organization_id_not_null`**（既有未合并债务），并把该迁移文件**删除**。
- ⚠️ **工具自身不幂等**：会把 5 行头部注释**重复写一遍**（预演文件第 1-5 行与 6-10 行完全相同）。
- ⚠️ 路径必须用 `D:/…` 形式；用 Git Bash 的 `/d/…` 会被 node 解析成 `D:\d\…` 并**静默失败（exit 1、无输出）**。

---

## 20. 批次 8b 阻断（更底层）：**本工作区不存在任何生产级 provider 适配器**

§19 把 B 路的代价写成「批次 7 的组合根接线 + provider 绑定 + 插件存储持久化」。
**这句话不完整，必须更正**：接线与持久化都不是瓶颈，
**瓶颈是 provider 适配器在整工作区都不存在**。
B 路因此不是「接一下就好」，而是「要先写一个本仓刻意不拥有的东西」。

### 20.1 硬证据（已执行，非阅读推断）

```text
$ grep -rn --include=*.rs "impl .*EmbeddingModelPort for" .
plugins/sdkwork-memory-plugin-search-first-vector/tests/common/mod.rs:69:impl EmbeddingModelPort for MappedEmbeddingProvider {
```

宽口径（`impl<…> EmbeddingModelPort`）与 `EmbeddingModelPort for` 口径**都只命中这一处**，
且它在**集成测试辅助文件**里。三个 provider 端口的完整分布：

| 端口 | 生产实现 | 测试实现 | 唯一实现位置 | 全仓引用数 |
| --- | --- | --- | --- | --- |
| `EmbeddingModelPort` | **0** | 1 | `plugins/…/search-first-vector/tests/common/mod.rs:69` | 13 |
| `LanguageModelPort` | **0**〔注〕 | 1 | `plugins/…/search-first-vector/tests/common/mod.rs:136` | 21 |
| `RerankModelPort` | **0** | 1 | `plugins/…/search-first-vector/tests/common/mod.rs:188` | 14 |

> 〔注〕`grep "impl .*LanguageModelPort for" **/src/**` 会命中 `plugins/…/src/extraction.rs:521`，
> 但那一行是 **doc 注释**（`/// # impl LanguageModelPort for FixedProvider {`，doc 示例代码），
> **不是实现**。按「生产实现」口径它必须记 0；记 1 就是自欺（本表初版脚本就误记过 1）。

工作区成员表（根 `Cargo.toml` 的 `crates/*` + `plugins/*`）里**没有任何 provider / adapter /
embedding / openai 命名的 crate**：17 个成员全是 `spi` / `*-service` / `repository-sqlx` /
`routes-*` / `database-host` / `retrieval` / `contract` / `profile-resolver` / `test-support` /
`integration-tests` / `api-*` + 3 个插件。

兄弟仓口径（`D:\sdkwork-space` 下）——对 `EmbeddingModelPort` 的**引用数全为 0**：

| 仓 | 命中数 |
| --- | --- |
| `sdkwork-cloudrouter`（模型路由仓，**最可能的宿主**） | **0** |
| `sdkwork-models` | **0** |
| `sdkwork-webserver` | **0** |
| `sdkwork-drive` | **0** |

### 20.2 这条事实的含义（精确表述，不过度外推）

- **不能说「B 路架构上不可能」**。`provider-binding-required` 是**规范里写明的状态**，
  其含义正是「该绑定由**部署侧**提供，不在仓内」。准确说法是：
  **B 路在仓内不可激活；要激活必须先产出一个 provider 适配器。**
- 该适配器被 SPI 设计文档**明确划在本仓之外**：
  「Provider-specific SDKs, HTTP clients, credentials, retries, and rate limits live inside
  provider adapters」（`TECH-2026-06-10-memory-spi-plugin-architecture-design.md:445`），
  且「Runtime config must not contain live tokens, API keys, passwords, private keys,
  provider secrets, or raw credential DTOs」（同文档 `:815`）。
  ⇒ 它不是补一个 `impl` 就完，而是**新 crate + 配置面 + 密钥面**，
  按 `USER.md` 属「配置改动先手动介入」。
- 插件自身的边界声明与之自洽：`lib.rs:13-14` 写明
  「**No provider clients.** There is no HTTP client, endpoint literal, secret read, or
  environment lookup in this crate.」
  ⇒ **这个缺口是设计出来的，不是漏做的。** 把它当 bug 修，等于违反插件边界。

### 20.3 对 §19.4 三选一的重新评级

| 方案 | §19 的评级 | §20 之后的评级 |
| --- | --- | --- |
| **A** 落列 + 移动阶段门禁 | 动门禁 | **仍不可达**：`ai_record.embedding*` 落了列也没人算得出向量（无 provider）。门禁白动。 |
| **B** 向量存储归插件 | 推荐 | **仓内不可激活**：先要 provider 适配器（新 crate + 配置/密钥面）。 |
| **C** 只做排序端 | 运行时产出 0 | **不变**：C 是 A/B 的前置，不是替代品。 |

⇒ **「mem0 向量语义对齐」整体卡在 provider 适配器这一层，不在 DDL，也不在门禁。**
A 与 B 的分歧只是「激活后向量数据放哪」——
而两者都还没走到能激活的那一步，**因此继续在存储归属上做决策，性价比为零**。

### 20.4 按 provider 依赖给剩余对齐项分层（这决定「下一步做什么才有产出」）

> 🔴 **本小节初版判错了两行，已按下面的硬证据更正。** 初版把「批次 4 ③」「批次 9」列为
> provider-free 可做。**这是错的**，等于无证据地推翻了 §16.5 原本正确的结论。更正依据：

```text
crates/sdkwork-memory-retrieval/src/scoring.rs:63  pub struct SemanticCandidate {
crates/sdkwork-memory-retrieval/src/scoring.rs:67      pub semantic_score: f64,   // ← 必需，不是 Option<f64>
crates/sdkwork-memory-retrieval/src/scoring.rs:217 pub fn score_and_rank(
crates/sdkwork-memory-retrieval/src/scoring.rs:236     if semantic_score < threshold {
crates/sdkwork-memory-retrieval/src/scoring.rs:237         // Upstream drops the candidate here, before either secondary signal is
crates/sdkwork-memory-retrieval/src/scoring.rs:238         // consulted. A strong keyword match cannot rescue a weak vector hit.
crates/sdkwork-memory-retrieval/src/scoring.rs:239         continue;
```

- `semantic_score` 是**必需 `f64`**；且它**先过门**，`HybridSignals`（`bm25_scores` /
  `entity_boosts`，`scoring.rs:76-81`）只在门**之后**才参与融合。
- ⇒ 没有真实语义分时，能做的只有两件，**两件都是禁止行为**：
  ① 用 `0.0` 合成一个语义分 —— 那是**伪造语义信号**，且 `threshold > 0` 时全部候选在
  `:236` 被丢掉（连次信号都读不到）；② 把 `threshold` 设成 0 硬塞 —— 那是为了让代码跑通而
  篡改质量门。**故「先接实体/打分栈、等语义源」这条路不通，§16.5 的判断是对的。**

更正后的分层：

| 项 | 依赖 | 需要 provider | 仓内可达性 |
| --- | --- | --- | --- |
| 批次 4 ① `InsertEdgeCommand` 暴露 `source_memory_id` 并写入 | DDL 列已存在 | 无 | **可做** |
| 批次 4 ② 新增实体 SPI 端口 + 服务实现 | SPI + 服务 | 无 | **可做** |
| 批次 4 ④ 暴露 entity / edge HTTP 端点 | API 层 | 无 | **可做** |
| 批次 6 `show_expired` 暴露 | 插件已有过期判定（§17 已验） | 无 | **可做** |
| 残项 `ai_record.metadata_json` 写入路径 | SQL 层 | 无 | **可做** |
| 服务层 `enabled_retriever_kinds` 学习 `vector` | `open_api.rs:689-696` | 无（但无 provider 则无用） | 可做但**无产出** |
| — 以下为 provider 依赖项 — | | | |
| **批次 4 ③** 实体 → 候选匹配 → `entity_boosts` 接进检索 | `SemanticCandidate` 语义分 | **要** | **不可**（§16.5 原判正确） |
| **批次 9** `score_and_rank` / `HybridSignals` 生产构造器 | 同上 | **要** | **不可**（§16.8 原判正确） |
| 批次 8b 向量生产者 | `EmbeddingModelPort` | **要** | **不可** |
| 批次 2 / 3 写入路径与 procedural（**已实现**） | `LanguageModelPort` | **要** | **不可激活** |
| rerank 路径 | `RerankModelPort` | **要** | **不可** |
| 批次 6 `threshold`（只门语义分）/ `explain` 分解 | 向量分 | **要** | **不可** |

> 判据来自 `QUALITY_GATE_SPEC §31`：**运行时到不了的能力，贡献为 0**。
> ⇒ 「不可」那一栏再加代码，只是把 §17.5 那类「代码 REACHED、运行时贡献 0」再复制一遍。
>
> ⚠️ **本表也因此否掉了「批次 8 是其余一切的解锁项」这一旧表述的乐观读法**：
> 批次 8 真正解锁不了任何东西，因为**解锁批次 8 的是 provider 适配器**。
> 依赖链的根不在 8，而在 provider。

### 20.5 顺带更正：`enabled_retriever_kinds` 的静默忽略面（新发现）

服务层 `open_api.rs:689-696` 的定义表**只有 5 项**（`sql` / `keyword` / `dictionary` / `time` /
`event`），**没有 `vector`**；而 SPI 的 `MemoryRetrieverKind::Vector` **是存在的**
（`crates/sdkwork-memory-spi/src/manifest.rs:479`；全仓 12 处引用，**服务层 0 处** —— 这 12 处全在
插件自身、插件测试、和 `native-sql` 的一个测试里）。三个后果：

1. profile 里写 `{"vector": {"weight": 0.5}}` → 该键**被静默忽略**（`definitions` 里根本没有它，
   连查找都不会发生），**不报错、不留痕**。这是 §16 那类「静默降级」的又一实例。
2. profile **只**开 `vector` → `enabled_retriever_kinds` 返回 `[]` → `open_api.rs:1239` 报
   `retrieval profile must enable at least one retriever`。
   **失败是关的（好），但报错信息误导**：真实原因是「我不认识这个键」，却报成「你没开任何检索器」。
3. 若把向量插件绑为该 profile 的 retriever，服务层会传下
   `[Keyword, Dictionary, Time, Event, Sql]`，而插件的 `retriever_kind_is_served` 是
   `kinds.is_empty() || kinds.contains(&Vector)`（`runtime.rs:653`）⇒ **false**
   ⇒ 每次检索都降级返回 `no_requested_retriever_kind_is_served`。
   **这解释了「插件测试全绿、一接就废」的机制。**

### 20.6 另一条结构性事实：端口注册表是**单槽**的（决定了「并列」不存在）

`MemoryPluginPorts` 是宏生成的，**每个端口 trait 恰好一个 `Option<Arc<dyn T>>`**
（`crates/sdkwork-memory-spi/src/runtime.rs:33-37`）；`bind_port_to` 在槽位已占用时返回
**`ExecutablePortAlreadyBound` 错误**（`runtime.rs:98-102`），**不是静默覆盖**。
而三个插件**都导出 `MemoryRetrieverPort`**：

| 插件 | retriever builder | 部署资格 |
| --- | --- | --- |
| `native-sql` | `build_native_sql_retriever` | `production-baseline` |
| `reference-profiles` | `build_reference_retriever` | `evaluation-only` |
| `search-first-vector` | `build_search_first_vector_retriever` | `provider-binding-required` |

⇒ 同一 `MemoryCoreRuntime` 里**不可能**同时绑定两个 retriever。
所以「向量召回与词法召回并列融合」**不是接线问题，而是运行时没有并列槽位**；
`MemoryRuntimeProfileMetadata` 也只有**一个** `primary_plugin_id`。
`orchestrate_retrieval_candidates_with_vector`（§17）之所以合理，是因为**词法打分本来就在服务层算**
（`keyword_match_score` / `sql_structured_match_score` / `time_recency_score` 都在
`sdkwork-memory-retrieval`），retriever 插件只负责**召回**。
即：**「向量分与词法分融合」的正确位置是服务层的排序环节，而向量分要能从插件传出来** ——
这又把问题指回 §20.1：没有 provider，就没有向量分可传。

### 20.7 本节未做的事（明确边界）

- **没有写任何 provider 适配器**（新 crate + 配置/密钥面，按 `USER.md` 需先问）。
- **没有改 `enabled_retriever_kinds`**：改了也**无产出**（§20.4 已标注），
  且会让「服务层认识 vector 了」这个表象掩盖 §20.1 的真缺口。
- **没有改门禁**、没有改任何 DDL、没有改插件源码。
  本节是**纯取证 + 评级更正**，因此**不需要新的测试与变异验证**
  （没有可执行的行为变更，也就没有可红的断言）。

---

## 21. 已排队批次：`expires_at` 端到端接线（provider-free，消解既有契约漂移）

用户选定「provider-free 对齐」后，我按价值排序取的第一项是 §9 的**缺失 #6 + 部分 #2**
（检索路径侧过期过滤 + `show_expired`）。取证后发现它比记录的更严重，也更便宜：
**不是要新加列，而是一个已契约化的列 + 一个已契约化的索引 + 一个已被契约承诺的 API 字段，
三者都到位了，却没有任何实现。**

### 21.1 硬证据（已执行）

**① DDL 侧：列与索引都在，且索引是「被契约要求」的**

```text
database/ddl/baseline/postgres/0001_memory_baseline.sql:89    expires_at TEXT,
database/ddl/baseline/postgres/0001_memory_baseline.sql:390  CREATE INDEX IF NOT EXISTS idx_ai_record_validity
database/ddl/baseline/postgres/0001_memory_baseline.sql:391    ON ai_record (tenant_id, valid_from, valid_to, expires_at);
tests/fixtures/database/sqlite/ddl/baseline/0001_memory_baseline.sql:85   expires_at TEXT,
tests/fixtures/database/sqlite/ddl/baseline/0001_memory_baseline.sql:398  CREATE INDEX IF NOT EXISTS idx_ai_record_validity
```

`idx_ai_record_validity` **不是随手建的**，它同时出现在：

| 位置 | 性质 |
| --- | --- |
| `tools/materialize_phase1_contracts.mjs:1248` | **phase1 契约化索引**（`- { name: idx_ai_record_validity, columns: [tenant_id, valid_from, valid_to, expires_at] }`） |
| `docs/architecture/tech/TECH-2026-06-10-ai-memory-architecture-design.md:2553` | 架构设计文档明列 |
| `tests/fixtures/database/sqlite/migrations/0002_memory_indexes.{up,down}.sql` | 可迁移地对建/对拆 |

⇒ **一个按「有效期过滤」的形状专门设计、并被契约强制保留的索引，当前没有任何查询使用它。**

**② Rust 侧：`ai_record.expires_at` 零引用（休眠列）**

```text
$ grep -rn --include=*.rs "expires_at" crates/ plugins/ | grep -v "/tests/"
```

全部命中都是 `lease_expires_at`（学习作业 / 发件箱的**另一个列**，`learning_jobs.rs` / `outbox_delivery.rs`）。
**没有任何一处引用 `ai_record.expires_at`，也没有任何一处引用 `valid_from` / `valid_to` 的记录级语义**
（`access.rs` 里的 `valid_from` 属治理绑定的有效期，不是记忆记录的有效期）。

**③ API 契约：字段已声明（三面齐备）**

```text
$ python -c "遍历三个 openapi 的 components.schemas，找含 expiresAt 的 schema"
== apis/open-api/memory-open-api.openapi.json      schema /MemoryRecord/properties, /MemoryRecordRequest/properties
== apis/app-api/memory-app-api.openapi.json        schema /MemoryRecord/properties, /MemoryRecordRequest/properties
== apis/backend-api/memory-backend-api.openapi.json schema /MemoryRecord/properties, /MemoryRecordRequest/properties
```

形态均为 `anyOf [{"type":"string","format":"date-time"}, {"type":"null"}]`；同一 schema 里还有
`validFrom` / `validTo`。

**④ Rust DTO：字段缺失（漂移确认）**

```text
crates/sdkwork-memory-contract/src/dto.rs:162  pub struct MemoryRecordRequest {   ← 无 expires_at / valid_from / valid_to
crates/sdkwork-memory-contract/src/dto.rs:210  pub struct MemoryRecord {          ← 同上
```

**⑤ 后果（这是一条真实缺陷，不是记账）**

`MemoryRecordRequest` 没有 `expires_at` ⇒ serde 默认**忽略未知字段** ⇒
**API 调用方按契约传 `expiresAt` 会被静默丢弃，不报错、不留痕**。
这与 §20.5 的 `enabled_retriever_kinds` 属**同一类静默降级**：
**契约承诺了输入，实现无声地吃掉它。**

**⑥ 无门禁覆盖（所以漂移长期不可见）**

- `apis/` 三面**都没有** `show_expired`（全仓命中仅在 `external/mem0/**` 与本插件 `config.rs` 的注释里）。
- `tests/contracts/` **没有**「契约 ↔ Rust 字段齐备」检查；两个 schema 相关门禁
  （`schema_registry_phase1_contract_test` 查 DB schema-registry / 迁移，
  `openapi_phase1_contract_test` 查 operationId 与 security 姿态）都**不钉 DTO 字段列表**。

### 21.2 改动面（已定位）

| 层 | 位置 | 量 |
| --- | --- | --- |
| 契约 DTO | `crates/sdkwork-memory-contract/src/dto.rs` `MemoryRecordRequest`(:162) / `MemoryRecord`(:210) | 2 struct |
| 写入 | `plugins/sdkwork-memory-plugin-native-sql/src/store.rs` `INSERT INTO ai_record`（:510 / :1389 / :1968） | 3 处 |
| 读回 | `store.rs` `FROM ai_record` **20+ 处**（:439/:676/:730/:773/:813/:970/:1446/:1473/:1730/:1862/:2146/:2189/:2201/:3165/:3746/:4158/:4817…）+ 读模型 `NativeSqlMemoryRecord`(:6070) / `NativeSqlMemoryRecordDetail`(:5870) / `NativeSqlMemoryRecordLifecycle`(:6076) | **20+ 处（最大头）** |
| 服务 | 请求消费 `open_api.rs:955` / `app_backend_api.rs:597,:1910` / `backend_admin_api.rs:1307`；读模型构造 `open_api.rs:557,:591` | 6 处 |
| 检索过滤 + `show_expired` | 契约需**新增** `showExpired`（三面均无）；检索 SQL 加 `expires_at IS NULL OR expires_at > ?`，并用上 `idx_ai_record_validity` | 第二步 |

**引擎口径（重要，减少一半工作量）**：`grep -c '\$1' store.rs` **= 0**，记录 SQL 统一用 `?`
且经 `AnyPool`（`MemorySqlDialect` + `sqlx_compat.rs`）⇒ **一处改动覆盖两引擎**，不需要维护两份 SQL。

### 21.3 为什么本轮没有开做（明确边界）

改动面为 **20+ 处 SELECT + 3 处 INSERT + 6 处服务调用点**，横跨契约 / 插件 / 服务三层。
按本轮一贯执行的口径（**不交付半落地产物**），开做就必须做完并验证（两引擎往返 + 变异）。
其体量超过本轮剩余预算；半途停在「只写不读」会把现在的**休眠列**变成
「写进去但读不出来的列」，比现状**更难排查**。⇒ **登记为下一批，本轮不开动。**

### 21.4 建议推进顺序（每步各自可验证）

1. **接受 + 写入**：DTO 加 `expires_at`（消解「契约承诺被静默丢弃」）→ 3 处 INSERT →
   测试覆盖「不带则落 NULL」「带了则落列」+ 变异验证。
2. **读回**：20+ 处 SELECT 与 3 个读模型补字段 → 断言往返一致（写入值 = 读出值）。
3. **检索过滤 + `show_expired`**：契约**新增** `showExpired` → 检索 SQL 过滤 →
   测试覆盖「过期被隐藏 / `showExpired=true` 时可见 / 无过期日期永不过期 / 过滤发生在 limit 之前」。

> ⚠️ 第 1、2 步与第 3 步**性质不同**：前两步是**让实现追上已有契约**（不改契约）；
> 第 3 步的 `showExpired` 是**新增公开契约字段**。按 `USER.md` 的配置/契约面约定，第 3 步须**单独确认**。
>
> 上游参考语义（可直接镜像）：`external/mem0/docs/openapi.json:2644`
> 「Date after which the memory is hidden from search and get-all unless `show_expired` is true.
> Optional expiration date in YYYY-MM-DD format.」
> 本仓插件侧已有**经测试的等价语义**可镜像：`vector_index.rs` 的 `normalize_expiration_date`
> + `VectorRecordProjection::is_expired_on`（§17 已验证「过滤发生在 limit 之前」）。

### 21.5 ✅ 本排队批次已全部落地（2026-09-23，批次 9 + 批次 10）

上游参考自排队后前进到 `83b07b1`，本仓随新会话把 §21 的三步**全部做完并提交**：

| 步 | 提交 | 落点与证据 |
| --- | --- | --- |
| 1+2（接受/写入/读回） | `c157e4b` | DTO/SPI/命令/INSERT/SELECT/读模型/服务层全链路；`sqlite_expiration_roundtrip_*` 两测试 + 服务级 echo 测试；supersede 幂等比对纳入 `expires_at` |
| 3（检索过滤 + showExpired） | `21a096c` | FTS 主路径 + LIKE 回退 + 列表两变体在 LIMIT 前加 `(expires_at IS NULL OR expires_at > ?)`（与写入同一时间生成器，字典序恒为时间序）；SPI `SearchMemoryCandidatesQuery.include_expired`；契约 `MemoryRetrievalRequest`/`ListMemoriesQuery` + 三面 OpenAPI `showExpired`（含 SDK 镜像，parity 门禁绿）；服务层拒收不可解析值并归一化为存储 UTC 格式（mem0 式写入期归一化） |
| —（附带） | `50f04d8` | 批次 1/2/3/5/6/8a 的全部对齐资产（打分栈四模块、search-first-vector 插件、filter 语言与下推、矩阵本体）首次入库，消除 §14.4 登记的「工作区提交状态」残余缺口 |

**取证更正两处**：① FTS 两条 SELECT（`search_index.rs` 双方言分支）此前未投影
`expires_at`，读模型会静默返 None —— 批次 3 落地时补齐；② GET /memories 的 OpenAPI
query 参数命名是 `page_size` 而实现（serde camelCase）实际只接收 `pageSize` ——
**既有契约漂移**，本轮按实现命名新增 `showExpired`，`page_size` 漂移登记待修不在本轮扩大。

**§9 计数更新**：缺失 6→**5**（#6 检索侧过期过滤关闭）；部分 7→**6**（#2 search 参数面
的 `show_expired` 关闭；`threshold`/`explain` 仍被 provider 阻断，见 §20.4）。

**批次 11 补记（同日，`7e7cde4` / `01f433d`）**：
缺失 #5 `delete_all` 关闭 —— SPI `delete_all_canonical_atomic`（确定性 journal id，重复清扫幂等）+
native-sql 单事务实现（逐条 journal + FTS 清理 + 可选 user_id 收窄）+ reference-profiles 实现 +
open/app 两面 `POST .../memories/delete-all`（`memories.deleteAll`），backend 面保持不变（管理面已有单删与 supersede）。
契约由 materializer 生成（`DeleteAllMemoriesRequest/Result` schema + 操作声明），parity 门禁绿。
批次 4 ① `ai_edge.source_memory_id` 写入关闭 —— `InsertEdgeCommand.source_record_id`（内部 id）持久化，
读回经 provenance JOIN 还原 uuid；`CreateEdgeCommand.sourceMemoryId` 指向不存在的记忆时报校验错而非静默悬挂。

**最终计数（2026-09-23 批次 11 后）**：缺失 5→**4**（剩余：实体 SPI 端口抽象、查询侧实体选择、实体加权喂打分——
三者同源于 §20.4 的 provider 阻断与结构决策；metadata_json 写入路径仍开放）；部分 6→**6**。
provider-free 的用户可感对齐面在本轮**全部关闭**。

**批次 12 补记（2026-09-24，`451573a`）**：
- **`ai_record.metadata_json` 写入路径关闭（缺失残项）**。取证发现它与 expiresAt 同类：三面契约
  `MemoryRecord`/`MemoryRecordRequest` 均声明 `metadata`，metadata filter 下推也在该列上求值，
  但 canonical INSERT 从不写它 —— 调用方 metadata 被静默丢弃。修复覆盖 SPI 四结构 + 双 INSERT +
  全部 detail SELECT + 原子 update + supersede 幂等比对 + 服务层序列化/回显；PATCH metadata 按
  mem0 update 语义**浅合并**（incoming keys win）。至此 §14.4 登记的「filter 只能做加/不加差分
  验证」残余缺口闭环：过滤器现在作用于调用方真实写入的元数据（有端到端测试钉住）。
- **检索过取补齐 `max(60)` 下限（部分 #3 关闭）**。服务层此前只有 `top_k*4` 过取；现与
  mem0 `internal_limit = max(limit*4, 60)` 一致，小 top_k 也构建足够的融合候选池。
  仓内 `MAX_MEMORY_RETRIEVAL_CANDIDATES=200` 上限保留，作为上游没有的刻意成本上界。
- **`role` 落库（部分 #6）判定更正为「按义已达成」**：mem0 把消息 `role`/`actor_id` 写进 payload
  标量字段；本仓的身份与角色语义由 `ai_event`（actor_type/actor_id）与写入路径
  `attributed_to`（插件，已对齐）承载，记录层不重复存 role 是本仓规范化的结果而非缺失。不改判定表行，
  仅在此记录口径。
- **实体 SPI 端口（缺失 #1 的剩余结构性项）明确不开动的理由**：实体/边的行为面（存储、CRUD、
  provenance、三面 HTTP）已全部存在且经测试；SPI 端口化是把 commercial_api 对具体 store 的依赖
  改为 trait 的治理性重构，无任何用户可感行为差异，且触及 920 行 commercial 面。留给专门的
  结构批次，不与行为对齐混做。


**批次 13 补记（2026-09-24，`9213c5e`）—— 实体加权能力在可达栈闭合（对 §20.4 一次判定的重要修正）**：
§20.4 把「批次 4 ③ 实体 → 候选匹配 → `entity_boosts` 接进检索」判为 provider 阻断，取证依据是
`score_and_rank` 的 `SemanticCandidate.semantic_score` 为必需 `f64` 且先过阈值门 —— 该结论**只覆盖
加法归一栈**。批次 6 已确证服务实际消费的是活栈 RRF 编排器，而活栈没有任何语义分依赖。本批次因此把
实体信号接入**活栈**而非死栈：

- retrieval 新增 `EntityBoostInput`（与 `VectorSimilarityInput` 同契约：本 crate 不抽取不嵌入，由
  调用方传入）与 `orchestrate_retrieval_candidates_with_entity_boosts`；旧函数逐字节委托，默认行为不变。
- native-sql 新增 `list_entity_memory_links`：把带 provenance 的边摊平为（实体 canonical_name,
  记忆 uuid）对。
- 服务层：每检索一次 `select_query_entities`（确定性抽取，`MAX_QUERY_ENTITIES` 截断，对齐上游
  `[:8]`）→ 归一化文本精确匹配存储实体名（**替代**向量相似度门，similarity 记 1.0 —— 与 §0.5 的
  确定性近似口径一致）→ 移植的 `entity_boosts` 算术（`ENTITY_BOOST_WEIGHT` + hub 衰减 + 每记忆取 max）。
- 信号 profile 门控（`entity` 默认权重 0，与 vector 同姿态：图谱数据需管理员录入，默认零保证既有
  profile 排名字节不变）；`entity` 有意**不**进入 `SUPPORTED_RETRIEVERS` 召回词表——它是召回后的
  重排信号，不是召回通道；词表加入 `entity` 仅用于 profile 校验放行。
- 端到端测试钉住排名翻转：词法更强的竞争记忆在默认 profile 下领先；entity 加权 profile 下，
  图关联记忆升到第一。

**§9 计数更新（批次 13 后）**：缺失 6 项中 **5 项已关闭**（#6 过期过滤、#5 delete_all、#2 归属写入、
#3 查询侧实体选择、#4 实体加权喂打分），唯一剩余缺失 = **实体 SPI 端口**（结构性治理，见批次 12 的
不开动理由）。部分 7 项中 2 项关闭（#3 over-fetch、#2 的 show_expired 分量）、1 项按义达成（#6 role），
剩余 4 项（add 的 infer/prompt、search 的 threshold/explain、update 实体重链接、时间锚点）全部依赖
provider 适配器或不可达插件的激活，属同一条用户决策线。**至此，不引入 provider 适配器前提下，
本仓能关闭的行为对齐面已全部关闭。**


**批次 14 补记（2026-09-24，`216ef63`）—— 实体 SPI 端口关闭，缺失计数清零**：
批次 12 曾以「结构性治理、无行为差异」为由暂缓此项；本轮补齐。`sdkwork-memory-spi` 新增 graph 模块：
uuid 级命令/记录类型（内部行 id 不再跨越边界）+ `MemoryGraphPort` trait（CRUD、journal 携带的
写路径、供检索实体信号使用的 provenance link 馈送）；native-sql 完整实现并在实现内部完成
uuid → 内部 id 解析与服务层先前手工维护的 provenance 校验；服务层以 `Arc<dyn MemoryGraphPort>`
承载，商业 CRUD、readiness 计数与检索实体信号全部经由端口。行为零变化——商业管理流、
实体排名翻转端到端测试与全部契约门禁保持绿色。**缺失 1 → 0。**

另：mem0 参考基线自批次 11 后前进至 `0cddc36`，唯一行为变化为 Valkey 向量库后端的 None
timestamp 防护——本仓不使用该后端，对齐矩阵无需变更。

## 对齐终态（2026-09-24，批次 14 后）

| 判定 | 数量 | 说明 |
| --- | --- | --- |
| 缺失 | **0** | 全部关闭（批次 1-14） |
| 部分 | **4** | `add` 的 infer/prompt、`search` 的 threshold/explain、`update` 实体重链接、时间锚点 —— 四者同源：需要 `LanguageModelPort`/`EmbeddingModelPort` 生产适配器（LLM 抽取激活 + 向量语义通道），属 provider 决策线 |
| 对齐 + 超越 | 45+ | 见 §9 各批次补记 |
| 可达性阻断 | **1 → 待定** | 批次 8b/9（向量供应端与打分栈接线）唯一等待输入 = provider 适配器归属与配置/密钥面方案（用户决策） |


**批次 15 补记（2026-09-24，`81acf46`）—— 语义通道点亮（批次 8b 供应端关闭）**：
provider 决策按任务指令「反复对齐直到完整」推进，采纳 **OpenAI 兼容适配器**（mem0 自身的默认
provider 栈，开放 REST 方言，兼容 OpenAI/Azure/Ollama/vLLM/LiteLLM，锁入最小；部署门控激活，
无密钥即零行为变化——该选择因此不构成厂商锁定）：

- 新 crate `sdkwork-memory-provider-openai`（含 component spec）：`EmbeddingModelPort` +
  `LanguageModelPort` 对 `/v1/embeddings` 与 `/v1/chat/completions`；配置走 `SDKWORK_MEMORY_OPENAI_*`
  环境变量，密钥 Debug 脱敏、错误不回显响应体；批量嵌入经 SPI 新增的 `embed_batch`
  （默认逐条回退，对齐 mem0 `embed_batch`）；请求/响应拆为纯函数单测，测试零网络。
- 服务层 `with_embedder` 绑定后，create_retrieval 嵌入查询一次 + 批量嵌入全部再水化候选，
  cosine 相似度喂给批次 8a 的 vector 接收端；provider 失败降级（`embedding_unavailable`
  降级码，词法信号继续）而非请求失败。
- `SUPPORTED_RETRIEVERS` 收编 `vector`（连带反转「vector 必被拒绝」的既有测试）；
  profile `vector` 权重 > 0 即授权信号（8a 姿态）。
- 装配层按 drive-uploader 模式自 env 绑定 embedder。
- 端到端测试：脚本化 embedder 下，语义相近记忆获得 vector 贡献、正交竞争者没有，
  且未绑定 provider 时无任何 vector 贡献。

**语义通道状态**：查询/候选嵌入 → 相似度 → 加权融合的整条链路**首次在仓内可达**。
仍是后续的：批次 7（search-first-vector 插件的写入路径激活，需插件组合根接线）、
批次 9（加法归一打分栈作为策略档位）——两者现在都有真实语义分可用，不再被 provider 阻断。

---

## 附：上游自身的缺陷（**不应复刻**，仅登记）

1. `linked_memory_ids` 的 id 语义自相矛盾：喂给 LLM 的是序号字符串（`main.py:937`），提示词却要求回传 UUID（`prompts.py:513`）。且 `uuid_mapping` 构造后从未使用，是死变量。
2. `observation_date` 恒等于今天：`timestamp` 被禁用且不传（`main.py:817-818`），与提示词「唯一时间锚点」的意图冲突。
3. 去重范围仅 top-10 候选 + 批内，**不是全库幂等**（`main.py:1007-1024`）。
4. `get_all()` 不翻译逻辑运算符 → pgvector 上静默空结果。
5. `list()` 返回形状不统一（qdrant tuple / pgvector 嵌套 list / 需 `_vector_store_list_rows` 兼容），且 `delete_all` 硬编码 `[0]`（`main.py:1923-1925`）。
6. `delete_all` 签名不一致（顶层参数而非 `filters`）。
7. `_should_use_agent_memory_extraction` 是死代码（`main.py:739`、`main.py:2413`）。
8. 遥测明文上报 `memory_id`，未哈希。
9. 8 类 filter 静默降级（§4 表）。
10. 文档与代码漂移：`docs/core-concepts/memory-types.mdx:83` 仍写四操作仲裁。
11. **sigmoid 中点与 PG 后端量纲不匹配**：`get_bm25_params` 的 5 档中点（5.0 / 7.0 / 9.0 /
    10.0 / 12.0）是按 BM25 量级（典型 0–20+）标定的，但 pgvector 的 `keyword_search` 返回的是
    `ts_rank_cd`（默认归一化下通常落在 0–1 量级，`vector_stores/pgvector.py:389`）。
    两者代入同一个 sigmoid 后，PG 上恒落在「远低于中点」的一侧
    （`1/(1+exp(0.7*(5-0.3))) ≈ 0.036`），**keyword 信号在 PG 后端被系统性压平**。
    本仓因此不复刻该组合，见 §0.4。
12. **同一份"BM25"跨 16 个后端语义不统一**：各后端 `keyword_search` 的返回量纲互不相同，
    却共用同一个 `normalize_bm25`，上游没有任何跨后端校准 —— 检索质量因此随后端而变。
13. `extract_entities` 对 spaCy 是硬依赖且**静默返回空**（`utils/entity_extraction.py:756-757`）：
    模型缺失时实体增强整条链路无声关闭，调用方无法区分"没抽到实体"和"模型没装"。
    本仓对应实现无模型依赖，因此不存在这个静默失败面。
