//! Runtime composition for the search-first vector plugin.
//!
//! The runtime is the only place where a provider port and the derived index meet.
//! Providers are injected, never constructed: this crate contains no HTTP client,
//! no endpoint literal, and no credential lookup, so a provider swap is a
//! composition-root decision rather than a code change here.

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use sdkwork_memory_spi::{
    EmbeddingCommand, EmbeddingModelPort, LanguageModelCommand, LanguageModelPort, MemoryIndexPort,
    MemoryIndexReceipt, MemoryRetrievalRecordCandidate, MemoryRetrieverKind, MemoryRetrieverPort,
    MemoryRetrieverResult, MemoryRetrieverSearchResult, MemoryScopeContext,
    MemorySensitivityReadScope, MemorySpiResult, RerankMemoryHitsCommand, RerankModelPort,
    RetrieveMemoryCandidatesCommand, SearchMemoryCandidatesQuery,
};

use crate::additive::{plan_additions, AdditivePlan};
use crate::config::SearchFirstVectorConfig;
use crate::error::{
    Result, SearchFirstVectorError, EMBEDDING_MODEL_PORT, INDEX_PORT, LANGUAGE_MODEL_PORT,
};
use crate::extraction::{
    extract_memories, today_utc_date, AdditiveExtractionRequest, ConversationTurn,
    ExistingMemoryView, ExtractionReport,
};
use crate::procedural::{build_procedural_prompt, ProceduralMemoryPlan};
use crate::vector_index::{
    ScopeKey, ScopedVectorIndex, VectorHit, VectorProjectionCommand, VectorProjectionOutcome,
    VectorRankFilters, VectorRecordProjection,
};

/// Retriever code reported through `MemoryRetrieverPort::retriever_code`.
pub const RETRIEVER_CODE: &str = "search_first_vector";

/// Index kind reported through `MemoryIndexPort::index_kind`.
pub const INDEX_KIND: &str = "vector";

/// Everything one additive write turn produced.
///
/// Both halves are reported together because a caller that only saw the plan
/// could not tell a response that yielded nothing from one whose emissions were
/// refused, and a caller that only saw the report could not tell what was
/// actually committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditiveWritePlan {
    /// What extraction produced, including anything it refused or truncated.
    pub report: ExtractionReport,
    /// What the plan would commit, suppress, and could not link.
    pub plan: AdditivePlan,
}

/// Reported when no embedding provider binding is available.
pub const DEGRADATION_EMBEDDING_PORT_UNBOUND: &str = "embedding_provider_unbound";

/// Reported when the bound embedding provider refused the request.
pub const DEGRADATION_EMBEDDING_CALL_FAILED: &str = "embedding_provider_failed";

/// Reported when the bound rerank provider refused the request.
pub const DEGRADATION_RERANK_CALL_FAILED: &str = "rerank_provider_failed";

/// Reported when none of the requested retriever kinds is served here.
pub const DEGRADATION_RETRIEVER_KIND_UNSERVED: &str = "no_requested_retriever_kind_is_served";

/// Search-first vector runtime.
pub struct SearchFirstVectorRuntime {
    config: SearchFirstVectorConfig,
    embedding: Option<Arc<dyn EmbeddingModelPort>>,
    language_model: Option<Arc<dyn LanguageModelPort>>,
    rerank: Option<Arc<dyn RerankModelPort>>,
    /// Present exactly when an embedding provider is bound, because a derived
    /// vector index without an embedding provider can never answer.
    index: Option<RwLock<ScopedVectorIndex>>,
}

impl std::fmt::Debug for SearchFirstVectorRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SearchFirstVectorRuntime")
            .field("config", &self.config)
            .field("embedding_bound", &self.embedding.is_some())
            .field("language_model_bound", &self.language_model.is_some())
            .field("rerank_bound", &self.rerank.is_some())
            .finish()
    }
}

impl SearchFirstVectorRuntime {
    /// Compose a runtime from validated configuration and injected providers.
    ///
    /// The embedding provider is optional so a deployment can boot before its
    /// provider binding is resolved: an unbound runtime answers every search with
    /// an explicit degradation instead of pretending to have searched.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::InvalidConfiguration`] when `config`
    /// fails validation or the bound embedding provider reports zero dimensions,
    /// and propagates a rejected index construction.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{
    ///     SearchFirstVectorConfig, SearchFirstVectorRuntime,
    /// };
    ///
    /// let runtime = SearchFirstVectorRuntime::without_providers(
    ///     SearchFirstVectorConfig::default(),
    /// )
    /// .unwrap();
    ///
    /// assert!(!runtime.embedding_bound());
    /// assert_eq!(runtime.indexed_len().unwrap(), 0);
    /// ```
    #[must_use = "a rejected runtime construction must be handled, not discarded"]
    pub fn new(
        config: SearchFirstVectorConfig,
        embedding: Option<Arc<dyn EmbeddingModelPort>>,
        language_model: Option<Arc<dyn LanguageModelPort>>,
        rerank: Option<Arc<dyn RerankModelPort>>,
    ) -> Result<Self> {
        config.validate()?;

        let index = match embedding.as_ref() {
            Some(provider) => Some(RwLock::new(ScopedVectorIndex::new(
                provider.dimensions(),
                &config,
            )?)),
            None => None,
        };

        Ok(Self {
            config,
            embedding,
            language_model,
            rerank,
            index,
        })
    }

    /// Compose a runtime with no provider bound.
    pub fn without_providers(config: SearchFirstVectorConfig) -> Result<Self> {
        Self::new(config, None, None, None)
    }

    /// Effective configuration.
    pub fn config(&self) -> &SearchFirstVectorConfig {
        &self.config
    }

    /// Whether an embedding provider is bound.
    pub fn embedding_bound(&self) -> bool {
        self.embedding.is_some()
    }

    /// Whether a language model provider is bound.
    pub fn language_model_bound(&self) -> bool {
        self.language_model.is_some()
    }

    /// Whether a rerank provider is bound.
    pub fn rerank_bound(&self) -> bool {
        self.rerank.is_some()
    }

    /// Embedding width this runtime's index accepts, when one exists.
    pub fn dimensions(&self) -> Option<usize> {
        self.index.as_ref().map(|index| {
            index
                .read()
                .map(|index| index.dimensions())
                .unwrap_or_default()
        })
    }

    /// Number of projections currently held.
    ///
    /// Reports `0` when no embedding provider is bound, because no index exists.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::IndexLockPoisoned`] when the index lock
    /// is poisoned.
    #[must_use = "the projection count must be inspected, not discarded"]
    pub fn indexed_len(&self) -> Result<usize> {
        let Some(lock) = self.index.as_ref() else {
            return Ok(0);
        };
        let index = lock
            .read()
            .map_err(|_| SearchFirstVectorError::IndexLockPoisoned)?;

        Ok(index.len())
    }

    /// Read one projection back, for inspection and tests.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::IndexLockPoisoned`] when the index lock
    /// is poisoned.
    #[must_use = "the looked-up projection must be inspected, not discarded"]
    pub fn projection(
        &self,
        scope: &MemoryScopeContext,
        memory_id: &str,
    ) -> Result<Option<VectorRecordProjection>> {
        let Some(lock) = self.index.as_ref() else {
            return Ok(None);
        };
        let index = lock
            .read()
            .map_err(|_| SearchFirstVectorError::IndexLockPoisoned)?;

        Ok(index
            .projection(&ScopeKey::from_scope(scope), memory_id)
            .cloned())
    }

    /// Project a record's content into the derived vector index.
    ///
    /// This is the supported write path. `MemoryIndexPort::index` cannot be used
    /// for it because that port carries only a memory id, and this plugin will
    /// not synthesise an embedding from an id it cannot read.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::RequiredPortMissing`] when no embedding
    /// provider is bound, [`SearchFirstVectorError::ProviderCallFailed`] when the
    /// provider refuses the request, and propagates the index's own validation.
    #[must_use = "a projection outcome must be handled, not discarded"]
    pub async fn project_record(
        &self,
        scope: &MemoryScopeContext,
        command: VectorProjectionCommand,
    ) -> Result<VectorProjectionOutcome> {
        let Some(embedding_provider) = self.embedding.as_ref() else {
            return Err(SearchFirstVectorError::RequiredPortMissing {
                port: EMBEDDING_MODEL_PORT,
            });
        };

        let embedding = embedding_provider
            .embed(EmbeddingCommand {
                input: command.canonical_text.clone(),
            })
            .await
            .map_err(|source| SearchFirstVectorError::ProviderCallFailed {
                port: EMBEDDING_MODEL_PORT,
                provider: embedding_provider.provider_code().to_string(),
                source,
            })?;

        let scope_key = ScopeKey::from_scope(scope);
        let lock = self
            .index
            .as_ref()
            .ok_or(SearchFirstVectorError::RequiredPortMissing {
                port: EMBEDDING_MODEL_PORT,
            })?;
        let mut index = lock
            .write()
            .map_err(|_| SearchFirstVectorError::IndexLockPoisoned)?;

        index.upsert(scope_key, command, embedding)
    }

    /// Remove a record's projection from the derived index.
    ///
    /// This is how the plugin honours its `deletionPropagation` declaration: a
    /// retired record whose projection survives would keep being returned as a
    /// candidate.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::RequiredPortMissing`] when no embedding
    /// provider is bound, because no index exists to remove from, and
    /// [`SearchFirstVectorError::IndexLockPoisoned`] when the index lock is
    /// poisoned.
    #[must_use = "the removal result must be handled, not discarded"]
    pub fn remove_record(&self, scope: &MemoryScopeContext, memory_id: &str) -> Result<bool> {
        let Some(lock) = self.index.as_ref() else {
            return Err(SearchFirstVectorError::RequiredPortMissing {
                port: EMBEDDING_MODEL_PORT,
            });
        };
        let mut index = lock
            .write()
            .map_err(|_| SearchFirstVectorError::IndexLockPoisoned)?;

        Ok(index.remove(&ScopeKey::from_scope(scope), memory_id))
    }

    /// Extract memories from conversation turns through the language model.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::RequiredPortMissing`] when no language
    /// model is bound, and propagates the language model port's own failures and
    /// an unparseable answer.
    #[must_use = "an extraction report must be handled, not discarded"]
    pub async fn extract_memories(
        &self,
        request: &AdditiveExtractionRequest,
    ) -> Result<ExtractionReport> {
        let Some(language_model) = self.language_model.as_ref() else {
            return Err(SearchFirstVectorError::RequiredPortMissing {
                port: LANGUAGE_MODEL_PORT,
            });
        };

        extract_memories(language_model.as_ref(), request).await
    }

    /// Summarise an agent's execution history into one procedural memory.
    ///
    /// The procedural path never runs additive extraction: the reference sends a
    /// dedicated summarisation prompt and stores the answer as a single record
    /// (`memory/main.py:1993-2037`). Callers reach it through
    /// [`crate::procedural::resolve_write_path`], which is what refuses every
    /// memory type other than the procedural one.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::RequiredPortMissing`] when no language
    /// model is bound, propagates the port's own failures unchanged, and returns
    /// [`SearchFirstVectorError::ProceduralSummaryEmpty`] or
    /// [`SearchFirstVectorError::ProceduralMetadataRequired`] when the answer or
    /// the record shape cannot produce a storable memory.
    #[must_use = "a procedural memory plan must be committed, not discarded"]
    pub async fn plan_procedural_memory(
        &self,
        turns: &[ConversationTurn],
        instruction: Option<&str>,
        has_metadata: bool,
    ) -> Result<ProceduralMemoryPlan> {
        let Some(language_model) = self.language_model.as_ref() else {
            return Err(SearchFirstVectorError::RequiredPortMissing {
                port: LANGUAGE_MODEL_PORT,
            });
        };

        let prompt = build_procedural_prompt(turns, instruction);
        let response = language_model
            .generate(LanguageModelCommand { prompt })
            .await
            .map_err(|source| SearchFirstVectorError::ProviderCallFailed {
                port: LANGUAGE_MODEL_PORT,
                provider: language_model.provider_code().to_string(),
                source,
            })?;

        // Fully qualified so the free function is not confused with this method.
        crate::procedural::plan_procedural_memory(&response, has_metadata)
    }

    /// Decide which extracted memories to commit.
    ///
    /// Deterministic and model-free: the same candidates and the same evidence
    /// always produce the same plan, so this step can be reasoned about and
    /// tested without a provider.
    pub fn plan_additions(
        &self,
        extracted: &[crate::extraction::ExtractedMemory],
        existing: &[ExistingMemoryView],
    ) -> AdditivePlan {
        plan_additions(extracted, existing)
    }

    /// Run the whole additive write path: extract, then plan.
    ///
    /// This is the complete mem0 V3 write path minus persistence, which stays
    /// with the composition root because the canonical store, not this plugin,
    /// owns the records. A later, separate step projects each committed memory
    /// through [`SearchFirstVectorRuntime::project_record`].
    ///
    /// # Errors
    ///
    /// Returns whatever [`SearchFirstVectorRuntime::extract_memories`] returns.
    #[must_use = "an additive write plan must be acted on, not discarded"]
    pub async fn plan_write(
        &self,
        request: &AdditiveExtractionRequest,
        existing: &[ExistingMemoryView],
    ) -> Result<AdditiveWritePlan> {
        let report = self.extract_memories(request).await?;
        let plan = self.plan_additions(&report.memories, existing);

        Ok(AdditiveWritePlan { report, plan })
    }

    async fn search_scoped_inner(
        &self,
        query: SearchMemoryCandidatesQuery,
    ) -> Result<MemoryRetrieverSearchResult> {
        let query_text = query.query.trim().to_string();
        if query_text.is_empty() {
            return Err(SearchFirstVectorError::BlankQuery);
        }
        self.config.validate_limit(query.limit)?;
        for (index, memory_type) in query.memory_types.iter().enumerate() {
            if memory_type.trim().is_empty() {
                return Err(SearchFirstVectorError::BlankMemoryType { index });
            }
        }

        // Refused first, and unconditionally: a filter this plugin cannot
        // evaluate must never be dropped, because dropping it returns records the
        // caller explicitly excluded instead of failing. Checked before the kind
        // degradation below so a hard refusal cannot be masked by a soft path,
        // and before any provider call so no speculative work is done for a
        // request that can never be satisfied.
        if query.metadata_filter.is_some() {
            return Err(SearchFirstVectorError::MetadataFilterUnsupported {
                reason: "this plugin projects no metadata document, so no filter operator can be evaluated",
            });
        }

        if !retriever_kind_is_served(&query.retriever_kinds) {
            tracing::warn!(
                requested = ?query.retriever_kinds,
                "search-first vector plugin does not serve the requested retriever kinds"
            );

            return Ok(degraded_result(
                vec![MemoryRetrieverKind::Vector],
                vec![DEGRADATION_RETRIEVER_KIND_UNSERVED.to_string()],
            ));
        }

        let (Some(embedding_provider), Some(index_lock)) =
            (self.embedding.as_ref(), self.index.as_ref())
        else {
            tracing::warn!(
                code = DEGRADATION_EMBEDDING_PORT_UNBOUND,
                "vector retrieval requested while no embedding provider is bound"
            );

            return Ok(degraded_result(
                vec![MemoryRetrieverKind::Vector],
                vec![DEGRADATION_EMBEDDING_PORT_UNBOUND.to_string()],
            ));
        };

        let query_embedding = match embedding_provider
            .embed(EmbeddingCommand { input: query_text })
            .await
        {
            Ok(embedding) => embedding,
            Err(error) => {
                tracing::warn!(
                    provider = embedding_provider.provider_code(),
                    error = %error,
                    code = DEGRADATION_EMBEDDING_CALL_FAILED,
                    "embedding provider refused a query embedding"
                );

                return Ok(degraded_result(
                    vec![MemoryRetrieverKind::Vector],
                    vec![DEGRADATION_EMBEDDING_CALL_FAILED.to_string()],
                ));
            }
        };

        let scope_key = ScopeKey::from_scope(&query.scope);
        let filters = VectorRankFilters {
            memory_types: query.memory_types.clone(),
            read_scope: query.read_scope,
            include_expired: self.config.include_expired,
            // Read once, here, so the whole ranking pass judges every candidate
            // against the same date.
            as_of_date: today_utc_date(),
        };

        // Filters and the limit are both applied inside the scope bucket, so the
        // lock is held only for the synchronous ranking step.
        let mut hits = {
            let index = index_lock
                .read()
                .map_err(|_| SearchFirstVectorError::IndexLockPoisoned)?;
            index.rank(&scope_key, &query_embedding, &filters, query.limit)?
        };

        let mut degradation_codes = Vec::new();
        if self.config.use_rerank_when_bound && hits.len() > 1 {
            if let Some(rerank_provider) = self.rerank.as_ref() {
                let memory_ids = hits
                    .iter()
                    .map(|hit| hit.projection.memory_id.clone())
                    .collect::<Vec<_>>();

                match rerank_provider
                    .rerank(RerankMemoryHitsCommand { memory_ids })
                    .await
                {
                    Ok(reordered) => {
                        hits = reorder_hits(hits, &reordered.memory_ids);
                    }
                    Err(error) => {
                        tracing::warn!(
                            provider = rerank_provider.provider_code(),
                            error = %error,
                            code = DEGRADATION_RERANK_CALL_FAILED,
                            "rerank provider refused the candidate ordering"
                        );
                        degradation_codes.push(DEGRADATION_RERANK_CALL_FAILED.to_string());
                    }
                }
            }
        }

        let records = hits
            .into_iter()
            .map(|hit| MemoryRetrievalRecordCandidate {
                memory_id: hit.projection.memory_id,
                subject: hit.projection.subject,
                predicate: hit.projection.predicate,
                object_text: hit.projection.object_text,
                canonical_text: hit.projection.canonical_text,
                created_at: hit.projection.created_at,
            })
            .collect();

        Ok(MemoryRetrieverSearchResult {
            records,
            // This plugin has no event retriever; event candidates stay with the
            // event-capable retrievers rather than being synthesised here.
            events: Vec::new(),
            degraded: !degradation_codes.is_empty(),
            unavailable_retriever_kinds: Vec::new(),
            degradation_codes,
        })
    }

    fn ensure_indexed(&self, memory_id: &str) -> Result<MemoryIndexReceipt> {
        if memory_id.trim().is_empty() {
            return Err(SearchFirstVectorError::BlankMemoryId {
                memory_id: memory_id.to_string(),
            });
        }

        let Some(lock) = self.index.as_ref() else {
            return Err(SearchFirstVectorError::RequiredPortMissing {
                port: EMBEDDING_MODEL_PORT,
            });
        };
        let index = lock
            .read()
            .map_err(|_| SearchFirstVectorError::IndexLockPoisoned)?;

        let scopes = index.scopes_holding(memory_id);
        match scopes.as_slice() {
            [] => Err(SearchFirstVectorError::RecordNotIndexed {
                memory_id: memory_id.to_string(),
            }),
            [_single] => Ok(MemoryIndexReceipt {
                memory_id: memory_id.to_string(),
            }),
            // The same canonical id in two scopes means the projection is
            // inconsistent; refuse instead of acting on an arbitrary scope.
            multiple => Err(SearchFirstVectorError::AmbiguousProjection {
                memory_id: memory_id.to_string(),
                scopes: multiple.len(),
            }),
        }
    }
}

#[async_trait]
impl MemoryRetrieverPort for SearchFirstVectorRuntime {
    fn retriever_code(&self) -> &str {
        RETRIEVER_CODE
    }

    fn supports_bounded_scoped_search(&self) -> bool {
        true
    }

    async fn retrieve(
        &self,
        _command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        Err(SearchFirstVectorError::ScopeRequired {
            operation: "MemoryRetrieverPort::retrieve",
            reason: "this plugin serves bounded, scope-aware search only".to_string(),
        }
        .into_spi_error())
    }

    async fn retrieve_scoped(
        &self,
        scope: MemoryScopeContext,
        command: RetrieveMemoryCandidatesCommand,
    ) -> MemorySpiResult<MemoryRetrieverResult> {
        // Bounded by `default_search_limit` rather than unbounded, and read at the
        // least privileged tier because this helper carries no read scope of its
        // own. Privilege is never widened to make a legacy call succeed.
        let query = SearchMemoryCandidatesQuery {
            scope,
            query: command.query,
            limit: self.config.default_search_limit,
            retriever_kinds: vec![MemoryRetrieverKind::Vector],
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Public,
            metadata_filter: None,
        };

        let result = self
            .search_scoped_inner(query)
            .await
            .map_err(SearchFirstVectorError::into_spi_error)?;

        Ok(MemoryRetrieverResult {
            memory_ids: result
                .records
                .into_iter()
                .map(|record| record.memory_id)
                .collect(),
        })
    }

    async fn search_scoped(
        &self,
        query: SearchMemoryCandidatesQuery,
    ) -> MemorySpiResult<MemoryRetrieverSearchResult> {
        self.search_scoped_inner(query)
            .await
            .map_err(SearchFirstVectorError::into_spi_error)
    }
}

#[async_trait]
impl MemoryIndexPort for SearchFirstVectorRuntime {
    fn index_kind(&self) -> &str {
        INDEX_KIND
    }

    async fn index(&self, memory_id: String) -> MemorySpiResult<MemoryIndexReceipt> {
        // Report the port whose operation failed, while the message names the
        // provider dependency that is actually missing.
        self.ensure_indexed(&memory_id)
            .map_err(|error| error.into_spi_error_on(INDEX_PORT))
    }
}

/// Whether any of the requested retriever kinds is served here.
///
/// An empty request means the caller applied no kind restriction. That cannot
/// widen the result beyond this plugin's single declared kind, so it is served.
fn retriever_kind_is_served(kinds: &[MemoryRetrieverKind]) -> bool {
    kinds.is_empty() || kinds.contains(&MemoryRetrieverKind::Vector)
}

fn degraded_result(
    unavailable_retriever_kinds: Vec<MemoryRetrieverKind>,
    degradation_codes: Vec<String>,
) -> MemoryRetrieverSearchResult {
    MemoryRetrieverSearchResult {
        records: Vec::new(),
        events: Vec::new(),
        degraded: true,
        unavailable_retriever_kinds,
        degradation_codes,
    }
}

/// Reorder hits to follow a rerank verdict.
///
/// An id the reranker did not mention keeps its relative vector order after the
/// mentioned ids, so a partial verdict can never drop a candidate.
fn reorder_hits(hits: Vec<VectorHit>, order: &[String]) -> Vec<VectorHit> {
    let mut remaining = hits.into_iter().map(Some).collect::<Vec<_>>();
    let mut ordered = Vec::with_capacity(remaining.len());

    for memory_id in order {
        let position = remaining.iter().position(|entry| {
            entry
                .as_ref()
                .is_some_and(|hit| &hit.projection.memory_id == memory_id)
        });

        if let Some(position) = position {
            if let Some(hit) = remaining[position].take() {
                ordered.push(hit);
            }
        }
    }

    ordered.extend(remaining.into_iter().flatten());

    ordered
}
