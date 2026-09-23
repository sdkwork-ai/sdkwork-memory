//! Plugin-local error surface.
//!
//! Every variant keeps its structured cause instead of flattening it into a
//! `String`, so [`std::error::Error::source`] stays traversable and an operator
//! can distinguish a misconfiguration from a provider refusal. The SPI error is
//! produced only at the port boundary, through
//! [`SearchFirstVectorError::into_spi_error`].

use sdkwork_memory_spi::MemorySpiError;
use thiserror::Error;

/// Ports exported by this plugin.
pub const RETRIEVER_PORT: &str = "MemoryRetrieverPort";
/// See [`RETRIEVER_PORT`].
pub const INDEX_PORT: &str = "MemoryIndexPort";
/// Injected embedding provider port.
pub const EMBEDDING_MODEL_PORT: &str = "EmbeddingModelPort";
/// Injected language model provider port.
pub const LANGUAGE_MODEL_PORT: &str = "LanguageModelPort";
/// Injected rerank provider port.
pub const RERANK_MODEL_PORT: &str = "RerankModelPort";

/// Result alias for plugin-local fallible operations.
pub type Result<T> = std::result::Result<T, SearchFirstVectorError>;

/// Failure raised by this plugin before it is projected onto the SPI boundary.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SearchFirstVectorError {
    /// The supplied configuration cannot produce a usable index.
    #[error("search-first vector configuration is invalid: {reason}")]
    InvalidConfiguration {
        /// Why the configuration was rejected.
        reason: String,
    },

    /// A retrieval request carried no usable tenant/space scope.
    #[error("{operation} requires an explicit tenant and space scope: {reason}")]
    ScopeRequired {
        /// Operation that refused to proceed without a scope.
        operation: &'static str,
        /// Why an explicit scope is required.
        reason: String,
    },

    /// The requested candidate limit is zero or above the bounded ceiling.
    ///
    /// The limit is refused rather than clamped: silently widening a bounded
    /// request would be a silent degradation of the caller's contract.
    #[error("requested candidate limit {requested} is out of bounds 1..={ceiling}")]
    LimitOutOfBounds {
        /// Limit the caller asked for.
        requested: u32,
        /// Largest limit this plugin accepts.
        ceiling: u32,
    },

    /// The query text was blank.
    #[error("query text must not be blank")]
    BlankQuery,

    /// A memory-type filter entry was blank.
    #[error("memory type filter entry at index {index} must not be blank")]
    BlankMemoryType {
        /// Position of the offending entry in the request filter list.
        index: usize,
    },

    /// The requested retriever kind is not served by this plugin.
    #[error("retriever kind {kind} is not served by this plugin")]
    UnsupportedRetrieverKind {
        /// Rendered retriever kind that was refused.
        kind: String,
    },

    /// The request carried a metadata filter this plugin cannot evaluate.
    ///
    /// Refused rather than ignored. Projections in this index carry no metadata
    /// document, so no operator can be honoured even in principle; returning the
    /// unfiltered candidate set instead would hand the caller records it
    /// explicitly excluded, and a filter narrowed to one owner would silently
    /// widen back to every owner. A failure is the only truthful answer, and it
    /// lets the caller route to a metadata-aware retriever.
    ///
    /// The reason is a compile-time constant rather than a runtime string: the
    /// refusal is structural here and never depends on the filter's contents, so
    /// no caller data can leak into the message.
    #[error("this plugin cannot evaluate a metadata filter: {reason}")]
    MetadataFilterUnsupported {
        /// Why the filter cannot be honoured by this plugin.
        reason: &'static str,
    },

    /// The requested memory type is not one this pipeline can create.
    ///
    /// Refused by name. The reference enumerates three memory types but accepts
    /// only the procedural one, and routing `semantic_memory` or
    /// `episodic_memory` into the ordinary write path would hide that asymmetry
    /// behind a write that looks like it succeeded.
    #[error("memory type {requested:?} cannot be created by this plugin")]
    UnsupportedMemoryType {
        /// The rejected value, quoted back so the caller sees what was compared.
        requested: String,
    },

    /// The model answered the procedural request with nothing to store.
    ///
    /// Refused rather than committed: an empty summary would become a memory
    /// with no content, which is indistinguishable from a memory whose content
    /// was lost.
    #[error("the model returned no content for the procedural memory summary")]
    ProceduralSummaryEmpty,

    /// A procedural record has no metadata carrier to hold its memory type.
    #[error("a procedural memory requires metadata, which carries its memory type")]
    ProceduralMetadataRequired,

    /// A port this operation cannot proceed without is not bound.
    #[error("the required {port} port is not bound")]
    RequiredPortMissing {
        /// SPI port that must be bound before this operation can succeed.
        port: &'static str,
    },

    /// An injected provider port returned an error.
    #[error("{port} call to provider {provider} failed")]
    ProviderCallFailed {
        /// Port the failing call was made through.
        port: &'static str,
        /// Provider code reported by the port, never a credential or endpoint.
        provider: String,
        /// Original SPI failure, preserved so the cause chain is not severed.
        #[source]
        source: MemorySpiError,
    },

    /// A memory id was blank or otherwise unusable.
    #[error("memory id {memory_id:?} is not a usable projection key")]
    BlankMemoryId {
        /// The offending identifier.
        memory_id: String,
    },

    /// A projection write would exceed the per-scope retention ceiling.
    #[error("the scope bucket is at its {max_entries} entry ceiling and refused the write")]
    IndexCapacityExhausted {
        /// Per-scope entry ceiling that was reached.
        max_entries: usize,
    },

    /// The requested memory id has never been projected into this index.
    ///
    /// Refused instead of synthesising a vector: a fabricated or zero vector has
    /// no similarity relationship, so accepting it would make scoring
    /// meaningless while still returning `Ok`.
    #[error("memory id {memory_id} has no projection in this index; project its content first")]
    RecordNotIndexed {
        /// Identifier that has no projection.
        memory_id: String,
    },

    /// One memory id is projected into more than one scope.
    ///
    /// Refused because the scope-free index lookup cannot choose between them,
    /// and choosing arbitrarily would act on the wrong tenant's or space's
    /// projection.
    #[error(
        "memory id {memory_id} is projected into {scopes} scopes and the lookup is scope-free"
    )]
    AmbiguousProjection {
        /// Identifier that resolved to more than one scope.
        memory_id: String,
        /// Number of scopes holding that identifier.
        scopes: usize,
    },

    /// A stored or supplied embedding does not match the index dimensionality.
    #[error("embedding for {memory_id} has {actual} dimensions but the index requires {expected}")]
    EmbeddingDimensionMismatch {
        /// Identifier the embedding belongs to, or `<query>` for a query vector.
        memory_id: String,
        /// Width this index accepts.
        expected: usize,
        /// Width actually supplied.
        actual: usize,
    },

    /// The language model answered memory extraction with an unparseable payload.
    #[error("memory extraction response was not parseable: {reason}")]
    MemoryExtractionUnparseable {
        /// Why the payload could not be interpreted.
        reason: String,
    },

    /// A projection carried an `expiration_date` outside the contract format.
    ///
    /// Refused at write time, as the reference implementation refuses it, rather
    /// than stored and then tolerated at read time: an unreadable expiry would
    /// otherwise make a memory's lifetime unknowable while still looking set.
    #[error("{value:?} is not an expiration date in YYYY-MM-DD format")]
    ExpirationDateInvalid {
        /// The offending value.
        value: String,
    },

    /// The index mutex was poisoned by a panicking writer.
    #[error("the vector index lock is poisoned")]
    IndexLockPoisoned,
}

impl SearchFirstVectorError {
    /// The SPI port this failure surfaced through.
    ///
    /// Used to build a truthful [`MemorySpiError`] without the caller having to
    /// remember which port it was on.
    pub fn port_name(&self) -> &'static str {
        match self {
            Self::RequiredPortMissing { port } => port,
            Self::ProviderCallFailed { port, .. } => port,
            Self::MemoryExtractionUnparseable { .. } | Self::ProceduralSummaryEmpty => {
                LANGUAGE_MODEL_PORT
            }
            Self::BlankMemoryId { .. }
            | Self::IndexCapacityExhausted { .. }
            | Self::RecordNotIndexed { .. }
            | Self::AmbiguousProjection { .. }
            | Self::EmbeddingDimensionMismatch { .. }
            | Self::ExpirationDateInvalid { .. }
            // Write-time request validations, reported on the index port for the
            // same reason as `ExpirationDateInvalid`: the write path is what feeds
            // the derived index, so that is the port whose operation failed. A
            // caller that knows better can name another port through
            // [`SearchFirstVectorError::into_spi_error_on`].
            | Self::UnsupportedMemoryType { .. }
            | Self::ProceduralMetadataRequired
            | Self::IndexLockPoisoned => INDEX_PORT,
            Self::InvalidConfiguration { .. }
            | Self::ScopeRequired { .. }
            | Self::LimitOutOfBounds { .. }
            | Self::BlankQuery
            | Self::BlankMemoryType { .. }
            | Self::UnsupportedRetrieverKind { .. }
            | Self::MetadataFilterUnsupported { .. } => RETRIEVER_PORT,
        }
    }

    /// Project this failure onto the provider-neutral SPI error, reporting
    /// `port` as the port whose operation failed.
    ///
    /// Use this when the calling context knows the port better than
    /// [`Self::port_name`] can: for example a missing embedding provider
    /// discovered during a `MemoryIndexPort` operation is still a
    /// `MemoryIndexPort` operation failure, with the missing dependency named in
    /// the message.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{
    ///     SearchFirstVectorError, EMBEDDING_MODEL_PORT, INDEX_PORT,
    /// };
    /// use sdkwork_memory_spi::MemorySpiError;
    ///
    /// let error = SearchFirstVectorError::RequiredPortMissing {
    ///     port: EMBEDDING_MODEL_PORT,
    /// };
    /// let spi_error = error.into_spi_error_on(INDEX_PORT);
    ///
    /// match spi_error {
    ///     MemorySpiError::PortOperationFailed { port, message } => {
    ///         assert_eq!(port, INDEX_PORT);
    ///         assert!(message.contains(EMBEDDING_MODEL_PORT));
    ///     }
    ///     other => panic!("expected a port operation failure, got {other:?}"),
    /// }
    /// ```
    pub fn into_spi_error_on(self, port: &'static str) -> MemorySpiError {
        match self {
            Self::ProviderCallFailed { source, .. } => source,
            other => MemorySpiError::PortOperationFailed {
                port: port.to_string(),
                message: other.to_string(),
            },
        }
    }

    /// Project this failure onto the provider-neutral SPI error.
    ///
    /// A provider failure keeps its original [`MemorySpiError`] instead of being
    /// re-wrapped in a string, so the SPI cause chain is not severed. Every other
    /// variant is reported as a `PortOperationFailed` on the port that raised it.
    ///
    /// The message is redacted by construction: this plugin never holds a
    /// credential value or endpoint literal, so no secret can appear in it.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{
    ///     SearchFirstVectorError, RETRIEVER_PORT,
    /// };
    /// use sdkwork_memory_spi::MemorySpiError;
    ///
    /// let error = SearchFirstVectorError::BlankQuery;
    /// let spi_error = error.into_spi_error();
    ///
    /// assert_eq!(
    ///     spi_error,
    ///     MemorySpiError::PortOperationFailed {
    ///         port: RETRIEVER_PORT.to_string(),
    ///         message: "query text must not be blank".to_string(),
    ///     }
    /// );
    /// ```
    pub fn into_spi_error(self) -> MemorySpiError {
        let port = self.port_name();

        self.into_spi_error_on(port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_failure_preserves_the_original_spi_cause() {
        let error = SearchFirstVectorError::ProviderCallFailed {
            port: EMBEDDING_MODEL_PORT,
            provider: "reviewed-embedding-provider".to_string(),
            source: MemorySpiError::PortOperationFailed {
                port: EMBEDDING_MODEL_PORT.to_string(),
                message: "upstream refused".to_string(),
            },
        };

        assert_eq!(
            error.into_spi_error(),
            MemorySpiError::PortOperationFailed {
                port: EMBEDDING_MODEL_PORT.to_string(),
                message: "upstream refused".to_string(),
            }
        );
    }

    #[test]
    fn retriever_variants_report_the_retriever_port() {
        assert_eq!(
            SearchFirstVectorError::BlankQuery.port_name(),
            RETRIEVER_PORT
        );
        assert_eq!(
            SearchFirstVectorError::LimitOutOfBounds {
                requested: 0,
                ceiling: 200
            }
            .port_name(),
            RETRIEVER_PORT
        );
        assert_eq!(
            SearchFirstVectorError::MetadataFilterUnsupported {
                reason: "this plugin projects no metadata document"
            }
            .port_name(),
            RETRIEVER_PORT
        );
    }

    #[test]
    fn extraction_failures_report_the_language_model_port() {
        assert_eq!(
            SearchFirstVectorError::MemoryExtractionUnparseable {
                reason: "no envelope".to_string()
            }
            .port_name(),
            LANGUAGE_MODEL_PORT
        );
        // A procedural summary is also model output, so an unusable one is a
        // language-model failure rather than a write failure.
        assert_eq!(
            SearchFirstVectorError::ProceduralSummaryEmpty.port_name(),
            LANGUAGE_MODEL_PORT
        );
    }

    #[test]
    fn index_variants_report_the_index_port() {
        assert_eq!(
            SearchFirstVectorError::RecordNotIndexed {
                memory_id: "m-1".to_string()
            }
            .port_name(),
            INDEX_PORT
        );
        assert_eq!(
            SearchFirstVectorError::AmbiguousProjection {
                memory_id: "m-1".to_string(),
                scopes: 2
            }
            .port_name(),
            INDEX_PORT
        );
        assert_eq!(
            SearchFirstVectorError::ExpirationDateInvalid {
                value: "not-a-date".to_string()
            }
            .port_name(),
            INDEX_PORT
        );
    }

    #[test]
    fn write_path_refusals_report_the_index_port() {
        // Neither refusal is a provider failure: both are the write path
        // declining a request before any port is exercised, so they surface on
        // the port that carries writes.
        assert_eq!(
            SearchFirstVectorError::UnsupportedMemoryType {
                requested: "semantic_memory".to_string()
            }
            .port_name(),
            INDEX_PORT
        );
        assert_eq!(
            SearchFirstVectorError::ProceduralMetadataRequired.port_name(),
            INDEX_PORT
        );
    }
}
