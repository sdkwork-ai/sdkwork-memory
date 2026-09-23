//! Injection-only configuration for the search-first vector plugin.
//!
//! The plugin never reads the process environment, never resolves an OS path,
//! and never stores a credential value. Everything it needs to locate a provider
//! arrives as an injected SPI port, whose binding the composition root resolves
//! through `ai_provider_binding.endpoint_ref` and `ai_provider_binding.secret_ref`.

use crate::error::{Result, SearchFirstVectorError};

/// Bounded candidate ceiling for one search.
///
/// Re-exported from the SPI so the plugin and the service cannot drift apart.
pub const MAX_SEARCH_LIMIT: u32 = sdkwork_memory_spi::MAX_MEMORY_RETRIEVAL_CANDIDATES;

/// Candidate limit used by the bounded legacy `retrieve_scoped` helper.
pub const DEFAULT_SEARCH_LIMIT: u32 = 32;

/// Default number of vectors retained per `(tenant, space)` bucket.
pub const DEFAULT_MAX_ENTRIES_PER_SCOPE: usize = 4_096;

/// Upper bound accepted for [`SearchFirstVectorConfig::max_entries_per_scope`].
pub const MAX_ENTRIES_PER_SCOPE_CEILING: usize = 1_000_000;

/// Whether an expired memory is returned by default.
///
/// The reference implementation defaults `show_expired` to `false`, so an
/// expired memory is hidden unless a caller asks for it. Keeping the default in
/// configuration lets a deployment change it without a request-shape change;
/// the per-request opt-in is a separate, later contract addition.
pub const DEFAULT_INCLUDE_EXPIRED: bool = false;

/// Bounded, validated configuration for [`crate::runtime::SearchFirstVectorRuntime`].
#[derive(Debug, Clone, PartialEq)]
pub struct SearchFirstVectorConfig {
    /// Maximum number of vectors retained per `(tenant, space)` bucket.
    ///
    /// This is the index's own hard bound, independent of the per-request limit.
    /// When a bucket is full, the entry with the smallest
    /// `(updated_at, memory_id)` is evicted so eviction is deterministic.
    pub max_entries_per_scope: usize,

    /// Candidate limit used only by the bounded `retrieve_scoped` compatibility
    /// helper. `search_scoped` always honours the caller's `limit`.
    pub default_search_limit: u32,

    /// Minimum cosine similarity for a candidate to be returned by `rank`.
    pub similarity_floor: f32,

    /// Whether a bound `RerankModelPort` is consulted when more than one
    /// candidate survives ranking.
    pub use_rerank_when_bound: bool,

    /// Whether a search returns memories whose `expiration_date` has passed.
    ///
    /// Defaults to [`DEFAULT_INCLUDE_EXPIRED`], which hides them. Expiry is
    /// evaluated inside the scope bucket, before truncation, so an expired
    /// memory can never displace a live one.
    pub include_expired: bool,
}

impl Default for SearchFirstVectorConfig {
    /// Defaults that make every bounded search succeed without further tuning.
    ///
    /// `similarity_floor` is `0.0` so a caller receives every in-scope candidate
    /// rather than a silently narrowed set; callers that want a narrower band set
    /// it explicitly.
    fn default() -> Self {
        Self {
            max_entries_per_scope: DEFAULT_MAX_ENTRIES_PER_SCOPE,
            default_search_limit: DEFAULT_SEARCH_LIMIT,
            similarity_floor: 0.0,
            use_rerank_when_bound: true,
            include_expired: DEFAULT_INCLUDE_EXPIRED,
        }
    }
}

impl SearchFirstVectorConfig {
    /// Validate every bound before the runtime can be constructed.
    ///
    /// Failing here rather than at request time keeps a misconfiguration from
    /// being reported as a per-request retrieval failure.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::InvalidConfiguration`] when
    /// `max_entries_per_scope` is zero or above [`MAX_ENTRIES_PER_SCOPE_CEILING`],
    /// when `default_search_limit` is outside `1..=MAX_SEARCH_LIMIT`, or when
    /// `similarity_floor` is not a finite value in `-1.0..=1.0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::SearchFirstVectorConfig;
    ///
    /// assert!(SearchFirstVectorConfig::default().validate().is_ok());
    ///
    /// let unusable = SearchFirstVectorConfig {
    ///     max_entries_per_scope: 0,
    ///     ..SearchFirstVectorConfig::default()
    /// };
    /// assert!(unusable.validate().is_err());
    /// ```
    #[must_use = "a rejected configuration must be handled, not discarded"]
    pub fn validate(&self) -> Result<()> {
        if self.max_entries_per_scope == 0 {
            return Err(SearchFirstVectorError::InvalidConfiguration {
                reason: "max_entries_per_scope must be at least 1".to_string(),
            });
        }
        if self.max_entries_per_scope > MAX_ENTRIES_PER_SCOPE_CEILING {
            return Err(SearchFirstVectorError::InvalidConfiguration {
                reason: format!(
                    "max_entries_per_scope {} exceeds the ceiling {}",
                    self.max_entries_per_scope, MAX_ENTRIES_PER_SCOPE_CEILING
                ),
            });
        }
        if self.default_search_limit == 0 || self.default_search_limit > MAX_SEARCH_LIMIT {
            return Err(SearchFirstVectorError::InvalidConfiguration {
                reason: format!(
                    "default_search_limit {} is out of bounds 1..={MAX_SEARCH_LIMIT}",
                    self.default_search_limit
                ),
            });
        }
        if !self.similarity_floor.is_finite() || !(-1.0..=1.0).contains(&self.similarity_floor) {
            return Err(SearchFirstVectorError::InvalidConfiguration {
                reason: format!(
                    "similarity_floor must be a finite value in -1.0..=1.0, got {}",
                    self.similarity_floor
                ),
            });
        }

        Ok(())
    }

    /// Validate a caller-supplied candidate limit against the bounded ceiling.
    ///
    /// A limit of zero or one above [`MAX_SEARCH_LIMIT`] is refused rather than
    /// clamped, because clamping would silently rewrite the caller's contract.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::LimitOutOfBounds`] when `requested` is
    /// zero or greater than [`MAX_SEARCH_LIMIT`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{SearchFirstVectorConfig, MAX_SEARCH_LIMIT};
    ///
    /// let config = SearchFirstVectorConfig::default();
    ///
    /// assert!(config.validate_limit(MAX_SEARCH_LIMIT).is_ok());
    /// assert!(config.validate_limit(0).is_err());
    /// assert!(config.validate_limit(MAX_SEARCH_LIMIT + 1).is_err());
    /// ```
    #[must_use = "an out-of-bounds limit must be handled, not discarded"]
    pub fn validate_limit(&self, requested: u32) -> Result<()> {
        if requested == 0 || requested > MAX_SEARCH_LIMIT {
            return Err(SearchFirstVectorError::LimitOutOfBounds {
                requested,
                ceiling: MAX_SEARCH_LIMIT,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_configuration_is_valid() {
        assert!(SearchFirstVectorConfig::default().validate().is_ok());
    }

    #[test]
    fn zero_entry_ceiling_is_refused() {
        let config = SearchFirstVectorConfig {
            max_entries_per_scope: 0,
            ..SearchFirstVectorConfig::default()
        };

        assert!(matches!(
            config.validate(),
            Err(SearchFirstVectorError::InvalidConfiguration { .. })
        ));
    }

    #[test]
    fn an_out_of_band_similarity_floor_is_refused() {
        let config = SearchFirstVectorConfig {
            similarity_floor: 1.5,
            ..SearchFirstVectorConfig::default()
        };

        assert!(matches!(
            config.validate(),
            Err(SearchFirstVectorError::InvalidConfiguration { .. })
        ));
    }

    #[test]
    fn expired_memories_are_hidden_by_default() {
        // Pinned so a future change to the default is a deliberate, visible act
        // rather than a silent widening of what a search returns.
        assert!(!SearchFirstVectorConfig::default().include_expired);
        const { assert!(!DEFAULT_INCLUDE_EXPIRED) };
    }

    #[test]
    fn limit_is_refused_outside_the_spi_ceiling() {
        let config = SearchFirstVectorConfig::default();

        assert!(matches!(
            config.validate_limit(0),
            Err(SearchFirstVectorError::LimitOutOfBounds { .. })
        ));
        assert!(matches!(
            config.validate_limit(MAX_SEARCH_LIMIT + 1),
            Err(SearchFirstVectorError::LimitOutOfBounds { .. })
        ));
        assert!(config.validate_limit(MAX_SEARCH_LIMIT).is_ok());
    }
}
