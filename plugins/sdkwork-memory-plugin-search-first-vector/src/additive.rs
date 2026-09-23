//! mem0 V3 additive write planning: content-hash deduplication and link
//! resolution.
//!
//! mem0's V3 write path is **ADD-only**. One extraction call yields a batch of
//! candidate memories; each is then either committed or dropped as a duplicate,
//! and any memory the model linked to an existing one carries that link forward.
//! Nothing is ever updated or deleted by this path, so there is no verdict to
//! arbitrate and no operation to choose between.
//!
//! Everything here is deterministic and model-free, which is what lets the
//! write path be tested without a provider.
//!
//! # Deliberate divergences from the reference implementation
//!
//! Two, both recorded rather than silently absorbed.
//!
//! 1. **The content digest is SHA-256, not MD5.** The reference uses
//!    `hashlib.md5` as a deduplication lookup key. The digest is only ever
//!    compared against itself, so the change has no semantic effect, and
//!    `sha256_hash` from `sdkwork-utils-rust` is the helper this workspace
//!    already mandates instead of duplicating a hash locally. A caller must
//!    therefore derive its digests with [`content_hash`]; a digest produced by a
//!    foreign implementation is not comparable to ours.
//! 2. **Links are validated, not discarded.** The reference asks the model to
//!    populate `linked_memory_ids` with ids from the offered evidence list and
//!    then never reads the field — its `uuid_mapping`, built to translate those
//!    ids back to real identifiers, is assigned and never consulted, and every
//!    `linked_memory_ids` access in the reference belongs to its separate entity
//!    store. This module instead keeps the links, checking each one against the
//!    evidence the caller actually offered and reporting any it could not
//!    verify. Nothing is invented, and nothing the model said is thrown away
//!    without a trace.

use std::collections::BTreeSet;

use sdkwork_utils_rust::sha256_hash;

use crate::extraction::{AttributedTo, ExistingMemoryView, ExtractedMemory};

/// Derive the content digest a memory is deduplicated by.
///
/// Callers that persist memories should store this value alongside the text so a
/// later write can deduplicate without re-reading the store.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::content_hash;
///
/// assert_eq!(content_hash("User drinks tea."), content_hash("User drinks tea."));
/// assert_ne!(content_hash("User drinks tea."), content_hash("User drinks coffee."));
/// ```
pub fn content_hash(text: &str) -> String {
    sha256_hash(text.as_bytes())
}

/// A memory awaiting commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedAddition {
    /// The memory text, exactly as extracted.
    pub text: String,
    /// Who the memory is about, when the model declared it recognisably.
    pub attributed_to: Option<AttributedTo>,
    /// Verified identifiers of existing memories this one relates to.
    ///
    /// Every entry was present in the evidence the caller offered, so committing
    /// these as edges cannot reference a memory the caller never sanctioned.
    pub linked_memory_ids: Vec<String>,
    /// The digest the addition was deduplicated by.
    pub content_hash: String,
}

/// Why a candidate memory was not committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateReason {
    /// A memory already known to the caller has the same content digest.
    KnownMemory {
        /// Identifier of the memory already holding that text.
        memory_id: String,
    },
    /// An earlier emission in the same response has the same content digest.
    EarlierEmission {
        /// Sequential identifier of the emission that already claimed the text.
        index: String,
    },
}

impl DuplicateReason {
    /// Short, stable label suitable for a metric or trace field.
    pub fn label(&self) -> &'static str {
        match self {
            Self::KnownMemory { .. } => "known_memory",
            Self::EarlierEmission { .. } => "earlier_emission",
        }
    }
}

/// A candidate memory dropped as a duplicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuppressedDuplicate {
    /// Sequential identifier of the dropped emission.
    pub index: String,
    /// Text that was dropped.
    pub text: String,
    /// Why it was dropped.
    pub reason: DuplicateReason,
}

/// A declared link that could not be verified against the offered evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnverifiedLink {
    /// Sequential identifier of the emission that declared the link.
    pub index: String,
    /// The identifier the model named, which the caller never offered.
    pub linked_memory_id: String,
}

/// Outcome of checking one memory's declared links against the evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkResolution {
    /// Links that name an offered memory, deduplicated, in declaration order.
    pub accepted: Vec<String>,
    /// Links that name a memory the caller never offered, in declaration order.
    pub unverified: Vec<String>,
}

/// Check one memory's declared links against the evidence the caller offered.
///
/// A link naming an id outside `existing` is not a link at all: honouring it
/// would let a model direct the store at a memory the caller never presented.
/// It is reported through [`LinkResolution::unverified`] rather than raised,
/// because dropping an unverifiable edge is safe while discarding an otherwise
/// valid memory over one bad link is not.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{
///     resolve_linked_memory_ids, AttributedTo, ExistingMemoryView, ExtractedMemory,
/// };
///
/// let memory = ExtractedMemory {
///     index: "0".to_string(),
///     text: "User walks Poppy daily.".to_string(),
///     attributed_to: Some(AttributedTo::User),
///     linked_memory_ids: vec!["m-1".to_string(), "m-ghost".to_string()],
/// };
/// let existing = vec![ExistingMemoryView {
///     memory_id: "m-1".to_string(),
///     text: "User has a dog named Poppy.".to_string(),
/// }];
///
/// let resolution = resolve_linked_memory_ids(&memory, &existing);
///
/// assert_eq!(resolution.accepted, vec!["m-1"]);
/// assert_eq!(resolution.unverified, vec!["m-ghost"]);
/// ```
pub fn resolve_linked_memory_ids(
    memory: &ExtractedMemory,
    existing: &[ExistingMemoryView],
) -> LinkResolution {
    let offered: BTreeSet<&str> = existing
        .iter()
        .map(|view| view.memory_id.trim())
        .filter(|memory_id| !memory_id.is_empty())
        .collect();

    let mut resolution = LinkResolution::default();
    let mut seen: BTreeSet<&str> = BTreeSet::new();

    for linked in memory.linked_memory_ids.iter().map(|id| id.trim()) {
        if linked.is_empty() || !seen.insert(linked) {
            continue;
        }

        if offered.contains(linked) {
            resolution.accepted.push(linked.to_string());
        } else {
            resolution.unverified.push(linked.to_string());
        }
    }

    resolution
}

/// What one additive write turn would do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdditivePlan {
    /// Memories to commit, in extraction order.
    pub additions: Vec<PlannedAddition>,
    /// Candidates dropped as duplicates, in extraction order.
    pub suppressed: Vec<SuppressedDuplicate>,
    /// Declared links that could not be verified, in extraction order.
    pub unverified_links: Vec<UnverifiedLink>,
}

impl AdditivePlan {
    /// Whether the plan commits nothing.
    pub fn is_empty(&self) -> bool {
        self.additions.is_empty()
    }
}

/// Decide which candidates to commit, without consulting a model.
///
/// Deduplication is by content digest: the first occurrence of a given text wins
/// and every later one is suppressed, whether it came from the store or from an
/// earlier emission in the same batch. That ordering is what makes the result
/// deterministic and independent of how the response was chunked.
///
/// # Examples
///
/// ```
/// use sdkwork_memory_plugin_search_first_vector::{
///     plan_additions, AttributedTo, ExistingMemoryView, ExtractedMemory,
/// };
///
/// let extracted = vec![
///     ExtractedMemory {
///         index: "0".to_string(),
///         text: "User drinks tea.".to_string(),
///         attributed_to: Some(AttributedTo::User),
///         linked_memory_ids: Vec::new(),
///     },
///     ExtractedMemory {
///         index: "1".to_string(),
///         text: "User drinks tea.".to_string(),
///         attributed_to: Some(AttributedTo::User),
///         linked_memory_ids: Vec::new(),
///     },
/// ];
///
/// let plan = plan_additions(&extracted, &[ExistingMemoryView {
///     memory_id: "m-1".to_string(),
///     text: "User keeps a garden.".to_string(),
/// }]);
///
/// assert_eq!(plan.additions.len(), 1);
/// assert_eq!(plan.suppressed.len(), 1);
/// assert_eq!(plan.suppressed[0].reason.label(), "earlier_emission");
/// ```
#[must_use = "an additive plan must be acted on, not discarded"]
pub fn plan_additions(
    extracted: &[ExtractedMemory],
    existing: &[ExistingMemoryView],
) -> AdditivePlan {
    let mut known: Vec<(String, String)> = Vec::new();
    for view in existing {
        let text = view.text.trim();
        if text.is_empty() {
            continue;
        }
        known.push((view.memory_id.trim().to_string(), content_hash(text)));
    }

    let mut committed: Vec<(String, String)> = Vec::new();
    let mut plan = AdditivePlan::default();

    for memory in extracted {
        let text = memory.text.trim();
        if text.is_empty() {
            continue;
        }
        let digest = content_hash(text);

        if let Some((memory_id, _)) = known
            .iter()
            .find(|(_, existing_digest)| existing_digest == &digest)
        {
            plan.suppressed.push(SuppressedDuplicate {
                index: memory.index.clone(),
                text: text.to_string(),
                reason: DuplicateReason::KnownMemory {
                    memory_id: memory_id.clone(),
                },
            });
            continue;
        }

        if let Some((index, _)) = committed
            .iter()
            .find(|(_, committed_digest)| committed_digest == &digest)
        {
            plan.suppressed.push(SuppressedDuplicate {
                index: memory.index.clone(),
                text: text.to_string(),
                reason: DuplicateReason::EarlierEmission {
                    index: index.clone(),
                },
            });
            continue;
        }

        committed.push((memory.index.clone(), digest.clone()));

        let resolution = resolve_linked_memory_ids(memory, existing);
        plan.unverified_links
            .extend(
                resolution
                    .unverified
                    .into_iter()
                    .map(|linked_memory_id| UnverifiedLink {
                        index: memory.index.clone(),
                        linked_memory_id,
                    }),
            );

        plan.additions.push(PlannedAddition {
            text: text.to_string(),
            attributed_to: memory.attributed_to,
            linked_memory_ids: resolution.accepted,
            content_hash: digest,
        });
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(index: &str, text: &str) -> ExtractedMemory {
        ExtractedMemory {
            index: index.to_string(),
            text: text.to_string(),
            attributed_to: Some(AttributedTo::User),
            linked_memory_ids: Vec::new(),
        }
    }

    fn memory_linking(index: &str, text: &str, links: &[&str]) -> ExtractedMemory {
        ExtractedMemory {
            linked_memory_ids: links.iter().map(|link| link.to_string()).collect(),
            ..memory(index, text)
        }
    }

    fn known(memory_id: &str, text: &str) -> ExistingMemoryView {
        ExistingMemoryView {
            memory_id: memory_id.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn a_novel_memory_is_planned_for_commit() {
        let plan = plan_additions(&[memory("0", "User drinks tea.")], &[]);

        assert_eq!(plan.additions.len(), 1);
        assert_eq!(plan.additions[0].text, "User drinks tea.");
        assert_eq!(
            plan.additions[0].content_hash,
            content_hash("User drinks tea.")
        );
        assert!(plan.suppressed.is_empty());
    }

    #[test]
    fn a_memory_already_known_is_suppressed() {
        let plan = plan_additions(
            &[memory("0", "User drinks tea.")],
            &[known("m-7", "User drinks tea.")],
        );

        assert!(plan.additions.is_empty());
        assert!(plan.is_empty());
        assert_eq!(
            plan.suppressed,
            vec![SuppressedDuplicate {
                index: "0".to_string(),
                text: "User drinks tea.".to_string(),
                reason: DuplicateReason::KnownMemory {
                    memory_id: "m-7".to_string()
                },
            }]
        );
    }

    #[test]
    fn the_second_identical_emission_is_suppressed_by_the_first() {
        let plan = plan_additions(
            &[
                memory("0", "User drinks tea."),
                memory("1", "User drinks tea."),
            ],
            &[],
        );

        assert_eq!(plan.additions.len(), 1);
        assert_eq!(plan.additions[0].text, "User drinks tea.");
        assert_eq!(plan.suppressed.len(), 1);
        assert_eq!(plan.suppressed[0].index, "1");
        assert_eq!(
            plan.suppressed[0].reason,
            DuplicateReason::EarlierEmission {
                index: "0".to_string()
            }
        );
    }

    #[test]
    fn deduplication_ignores_surrounding_whitespace() {
        let plan = plan_additions(
            &[
                memory("0", "User drinks tea."),
                memory("1", "  User drinks tea.  "),
            ],
            &[],
        );

        assert_eq!(plan.additions.len(), 1);
        assert_eq!(plan.suppressed.len(), 1);
    }

    #[test]
    fn the_store_is_consulted_before_the_batch() {
        // Both reasons apply; the store is the more informative one, and the
        // ordering must be stable so the choice is not accidental.
        let plan = plan_additions(
            &[
                memory("0", "User drinks tea."),
                memory("1", "User drinks tea."),
            ],
            &[known("m-7", "User drinks tea.")],
        );

        assert!(plan.additions.is_empty());
        assert_eq!(plan.suppressed.len(), 2);
        assert_eq!(plan.suppressed[0].reason.label(), "known_memory");
        assert_eq!(plan.suppressed[1].reason.label(), "known_memory");
    }
    #[test]
    fn distinct_memories_are_all_planned() {
        let plan = plan_additions(
            &[
                memory("0", "User drinks tea."),
                memory("1", "User keeps a garden."),
            ],
            &[known("m-1", "User lives in Berlin.")],
        );

        assert_eq!(plan.additions.len(), 2);
        assert!(plan.suppressed.is_empty());
    }

    #[test]
    fn an_offered_link_is_accepted() {
        let plan = plan_additions(
            &[memory_linking("0", "User walks Poppy daily.", &["m-1"])],
            &[known("m-1", "User has a dog named Poppy.")],
        );

        assert_eq!(plan.additions[0].linked_memory_ids, vec!["m-1"]);
        assert!(plan.unverified_links.is_empty());
    }

    #[test]
    fn a_link_to_an_offered_but_unrelated_memory_is_still_accepted() {
        // The contract asks the model to link related memories; judging relevance
        // is the model's job, and the caller's list is what bounds it.
        let plan = plan_additions(
            &[memory_linking("0", "User moved to Lisbon.", &["m-1"])],
            &[known("m-1", "User has a dog named Poppy.")],
        );

        assert_eq!(plan.additions[0].linked_memory_ids, vec!["m-1"]);
    }

    #[test]
    fn a_link_the_caller_never_offered_is_reported_and_dropped() {
        let plan = plan_additions(
            &[
                memory_linking("0", "User walks Poppy daily.", &["m-1", "m-ghost"]),
                memory_linking("1", "User moved to Lisbon.", &["m-also-ghost"]),
            ],
            &[known("m-1", "User has a dog named Poppy.")],
        );

        assert_eq!(plan.additions[0].linked_memory_ids, vec!["m-1"]);
        assert!(plan.additions[1].linked_memory_ids.is_empty());
        assert_eq!(
            plan.unverified_links,
            vec![
                UnverifiedLink {
                    index: "0".to_string(),
                    linked_memory_id: "m-ghost".to_string(),
                },
                UnverifiedLink {
                    index: "1".to_string(),
                    linked_memory_id: "m-also-ghost".to_string(),
                },
            ]
        );
    }

    #[test]
    fn duplicate_declared_links_are_collapsed() {
        let resolution = resolve_linked_memory_ids(
            &memory_linking("0", "User walks Poppy daily.", &["m-1", " m-1 ", "m-1"]),
            &[known("m-1", "User has a dog named Poppy.")],
        );

        assert_eq!(resolution.accepted, vec!["m-1"]);
        assert!(resolution.unverified.is_empty());
    }

    #[test]
    fn a_blank_declared_link_is_ignored() {
        let resolution = resolve_linked_memory_ids(
            &memory_linking("0", "User walks Poppy daily.", &["", "   ", "m-1"]),
            &[known("m-1", "User has a dog named Poppy.")],
        );

        assert_eq!(resolution.accepted, vec!["m-1"]);
        assert!(resolution.unverified.is_empty());
    }

    #[test]
    fn a_suppressed_duplicate_contributes_no_links() {
        let plan = plan_additions(
            &[
                memory("0", "User drinks tea."),
                memory_linking("1", "User drinks tea.", &["m-1"]),
            ],
            &[known("m-1", "User keeps a garden.")],
        );

        assert_eq!(plan.additions.len(), 1);
        assert!(plan.additions[0].linked_memory_ids.is_empty());
        assert!(plan.unverified_links.is_empty());
    }

    #[test]
    fn a_blank_known_text_is_not_usable_evidence() {
        let plan = plan_additions(&[memory("0", "User drinks tea.")], &[known("m-1", "   ")]);

        assert_eq!(plan.additions.len(), 1);
        assert!(plan.suppressed.is_empty());
    }

    #[test]
    fn a_known_text_deduplicates_even_when_its_id_is_blank() {
        // The text is what is known; the identifier is only what gets reported,
        // and an absent one is reported as absent rather than invented.
        let plan = plan_additions(
            &[memory_linking("0", "User drinks tea.", &["m-1"])],
            &[
                known("   ", "User drinks tea."),
                known("m-1", "User keeps a garden."),
            ],
        );

        assert!(plan.additions.is_empty());
        assert_eq!(
            plan.suppressed[0].reason,
            DuplicateReason::KnownMemory {
                memory_id: String::new()
            }
        );
    }

    #[test]
    fn an_empty_batch_plans_nothing() {
        let plan = plan_additions(&[], &[known("m-1", "User drinks tea.")]);

        assert!(plan.is_empty());
        assert!(plan.suppressed.is_empty());
        assert!(plan.unverified_links.is_empty());
    }

    #[test]
    fn attribution_survives_planning() {
        let mut assistant_memory = memory("0", "User was advised to rest.");
        assistant_memory.attributed_to = Some(AttributedTo::Assistant);

        let plan = plan_additions(&[assistant_memory], &[]);

        assert_eq!(
            plan.additions[0].attributed_to,
            Some(AttributedTo::Assistant)
        );
    }

    #[test]
    fn planning_is_deterministic_across_runs() {
        let extracted = vec![
            memory("0", "User drinks tea."),
            memory_linking("1", "User walks Poppy daily.", &["m-1", "m-ghost"]),
            memory("2", "User drinks tea."),
        ];
        let existing = vec![known("m-1", "User has a dog named Poppy.")];

        let first = plan_additions(&extracted, &existing);
        let second = plan_additions(&extracted, &existing);

        assert_eq!(first, second);
    }
}
