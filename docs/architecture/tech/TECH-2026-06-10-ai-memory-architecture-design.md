> Status: archived design record. It is not current capability or release authority; use `TECH_ARCHITECTURE.md`.
>
> Migrated from `docs/superpowers/specs/2026-06-10-ai-memory-architecture-design.md` on 2026-06-24.
> Owner: SDKWork maintainers

Date: 2026-06-10
Status: Draft for review

## 1. Purpose

This document defines a complete, evolvable architecture for an AI memory module. The module must support self-learning memory, habit-forming memory, multi-scope long-term memory, provider-agnostic model integration, and retrieval without requiring embeddings.

The design is intended to start with a practical no-embedding MVP and grow into a final platform-grade memory service with optional semantic search, graph memory, file/code memory, temporal facts, evaluation, and governance.

## 2. Core Positioning

The memory module is a stateful infrastructure service for AI agents and AI applications. It is not a vector database, not a chat-history table, and not a prompt-summary utility.

The central concept is:

```text
Canonical memory records are the source of truth.
Indexes are optional, replaceable, and rebuildable.
Retrieval is multi-channel.
Context assembly is policy-aware.
Learning is evidence-based and auditable.
```

Embeddings are useful, but optional. A strong memory system must also work through SQL filters, keyword search, dictionaries, grep/file search, graph relations, time indexes, and event replay.

## 3. Goals

1. Provide long-term memory for users, projects, organizations, agents, and global platform knowledge.
2. Support self-learning from conversations, tool calls, files, user feedback, task outcomes, and business events.
3. Support habit-forming memory that evolves from repeated evidence instead of one-shot assumptions.
4. Support retrieval without embeddings through SQL, keyword, dictionary, graph, grep/file, time, and event retrievers.
5. Support optional embeddings, rerankers, and LLMs through standard provider-agnostic interfaces.
6. Preserve source evidence for every memory so memories can be explained, corrected, deleted, replayed, and reprocessed.
7. Support temporal validity, supersession, confidence, importance, sensitivity, and access scope.
8. Provide safe user and tenant controls: view, edit, delete, export, disable learning, audit access, and enforce privacy policies.
9. Provide evaluation and observability so memory quality can be measured before and after model, prompt, or retriever changes.
10. Allow staged delivery from MVP to final architecture without reworking the core model.

## 4. Non-Goals

1. The first version will not fine-tune model weights.
2. The first version will not require embeddings or a vector database.
3. The first version will not require a graph database.
4. The memory service will not treat raw conversation history as final memory.
5. The memory service will not allow LLM-generated memories to become stable facts without policy, source, confidence, and lifecycle checks.
6. The memory service will not store secrets such as passwords, tokens, API keys, or private credentials as normal memories.

## 5. Design Principles

### 5.1 Canonical First

The canonical memory store is the only authoritative memory state. Search engines, vector stores, graph stores, file indexes, and caches are derived indexes.

If an index is lost, stale, or migrated, it must be rebuildable from canonical records and event evidence.

### 5.2 Embedding Optional

Embedding is a retriever plugin, not a system dependency. The system must run with:

```text
SQL + keyword + dictionary + time + event retrievers
```

and later add:

```text
vector + rerank + graph + grep/file retrievers
```

### 5.3 Evidence-Based Learning

Every learned memory must link to source events. The system learns by accumulating evidence and updating memory lifecycle state.

### 5.4 State Machine Over One-Shot Summaries

Self-learning and habit-forming memory should use lifecycle transitions:

```text
observed -> candidate -> tentative -> confirmed -> stable
                                      -> rejected
                                      -> superseded
                                      -> expired
                                      -> deleted
```

### 5.5 Scoping Is Mandatory

Every memory must have explicit scope:

```text
tenant, user, project, organization, agent, global
```

No retrieval path may bypass scope and access policy checks.

### 5.6 Temporal Validity

Facts change. Memory records must support `valid_from`, `valid_to`, `expires_at`, and `supersedes` relationships. Old records should usually be superseded rather than physically removed, except when deletion is required.

### 5.7 Explainability

Retrieval results and assembled context must explain why a memory was used:

```text
source evidence, retriever, match reason, score, confidence, freshness, scope
```

### 5.8 Provider Agnostic

The memory service must define internal interfaces for language models, embeddings, rerankers, tokenizers, and extraction policies. Vendor APIs are adapter details.

## 6. Memory Types

### 6.1 Working Memory

Short-lived state for the current request or tool chain. It is not persisted as long-term memory unless explicitly promoted.

Examples:

```text
current plan step, temporary parsed query, intermediate tool result
```

### 6.2 Session Memory

Short or medium-term summary and state for one conversation session.

Examples:

```text
current task goal, unresolved user question, latest decision in this session
```

### 6.3 Semantic Memory

Stable facts and preferences.

Examples:

```text
user prefers Chinese technical answers
project uses Spring Boot
tenant requires audit logging
```

### 6.4 Episodic Memory

Specific past events and outcomes.

Examples:

```text
on 2026-06-10 login failure was caused by Redis token TTL
user rejected approach A and chose approach B
```

### 6.5 Procedural Memory

Reusable process knowledge and learned ways of doing tasks.

Examples:

```text
when debugging SDKWork login, check token lifecycle, Redis session, env config, and SDK base URL
for architecture work, produce design doc before implementation
```

### 6.6 Habit Memory

Inferred or confirmed recurring preferences and behavior patterns.

Examples:

```text
for architecture discussions, user prefers detailed Chinese responses with implementation steps
when discussing code, user usually expects direct edits rather than high-level advice
```

### 6.7 Relationship Memory

Entity and relation facts, optionally temporal.

Examples:

```text
user U works on project P
project P depends on service S
fact F was superseded by fact G
procedure X solved incident Y
```

### 6.8 Domain Knowledge Memory

Knowledge imported from documents, APIs, manuals, repositories, or business systems. This is close to RAG, but must still obey memory scope, source, version, and freshness rules when used as agent memory.

## 7. High-Level Architecture

```text
Applications / Agents / SDKs
  |
  v
Memory API Gateway
  |
  +-- Event Collector
  |     stores raw conversations, tool calls, file events, feedback, and business events
  |
  +-- Learning Engine
  |     extracts candidate memories and learning signals
  |
  +-- Policy Engine
  |     checks scope, sensitivity, retention, tenant policy, and user consent
  |
  +-- Consolidation Engine
  |     deduplicates, merges, supersedes, rejects, or promotes memories
  |
  +-- Habit Engine
  |     forms and decays habit memories from repeated evidence and feedback
  |
  +-- Canonical Memory Store
  |     stores authoritative memory records, events, entities, edges, versions, and audit logs
  |
  +-- Index Manager
  |     builds and repairs SQL, keyword, dictionary, time, vector, graph, and file indexes
  |
  +-- Retriever Registry
  |     hosts pluggable retrievers
  |
  +-- Retrieval Orchestrator
  |     selects retrievers, fuses results, resolves conflicts, and explains matches
  |
  +-- Context Assembler
  |     generates model-ready memory blocks under policy and token budgets
  |
  +-- Model Provider Layer
  |     standard interfaces for LLM, embedding, rerank, tokenizer, and moderation providers
  |
  +-- Evaluation and Observability
        measures write quality, retrieval quality, habit accuracy, privacy, and usefulness
```

## 8. Storage Model

### 8.1 `memory_events`

Append-only raw evidence log.

Important fields:

```text
id
tenant_id
user_id
project_id
agent_id
session_id
event_type
event_time
actor_type
content
structured_payload
source_uri
hash
sensitivity
retention_policy
created_at
```

Common event types:

```text
user_message
assistant_message
tool_call
tool_result
file_change
business_event
feedback_positive
feedback_negative
explicit_remember
explicit_forget
correction
task_success
task_failure
```

### 8.2 `memory_records`

Authoritative memory facts.

Important fields:

```text
id
tenant_id
user_id
project_id
org_id
agent_id
scope
type
subject
predicate
object
content
structured_data
keywords
aliases
entities
tags
source_event_ids
derived_from_memory_ids
confidence
importance
sensitivity
status
stage
valid_from
valid_to
expires_at
supersedes_memory_id
superseded_by_memory_id
created_by
created_at
updated_at
deleted_at
index_hints
metadata
```

Status values:

```text
active
candidate
tentative
stable
rejected
superseded
expired
deleted
pending_review
```

Type values:

```text
semantic
episodic
procedural
habit
relationship
project_state
domain_knowledge
session
```

### 8.3 `memory_entities`

Entity dictionary and identity layer.

Fields:

```text
id
tenant_id
entity_type
canonical_name
aliases
external_ids
attributes
status
created_at
updated_at
```

### 8.4 `memory_edges`

Graph-compatible relationship records.

Fields:

```text
id
tenant_id
source_entity_id
relation_type
target_entity_id
source_memory_id
confidence
valid_from
valid_to
status
created_at
updated_at
```

### 8.5 `habit_memories`

Habit-specific state can be stored as specialized rows or as `memory_records.type = habit`. A separate table is useful for scoring and lifecycle.

Fields:

```text
id
tenant_id
user_id
project_id
agent_id
habit_key
context_pattern
preferred_behavior
evidence_count
positive_feedback_count
negative_feedback_count
success_count
contradiction_count
confidence
stage
activation_policy
exceptions
last_observed_at
created_at
updated_at
```

### 8.6 `memory_audit_logs`

Audit records for compliance and debugging.

Fields:

```text
id
tenant_id
actor_id
actor_type
action
resource_type
resource_id
request_id
reason
before_state
after_state
created_at
```

### 8.7 `memory_index_jobs`

Tracks async index updates and rebuilds.

Fields:

```text
id
tenant_id
memory_id
index_kind
operation
status
attempt_count
last_error
created_at
updated_at
```

## 9. Index Architecture

### 9.1 Index Kinds

```text
sql
keyword
dictionary
time
event
vector
graph
grep_file
custom
```

### 9.2 Index Ownership

The canonical store owns memory truth. Indexes own retrieval acceleration and specialized lookup only.

Index update rules:

1. Canonical write succeeds first.
2. Index jobs are created in the same transaction or through an outbox.
3. Index jobs update derived stores asynchronously.
4. Retrieval may tolerate stale optional indexes but must not return deleted or unauthorized canonical records.
5. Retrieval results must rehydrate and revalidate canonical records before context assembly.

### 9.3 Index Hints

Each memory record contains index hints:

```json
{
  "sql": true,
  "keyword": true,
  "dictionary": false,
  "time": true,
  "event": true,
  "vector": false,
  "graph": false,
  "grep_file": false
}
```

Examples:

```text
User confirmed preference:
  dictionary=true, keyword=true, vector=false

Debugging incident:
  keyword=true, time=true, vector=true, graph=false

Project dependency relation:
  graph=true, keyword=true, vector=false

Code file fact:
  grep_file=true, keyword=true, time=true
```

## 10. Retriever Architecture

### 10.1 Retriever Interface

```ts
export interface MemoryRetriever {
  name(): string;
  kind(): RetrieverKind;
  capabilities(): RetrieverCapabilities;
  retrieve(request: MemoryRetrieveRequest): Promise<MemoryRetrieveResult>;
}
```

```ts
export type RetrieverKind =
  | "sql"
  | "keyword"
  | "dictionary"
  | "time"
  | "event"
  | "vector"
  | "graph"
  | "grep_file"
  | "custom";
```

```ts
export interface MemoryRetrieveRequest {
  tenantId: string;
  userId?: string;
  projectId?: string;
  orgId?: string;
  agentId?: string;
  sessionId?: string;

  query?: string;
  currentMessage?: string;
  currentTask?: string;

  entities?: string[];
  memoryTypes?: MemoryType[];
  scopes?: MemoryScope[];
  filters?: Record<string, unknown>;

  timeRange?: {
    from?: string;
    to?: string;
  };

  limit?: number;
  minScore?: number;
  policy?: RetrievalPolicy;
  debug?: boolean;
}
```

```ts
export interface MemoryCandidateHit {
  memoryId: string;
  retriever: string;
  matchType:
    | "exact"
    | "keyword"
    | "dictionary"
    | "time"
    | "event"
    | "semantic"
    | "graph"
    | "file";
  score: number;
  reason: string;
  evidence?: string[];
  highlights?: string[];
  metadata?: Record<string, unknown>;
}
```

### 10.2 SQL Retriever

Use for:

```text
scope filters
memory type filters
status filters
time ranges
known IDs
exact structured fields
```

This retriever is mandatory in MVP.

### 10.3 Keyword Retriever

Use for:

```text
exact terms
error codes
API names
configuration keys
product names
technical identifiers
```

Implementation options:

```text
PostgreSQL full text search
OpenSearch / Elasticsearch
Lucene
SQLite FTS for embedded mode
```

This retriever is mandatory in MVP.

### 10.4 Dictionary Retriever

Use for:

```text
explicit preferences
tenant rules
project terms
aliases
controlled vocabulary
habit keys
feature flags
```

This retriever is mandatory in MVP because it provides low-cost, precise, editable memory.

### 10.5 Time Retriever

Use for:

```text
recent project state
last session state
latest correction
valid facts
expired facts
superseded facts
```

This retriever is mandatory in MVP.

### 10.6 Event Retriever

Use for:

```text
source tracing
memory explanation
replay
debugging
audit
```

This retriever is mandatory in MVP.

### 10.7 Vector Retriever

Use for:

```text
semantic similarity
vague user recall
similar past cases
related procedural memory
natural-language document memory
```

This retriever is optional and can be introduced after MVP.

### 10.8 Graph Retriever

Use for:

```text
entity relationship traversal
dependency reasoning
temporal fact relations
supersession chains
user/project/org relationships
incident/procedure/result links
```

Implementation can start in relational tables and later move to a graph database.

### 10.9 Grep/File Retriever

Use for:

```text
repository files
configuration files
logs
local documents
tool-generated artifacts
```

This is important for developer assistants and project memory. It should support exact file scanning and indexed file search. File facts can later be promoted into canonical memory.

## 11. Retrieval Orchestration

### 11.1 Flow

```text
1. Understand request
2. Determine policy and scope
3. Select retrievers
4. Run retrievers in parallel where possible
5. Rehydrate canonical memory records
6. Revalidate authorization and status
7. Normalize scores
8. Fuse duplicate hits
9. Resolve conflicts
10. Apply token and sensitivity budgets
11. Produce explainable context candidates
```

### 11.2 Retriever Selection

The orchestrator should not run all retrievers blindly. It should select based on request features:

```text
Known IDs or filters:
  SQL

Exact technical names:
  keyword, dictionary, grep_file

User preference or policy:
  dictionary, SQL

Recent state:
  time, event

Relationship question:
  graph, SQL, keyword

Vague memory recall:
  vector, keyword, time

Code or file question:
  grep_file, keyword, SQL
```

### 11.3 Score Fusion

Initial scoring can be rule-based:

```text
final_score =
  retrieval_score * 0.35
+ memory_confidence * 0.20
+ importance * 0.15
+ source_reliability * 0.10
+ scope_priority * 0.10
+ freshness * 0.10
- sensitivity_penalty
- staleness_penalty
- conflict_penalty
```

Score normalization must be per retriever because different retrievers produce different score distributions.

### 11.4 Conflict Resolution

Conflict rules:

1. Deleted memory is never returned.
2. Expired memory is returned only for explanation or history requests.
3. Superseded memory is returned only if the superseding memory is included or if history is requested.
4. More recent explicit correction beats older inferred memory.
5. Confirmed user memory beats inferred habit memory.
6. Tenant policy beats user preference.
7. Current user instruction beats historical habit memory for the current turn.

### 11.5 Context Assembly

Context assembly returns a compact memory block, not raw retrieval results.

Example:

```text
<memory_context>
User preferences:
- User prefers Chinese technical architecture answers. source=ai_123 confidence=0.91

Project state:
- This project is designing an embedding-optional, multi-index AI memory service. source=ai_456 confidence=0.88

Procedural guidance:
- For this memory module, canonical records are the source of truth; indexes are optional and rebuildable. source=ai_789 confidence=0.93
</memory_context>
```

The context assembler must include:

```text
memory content
scope
confidence
source IDs
validity
reason for inclusion
```

It must avoid:

```text
deleted memory
unauthorized memory
stale facts presented as active
raw sensitive data
large unfiltered event dumps
```

## 12. Self-Learning Memory

### 12.1 Learning Inputs

The system learns from:

```text
user messages
assistant messages
tool calls
tool results
file changes
business events
explicit remember requests
explicit forget requests
user corrections
positive feedback
negative feedback
task success
task failure
repeated behavior
```

### 12.2 Learning Pipeline

```text
1. Observe event
2. Store event in append-only log
3. Extract learning signals
4. Generate memory candidates
5. Apply policy and sensitivity filters
6. Deduplicate against existing memory
7. Detect conflicts
8. Decide action
9. Write or update canonical memory
10. Trigger index updates
11. Record audit trail
12. Update evaluation counters
```

### 12.3 Candidate Memory

Candidate memory is not final memory. It is a proposed state update.

Candidate structure:

```ts
export interface MemoryCandidate {
  tenantId: string;
  userId?: string;
  projectId?: string;
  agentId?: string;
  type: MemoryType;
  scope: MemoryScope;
  subject?: string;
  predicate?: string;
  object?: string;
  content: string;
  structuredData?: Record<string, unknown>;
  keywords: string[];
  aliases: string[];
  entities: string[];
  tags: string[];
  sourceEventIds: string[];
  confidence: number;
  importance: number;
  sensitivity: SensitivityLevel;
  proposedAction:
    | "create"
    | "reinforce"
    | "update"
    | "supersede"
    | "reject"
    | "review";
  reason: string;
}
```

### 12.4 Decision Rules

Suggested default rules:

```text
explicit_remember:
  create or update unless blocked by safety policy

explicit_forget:
  delete or suppress matching memory and linked indexes

correction:
  supersede old memory and create new active memory

single weak inference:
  keep as candidate or event evidence only

repeated weak inference:
  promote to tentative or habit candidate

high sensitivity:
  reject, redact, or require review

contradictory evidence:
  keep both events, update active memory based on policy and confidence
```

### 12.5 Consolidation Without Embeddings

Deduplication and consolidation must work without vector similarity.

Methods:

```text
subject + predicate + object comparison
normalized content hash
keyword overlap
alias dictionary
entity matching
structured key comparison
time-window comparison
graph neighbor comparison
source event clustering
```

Embeddings can later improve fuzzy matching but must not be required.

## 13. Habit-Forming Memory

### 13.1 Habit Definition

A habit is a recurring user, project, or organization preference that the system has enough evidence to apply proactively in matching contexts.

Habit examples:

```text
answer architecture questions in detailed Chinese
prefer implementation-ready plans over abstract descriptions
when discussing SDK integration, avoid raw HTTP and use SDK abstractions
```

### 13.2 Habit Lifecycle

```text
observed
  one or more events suggest a pattern

candidate
  repeated evidence exists but confidence is low

tentative
  system may apply lightly, with lower priority

confirmed
  user explicitly confirmed or repeated positive outcomes support it

stable
  habit is regularly applied in matching contexts

decayed
  habit has not appeared recently or has been contradicted

rejected
  user rejected or policy blocked it
```

### 13.3 Habit Scoring

Initial rule-based formula:

```text
confidence =
  explicit_confirmation * 0.35
+ repeated_evidence * 0.25
+ positive_feedback * 0.20
+ task_success * 0.15
+ source_reliability * 0.05
- negative_feedback * 0.30
- contradiction * 0.25
- age_decay * 0.10
```

Activation thresholds:

```text
candidate: confidence >= 0.35 and evidence_count >= 2
tentative: confidence >= 0.55 and evidence_count >= 3
confirmed: confidence >= 0.75 or explicit confirmation
stable: confidence >= 0.85 and repeated successful application
decayed: confidence < 0.45 or no recent evidence beyond configured window
```

### 13.4 Habit Priority

Priority order:

```text
system policy
developer policy
tenant or organization policy
current user instruction
explicit confirmed memory
project procedural memory
stable habit memory
tentative inferred habit
episodic examples
```

Habit memory must never override safety, access control, or current explicit user instructions.

### 13.5 Habit Activation

Habit activation requires:

```text
scope match
context pattern match
status active/stable
confidence threshold
no current contradiction
policy allows activation
token budget available
```

## 14. Model Provider Abstraction

### 14.1 Language Model Interface

```ts
export interface LanguageModel {
  id(): string;
  provider(): string;
  capabilities(): ModelCapabilities;

  generate(request: GenerateRequest): Promise<GenerateResult>;
  stream?(request: GenerateRequest): AsyncIterable<GenerateChunk>;

  generateObject<T>(
    request: GenerateObjectRequest<T>
  ): Promise<GenerateObjectResult<T>>;

  countTokens?(input: TokenCountInput): Promise<TokenCountResult>;
}
```

Capabilities:

```ts
export interface ModelCapabilities {
  contextWindow: number;
  maxOutputTokens?: number;
  supportsStreaming: boolean;
  supportsToolCalling: boolean;
  supportsStructuredOutput: boolean;
  supportsJsonSchema: boolean;
  supportsVision: boolean;
  supportsAudio: boolean;
  supportsReasoningEffort?: boolean;
}
```

### 14.2 Embedding Model Interface

Embedding is optional.

```ts
export interface EmbeddingModel {
  id(): string;
  provider(): string;
  dimensions(): number;
  metric(): "cosine" | "dot" | "l2";
  capabilities(): EmbeddingCapabilities;

  embedQuery(input: EmbedInput): Promise<EmbeddingResult>;
  embedDocuments(inputs: EmbedInput[]): Promise<EmbeddingBatchResult>;
}
```

Embedding metadata must be stored with vectors:

```text
provider
model
dimension
metric
embedding_space_id
embedding_version
input_type
chunking_version
created_at
```

Vectors from different embedding spaces must not be mixed.

### 14.3 Rerank Interface

Reranking is optional.

```ts
export interface RerankModel {
  id(): string;
  provider(): string;
  rerank(request: RerankRequest): Promise<RerankResult>;
}
```

### 14.4 Provider Registry

```ts
export interface ModelProviderRegistry {
  getLanguageModel(id: string): LanguageModel;
  getEmbeddingModel(id: string): EmbeddingModel | undefined;
  getRerankModel(id: string): RerankModel | undefined;
}
```

Adapters can support:

```text
OpenAI
OpenAI-compatible APIs
Anthropic
Gemini
local vLLM
Ollama
LM Studio
custom enterprise gateways
```

## 15. Public API Design

### 15.1 Event APIs

```text
POST /v1/events
GET  /v1/events/{id}
GET  /v1/events
```

### 15.2 Memory APIs

```text
POST   /v1/memories
GET    /v1/memories/{id}
GET    /v1/memories
PATCH  /v1/memories/{id}
DELETE /v1/memories/{id}
POST   /v1/memories/forget
POST   /v1/memories/extract
POST   /v1/memories/consolidate
```

### 15.3 Retrieval APIs

```text
POST /v1/retrieve
POST /v1/context/assemble
POST /v1/context/explain
```

### 15.4 Habit APIs

```text
GET   /v1/habits
GET   /v1/habits/{id}
PATCH /v1/habits/{id}
POST  /v1/habits/{id}/confirm
POST  /v1/habits/{id}/reject
```

### 15.5 Governance APIs

```text
GET  /v1/audit-logs
GET  /v1/memory-sources/{memoryId}
POST /v1/export
POST /v1/learning/disable
POST /v1/learning/enable
```

### 15.6 Admin APIs

```text
GET  /v1/admin/indexes
POST /v1/admin/indexes/rebuild
GET  /v1/admin/evals
POST /v1/admin/evals/run
GET  /v1/admin/provider-health
```

## 16. Error Handling

### 16.1 Write Failures

Rules:

1. Event writes must be durable before learning begins.
2. Candidate extraction failure must not fail the user request unless extraction was explicitly requested.
3. Canonical memory write failure must not create index records.
4. Index write failure creates retryable index jobs.
5. Provider failure should return degraded results if deterministic retrievers are available.

### 16.2 Retrieval Failures

Rules:

1. Mandatory retrievers failing should return an error or degraded response based on policy.
2. Optional retrievers failing should be reported in debug metadata.
3. Context assembly must never include unauthorized, deleted, or unvalidated stale records.

### 16.3 Provider Failures

LLM/embedding/rerank adapters need:

```text
timeouts
retries with backoff
circuit breakers
rate-limit handling
fallback providers
cost limits
request tracing
```

## 17. Privacy, Security, and Governance

### 17.1 Sensitivity Levels

```text
public
internal
private
pii
secret
regulated
```

Default policy:

```text
secret: never store as normal memory
regulated: require explicit tenant policy
pii: store only when allowed and needed
private: store with user visibility and deletion controls
public/internal: store according to scope policy
```

### 17.2 User Controls

Users or tenants should be able to:

```text
view memories
edit memories
delete memories
export memories
disable learning
disable habit inference
reject inferred habits
inspect sources
```

### 17.3 Access Control

All APIs and retrievers must enforce:

```text
tenant isolation
user scope
project scope
organization scope
agent scope
role permissions
sensitivity policy
retention policy
```

### 17.4 Deletion

Deletion must:

```text
mark canonical records deleted or physically delete when required
remove derived index entries
record audit logs
prevent future retrieval
optionally suppress re-learning from old event evidence
```

## 18. Evaluation

### 18.1 Write Quality Metrics

```text
memory write precision
memory write recall
duplicate rate
false memory rate
sensitive memory false positive rate
candidate rejection accuracy
```

### 18.2 Retrieval Metrics

```text
retrieval precision
retrieval recall
freshness accuracy
scope accuracy
conflict resolution accuracy
latency by retriever
cost by retriever
```

### 18.3 Habit Metrics

```text
habit formation precision
habit activation precision
habit contradiction rate
user rejection rate
positive feedback rate
```

### 18.4 End-to-End Metrics

```text
task success lift
follow-up question reduction
user correction reduction
token savings
memory usefulness rating
privacy incident count
```

### 18.5 Eval Harness

The eval system should support:

```text
fixed test datasets
event replay
golden memory outputs
retrieval expected IDs
context assembly snapshots
provider comparison
prompt version comparison
regression gates
```

## 19. Observability

Track:

```text
event ingestion volume
candidate extraction rate
candidate acceptance rate
memory growth rate
retriever latency
retriever hit rate
index job lag
provider latency
provider error rate
context token usage
deletion propagation time
habit promotion rate
habit rejection rate
```

Every context assembly response should include debug metadata in non-production or authorized diagnostic mode:

```text
retrievers used
candidate counts
selected memory IDs
excluded memory reasons
score components
policy decisions
provider calls
```

## 20. Phased Roadmap

### Phase 1: No-Embedding MVP

Purpose: build a reliable, explainable, provider-light memory service.

Deliverables:

```text
event log
canonical memory store
memory CRUD
explicit remember/forget
candidate memory lifecycle
SQL retriever
keyword retriever
dictionary retriever
time retriever
event retriever
retriever registry
retrieval orchestrator
context assembler
open-api contract boundary
app-api contract boundary
backend-api contract boundary
habit learner state machine
basic policy engine
audit logs
basic eval harness
```

Expected result:

```text
The system can store, retrieve, explain, update, and forget useful memory without embeddings.
```

### Phase 2: Self-Learning and Provider Abstraction

Purpose: introduce LLM-assisted extraction while keeping deterministic safeguards.

Deliverables:

```text
LanguageModel interface
provider registry
structured extraction prompts
candidate judge
consolidation engine
sensitivity classifier
feedback learning
habit confirmation/rejection APIs
provider health checks
cost and rate limits
```

Expected result:

```text
The system can learn from events and feedback while preserving evidence, review, and policy controls.
```

### Phase 3: Optional Semantic Layer

Purpose: add embedding and reranking without changing the core memory model.

Deliverables:

```text
EmbeddingModel interface
vector retriever
embedding index jobs
embedding versioning
index rebuild workflow
RerankModel interface
hybrid scoring
semantic dedup assistance
semantic eval datasets
```

Expected result:

```text
The system can recall vague, semantic, and similar-case memories when embedding providers are enabled.
```

### Phase 4: Graph and File Intelligence

Purpose: support project/code memory, relationship memory, and temporal facts.

Deliverables:

```text
entity resolver
relationship memory
graph retriever
temporal edges
grep/file retriever
file source tracking
code/project memory extraction
fact supersession chains
graph-backed explanation
```

Expected result:

```text
The system can reason over people, projects, services, files, incidents, procedures, and changing facts.
```

### Phase 5: Platform-Grade Governance and Optimization

Purpose: make memory safe and scalable for enterprise use.

Deliverables:

```text
tenant-level learning policies
advanced retention rules
data export
hard-delete workflows
redaction workflows
admin dashboard
memory quality dashboard
eval regression gates
multi-provider routing
cost optimization
large-scale index partitioning
SLOs and alerting
```

Expected result:

```text
The system is ready as a platform memory layer for multiple agents, applications, tenants, and business domains.
```

## 21. Suggested MVP Technology Choices

The design does not require one stack, but the MVP can be implemented with:

```text
PostgreSQL for canonical store, full-text search, relational graph tables, and time queries
Redis for short-term/session cache and rate limits
Background worker for extraction, consolidation, and index jobs
REST API first, with SDKs later
OpenAPI contract for public APIs
Provider adapters for LLMs only when Phase 2 starts
No vector database in Phase 1
```

If using TypeScript:

```text
NestJS or Fastify
Prisma or Drizzle
BullMQ or Temporal
Zod or JSON Schema for structured contracts
```

If using Java:

```text
Spring Boot
Spring Data / MyBatis
Flyway or Liquibase
Spring AI adapter layer only behind internal interfaces
Quartz / Temporal / message queue workers
```

## 22. Key Design Risks

### 22.1 Memory Pollution

Risk: the system learns low-value or wrong memories.

Mitigation:

```text
candidate lifecycle
confidence thresholds
source evidence
review state
eval harness
user controls
```

### 22.2 Scope Leakage

Risk: memory from one user or tenant appears in another context.

Mitigation:

```text
mandatory scope fields
policy checks before and after retrieval
tenant partitioning
tests for cross-scope leakage
```

### 22.3 Stale Facts

Risk: outdated facts remain active.

Mitigation:

```text
valid_from / valid_to
supersession chains
correction events
freshness scoring
time retriever
```

### 22.4 Over-Reliance on LLMs

Risk: LLM extraction or judging becomes the source of truth.

Mitigation:

```text
LLM produces candidates only
canonical store owns state
policy engine validates
consolidation engine applies deterministic rules
```

### 22.5 Embedding Lock-In

Risk: memory quality depends on one embedding provider or vector index.

Mitigation:

```text
embedding optional
embedding metadata and versioning
vector retriever plugin
rebuildable indexes
deterministic retrievers in MVP
```

## 23. Open Decisions

1. Primary implementation stack: TypeScript/NestJS or Java/Spring Boot.
2. Whether SDKWork memory should be a standalone service, embedded library, or both.
3. Whether tenant isolation is database-level, schema-level, or row-level.
4. Whether Phase 1 uses PostgreSQL full-text only or also OpenSearch.
5. Whether graph memory starts as relational tables or uses a graph database from Phase 4.
6. Which model providers should be supported first in Phase 2.
7. Whether a memory management UI is part of MVP or Phase 5.

## 24. Acceptance Criteria

The design is successful if:

1. The system works without embeddings.
2. Embeddings can be added later without changing canonical memory records.
3. Every memory has scope, source, status, confidence, and lifecycle state.
4. Retrieval can use multiple retrievers and explain why each memory was selected.
5. Habit memory forms gradually from repeated evidence and feedback.
6. Users and tenants can inspect, edit, delete, export, and disable learning.
7. The service can evaluate write quality, retrieval quality, habit quality, and privacy behavior.
8. The architecture can evolve from MVP to final platform version without replacing the core model.

## 25. SDKWork Standard Alignment

This module must follow SDKWork standards instead of defining a private convention.

Normative standard inputs:

```text
../sdkwork-specs/SOUL.md
../sdkwork-specs/API_SPEC.md
../sdkwork-specs/SDK_SPEC.md
../sdkwork-specs/SDK_WORKSPACE_GENERATION_SPEC.md
../sdkwork-specs/WEB_BACKEND_SPEC.md
../sdkwork-specs/DATABASE_SPEC.md
../sdkwork-specs/EVENT_SPEC.md
../sdkwork-specs/PRIVACY_SPEC.md
../sdkwork-specs/OBSERVABILITY_SPEC.md
../sdkwork-specs/DOMAIN_SPEC.md
../sdkwork-specs/NAMING_SPEC.md
```

Reference implementation patterns:

```text
../sdkwork-drive
../sdkwork-knowledgebase
```

Borrowed architecture patterns:

1. Follow `sdkwork-knowledgebase` for the Rust-style split between contract crate, product service, app-api, backend-api, storage adapter, generated SDK families, and test support.
2. Follow `sdkwork-drive` for multi-surface API ownership, SDK family metadata, schema registry, PostgreSQL/SQLite parity, runtime config, and storage lifecycle boundaries.
3. Use OpenAPI as the HTTP contract authority.
4. Use generated SDKs as the only app/backend HTTP transport boundary.
5. Keep dependency capabilities in dependency SDKs or approved composed wrappers; do not copy dependency-owned operations into Memory SDK generation inputs.
6. Define tables through schema registry contracts before migrations.
7. Keep generated SDK output untouched.

Canonical Memory identity:

```text
Application/repository: sdkwork-memory
Domain: intelligence
Capability: memory
API path segment: memory
OpenAPI tag: memory
SDK family stem: memory
Database prefix: ai_
Event prefix: memory.
Permission prefix: memory.
```

SDKWork API surfaces:

```text
App API:
  Prefix: /app/v3/api
  Authority metadata: sdkwork-memory-app-api (retired draft alias: sdkwork-memory.app)
  OpenAPI file: sdks/sdkwork-memory-app-sdk/openapi/memory-app-api.openapi.json
  SDK family: sdks/sdkwork-memory-app-sdk

Backend API:
  Prefix: /backend/v3/api
  Authority metadata: sdkwork-memory-backend-api (retired draft alias: sdkwork-memory.backend)
  OpenAPI file: sdks/sdkwork-memory-backend-sdk/openapi/memory-backend-api.openapi.json
  SDK family: sdks/sdkwork-memory-backend-sdk

Open API:
  Prefix: /mem/v3/api
  Authority metadata: sdkwork-memory-open-api
  OpenAPI file: sdks/sdkwork-memory-sdk/openapi/memory-open-api.openapi.json
  SDK family: sdks/sdkwork-memory-sdk
  Credential mode: X-API-Key / ApiKey

RPC SDK:
  Optional later as sdkwork-memory-rpc-sdk for backend-to-backend or local/private high-throughput runtimes.
```

## 26. SDKWork Workspace Shape

Recommended application-root structure:

```text
sdkwork-memory/
  AGENTS.md
  CODEX.md
  GEMINI.md
  sdkwork.app.config.json
  specs/
    README.md
    component.spec.json
  docs/
    schema-registry/
      tables/
        001-memory-core.yaml
        002-memory-learning.yaml
        003-memory-retrieval.yaml
        004-memory-provider.yaml
        005-memory-governance.yaml
    superpowers/
      specs/
      plans/
  crates/
    sdkwork-memory-contract/
    sdkwork-memory-retrieval/
    sdkwork-intelligence-memory-service/
    sdkwork-intelligence-memory-repository-sqlx/
    sdkwork-memory-test-support/
    sdkwork-routes-memory-open-api/
    sdkwork-routes-memory-app-api/
    sdkwork-routes-memory-backend-api/
  plugins/
    sdkwork-memory-plugin-native-sql/
    sdkwork-memory-plugin-reference-profiles/
  sdks/
    sdkwork-memory-sdk/
      sdk-manifest.json
      sdk-manifest.json
      specs/component.spec.json
      openapi/
        memory-open-api.openapi.json
        memory-open-api.sdkgen.yaml
      sdkwork-memory-sdk-typescript/
        generated/server-openapi/
        composed/
    sdkwork-memory-app-sdk/
      sdk-manifest.json
      sdk-manifest.json
      specs/component.spec.json
      openapi/
        memory-app-api.openapi.json
        memory-app-api.sdkgen.yaml
      sdkwork-memory-app-sdk-typescript/
        generated/server-openapi/
        composed/
    sdkwork-memory-backend-sdk/
      sdk-manifest.json
      sdk-manifest.json
      specs/component.spec.json
      openapi/
        memory-backend-api.openapi.json
        memory-backend-api.sdkgen.yaml
      sdkwork-memory-backend-sdk-typescript/
        generated/server-openapi/
        composed/
  tools/
    memory_openapi_export.mjs
    memory_sdk_generate.mjs
    memory_schema_quality_gate.mjs
    verify_phase1.ps1
```

The Rust-oriented shape mirrors `sdkwork-knowledgebase`. A Java/Spring implementation may be used for SaaS, but it must expose the same OpenAPI contracts, operationIds, DTO schemas, problem-detail errors, auth semantics, and SDK family metadata.

Component responsibilities:

| Component | Responsibility |
| --- | --- |
| `sdkwork-memory-contract` | Public DTOs, enums, operation IDs, API schema helpers, provider manifest contracts. |
| `sdkwork-memory-retrieval` | ID helpers, scoring utilities, lifecycle state machines, query planning helpers. |
| `sdkwork-intelligence-memory-service` | Use cases, policy engine, learning engine, consolidation engine, retrieval orchestration, context assembly. |
| `sdkwork-intelligence-memory-repository-sqlx` | PostgreSQL/SQLite migrations, repository implementations, schema migration registry. |
| `sdkwork-routes-memory-open-api` | Open API path constants, route manifest, router registration, and handler boundary for `/mem/v3/api`. |
| `sdkwork-routes-memory-app-api` | App API path constants, route manifest, router registration, and handler boundary for `/app/v3/api`. |
| `sdkwork-routes-memory-backend-api` | Backend API path constants, route manifest, router registration, and handler boundary for `/backend/v3/api`. |
| `sdkwork-memory-plugin-native-sql` | Native SQL plugin: adapter from Memory context/retrieval contracts to SQL storage and agent runtimes. |
| `sdkwork-memory-plugin-reference-profiles` | Reference profiles plugin: adapter for Drive-backed exports, imports, snapshots, replay packages, and large event payload references. |
| `sdkwork-memory-test-support` | Fake stores, fake providers, deterministic event fixtures, eval datasets. |

## 27. Multi-Implementation Abstraction

Memory must support different implementation strategies behind one stable contract.

### 27.1 Implementation Families

Supported implementation families:

| Family | Purpose | Canonical store | Typical indexes |
| --- | --- | --- | --- |
| `native_sql` | Default MVP and production baseline. | PostgreSQL/SQLite SQL tables. | SQL, keyword, dictionary, time, event. |
| `event_sourced` | Strong replay, audit, and projection rebuild. | Append-only event log plus projections. | SQL, time, event, optional keyword/vector/graph. |
| `graph_temporal` | Relationship-heavy and temporal fact memory. | SQL record store plus graph projection, or graph-native canonical backend when approved. | Graph, SQL, keyword, time. |
| `search_first` | High-volume text memory and logs. | SQL record store with search projection. | OpenSearch/Lucene/BM25, SQL, time. |
| `local_embedded` | Desktop/local/private lightweight mode. | SQLite plus local files. | SQLite FTS, dictionary, grep/file, time. |
| `external_provider_bridge` | Bridge to Mem0/Zep/Letta or enterprise memory engines. | External provider plus local shadow/audit records. | Provider-specific, plus local audit/search. |
| `hybrid_platform` | Final platform-grade deployment. | SQL canonical store plus optional event sourcing, graph, search, vector, and file indexes. | All retrievers enabled by policy. |

### 27.2 Provider Manifest

Every implementation provider must expose a machine-readable manifest:

```ts
export interface MemoryImplementationManifest {
  implementationId: string;
  kind:
    | "native_sql"
    | "event_sourced"
    | "graph_temporal"
    | "search_first"
    | "local_embedded"
    | "external_provider_bridge"
    | "hybrid_platform"
    | "custom";
  displayName: string;
  version: string;
  supportedDeploymentModes: Array<"saas" | "private" | "local" | "desktop" | "test">;
  capabilities: MemoryImplementationCapabilities;
  storageEngines: string[];
  retrieverKinds: RetrieverKind[];
  consistency: {
    readYourWrites: boolean;
    eventReplay: boolean;
    indexRebuild: boolean;
    crossRegionReplication: boolean;
  };
  migration: {
    exportSupported: boolean;
    importSupported: boolean;
    dualWriteSupported: boolean;
    shadowReadSupported: boolean;
  };
}
```

Capabilities:

```ts
export interface MemoryImplementationCapabilities {
  eventLog: boolean;
  recordCrud: boolean;
  candidateLifecycle: boolean;
  habitLearning: boolean;
  policyEngine: boolean;
  retrievalProfiles: boolean;
  contextAssembly: boolean;
  deletionPropagation: boolean;
  auditLog: boolean;
  evalRuns: boolean;
  vectorOptional: boolean;
  graphOptional: boolean;
  fileGrepOptional: boolean;
}
```

### 27.3 Runtime Implementation Profile

The active implementation must be selected through configuration and database-backed bindings, not through code forks.

```ts
export interface MemoryImplementationProfile {
  profileId: string;
  tenantId: string;
  name: string;
  deploymentMode: "saas" | "private" | "local" | "desktop" | "test";
  canonicalStore: string;
  eventStore: string;
  policyStore: string;
  providerBindings: Record<string, string>;
  retrieverBindings: Record<RetrieverKind, string>;
  learningProvider?: string;
  embeddingProvider?: string;
  rerankProvider?: string;
  graphProvider?: string;
  fileProvider?: string;
  fallbackProfileId?: string;
  status: "active" | "disabled" | "migrating" | "deprecated";
}
```

Rules:

1. App API and Backend API contracts must not change when the implementation profile changes.
2. Hot switching is allowed for optional retrievers and derived indexes.
3. Canonical store switching requires migration workflow, export/import verification, and usually a write freeze, dual write, or shadow read window.
4. External provider bridges must keep local `ai_record`, `ai_event`, and `ai_audit_log` shadow records sufficient for source tracing, deletion, export, and SDKWork governance.
5. Provider-specific DTOs must not leak into public app-api/backend-api schemas.
6. Every implementation provider must pass the same contract test suite.

### 27.4 Core Ports

Product services depend on ports, not concrete stores:

```ts
export interface MemoryRecordStore {
  create(command: CreateMemoryRecordCommand): Promise<MemoryRecord>;
  retrieve(scope: MemoryScopeContext, memoryId: string): Promise<MemoryRecord | null>;
  list(query: MemoryRecordQuery): Promise<Page<MemoryRecord>>;
  update(command: UpdateMemoryRecordCommand): Promise<MemoryRecord>;
  markDeleted(command: DeleteMemoryRecordCommand): Promise<void>;
}

export interface MemoryEventStore {
  append(command: AppendMemoryEventCommand): Promise<MemoryEvent>;
  retrieve(eventId: string): Promise<MemoryEvent | null>;
  list(query: MemoryEventQuery): Promise<Page<MemoryEvent>>;
}

export interface MemoryCandidateStore {
  create(candidate: MemoryCandidate): Promise<MemoryCandidate>;
  decide(command: DecideMemoryCandidateCommand): Promise<MemoryDecisionResult>;
  list(query: MemoryCandidateQuery): Promise<Page<MemoryCandidate>>;
}

export interface MemoryIndexProvider {
  kind(): RetrieverKind;
  index(command: IndexMemoryCommand): Promise<IndexResult>;
  remove(command: RemoveMemoryIndexCommand): Promise<void>;
  rebuild(command: RebuildMemoryIndexCommand): AsyncIterable<IndexRebuildProgress>;
}

export interface MemoryRetriever {
  name(): string;
  kind(): RetrieverKind;
  retrieve(request: MemoryRetrieveRequest): Promise<MemoryRetrieveResult>;
}

export interface MemoryImplementationProvider {
  manifest(): MemoryImplementationManifest;
  buildRuntime(profile: MemoryImplementationProfile): Promise<MemoryRuntime>;
}
```

The `MemoryRuntime` composes stores, retrievers, providers, policy engine, learning engine, and context assembler for one active profile.

## 28. SDK Families And Dependency Policy

### 28.1 SDK Families

MVP SDK families:

```text
sdks/sdkwork-memory-sdk
sdks/sdkwork-memory-app-sdk
sdks/sdkwork-memory-backend-sdk
```

Future optional SDK families:

```text
sdks/sdkwork-memory-rpc-sdk   # RPC, only if backend/native high-throughput integration is needed
```

Open SDK assembly metadata should declare:

```json
{
  "workspace": "sdkwork-memory-sdk",
  "title": "SDKWork Memory Open API SDK",
  "apiVersion": "0.1.0",
  "openapiVersion": "3.1.2",
  "authoritySpec": "openapi/memory-open-api.openapi.json",
  "generationInputSpec": "openapi/memory-open-api.openapi.json",
  "apiAuthority": "sdkwork-memory-open-api",
  "discoverySurface": {
    "sdkTarget": "open-api",
    "apiPrefix": "/mem/v3/api",
    "schemaUrl": "/mem/v3/openapi.json",
    "generatedProtocols": ["http-openapi"],
    "manualTransports": []
  },
  "sdkOwner": "sdkwork-memory",
  "sdkDependencies": []
}
```

App SDK assembly metadata should declare:

```json
{
  "workspace": "sdkwork-memory-app-sdk",
  "title": "SDKWork Memory App API SDK",
  "apiVersion": "0.1.0",
  "openapiVersion": "3.1.2",
  "authoritySpec": "openapi/memory-app-api.openapi.json",
  "generationInputSpec": "openapi/memory-app-api.openapi.json",
  "apiAuthority": "sdkwork-memory.app",
  "discoverySurface": {
    "sdkTarget": "app",
    "apiPrefix": "/app/v3/api",
    "schemaUrl": "/app/v3/openapi.json",
    "generatedProtocols": ["http-openapi"],
    "manualTransports": []
  },
  "sdkOwner": "sdkwork-memory",
  "sdkDependencies": []
}
```

Backend SDK assembly metadata should mirror the backend surface:

```json
{
  "workspace": "sdkwork-memory-backend-sdk",
  "title": "SDKWork Memory Backend API SDK",
  "apiVersion": "0.1.0",
  "openapiVersion": "3.1.2",
  "authoritySpec": "openapi/memory-backend-api.openapi.json",
  "generationInputSpec": "openapi/memory-backend-api.openapi.json",
  "apiAuthority": "sdkwork-memory.backend",
  "discoverySurface": {
    "sdkTarget": "backend",
    "apiPrefix": "/backend/v3/api",
    "schemaUrl": "/backend/v3/openapi.json",
    "generatedProtocols": ["http-openapi"],
    "manualTransports": []
  },
  "sdkOwner": "sdkwork-memory",
  "sdkDependencies": []
}
```

### 28.2 Dependency SDKs

Required dependency policy:

| Dependency | App SDK | Backend SDK | Purpose |
| --- | --- | --- | --- |
| `sdkwork-appbase` | Required | Required for backend-admin | IAM/session, tenant, user, organization, request context. |
| `sdkwork-drive` | Optional in app; recommended in backend | Recommended | Memory import/export packages, replay packages, large payload references, snapshots. |
| `sdkwork-knowledgebase` | Optional | Optional | Domain knowledge retrieval and context packs when memory composes with knowledgebase. |

Dependency rules:

1. Memory must consume appbase and Drive through generated SDKs or approved product-layer adapters.
2. Memory SDKs must not copy appbase, Drive, or Knowledgebase operations into Memory OpenAPI authorities.
3. If Memory exposes a composed facade over dependency capability, it must declare `dependencyApiExports` and implement it outside generated transport output.
4. App/user-facing Memory clients must use app SDK clients only.
5. Backend-admin Memory clients must use backend SDK clients only.

## 29. Open API Contract Draft

All open-api paths use `/mem/v3/api`. Protected operations require the Open API credential mode `ApiKey` through the `X-API-Key` header. DTO fields containing `int64` values must serialize as strings.

The Open API surface is for external integrations, API-key clients, server-to-server integrations, and public/domain Memory SDK consumers. It must not expose auth/session endpoints, backend-only implementation profile switching, provider secret binding management, migration jobs, retention jobs, audit log listing, or operator-only diagnostics.

Base path:

```text
/mem/v3/api/memory
```

Operation draft:

| Method | Path | operationId | Purpose |
| --- | --- | --- | --- |
| `GET` | `/mem/v3/api/memory/capabilities` | `capabilities.retrieve` | Retrieve supported memory capabilities and implementation-safe feature flags. |
| `POST` | `/mem/v3/api/memory/events` | `events.create` | Append external evidence event through API-key context. |
| `GET` | `/mem/v3/api/memory/events/{eventId}` | `events.retrieve` | Retrieve an allowed external event projection. |
| `GET` | `/mem/v3/api/memory/memories` | `memories.list` | List/search memory records allowed to the API key. |
| `POST` | `/mem/v3/api/memory/memories` | `memories.create` | Explicitly create memory from external integration. |
| `GET` | `/mem/v3/api/memory/memories/{memoryId}` | `memories.retrieve` | Retrieve a memory record. |
| `PATCH` | `/mem/v3/api/memory/memories/{memoryId}` | `memories.update` | Update a memory record without exposing backend-only controls. |
| `DELETE` | `/mem/v3/api/memory/memories/{memoryId}` | `memories.delete` | Delete/suppress a memory record through scoped API-key authority. |
| `POST` | `/mem/v3/api/memory/retrievals` | `retrievals.create` | Retrieve memory using a server-approved profile. |
| `GET` | `/mem/v3/api/memory/retrievals/{retrievalId}` | `retrievals.retrieve` | Retrieve retrieval trace projection allowed to the API key. |
| `POST` | `/mem/v3/api/memory/context_packs` | `contextPacks.create` | Assemble model-ready memory context. |
| `GET` | `/mem/v3/api/memory/context_packs/{contextPackId}` | `contextPacks.retrieve` | Retrieve assembled context pack metadata. |
| `POST` | `/mem/v3/api/memory/feedback` | `feedback.create` | Submit usefulness, correction, or suppression feedback. |
| `POST` | `/mem/v3/api/memory/extractions` | `extractions.create` | Extract candidate memories from supplied external events/messages. |
| `GET` | `/mem/v3/api/memory/candidates` | `candidates.list` | List API-key-visible candidate memories. |
| `GET` | `/mem/v3/api/memory/candidates/{candidateId}` | `candidates.retrieve` | Retrieve a candidate memory. |
| `GET` | `/mem/v3/api/memory/provider_health` | `providerHealth.retrieve` | Retrieve a redacted provider health summary. |

Open API scopes:

```text
memory.capabilities.read
memory.events.write
memory.events.read
memory.records.read
memory.records.write
memory.records.delete
memory.retrievals.create
memory.contextPacks.create
memory.feedback.write
memory.extractions.create
memory.candidates.read
memory.providers.health.read
```

Open API DTOs:

```text
MemoryCapabilities
MemoryEvent
MemoryEventRequest
MemoryRecord
MemoryRecordRequest
MemoryRecordList
MemoryRetrievalRequest
MemoryRetrievalResult
MemoryRetrievalTrace
MemoryContextPackRequest
MemoryContextPack
MemoryFeedbackRequest
MemoryExtractionRequest
MemoryCandidate
MemoryCandidateList
MemoryProviderHealth
ProblemDetail
```

## 30. App API Contract Draft

All app-api paths use `/app/v3/api`. All protected operations require SDKWork dual-token context. DTO fields containing `int64` values must serialize as strings.

Base path:

```text
/app/v3/api/memory
```

Operation draft:

| Method | Path | operationId | Purpose |
| --- | --- | --- | --- |
| `POST` | `/app/v3/api/memory/spaces` | `spaces.create` | Create user/project memory space. |
| `GET` | `/app/v3/api/memory/spaces` | `spaces.list` | List accessible memory spaces. |
| `GET` | `/app/v3/api/memory/spaces/{spaceId}` | `spaces.retrieve` | Retrieve memory space. |
| `PATCH` | `/app/v3/api/memory/spaces/{spaceId}` | `spaces.update` | Update memory space settings. |
| `POST` | `/app/v3/api/memory/events` | `events.create` | Append raw memory evidence event. |
| `GET` | `/app/v3/api/memory/events/{eventId}` | `events.retrieve` | Retrieve safe event metadata/content. |
| `POST` | `/app/v3/api/memory/memories` | `memories.create` | Explicitly create memory. |
| `GET` | `/app/v3/api/memory/memories` | `memories.list` | List/search memory records. |
| `GET` | `/app/v3/api/memory/memories/{memoryId}` | `memories.retrieve` | Retrieve memory record. |
| `PATCH` | `/app/v3/api/memory/memories/{memoryId}` | `memories.update` | Update memory record. |
| `DELETE` | `/app/v3/api/memory/memories/{memoryId}` | `memories.delete` | Delete/suppress memory record. |
| `GET` | `/app/v3/api/memory/memories/{memoryId}/sources` | `memories.sources.list` | Explain memory source events. |
| `POST` | `/app/v3/api/memory/forget_requests` | `forgetRequests.create` | Request scoped memory deletion/forgetting. |
| `GET` | `/app/v3/api/memory/forget_requests/{forgetRequestId}` | `forgetRequests.retrieve` | Retrieve forget workflow status. |
| `POST` | `/app/v3/api/memory/extractions` | `extractions.create` | Extract candidate memories from supplied events/messages. |
| `GET` | `/app/v3/api/memory/candidates` | `candidates.list` | List user-reviewable memory candidates. |
| `GET` | `/app/v3/api/memory/candidates/{candidateId}` | `candidates.retrieve` | Retrieve memory candidate. |
| `POST` | `/app/v3/api/memory/candidates/{candidateId}/approve` | `candidates.approve` | Approve candidate memory. |
| `POST` | `/app/v3/api/memory/candidates/{candidateId}/reject` | `candidates.reject` | Reject candidate memory. |
| `GET` | `/app/v3/api/memory/habits` | `habits.list` | List habit memories. |
| `GET` | `/app/v3/api/memory/habits/{habitId}` | `habits.retrieve` | Retrieve habit memory. |
| `PATCH` | `/app/v3/api/memory/habits/{habitId}` | `habits.update` | Update habit activation/settings. |
| `POST` | `/app/v3/api/memory/habits/{habitId}/confirm` | `habits.confirm` | Confirm inferred habit. |
| `POST` | `/app/v3/api/memory/habits/{habitId}/reject` | `habits.reject` | Reject inferred habit. |
| `POST` | `/app/v3/api/memory/retrievals` | `retrievals.create` | Retrieve memory using configured profile. |
| `GET` | `/app/v3/api/memory/retrievals/{retrievalId}` | `retrievals.retrieve` | Retrieve retrieval trace allowed to user. |
| `POST` | `/app/v3/api/memory/context_packs` | `contextPacks.create` | Assemble model-ready memory context. |
| `GET` | `/app/v3/api/memory/context_packs/{contextPackId}` | `contextPacks.retrieve` | Retrieve assembled context pack metadata. |
| `POST` | `/app/v3/api/memory/feedback` | `feedback.create` | Submit usefulness/correction feedback. |
| `POST` | `/app/v3/api/memory/export_jobs` | `exportJobs.create` | Request user/space memory export. |
| `GET` | `/app/v3/api/memory/export_jobs/{exportJobId}` | `exportJobs.retrieve` | Retrieve export job status and Drive ref. |
| `GET` | `/app/v3/api/memory/learning_settings` | `learningSettings.retrieve` | Retrieve user learning settings. |
| `PATCH` | `/app/v3/api/memory/learning_settings` | `learningSettings.update` | Enable/disable learning and habit inference. |

App API permissions:

```text
memory.spaces.read
memory.spaces.write
memory.events.write
memory.records.read
memory.records.write
memory.records.delete
memory.candidates.review
memory.habits.read
memory.habits.write
memory.retrievals.create
memory.contextPacks.create
memory.feedback.write
memory.exports.create
memory.learningSettings.write
```

App API DTOs:

```text
MemorySpace
MemorySpaceRequest
MemoryEvent
MemoryEventRequest
MemoryRecord
MemoryRecordRequest
MemoryRecordList
MemoryCandidate
MemoryCandidateList
MemoryCandidateDecisionRequest
MemoryHabit
MemoryHabitList
MemoryRetrievalRequest
MemoryRetrievalResult
MemoryRetrievalTrace
MemoryContextPackRequest
MemoryContextPack
MemoryFeedbackRequest
MemoryForgetRequest
MemoryExportJob
MemoryLearningSettings
ProblemDetail
```

## 31. Backend API Contract Draft

All backend-api paths use `/backend/v3/api`. Backend API is for `backend-admin`, operators, control plane, support, audit, and automation. It must not expose login/session endpoints.

Base path:

```text
/backend/v3/api/memory
```

Operation draft:

| Method | Path | operationId | Purpose |
| --- | --- | --- | --- |
| `GET` | `/backend/v3/api/memory/spaces` | `spaces.list` | Admin list memory spaces. |
| `GET` | `/backend/v3/api/memory/spaces/{spaceId}` | `spaces.retrieve` | Admin retrieve memory space. |
| `PATCH` | `/backend/v3/api/memory/spaces/{spaceId}` | `spaces.update` | Admin update memory space state/policy. |
| `GET` | `/backend/v3/api/memory/memories` | `memories.list` | Admin search memory records. |
| `GET` | `/backend/v3/api/memory/memories/{memoryId}` | `memories.retrieve` | Admin retrieve memory record. |
| `PATCH` | `/backend/v3/api/memory/memories/{memoryId}` | `memories.update` | Admin correct/suppress memory record. |
| `POST` | `/backend/v3/api/memory/memories/{memoryId}/supersede` | `memories.supersede` | Supersede memory with corrected fact. |
| `GET` | `/backend/v3/api/memory/events` | `events.list` | Admin list evidence events. |
| `GET` | `/backend/v3/api/memory/events/{eventId}` | `events.retrieve` | Admin retrieve event details. |
| `GET` | `/backend/v3/api/memory/candidates` | `candidates.list` | Admin list candidate memories. |
| `POST` | `/backend/v3/api/memory/candidates/{candidateId}/approve` | `candidates.approve` | Admin approve candidate. |
| `POST` | `/backend/v3/api/memory/candidates/{candidateId}/reject` | `candidates.reject` | Admin reject candidate. |
| `POST` | `/backend/v3/api/memory/extraction_jobs` | `extractionJobs.create` | Run extraction over events. |
| `GET` | `/backend/v3/api/memory/extraction_jobs/{jobId}` | `extractionJobs.retrieve` | Retrieve extraction job. |
| `POST` | `/backend/v3/api/memory/consolidation_jobs` | `consolidationJobs.create` | Run dedupe/merge/supersession. |
| `POST` | `/backend/v3/api/memory/indexes` | `indexes.create` | Create index definition. |
| `GET` | `/backend/v3/api/memory/indexes` | `indexes.list` | List index definitions. |
| `GET` | `/backend/v3/api/memory/indexes/{indexId}` | `indexes.retrieve` | Retrieve index definition. |
| `PATCH` | `/backend/v3/api/memory/indexes/{indexId}` | `indexes.update` | Update index definition. |
| `POST` | `/backend/v3/api/memory/indexes/{indexId}/rebuild` | `indexes.rebuild` | Rebuild derived index. |
| `POST` | `/backend/v3/api/memory/retrieval_profiles` | `retrievalProfiles.create` | Create retrieval profile. |
| `GET` | `/backend/v3/api/memory/retrieval_profiles` | `retrievalProfiles.list` | List retrieval profiles. |
| `GET` | `/backend/v3/api/memory/retrieval_profiles/{profileId}` | `retrievalProfiles.retrieve` | Retrieve retrieval profile. |
| `PATCH` | `/backend/v3/api/memory/retrieval_profiles/{profileId}` | `retrievalProfiles.update` | Update retrieval profile. |
| `POST` | `/backend/v3/api/memory/implementation_profiles` | `implementationProfiles.create` | Create implementation profile. |
| `GET` | `/backend/v3/api/memory/implementation_profiles` | `implementationProfiles.list` | List implementation profiles. |
| `GET` | `/backend/v3/api/memory/implementation_profiles/{profileId}` | `implementationProfiles.retrieve` | Retrieve implementation profile. |
| `PATCH` | `/backend/v3/api/memory/implementation_profiles/{profileId}` | `implementationProfiles.update` | Update implementation profile. |
| `POST` | `/backend/v3/api/memory/provider_bindings` | `providerBindings.create` | Register provider binding. |
| `GET` | `/backend/v3/api/memory/provider_bindings` | `providerBindings.list` | List provider bindings. |
| `PATCH` | `/backend/v3/api/memory/provider_bindings/{bindingId}` | `providerBindings.update` | Update provider binding. |
| `GET` | `/backend/v3/api/memory/provider_health` | `providerHealth.retrieve` | Retrieve provider health summary. |
| `POST` | `/backend/v3/api/memory/eval_runs` | `evalRuns.create` | Start memory eval run. |
| `GET` | `/backend/v3/api/memory/eval_runs` | `evalRuns.list` | List eval runs. |
| `GET` | `/backend/v3/api/memory/eval_runs/{evalRunId}` | `evalRuns.retrieve` | Retrieve eval run result. |
| `GET` | `/backend/v3/api/memory/retrieval_traces` | `retrievalTraces.list` | List retrieval traces. |
| `GET` | `/backend/v3/api/memory/retrieval_traces/{traceId}` | `retrievalTraces.retrieve` | Retrieve retrieval trace. |
| `GET` | `/backend/v3/api/memory/audit_logs` | `auditLogs.list` | List memory audit logs. |
| `POST` | `/backend/v3/api/memory/retention_jobs` | `retentionJobs.create` | Run retention/deletion propagation job. |
| `POST` | `/backend/v3/api/memory/migration_jobs` | `migrationJobs.create` | Start implementation/profile migration. |
| `GET` | `/backend/v3/api/memory/migration_jobs/{migrationJobId}` | `migrationJobs.retrieve` | Retrieve migration job. |

Backend API permissions:

```text
memory.admin.spaces.read
memory.admin.spaces.write
memory.admin.records.read
memory.admin.records.write
memory.admin.events.read
memory.admin.candidates.review
memory.admin.indexes.manage
memory.admin.retrievalProfiles.manage
memory.admin.implementationProfiles.manage
memory.admin.providers.manage
memory.admin.evals.run
memory.admin.traces.read
memory.admin.audit.read
memory.admin.retention.run
memory.admin.migrations.run
```

Backend API DTOs:

```text
MemoryImplementationProfile
MemoryImplementationProfileRequest
MemoryProviderBinding
MemoryProviderBindingRequest
MemoryProviderHealth
MemoryIndex
MemoryIndexRequest
MemoryIndexRebuildRequest
MemoryRetrievalProfile
MemoryRetrievalProfileRequest
MemoryExtractionJob
MemoryConsolidationJob
MemoryEvalRun
MemoryEvalRunRequest
MemoryAuditLog
MemoryRetentionJob
MemoryMigrationJob
ProblemDetail
```

## 32. Database And Storage Design

### 32.1 Storage Principles

Memory storage follows the SDKWork database standard:

1. Tables are defined in schema registry before migrations.
2. PostgreSQL is the server/production target.
3. SQLite is the local/private/test target where feasible.
4. SQL canonical tables are the default source of truth.
5. Derived indexes are rebuildable.
6. High-frequency filters must be real columns, not hidden only in JSON.
7. `int64` values serialize as strings in API/SDK contracts.
8. Sensitive payloads must be redacted, encrypted, externalized to Drive, or rejected by policy.

### 32.2 Table Groups

Schema registry files:

```text
docs/schema-registry/tables/001-memory-core.yaml
docs/schema-registry/tables/002-memory-learning.yaml
docs/schema-registry/tables/003-memory-retrieval.yaml
docs/schema-registry/tables/004-memory-provider.yaml
docs/schema-registry/tables/005-memory-governance.yaml
```

Physical migrations:

```text
database/migrations/postgres/0001_memory_phase1.up.sql
tests/fixtures/database/sqlite/migrations/0001_memory_phase1.up.sql
```

### 32.3 Core Tables

#### `ai_space`

Memory namespace for user, project, organization, agent, or global memory.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
organization_id BIGINT NOT NULL DEFAULT 0
owner_subject_type VARCHAR(64) NOT NULL
owner_subject_id VARCHAR(128) NOT NULL
space_type VARCHAR(64) NOT NULL
display_name VARCHAR(200) NOT NULL
description TEXT
default_policy_id BIGINT
implementation_profile_id BIGINT
learning_enabled BOOLEAN NOT NULL DEFAULT TRUE
habit_learning_enabled BOOLEAN NOT NULL DEFAULT TRUE
status INTEGER NOT NULL
created_by BIGINT
updated_by BIGINT
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_space_uuid (tenant_id, uuid)
uk_ai_space_owner_type (tenant_id, owner_subject_type, owner_subject_id, space_type)
idx_ai_space_tenant_status (tenant_id, status, updated_at)
```

#### `ai_event`

Append-only evidence log.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
organization_id BIGINT NOT NULL DEFAULT 0
space_id BIGINT NOT NULL
session_id VARCHAR(128)
agent_id VARCHAR(128)
actor_type VARCHAR(64) NOT NULL
actor_id BIGINT
event_type VARCHAR(64) NOT NULL
event_time TIMESTAMP NOT NULL
source_uri TEXT
content_text_redacted TEXT
payload_ref_type VARCHAR(64)
payload_drive_object_ref_id BIGINT
payload_hash VARCHAR(128)
structured_payload JSONB
sensitivity VARCHAR(64) NOT NULL
retention_policy_id BIGINT
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_event_uuid (tenant_id, uuid)
idx_ai_event_space_time (tenant_id, space_id, event_time, id)
idx_ai_event_session_time (tenant_id, session_id, event_time)
idx_ai_event_type_time (tenant_id, event_type, event_time)
idx_ai_event_hash (tenant_id, payload_hash)
```

#### `ai_record`

Canonical memory record.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
organization_id BIGINT NOT NULL DEFAULT 0
space_id BIGINT NOT NULL
user_id BIGINT
project_id VARCHAR(128)
agent_id VARCHAR(128)
scope VARCHAR(64) NOT NULL
memory_type VARCHAR(64) NOT NULL
subject VARCHAR(256)
predicate VARCHAR(128)
object_text TEXT
content TEXT NOT NULL
structured_data JSONB
keywords JSONB
aliases JSONB
entities JSONB
tags JSONB
confidence DOUBLE PRECISION NOT NULL
importance DOUBLE PRECISION NOT NULL
sensitivity VARCHAR(64) NOT NULL
stage VARCHAR(64) NOT NULL
status INTEGER NOT NULL
valid_from TIMESTAMP
valid_to TIMESTAMP
expires_at TIMESTAMP
supersedes_memory_id BIGINT
superseded_by_memory_id BIGINT
created_by_type VARCHAR(64) NOT NULL
created_by_id BIGINT
updated_by BIGINT
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
deleted_at TIMESTAMP
deleted_by BIGINT
version BIGINT NOT NULL DEFAULT 0
index_hints JSONB
metadata JSONB
```

Indexes:

```text
uk_ai_record_uuid (tenant_id, uuid)
idx_ai_record_scope_type_status (tenant_id, space_id, scope, memory_type, status, updated_at)
idx_ai_record_user_type (tenant_id, user_id, memory_type, status, updated_at)
idx_ai_record_subject_predicate (tenant_id, space_id, subject, predicate, status)
idx_ai_record_validity (tenant_id, valid_from, valid_to, expires_at)
idx_ai_record_supersession (tenant_id, supersedes_memory_id, superseded_by_memory_id)
```

#### `ai_record_source`

Many-to-many link between memory records and evidence events.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
memory_id BIGINT NOT NULL
event_id BIGINT NOT NULL
source_role VARCHAR(64) NOT NULL
evidence_weight DOUBLE PRECISION
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_record_source_uuid (tenant_id, uuid)
uk_ai_record_source_pair (tenant_id, memory_id, event_id, source_role)
idx_ai_record_source_event (tenant_id, event_id)
```

#### `ai_entity`

Entity dictionary for graph, dictionary, and dedupe.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT NOT NULL
entity_type VARCHAR(64) NOT NULL
canonical_name VARCHAR(256) NOT NULL
aliases JSONB
external_refs JSONB
attributes JSONB
sensitivity VARCHAR(64) NOT NULL
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_entity_uuid (tenant_id, uuid)
uk_ai_entity_name (tenant_id, space_id, entity_type, canonical_name)
idx_ai_entity_type_status (tenant_id, space_id, entity_type, status)
```

#### `ai_edge`

Graph-compatible relationship record.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT NOT NULL
source_entity_id BIGINT NOT NULL
relation_type VARCHAR(128) NOT NULL
target_entity_id BIGINT NOT NULL
source_memory_id BIGINT
confidence DOUBLE PRECISION NOT NULL
valid_from TIMESTAMP
valid_to TIMESTAMP
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_edge_uuid (tenant_id, uuid)
idx_ai_edge_source (tenant_id, space_id, source_entity_id, relation_type, status)
idx_ai_edge_target (tenant_id, space_id, target_entity_id, relation_type, status)
idx_ai_edge_validity (tenant_id, valid_from, valid_to)
```

### 32.4 Learning Tables

#### `ai_candidate`

Candidate memory proposed by extraction or user action.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT NOT NULL
candidate_type VARCHAR(64) NOT NULL
proposed_action VARCHAR(64) NOT NULL
memory_type VARCHAR(64) NOT NULL
scope VARCHAR(64) NOT NULL
content TEXT NOT NULL
structured_data JSONB
keywords JSONB
entities JSONB
source_event_ids JSONB
target_memory_id BIGINT
confidence DOUBLE PRECISION NOT NULL
importance DOUBLE PRECISION NOT NULL
sensitivity VARCHAR(64) NOT NULL
decision_state VARCHAR(64) NOT NULL
decision_reason TEXT
decided_by BIGINT
decided_at TIMESTAMP
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_candidate_uuid (tenant_id, uuid)
idx_ai_candidate_state (tenant_id, space_id, decision_state, updated_at)
idx_ai_candidate_target (tenant_id, target_memory_id)
```

#### `ai_habit`

Habit memory state.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT NOT NULL
user_id BIGINT
agent_id VARCHAR(128)
habit_key VARCHAR(256) NOT NULL
context_pattern TEXT NOT NULL
preferred_behavior TEXT NOT NULL
evidence_count INTEGER NOT NULL DEFAULT 0
positive_feedback_count INTEGER NOT NULL DEFAULT 0
negative_feedback_count INTEGER NOT NULL DEFAULT 0
success_count INTEGER NOT NULL DEFAULT 0
contradiction_count INTEGER NOT NULL DEFAULT 0
confidence DOUBLE PRECISION NOT NULL
stage VARCHAR(64) NOT NULL
activation_policy JSONB
exceptions JSONB
last_observed_at TIMESTAMP
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_habit_uuid (tenant_id, uuid)
uk_ai_habit_key (tenant_id, space_id, user_id, habit_key)
idx_ai_habit_stage (tenant_id, space_id, stage, confidence, updated_at)
```

#### `ai_habit_signal`

Evidence used to form habits.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT NOT NULL
habit_id BIGINT
event_id BIGINT NOT NULL
signal_type VARCHAR(64) NOT NULL
signal_weight DOUBLE PRECISION NOT NULL
observed_at TIMESTAMP NOT NULL
metadata JSONB
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_habit_signal_uuid (tenant_id, uuid)
idx_ai_habit_signal_habit (tenant_id, habit_id, observed_at)
idx_ai_habit_signal_event (tenant_id, event_id)
```

#### `ai_learning_job`

Async extraction, consolidation, decay, retention, and migration jobs.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT
job_type VARCHAR(64) NOT NULL
state VARCHAR(64) NOT NULL
priority INTEGER NOT NULL DEFAULT 0
progress INTEGER NOT NULL DEFAULT 0
idempotency_key VARCHAR(128) NOT NULL
request_id VARCHAR(64)
trace_id VARCHAR(128)
error_code VARCHAR(128)
error_detail VARCHAR(4000)
started_at TIMESTAMP
finished_at TIMESTAMP
metadata JSONB
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_learning_job_uuid (tenant_id, uuid)
uk_ai_learning_job_idempotency (tenant_id, job_type, idempotency_key)
idx_ai_learning_job_state (tenant_id, job_type, state, priority, created_at)
```

### 32.5 Retrieval And Index Tables

#### `ai_index`

Derived index definition.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT NOT NULL
index_kind VARCHAR(64) NOT NULL
provider_binding_id BIGINT
schema_version VARCHAR(128) NOT NULL
embedding_provider_id VARCHAR(128)
embedding_model VARCHAR(128)
embedding_dimension INTEGER
embedding_metric VARCHAR(64)
index_config JSONB
last_rebuild_job_id BIGINT
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_index_uuid (tenant_id, uuid)
uk_ai_index_kind_space (tenant_id, space_id, index_kind, schema_version)
idx_ai_index_status (tenant_id, space_id, index_kind, status)
```

#### `ai_index_entry`

Provider-neutral pointer to derived index state.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
index_id BIGINT NOT NULL
memory_id BIGINT NOT NULL
entry_kind VARCHAR(64) NOT NULL
entry_hash VARCHAR(128) NOT NULL
external_ref TEXT
metadata JSONB
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_index_entry_uuid (tenant_id, uuid)
uk_ai_index_entry_memory (tenant_id, index_id, memory_id, entry_kind)
idx_ai_index_entry_hash (tenant_id, index_id, entry_hash)
```

#### `ai_retrieval_profile`

Retrieval strategy configuration.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT
name VARCHAR(200) NOT NULL
strategy VARCHAR(64) NOT NULL
retriever_plan JSONB NOT NULL
top_k INTEGER NOT NULL
min_score DOUBLE PRECISION
rerank_enabled BOOLEAN NOT NULL DEFAULT FALSE
query_rewrite_enabled BOOLEAN NOT NULL DEFAULT FALSE
context_budget_tokens INTEGER NOT NULL
filter_policy JSONB
citation_policy JSONB
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_retrieval_profile_uuid (tenant_id, uuid)
idx_ai_retrieval_profile_scope (tenant_id, space_id, status, updated_at)
```

#### `ai_retrieval_trace`

Retrieval and context assembly trace.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT
actor_id BIGINT
retrieval_profile_id BIGINT
query_hash VARCHAR(128) NOT NULL
query_text_redacted TEXT
request_payload JSONB
retriever_plan JSONB
latency_ms BIGINT
result_count INTEGER NOT NULL DEFAULT 0
context_token_count INTEGER
error_code VARCHAR(128)
error_detail VARCHAR(4000)
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_retrieval_trace_uuid (tenant_id, uuid)
idx_ai_retrieval_trace_profile_created (tenant_id, retrieval_profile_id, created_at)
idx_ai_retrieval_trace_actor_created (tenant_id, actor_id, created_at)
```

#### `ai_retrieval_hit`

Per-memory retrieval hit evidence.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
retrieval_trace_id BIGINT NOT NULL
memory_id BIGINT NOT NULL
retriever_kind VARCHAR(64) NOT NULL
retriever_name VARCHAR(128) NOT NULL
match_type VARCHAR(64) NOT NULL
score DOUBLE PRECISION
result_rank INTEGER NOT NULL
match_reason VARCHAR(512)
highlights JSONB
metadata JSONB
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_retrieval_hit_uuid (tenant_id, uuid)
idx_ai_retrieval_hit_trace_rank (tenant_id, retrieval_trace_id, result_rank)
idx_ai_retrieval_hit_memory (tenant_id, memory_id, status)
```

#### `ai_context_pack`

Assembled model-ready memory context.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT
retrieval_trace_id BIGINT
actor_id BIGINT
context_hash VARCHAR(128) NOT NULL
context_text_redacted TEXT NOT NULL
selected_memory_ids JSONB
token_count INTEGER
source_summary JSONB
metadata JSONB
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_context_pack_uuid (tenant_id, uuid)
idx_ai_context_pack_trace (tenant_id, retrieval_trace_id)
idx_ai_context_pack_actor_created (tenant_id, actor_id, created_at)
```

### 32.6 Provider And Policy Tables

#### `ai_implementation_profile`

Runtime profile for switching implementation families.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
name VARCHAR(200) NOT NULL
implementation_kind VARCHAR(64) NOT NULL
deployment_mode VARCHAR(64) NOT NULL
canonical_store_binding_id BIGINT
event_store_binding_id BIGINT
policy_store_binding_id BIGINT
retriever_bindings JSONB
provider_bindings JSONB
fallback_profile_id BIGINT
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_implementation_profile_uuid (tenant_id, uuid)
idx_ai_implementation_profile_kind (tenant_id, implementation_kind, status)
```

#### `ai_provider_binding`

Model, embedding, search, graph, external memory, and storage provider binding.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
provider_kind VARCHAR(64) NOT NULL
provider_code VARCHAR(128) NOT NULL
display_name VARCHAR(200) NOT NULL
capabilities JSONB NOT NULL
config_ref VARCHAR(512)
safe_config JSONB
secret_ref VARCHAR(512)
health_state VARCHAR(64)
last_health_check_at TIMESTAMP
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_provider_binding_uuid (tenant_id, uuid)
uk_ai_provider_binding_code (tenant_id, provider_kind, provider_code)
idx_ai_provider_binding_health (tenant_id, provider_kind, health_state, updated_at)
```

#### `ai_policy`

Learning, retention, sensitivity, retrieval, and deletion policy.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
policy_type VARCHAR(64) NOT NULL
name VARCHAR(200) NOT NULL
scope VARCHAR(64) NOT NULL
policy_document JSONB NOT NULL
status INTEGER NOT NULL
created_by BIGINT
updated_by BIGINT
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_policy_uuid (tenant_id, uuid)
idx_ai_policy_type_scope (tenant_id, policy_type, scope, status)
```

### 32.7 Governance Tables

#### `ai_audit_log`

Audit log for memory operations.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
organization_id BIGINT NOT NULL DEFAULT 0
actor_type VARCHAR(64) NOT NULL
actor_id BIGINT
action VARCHAR(128) NOT NULL
resource_type VARCHAR(64) NOT NULL
resource_id VARCHAR(128) NOT NULL
result VARCHAR(64) NOT NULL
request_id VARCHAR(64)
trace_id VARCHAR(128)
api_surface VARCHAR(64)
operation_id VARCHAR(128)
reason TEXT
before_state JSONB
after_state JSONB
created_at TIMESTAMP NOT NULL
```

Indexes:

```text
uk_ai_audit_log_uuid (tenant_id, uuid)
idx_ai_audit_actor_time (tenant_id, actor_type, actor_id, created_at)
idx_ai_audit_resource_time (tenant_id, resource_type, resource_id, created_at)
idx_ai_audit_action_time (tenant_id, action, created_at)
```

#### `ai_eval_run`

Memory quality evaluation run.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
space_id BIGINT
eval_type VARCHAR(64) NOT NULL
dataset_ref VARCHAR(512)
profile_id BIGINT
state VARCHAR(64) NOT NULL
metrics JSONB
report_ref VARCHAR(512)
started_at TIMESTAMP
finished_at TIMESTAMP
error_code VARCHAR(128)
error_detail VARCHAR(4000)
status INTEGER NOT NULL
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_eval_run_uuid (tenant_id, uuid)
idx_ai_eval_run_type_state (tenant_id, eval_type, state, created_at)
```

#### `ai_outbox_event`

Transactional event publication.

```text
id BIGINT PRIMARY KEY
uuid VARCHAR(64) NOT NULL
tenant_id BIGINT NOT NULL
event_type VARCHAR(128) NOT NULL
aggregate_type VARCHAR(64) NOT NULL
aggregate_id VARCHAR(128) NOT NULL
payload JSONB NOT NULL
payload_hash VARCHAR(128) NOT NULL
publish_state VARCHAR(64) NOT NULL
attempt_count INTEGER NOT NULL DEFAULT 0
last_error VARCHAR(4000)
created_at TIMESTAMP NOT NULL
updated_at TIMESTAMP NOT NULL
version BIGINT NOT NULL DEFAULT 0
```

Indexes:

```text
uk_ai_outbox_event_uuid (tenant_id, uuid)
idx_ai_outbox_state (tenant_id, publish_state, created_at)
```

## 33. Event Contract Draft

Memory emits versioned domain events using the SDKWork event standard.

Event types:

```text
memory.space.created
memory.event.appended
memory.record.created
memory.record.updated
memory.record.deleted
memory.record.superseded
memory.candidate.created
memory.candidate.approved
memory.candidate.rejected
memory.habit.promoted
memory.habit.decayed
memory.index.rebuild_requested
memory.index.rebuild_completed
memory.context_pack.created
memory.retention.deleted
memory.provider.health_changed
```

Envelope fields:

```text
id
type
source = sdkwork-memory
specversion = 1.0
time
tenantId
organizationId
subject
data
```

Sensitive memory content must not be published in events. Events carry IDs, redacted summaries, states, and safe metadata.

## 34. Implementation Switching And Migration

Implementation switching is a governed workflow.

Switch classes:

| Switch | Risk | Requirements |
| --- | --- | --- |
| Add optional retriever | Low | Register provider, create index, run eval, enable in retrieval profile. |
| Disable optional retriever | Low | Remove from retrieval profile; keep canonical records unchanged. |
| Change LLM extraction provider | Medium | Run candidate extraction eval, shadow extraction, compare write precision/recall. |
| Change embedding model | Medium | Create new embedding space, dual index, rebuild, shadow retrieve, cut over. |
| Change canonical store | High | Export/import, dual write or freeze, consistency checks, rollback plan, human approval. |
| Bridge to external provider | High | Shadow records, deletion propagation tests, export/import contract, privacy review. |

Migration job stages:

```text
planned
validated
exporting
importing
dual_writing
shadow_reading
verifying
cutting_over
completed
failed
rolled_back
```

Required migration evidence:

```text
source profile
target profile
record counts
event counts
source hash sample
target hash sample
retrieval eval result
deletion propagation result
rollback plan
operator approval
```

## 35. API/OpenAPI Standard Checklist

The generated OpenAPI contracts must pass this checklist:

1. Open API paths start with `/mem/v3/api` and do not use `/app/v3/api` or `/backend/v3/api`.
2. App API paths start with `/app/v3/api`.
3. Backend API paths start with `/backend/v3/api`.
4. No open-api or backend-api login/session endpoints exist.
5. Tags use `memory`.
6. Operation IDs use dotted lowerCamelCase resource style.
7. Every generated operation declares `x-sdkwork-owner: sdkwork-memory`.
8. Open operations declare `x-sdkwork-api-authority: sdkwork-memory-open-api`.
9. App operations declare `x-sdkwork-api-authority: sdkwork-memory-app-api` (early drafts of this record showed the dotted `sdkwork-memory.app` alias; it was retired — API_SPEC requires kebab-case identities).
10. Backend operations declare `x-sdkwork-api-authority: sdkwork-memory-backend-api` (early drafts of this record showed the dotted `sdkwork-memory.backend` alias; it was retired — API_SPEC requires kebab-case identities).
11. Open API `components.securitySchemes` declares only `ApiKey` / `X-API-Key` for protected operations.
12. App and Backend API `components.securitySchemes` declare only `AuthToken` and `AccessToken` for protected operations.
13. Protected open-api operations use `ApiKey` security and `x-sdkwork-auth-mode: api-key`.
14. Protected app/backend operations use dual-token security.
15. Public operations, if any, explicitly set `security: []` and `x-sdkwork-auth-mode: anonymous`.
16. All errors include `application/problem+json`.
17. List APIs are paginated.
18. Retriable create/command APIs support `Idempotency-Key`.
19. `int64` and decimal API fields serialize as strings.
20. Sensitive fields are redacted or write-only.
21. Generated SDKs compile and expose resource-style methods.

## 36. Updated Roadmap For SDKWork Delivery

### Phase 0: Standard Skeleton

Deliver:

```text
AGENTS.md and shims
sdkwork.app.config.json
specs/component.spec.json
docs/schema-registry/tables/*.yaml
sdks/sdkwork-memory-sdk skeleton
sdks/sdkwork-memory-app-sdk skeleton
sdks/sdkwork-memory-backend-sdk skeleton
OpenAPI authority drafts for open/app/backend
SDK assembly metadata
verification script skeleton
```

### Phase 1: No-Embedding Native SQL MVP

Deliver:

```text
PostgreSQL/SQLite migrations
ai_space, ai_event, ai_record, ai_record_source, ai_candidate, ai_habit
SQL/keyword/dictionary/time/event retrievers
open-api route coverage
app-api route coverage
backend-api route coverage for indexes/profiles/providers
generated TypeScript open/app/backend SDKs
basic eval harness
```

### Phase 2: Self-Learning Provider Layer

Deliver:

```text
LanguageModel provider interface
structured extractor
candidate judge
habit signal pipeline
policy/sensitivity checks
provider bindings
implementation profiles
provider health
```

### Phase 3: Optional Search, Vector, And Rerank

Deliver:

```text
OpenSearch/Lucene provider
EmbeddingModel interface
Vector retriever
RerankModel interface
index rebuild jobs
retrieval eval gates
```

### Phase 4: Graph, File, And External Provider Bridge

Deliver:

```text
entity resolver
graph retriever
grep/file retriever
Drive-backed replay/export packages
Mem0/Zep/Letta bridge adapters where approved
external provider deletion propagation tests
```

### Phase 5: Platform-Grade Governance

Deliver:

```text
memory admin dashboard API
retention/deletion automation
tenant policy management
eval dashboards
observability dashboards
multi-language SDK generation
RPC SDK if required
release and migration runbooks
```

## 37. Updated Open Decisions

1. Whether the first implementation runtime is Rust-first like Knowledgebase/Drive or Java/Spring-first with Rust local/private parity later.
2. Whether Drive is a required dependency from Phase 1 or optional until export/import packages are implemented.
3. Whether Knowledgebase integration is a dependency SDK, provider adapter, or only a documented composition pattern in Phase 1.
4. Whether table IDs use service-generated Snowflake IDs like Knowledgebase or Drive-style varchar logical IDs.
5. Whether OpenSearch is introduced in Phase 1 or keyword retrieval starts with PostgreSQL full-text and SQLite FTS only.
6. Whether graph memory starts with relational `ai_entity`/`ai_edge` tables or an external graph provider behind the same port.
7. Which implementation provider bridges are first-class in the standard: native SQL only, or native SQL plus one external bridge.
