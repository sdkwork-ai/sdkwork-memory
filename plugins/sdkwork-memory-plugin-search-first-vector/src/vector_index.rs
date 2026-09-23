//! Bounded, scope-keyed, deterministic in-memory vector index.
//!
//! This is the derived projection behind `MemoryIndexPort`. It is deliberately
//! **not** a canonical store: per the repository integration policy the
//! `ai_record` and `ai_event` tables remain the source of truth and every index
//! is derived and rebuildable.
//!
//! Three properties are load-bearing, because the mem0-derived implementation
//! this replaces got each of them wrong:
//!
//! 1. **Scope is the bucket key.** A vector lives in exactly one
//!    `(tenant_id, space_id)` bucket, so isolation is structural rather than a
//!    post-hoc filter that a large `limit` could bypass.
//! 2. **Filtering precedes truncation.** Sensitivity and memory-type filters run
//!    inside the bucket *before* the `limit` is applied, so truncation can never
//!    prefer a forbidden record over a permitted one.
//! 3. **Nothing is fabricated.** A projection is refused rather than padded with
//!    zeros, and identifiers and timestamps come from the caller or the clock —
//!    never invented.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

use sdkwork_memory_spi::{MemoryScopeContext, MemorySensitivityReadScope};
use sdkwork_utils_rust::{format_datetime, now, sha256_hash};

use crate::config::SearchFirstVectorConfig;
use crate::error::{Result, SearchFirstVectorError};

/// Tenant/space isolation key derived from a [`MemoryScopeContext`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScopeKey {
    /// Owning tenant.
    pub tenant_id: i64,
    /// Owning space inside the tenant.
    pub space_id: i64,
}

impl ScopeKey {
    /// Derive the isolation key from an explicit SPI scope.
    ///
    /// Only the tenant and space participate: organisation and user are not
    /// isolation boundaries for a derived index, and treating them as such would
    /// fragment one space's projection across buckets.
    pub fn from_scope(scope: &MemoryScopeContext) -> Self {
        Self {
            tenant_id: scope.tenant_id,
            space_id: scope.space_id,
        }
    }
}

/// Sensitivity tier carried by a projected record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordSensitivity {
    /// Readable by any read scope.
    Public,
    /// Readable by `Elevated` and `Owner` read scopes.
    Elevated,
    /// Readable only by the `Owner` read scope.
    Owner,
}

impl RecordSensitivity {
    fn rank(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Elevated => 1,
            Self::Owner => 2,
        }
    }

    /// Whether a request carrying `read_scope` may observe this tier.
    ///
    /// The gate is conservative by construction: a record stored at the `Owner`
    /// tier is never returned to a narrower read scope, and no read scope is ever
    /// widened on the caller's behalf.
    pub fn admitted_for(self, read_scope: MemorySensitivityReadScope) -> bool {
        let admitted_rank = match read_scope {
            MemorySensitivityReadScope::Public => 0,
            MemorySensitivityReadScope::Elevated => 1,
            MemorySensitivityReadScope::Owner => 2,
        };

        self.rank() <= admitted_rank
    }
}

/// Projection metadata retained beside a vector.
///
/// These fields are candidate projections, not canonical truth. The service must
/// rehydrate every returned candidate through `MemoryRecordStorePort`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorRecordProjection {
    /// Canonical memory identifier supplied by the composition root.
    pub memory_id: String,
    /// Memory type used by the `memory_types` request filter.
    pub memory_type: String,
    /// Optional subject slot.
    pub subject: Option<String>,
    /// Optional predicate slot.
    pub predicate: Option<String>,
    /// Object slot text.
    pub object_text: String,
    /// Canonical text used for display and content-hash deduplication.
    pub canonical_text: String,
    /// Date after which the memory is hidden from searches, as `YYYY-MM-DD`.
    ///
    /// Validated at write time by [`normalize_expiration_date`]; a projection
    /// therefore never carries a date nobody can read.
    pub expiration_date: Option<String>,
    /// Sensitivity tier the record was projected at.
    pub sensitivity: RecordSensitivity,
    /// Derived SHA-256 of `canonical_text`.
    ///
    /// Derived at write time and actually consulted by duplicate detection,
    /// unlike the mem0-derived implementation which computed a hash and then
    /// never read it.
    pub content_hash: String,
    /// Creation timestamp of the projection.
    pub created_at: String,
    /// Last refresh timestamp of the projection.
    pub updated_at: String,
}

/// Projection write command.
///
/// The content hash and timestamps are derived by the index, not supplied, so a
/// caller cannot desynchronise the stored hash from the stored text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorProjectionCommand {
    /// Canonical memory identifier.
    pub memory_id: String,
    /// Memory type used by the `memory_types` request filter.
    pub memory_type: String,
    /// Optional subject slot.
    pub subject: Option<String>,
    /// Optional predicate slot.
    pub predicate: Option<String>,
    /// Object slot text.
    pub object_text: String,
    /// Canonical text.
    pub canonical_text: String,
    /// Date after which the memory is hidden from searches, as `YYYY-MM-DD`.
    ///
    /// A value outside that format is refused by
    /// [`ScopedVectorIndex::upsert`] rather than stored.
    pub expiration_date: Option<String>,
    /// Sensitivity tier to project at.
    pub sensitivity: RecordSensitivity,
}

/// Read a `YYYY-MM-DD` date, rejecting anything a calendar would reject.
///
/// The format is fixed-width, so a validated value also orders chronologically
/// as a string. Rejecting impossible dates matters: `2023-02-30` would order
/// plausibly while naming a day that never existed.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::normalize_expiration_date;
///
/// assert_eq!(
///     normalize_expiration_date("  2023-05-24  "),
///     Some("2023-05-24".to_string())
/// );
/// assert_eq!(
///     normalize_expiration_date("2024-02-29"),
///     Some("2024-02-29".to_string())
/// );
/// assert_eq!(normalize_expiration_date("2023-02-29"), None);
/// assert_eq!(normalize_expiration_date("2023-5-4"), None);
/// assert_eq!(normalize_expiration_date("2023-13-01"), None);
/// assert_eq!(normalize_expiration_date("not a date"), None);
/// ```
pub fn normalize_expiration_date(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }

    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return None;
    }

    let year: u32 = trimmed.get(0..4)?.parse().ok()?;
    let month: u32 = trimmed.get(5..7)?.parse().ok()?;
    let day: u32 = trimmed.get(8..10)?.parse().ok()?;

    if month == 0 || month > 12 || day == 0 || day > days_in_month(year, month) {
        return None;
    }

    Some(trimmed.to_string())
}

impl VectorRecordProjection {
    /// Whether this record's lifetime had already passed on `as_of_date`.
    ///
    /// `as_of_date` is compared as a plain `YYYY-MM-DD` string against the stored
    /// date, which orders chronologically for that fixed-width format. Both sides
    /// are validated on the way in, so the comparison cannot be reached with a
    /// value that would order wrongly.
    ///
    /// A record with no expiration date never expires, and a reference date that
    /// cannot be read yields `false` — expiry is never inferred from something
    /// that could not be parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{
    ///     RecordSensitivity, VectorProjectionCommand, ScopedVectorIndex,
    ///     SearchFirstVectorConfig, ScopeKey,
    /// };
    ///
    /// let mut index = ScopedVectorIndex::new(2, &SearchFirstVectorConfig::default()).unwrap();
    /// let scope = ScopeKey { tenant_id: 1, space_id: 1 };
    ///
    /// for (memory_id, expiration_date) in [
    ///     ("m-expired", Some("2023-05-01".to_string())),
    ///     ("m-live", Some("2099-05-01".to_string())),
    ///     ("m-forever", None),
    /// ] {
    ///     index.upsert(
    ///         scope,
    ///         VectorProjectionCommand {
    ///             memory_id: memory_id.to_string(),
    ///             memory_type: "fact".to_string(),
    ///             subject: None,
    ///             predicate: None,
    ///             object_text: "tea".to_string(),
    ///             canonical_text: "the user drinks tea".to_string(),
    ///             expiration_date,
    ///             sensitivity: RecordSensitivity::Public,
    ///         },
    ///         vec![1.0, 0.0],
    ///     )
    ///     .unwrap();
    /// }
    ///
    /// let projection = index.projection(&scope, "m-expired").unwrap();
    /// assert!(projection.is_expired_on("2023-05-02"));
    /// assert!(!projection.is_expired_on("2023-05-01"));
    ///
    /// let projection = index.projection(&scope, "m-forever").unwrap();
    /// assert!(!projection.is_expired_on("2099-05-02"));
    /// ```
    #[must_use = "an expiry verdict must be inspected, not discarded"]
    pub fn is_expired_on(&self, as_of_date: &str) -> bool {
        let (Some(expiration), Some(reference)) = (
            self.expiration_date.as_deref(),
            normalize_expiration_date(as_of_date),
        ) else {
            return false;
        };

        // A date is expired once the reference date has passed it, matching the
        // reference implementation's strict `<` comparison.
        reference.as_str() > expiration
    }
}

fn is_leap_year(year: u32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// What a projection write actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VectorProjectionOutcome {
    /// A previously unknown memory id was added.
    Inserted,
    /// A known memory id was refreshed in place.
    Refreshed,
    /// A previously unknown memory id was added after evicting the oldest entry.
    InsertedAfterEviction {
        /// Identifier of the projection that was displaced to make room.
        evicted_memory_id: String,
    },
}

impl VectorProjectionOutcome {
    /// The evicted memory id, when the write displaced one.
    ///
    /// `None` for [`VectorProjectionOutcome::Inserted`] and
    /// [`VectorProjectionOutcome::Refreshed`], which displace nothing.
    #[must_use = "an eviction must be reported, not discarded"]
    pub fn evicted_memory_id(&self) -> Option<&str> {
        match self {
            Self::InsertedAfterEviction { evicted_memory_id } => Some(evicted_memory_id),
            Self::Inserted | Self::Refreshed => None,
        }
    }
}

/// Filters applied inside the scope bucket before truncation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorRankFilters {
    /// Allowed memory types. Empty means no type restriction.
    pub memory_types: Vec<String>,
    /// Maximum sensitivity tier the caller may observe.
    pub read_scope: MemorySensitivityReadScope,
    /// Whether records whose expiration date has passed are admitted.
    pub include_expired: bool,
    /// Date expiry is judged against, as `YYYY-MM-DD`.
    ///
    /// Supplied rather than read from the clock so a ranking call is pure: the
    /// same index, query, and filters always produce the same result.
    pub as_of_date: String,
}

impl VectorRankFilters {
    /// Filters that admit every tier and every lifetime in the bucket.
    pub fn unrestricted() -> Self {
        Self {
            memory_types: Vec::new(),
            read_scope: MemorySensitivityReadScope::Owner,
            include_expired: true,
            as_of_date: crate::extraction::today_utc_date(),
        }
    }
}

/// A scored projection returned by [`ScopedVectorIndex::rank`].
#[derive(Debug, Clone, PartialEq)]
pub struct VectorHit {
    /// The projected record.
    pub projection: VectorRecordProjection,
    /// Cosine similarity against the query embedding.
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct VectorEntry {
    projection: VectorRecordProjection,
    embedding: Vec<f32>,
}

#[derive(Debug, Default)]
struct ScopeBucket {
    entries: HashMap<String, VectorEntry>,
    /// `(updated_at, memory_id)` ordering used for deterministic eviction.
    recency: BTreeSet<(String, String)>,
}

impl ScopeBucket {
    fn insert(
        &mut self,
        projection: VectorRecordProjection,
        embedding: Vec<f32>,
    ) -> VectorProjectionOutcome {
        let memory_id = projection.memory_id.clone();
        let recency_key = (projection.updated_at.clone(), memory_id.clone());

        let replaced = self.entries.insert(
            memory_id.clone(),
            VectorEntry {
                projection,
                embedding,
            },
        );

        if let Some(previous) = replaced {
            self.recency
                .remove(&(previous.projection.updated_at, memory_id));
            self.recency.insert(recency_key);

            return VectorProjectionOutcome::Refreshed;
        }

        self.recency.insert(recency_key);

        VectorProjectionOutcome::Inserted
    }

    fn evict_oldest(&mut self) -> Option<String> {
        let oldest = self.recency.iter().next()?.clone();
        self.recency.remove(&oldest);
        self.entries.remove(&oldest.1);

        Some(oldest.1)
    }

    fn remove(&mut self, memory_id: &str) -> bool {
        match self.entries.remove(memory_id) {
            Some(entry) => {
                self.recency
                    .remove(&(entry.projection.updated_at, memory_id.to_string()));
                true
            }
            None => false,
        }
    }
}

/// Bounded, scope-keyed vector index.
#[derive(Debug)]
pub struct ScopedVectorIndex {
    dimensions: usize,
    max_entries_per_scope: usize,
    similarity_floor: f32,
    scopes: HashMap<ScopeKey, ScopeBucket>,
    projected_total: usize,
}

impl ScopedVectorIndex {
    /// Build an index for embeddings of `dimensions` width.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::InvalidConfiguration`] when `config`
    /// fails [`SearchFirstVectorConfig::validate`], or when `dimensions` is zero
    /// because a zero-width embedding cannot carry similarity.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{
    ///     ScopedVectorIndex, SearchFirstVectorConfig,
    /// };
    ///
    /// let index = ScopedVectorIndex::new(4, &SearchFirstVectorConfig::default()).unwrap();
    ///
    /// assert_eq!(index.dimensions(), 4);
    /// assert!(index.is_empty());
    /// assert!(ScopedVectorIndex::new(0, &SearchFirstVectorConfig::default()).is_err());
    /// ```
    #[must_use = "a rejected index construction must be handled, not discarded"]
    pub fn new(dimensions: usize, config: &SearchFirstVectorConfig) -> Result<Self> {
        config.validate()?;

        if dimensions == 0 {
            return Err(SearchFirstVectorError::InvalidConfiguration {
                reason: "embedding dimensions must be at least 1".to_string(),
            });
        }

        Ok(Self {
            dimensions,
            max_entries_per_scope: config.max_entries_per_scope,
            similarity_floor: config.similarity_floor,
            scopes: HashMap::new(),
            projected_total: 0,
        })
    }

    /// Embedding width this index accepts.
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Per-scope retention ceiling.
    pub fn max_entries_per_scope(&self) -> usize {
        self.max_entries_per_scope
    }

    /// Total number of projections across every scope.
    pub fn len(&self) -> usize {
        self.projected_total
    }

    /// Whether the index holds no projection at all.
    pub fn is_empty(&self) -> bool {
        self.projected_total == 0
    }

    /// Number of projections held in one scope bucket.
    pub fn scope_len(&self, key: &ScopeKey) -> usize {
        self.scopes
            .get(key)
            .map_or(0, |bucket| bucket.entries.len())
    }

    /// Read one projection without exposing its embedding.
    ///
    /// Returns `None` for an unknown id and for a known id in a different scope
    /// bucket, so a caller cannot use this to probe another tenant's or space's
    /// projection.
    #[must_use = "the looked-up projection must be inspected, not discarded"]
    pub fn projection(&self, key: &ScopeKey, memory_id: &str) -> Option<&VectorRecordProjection> {
        self.scopes
            .get(key)
            .and_then(|bucket| bucket.entries.get(memory_id))
            .map(|entry| &entry.projection)
    }

    /// Every scope currently holding a projection for `memory_id`, in ascending order.
    ///
    /// Returns all matches rather than the first so a scope-free lookup can refuse
    /// an ambiguous id instead of silently acting on an arbitrary scope.
    pub fn scopes_holding(&self, memory_id: &str) -> Vec<ScopeKey> {
        let mut scopes = self
            .scopes
            .iter()
            .filter(|(_, bucket)| bucket.entries.contains_key(memory_id))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        scopes.sort_unstable();

        scopes
    }

    /// Insert or refresh a projection.
    ///
    /// Refuses a blank memory id and a mis-sized embedding instead of storing
    /// either, and evicts the deterministically oldest entry when the bucket is
    /// full.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::BlankMemoryId`] when the command carries
    /// a blank identifier, and
    /// [`SearchFirstVectorError::EmbeddingDimensionMismatch`] when the embedding
    /// width differs from [`ScopedVectorIndex::dimensions`].
    ///
    /// # Examples
    ///
    /// ```
    /// use sdkwork_memory_plugin_search_first_vector::{
    ///     RecordSensitivity, ScopedVectorIndex, ScopeKey, SearchFirstVectorConfig,
    ///     VectorProjectionCommand, VectorProjectionOutcome,
    /// };
    ///
    /// let mut index = ScopedVectorIndex::new(2, &SearchFirstVectorConfig::default()).unwrap();
    /// let scope = ScopeKey { tenant_id: 1, space_id: 1 };
    ///
    /// let outcome = index
    ///     .upsert(
    ///         scope,
    ///         VectorProjectionCommand {
    ///             memory_id: "m-1".to_string(),
    ///             memory_type: "fact".to_string(),
    ///             subject: None,
    ///             predicate: None,
    ///             object_text: "tea".to_string(),
    ///             canonical_text: "the user drinks tea".to_string(),
    ///             expiration_date: None,
    ///             sensitivity: RecordSensitivity::Public,
    ///         },
    ///         vec![1.0, 0.0],
    ///     )
    ///     .unwrap();
    ///
    /// assert_eq!(outcome, VectorProjectionOutcome::Inserted);
    /// ```
    #[must_use = "a projection outcome must be handled, not discarded"]
    pub fn upsert(
        &mut self,
        key: ScopeKey,
        mut command: VectorProjectionCommand,
        embedding: Vec<f32>,
    ) -> Result<VectorProjectionOutcome> {
        if command.memory_id.trim().is_empty() {
            return Err(SearchFirstVectorError::BlankMemoryId {
                memory_id: command.memory_id,
            });
        }
        if embedding.len() != self.dimensions {
            return Err(SearchFirstVectorError::EmbeddingDimensionMismatch {
                memory_id: command.memory_id,
                expected: self.dimensions,
                actual: embedding.len(),
            });
        }

        if let Some(supplied) = command.expiration_date.take() {
            match normalize_expiration_date(&supplied) {
                Some(normalized) => command.expiration_date = Some(normalized),
                None => {
                    return Err(SearchFirstVectorError::ExpirationDateInvalid { value: supplied });
                }
            }
        }

        let timestamp = format_datetime(now(), None);
        let max_entries = self.max_entries_per_scope;
        let bucket = self.scopes.entry(key).or_default();

        // A refresh of a known id never grows the bucket.
        if let Some(existing) = bucket.entries.get(&command.memory_id) {
            let created_at = existing.projection.created_at.clone();
            let projection = build_projection(command, created_at, timestamp);

            return Ok(bucket.insert(projection, embedding));
        }

        let mut evicted_memory_id = None;
        while bucket.entries.len() >= max_entries {
            match bucket.evict_oldest() {
                Some(evicted) => {
                    self.projected_total -= 1;
                    evicted_memory_id = Some(evicted);
                }
                None => break,
            }
        }

        let projection = build_projection(command, timestamp.clone(), timestamp);
        let outcome = bucket.insert(projection, embedding);
        self.projected_total += 1;

        Ok(match evicted_memory_id {
            Some(evicted_memory_id) => {
                VectorProjectionOutcome::InsertedAfterEviction { evicted_memory_id }
            }
            None => outcome,
        })
    }

    /// Remove one projection, reporting whether it existed.
    ///
    /// This is how the plugin honours `deletionPropagation`: retiring a record
    /// must remove its derived projection, or the index keeps answering with a
    /// record the canonical store no longer has.
    pub fn remove(&mut self, key: &ScopeKey, memory_id: &str) -> bool {
        let removed = self
            .scopes
            .get_mut(key)
            .is_some_and(|bucket| bucket.remove(memory_id));

        if removed {
            self.projected_total -= 1;
        }

        removed
    }

    /// Rank candidates inside one scope bucket.
    ///
    /// Filters are applied before `limit`, ranking is ordered by descending
    /// score then ascending `memory_id`, and the bucket itself is the only thing
    /// scanned — so both the isolation boundary and the result bound are
    /// structural.
    ///
    /// Expiry is applied here, beside the other filters, so an expired memory can
    /// never displace a live one from the truncated result.
    ///
    /// # Errors
    ///
    /// Returns [`SearchFirstVectorError::EmbeddingDimensionMismatch`] when the
    /// query embedding width differs from [`ScopedVectorIndex::dimensions`].
    #[must_use = "ranked hits must be handled, not discarded"]
    pub fn rank(
        &self,
        key: &ScopeKey,
        query_embedding: &[f32],
        filters: &VectorRankFilters,
        limit: u32,
    ) -> Result<Vec<VectorHit>> {
        if query_embedding.len() != self.dimensions {
            return Err(SearchFirstVectorError::EmbeddingDimensionMismatch {
                memory_id: "<query>".to_string(),
                expected: self.dimensions,
                actual: query_embedding.len(),
            });
        }

        let Some(bucket) = self.scopes.get(key) else {
            return Ok(Vec::new());
        };

        let mut hits = Vec::new();
        for entry in bucket.entries.values() {
            if !entry
                .projection
                .sensitivity
                .admitted_for(filters.read_scope)
            {
                continue;
            }
            if !filters.memory_types.is_empty()
                && !filters
                    .memory_types
                    .iter()
                    .any(|memory_type| memory_type == &entry.projection.memory_type)
            {
                continue;
            }
            if !filters.include_expired && entry.projection.is_expired_on(&filters.as_of_date) {
                continue;
            }

            let score = cosine_similarity(query_embedding, &entry.embedding);
            if score < self.similarity_floor {
                continue;
            }

            hits.push(VectorHit {
                projection: entry.projection.clone(),
                score,
            });
        }

        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.projection.memory_id.cmp(&right.projection.memory_id))
        });
        hits.truncate(limit as usize);

        Ok(hits)
    }
}

fn build_projection(
    command: VectorProjectionCommand,
    created_at: String,
    updated_at: String,
) -> VectorRecordProjection {
    let content_hash = sha256_hash(command.canonical_text.as_bytes());

    VectorRecordProjection {
        memory_id: command.memory_id,
        memory_type: command.memory_type,
        subject: command.subject,
        predicate: command.predicate,
        object_text: command.object_text,
        canonical_text: command.canonical_text,
        expiration_date: command.expiration_date,
        sensitivity: command.sensitivity,
        content_hash,
        created_at,
        updated_at,
    }
}

/// Cosine similarity between two equally sized vectors.
///
/// Accumulates in `f64` to keep large-dimension sums meaningful, and returns
/// `0.0` for a zero-norm operand instead of producing `NaN`, because `NaN`
/// comparisons would make the ranking order non-deterministic.
pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0f64;
    let mut left_norm = 0.0f64;
    let mut right_norm = 0.0f64;

    for (left_value, right_value) in left.iter().zip(right.iter()) {
        let left_value = f64::from(*left_value);
        let right_value = f64::from(*right_value);
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }

    if left_norm == 0.0 || right_norm == 0.0 {
        return 0.0;
    }

    let magnitude = left_norm.sqrt() * right_norm.sqrt();
    if magnitude == 0.0 {
        return 0.0;
    }

    let similarity = dot / magnitude;
    if !similarity.is_finite() {
        return 0.0;
    }

    // Clamp away float noise so a perfect match cannot score above 1.0.
    similarity.clamp(-1.0, 1.0) as f32
}

/// Compare two scores for deterministic descending ordering.
pub fn compare_scores(left: f32, right: f32) -> Ordering {
    right.total_cmp(&left)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> SearchFirstVectorConfig {
        SearchFirstVectorConfig::default()
    }

    fn command(
        memory_id: &str,
        text: &str,
        sensitivity: RecordSensitivity,
    ) -> VectorProjectionCommand {
        VectorProjectionCommand {
            memory_id: memory_id.to_string(),
            memory_type: "fact".to_string(),
            subject: Some("user".to_string()),
            predicate: Some("prefers".to_string()),
            object_text: text.to_string(),
            canonical_text: text.to_string(),
            expiration_date: None,
            sensitivity,
        }
    }

    #[test]
    fn cosine_similarity_is_one_for_identical_directions() {
        let similarity = cosine_similarity(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]);

        assert!((similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_is_zero_for_a_zero_vector_instead_of_nan() {
        assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert!(!cosine_similarity(&[0.0, 0.0], &[1.0, 1.0]).is_nan());
    }

    #[test]
    fn scope_buckets_are_isolated_by_tenant_and_space() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope_a = ScopeKey {
            tenant_id: 1,
            space_id: 10,
        };
        let scope_b = ScopeKey {
            tenant_id: 1,
            space_id: 11,
        };

        index
            .upsert(
                scope_a,
                command("m-1", "alpha", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();

        let hits = index
            .rank(
                &scope_b,
                &[1.0, 0.0],
                &VectorRankFilters::unrestricted(),
                10,
            )
            .unwrap();

        assert!(
            hits.is_empty(),
            "a different space must not see the projection"
        );
        assert_eq!(index.scope_len(&scope_a), 1);
        assert_eq!(index.scope_len(&scope_b), 0);
    }

    #[test]
    fn sensitivity_gate_is_never_widened() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                command("m-public", "pub", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();
        index
            .upsert(
                scope,
                command("m-owner", "own", RecordSensitivity::Owner),
                vec![1.0, 0.0],
            )
            .unwrap();

        let public_only = index
            .rank(
                &scope,
                &[1.0, 0.0],
                &VectorRankFilters {
                    memory_types: Vec::new(),
                    read_scope: MemorySensitivityReadScope::Public,
                    ..VectorRankFilters::unrestricted()
                },
                10,
            )
            .unwrap();

        assert_eq!(public_only.len(), 1);
        assert_eq!(public_only[0].projection.memory_id, "m-public");
    }

    #[test]
    fn limit_bound_is_applied_after_sensitivity_filtering() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        for offset in 0..5 {
            index
                .upsert(
                    scope,
                    command(
                        &format!("m-owner-{offset}"),
                        "restricted",
                        RecordSensitivity::Owner,
                    ),
                    vec![1.0, 0.0],
                )
                .unwrap();
        }
        index
            .upsert(
                scope,
                command("m-public", "open", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();

        let hits = index
            .rank(
                &scope,
                &[1.0, 0.0],
                &VectorRankFilters {
                    memory_types: Vec::new(),
                    read_scope: MemorySensitivityReadScope::Public,
                    ..VectorRankFilters::unrestricted()
                },
                1,
            )
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].projection.memory_id, "m-public");
    }

    #[test]
    fn ranking_is_deterministic_and_ordered_by_descending_score() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                command("m-b", "close", RecordSensitivity::Public),
                vec![1.0, 0.1],
            )
            .unwrap();
        index
            .upsert(
                scope,
                command("m-a", "closer", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();

        let hits = index
            .rank(&scope, &[1.0, 0.0], &VectorRankFilters::unrestricted(), 10)
            .unwrap();

        assert_eq!(hits[0].projection.memory_id, "m-a");
        assert_eq!(hits[1].projection.memory_id, "m-b");
        assert!(hits[0].score >= hits[1].score);
    }

    #[test]
    fn mis_sized_embedding_is_refused_rather_than_padded() {
        let mut index = ScopedVectorIndex::new(3, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        let error = index
            .upsert(
                scope,
                command("m-1", "alpha", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap_err();

        assert!(matches!(
            error,
            SearchFirstVectorError::EmbeddingDimensionMismatch { .. }
        ));
        assert!(index.is_empty());
    }

    #[test]
    fn bucket_bound_evicts_the_oldest_projection() {
        let mut index = ScopedVectorIndex::new(
            2,
            &SearchFirstVectorConfig {
                max_entries_per_scope: 2,
                ..config()
            },
        )
        .unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        for memory_id in ["m-1", "m-2", "m-3"] {
            index
                .upsert(
                    scope,
                    command(memory_id, memory_id, RecordSensitivity::Public),
                    vec![1.0, 0.0],
                )
                .unwrap();
        }

        assert_eq!(index.scope_len(&scope), 2);
        assert_eq!(index.len(), 2);
        assert!(index.projection(&scope, "m-1").is_none());
        assert!(index.projection(&scope, "m-3").is_some());
    }

    #[test]
    fn removal_propagates_out_of_the_derived_index() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                command("m-1", "alpha", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();

        assert!(index.remove(&scope, "m-1"));
        assert!(!index.remove(&scope, "m-1"));
        assert!(index.is_empty());
    }

    #[test]
    fn blank_memory_id_is_refused() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        let error = index
            .upsert(
                scope,
                command("   ", "alpha", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap_err();

        assert!(matches!(
            error,
            SearchFirstVectorError::BlankMemoryId { .. }
        ));
    }

    #[test]
    fn refresh_keeps_the_original_creation_timestamp_and_derives_a_fresh_hash() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                command("m-1", "alpha", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();
        let created_at = index.projection(&scope, "m-1").unwrap().created_at.clone();

        let outcome = index
            .upsert(
                scope,
                command("m-1", "beta", RecordSensitivity::Public),
                vec![1.0, 0.0],
            )
            .unwrap();

        let refreshed = index.projection(&scope, "m-1").unwrap();
        assert_eq!(outcome, VectorProjectionOutcome::Refreshed);
        assert_eq!(refreshed.created_at, created_at);
        assert_eq!(refreshed.canonical_text, "beta");
        assert_eq!(refreshed.content_hash, sha256_hash(b"beta"));
        assert_eq!(index.len(), 1);
    }

    fn dated_command(
        memory_id: &str,
        text: &str,
        expiration_date: Option<&str>,
    ) -> VectorProjectionCommand {
        VectorProjectionCommand {
            expiration_date: expiration_date.map(str::to_string),
            ..command(memory_id, text, RecordSensitivity::Public)
        }
    }

    #[test]
    fn a_malformed_expiration_date_is_refused_at_write_time() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        let error = index
            .upsert(
                scope,
                dated_command("m-1", "tea", Some("2023-02-30")),
                vec![1.0, 0.0],
            )
            .unwrap_err();

        assert_eq!(
            error,
            SearchFirstVectorError::ExpirationDateInvalid {
                value: "2023-02-30".to_string()
            }
        );
        assert!(index.is_empty(), "a refused write must not be projected");
    }

    #[test]
    fn an_expiration_date_is_normalised_on_the_way_in() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                dated_command("m-1", "tea", Some("  2023-05-24  ")),
                vec![1.0, 0.0],
            )
            .unwrap();

        assert_eq!(
            index
                .projection(&scope, "m-1")
                .unwrap()
                .expiration_date
                .as_deref(),
            Some("2023-05-24")
        );
    }

    #[test]
    fn an_expired_projection_is_hidden_unless_it_is_asked_for() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                dated_command("m-expired", "old", Some("2023-05-01")),
                vec![1.0, 0.0],
            )
            .unwrap();
        index
            .upsert(
                scope,
                dated_command("m-live", "new", Some("2099-05-01")),
                vec![1.0, 0.0],
            )
            .unwrap();
        index
            .upsert(
                scope,
                dated_command("m-open", "forever", None),
                vec![1.0, 0.0],
            )
            .unwrap();

        let hidden = index
            .rank(
                &scope,
                &[1.0, 0.0],
                &VectorRankFilters {
                    include_expired: false,
                    as_of_date: "2023-05-02".to_string(),
                    ..VectorRankFilters::unrestricted()
                },
                10,
            )
            .unwrap();
        let mut ids: Vec<&str> = hidden
            .iter()
            .map(|hit| hit.projection.memory_id.as_str())
            .collect();
        ids.sort_unstable();

        assert_eq!(ids, vec!["m-live", "m-open"]);

        let shown = index
            .rank(
                &scope,
                &[1.0, 0.0],
                &VectorRankFilters {
                    include_expired: true,
                    as_of_date: "2023-05-02".to_string(),
                    ..VectorRankFilters::unrestricted()
                },
                10,
            )
            .unwrap();

        assert_eq!(shown.len(), 3);
    }

    #[test]
    fn the_expiration_date_itself_is_still_visible() {
        // The comparison is strict, matching the reference implementation: a
        // memory expiring today is not yet expired.
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                dated_command("m-1", "tea", Some("2023-05-02")),
                vec![1.0, 0.0],
            )
            .unwrap();

        let projection = index.projection(&scope, "m-1").unwrap();
        assert!(!projection.is_expired_on("2023-05-02"));
        assert!(projection.is_expired_on("2023-05-03"));
    }

    #[test]
    fn an_unreadable_reference_date_never_marks_a_memory_expired() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        index
            .upsert(
                scope,
                dated_command("m-1", "tea", Some("2023-05-02")),
                vec![1.0, 0.0],
            )
            .unwrap();

        let projection = index.projection(&scope, "m-1").unwrap();
        assert!(!projection.is_expired_on("not-a-date"));
        assert!(!projection.is_expired_on("2023-5-2"));
    }

    #[test]
    fn expiry_filters_before_the_limit_so_a_live_memory_is_never_displaced() {
        let mut index = ScopedVectorIndex::new(2, &config()).unwrap();
        let scope = ScopeKey {
            tenant_id: 1,
            space_id: 1,
        };

        // The expired memory scores highest; a limit applied before filtering
        // would let it consume the only slot.
        index
            .upsert(
                scope,
                dated_command("m-expired", "closest", Some("2023-05-01")),
                vec![1.0, 0.0],
            )
            .unwrap();
        index
            .upsert(
                scope,
                dated_command("m-live", "further", Some("2099-05-01")),
                vec![1.0, 0.4],
            )
            .unwrap();

        let hits = index
            .rank(
                &scope,
                &[1.0, 0.0],
                &VectorRankFilters {
                    include_expired: false,
                    as_of_date: "2023-05-02".to_string(),
                    ..VectorRankFilters::unrestricted()
                },
                1,
            )
            .unwrap();

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].projection.memory_id, "m-live");
    }

    #[test]
    fn normalize_expiration_date_accepts_only_real_calendar_dates() {
        assert_eq!(
            normalize_expiration_date("2024-02-29"),
            Some("2024-02-29".to_string())
        );
        assert_eq!(normalize_expiration_date("2023-02-29"), None);
        assert_eq!(normalize_expiration_date("1900-02-29"), None);
        assert_eq!(
            normalize_expiration_date("2000-02-29"),
            Some("2000-02-29".to_string())
        );
        assert_eq!(normalize_expiration_date("2023-04-31"), None);
        assert_eq!(normalize_expiration_date("2023-00-10"), None);
        assert_eq!(normalize_expiration_date("2023-12-00"), None);
        assert_eq!(normalize_expiration_date("2023-12-32"), None);
        assert_eq!(normalize_expiration_date("2023-1-01"), None);
        assert_eq!(normalize_expiration_date("23-01-01"), None);
        assert_eq!(normalize_expiration_date("2023-01-01T00:00:00Z"), None);
        assert_eq!(normalize_expiration_date(""), None);
    }
}
