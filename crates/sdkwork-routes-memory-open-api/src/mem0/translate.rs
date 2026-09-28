//! Translation between the canonical memory contract and the mem0 wire.

use sdkwork_memory_contract::{MemoryEntity, MemoryRecord};
use sdkwork_utils_rust::sha256_hash;

use crate::mem0::dto::{Mem0EntityScope, Mem0HistoryEntry, Mem0Memory};

/// The four mem0 entity scopes paired with the metadata key each is persisted
/// under. The pairing is the contract: `filters: {"user_id": "..."}` only works
/// because the value is stored under exactly that key.
pub const MEM0_ENTITY_SCOPES: [(&str, &str); 4] = [
    ("user", "user_id"),
    ("agent", "agent_id"),
    ("run", "run_id"),
    ("app", "app_id"),
];

/// Context budget the compatibility wire requests.
///
/// mem0 has no context-budget concept — `search` is bounded by `top_k` alone —
/// while the canonical retrieval operation requires a positive budget. The value
/// only bounds the assembled context, never the hit list, so a fixed generous
/// default leaves mem0's `top_k` semantics exactly as upstream defines them.
pub const MEM0_CONTEXT_BUDGET_TOKENS: i32 = 4096;

/// mem0's documented platform default for `search` when the caller omits
/// `top_k` (`docs/migration/platform-v2-to-v3.mdx`).
pub const MEM0_DEFAULT_TOP_K: i64 = 10;

/// Upper bound this surface accepts for `top_k`.
///
/// The canonical retriever clamps to this ceiling internally. Upstream mem0
/// accepts up to 1000, so a larger request is refused with a named reason rather
/// than silently answered with fewer results than asked for.
pub const MEM0_MAX_TOP_K: i64 = 100;

/// Record scope label used for memories written through the compatibility wire.
///
/// mem0 has no scope concept; this label keeps compatibility records
/// distinguishable from records the canonical surfaces write, without inventing
/// a second storage shape.
pub const MEM0_RECORD_SCOPE: &str = "mem0";

/// Canonical memory-mutation audit actions mapped onto mem0's closed event enum.
///
/// mem0's platform schema declares `event` as `enum [ADD, UPDATE, DELETE]`
/// (`external/mem0/docs/openapi.json`, `/v1/memories/{memory_id}/history/`), so
/// the mapping has to be total over the canonical vocabulary rather than
/// best-effort. This is that vocabulary in full, taken from the journal writers
/// in `sdkwork-intelligence-memory-service` (`open_api.rs`,
/// `backend_admin_api.rs`, `candidate_promotion.rs`) — every row written with
/// `audit_resource_type = "memory_record"`. A test pins the list, and the
/// handler refuses by name on any action it does not cover, so a newly
/// introduced action cannot be dropped from a caller's history unnoticed.
///
/// Two entries are semantic rather than literal: `supersede` is mem0's UPDATE
/// (a replacement text was accepted over the previous one — the same arbitration
/// mem0's own ADD/UPDATE/DELETE pipeline performs), and `promoted` is mem0's ADD
/// (the record came into existence from a reviewed candidate).
pub const MEM0_AUDIT_ACTION_EVENTS: [(&str, &str); 5] = [
    ("memory.record.create", "ADD"),
    ("memory.candidate.promoted", "ADD"),
    ("memory.record.update", "UPDATE"),
    ("memory.record.supersede", "UPDATE"),
    ("memory.record.delete", "DELETE"),
];

/// Maps a mutating journal action onto the mem0 event vocabulary, or `None` when
/// the action has no mem0 representation.
pub fn event_from_audit_action(action: &str) -> Option<&'static str> {
    MEM0_AUDIT_ACTION_EVENTS
        .iter()
        .find(|(canonical, _)| *canonical == action)
        .map(|(_, event)| *event)
}

/// The value a record carries for one mem0 entity scope, if it was set at write
/// time.
pub fn record_entity(record: &MemoryRecord, metadata_key: &str) -> Option<String> {
    record
        .metadata
        .as_ref()?
        .get(metadata_key)?
        .as_str()
        .map(str::to_owned)
}

/// Builds the mem0 memory object for a canonical record.
///
/// `hash` is the canonical content digest. mem0's own `hash` is an MD5 of the
/// memory text; this surface reports the digest the canonical store already
/// derives from the same text, which is comparable within this deployment but
/// not byte-identical to upstream's. The divergence is deliberate and recorded —
/// duplicating upstream's MD5 would create a second, unshared hash derivation.
pub fn memory_from_record(
    record: &MemoryRecord,
    event: Option<&str>,
    score: Option<f64>,
) -> Mem0Memory {
    let user_id = record_entity(record, "user_id");
    let agent_id = record_entity(record, "agent_id");
    let run_id = record_entity(record, "run_id");
    let app_id = record_entity(record, "app_id");
    Mem0Memory {
        id: record
            .uuid
            .clone()
            .unwrap_or_else(|| record.memory_id.to_string()),
        messages: None,
        event: event.map(str::to_owned),
        memory: Some(record.canonical_text.clone()),
        user_id,
        agent_id,
        run_id,
        app_id,
        hash: Some(sha256_hash(record.canonical_text.as_bytes())),
        score,
        metadata: record.metadata.clone(),
        categories: None,
        created_at: Some(record.created_at.clone()),
        updated_at: Some(record.updated_at.clone()),
        expiration_date: record.expires_at.clone(),
    }
}

/// Builds a mem0 history entry from one audit row.
///
/// The content fields are null by construction: the canonical journal records
/// *that* a mutation was accepted, by whom, and when — never the memory text.
/// Copying record text into the audit trail would create a second copy of
/// personal data under a different retention and erasure path, so the
/// compatibility wire reports the events it can prove instead of fabricating
/// snapshots it does not hold. The official clients accept this: the TypeScript
/// client types both `oldMemory` and `newMemory` as `string | null`.
///
/// Returns `None` when the action has no mem0 event; the caller refuses the
/// request rather than reporting an incomplete event log.
pub fn history_entry_from_audit(
    audit_id: &str,
    memory_id: &str,
    action: &str,
    created_at: &str,
    user_id: Option<&str>,
) -> Option<Mem0HistoryEntry> {
    let event = event_from_audit_action(action)?;
    Some(Mem0HistoryEntry {
        id: audit_id.to_owned(),
        memory_id: memory_id.to_owned(),
        input: None,
        old_memory: None,
        new_memory: None,
        user_id: user_id.map(str::to_owned),
        categories: None,
        event: event.to_owned(),
        created_at: created_at.to_owned(),
        // An audit row is immutable, so its last modification is its creation.
        updated_at: created_at.to_owned(),
    })
}

/// Builds the mem0 entity-scope object for one canonical graph entity.
///
/// `owner` is supplied by the caller rather than read off the entity: the
/// canonical graph entity records which *space* it belongs to, while mem0's
/// platform reports the owning account. On this surface the mem0 "project" is
/// the SDKWork tenant the caller authenticated against — the same identity
/// `GET /v1/ping/` reports as `org_id`/`project_id` — so the tenant is the
/// account that owns every entity in it.
pub fn entity_scope_from_record(entity: &MemoryEntity, owner: &str) -> Mem0EntityScope {
    Mem0EntityScope {
        id: entity.entity_id.clone(),
        name: entity.canonical_name.clone(),
        owner: owner.to_owned(),
        entity_type: entity.entity_type.clone(),
        metadata: entity.attributes.clone(),
        created_at: entity.created_at.clone(),
        updated_at: entity.updated_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mapping must be total over the canonical `memory_record` journal
    /// vocabulary. The expected set is transcribed from the journal writers in
    /// `sdkwork-intelligence-memory-service`; if a new action is added there,
    /// this test is where the omission surfaces instead of becoming a silently
    /// truncated history.
    #[test]
    fn audit_action_mapping_covers_the_journal_vocabulary() {
        let mut actions: Vec<&str> = MEM0_AUDIT_ACTION_EVENTS
            .iter()
            .map(|(action, _)| *action)
            .collect();
        actions.sort_unstable();
        assert_eq!(
            actions,
            vec![
                "memory.candidate.promoted",
                "memory.record.create",
                "memory.record.delete",
                "memory.record.supersede",
                "memory.record.update",
            ]
        );
    }

    #[test]
    fn audit_actions_map_onto_the_closed_mem0_event_enum() {
        for (canonical, expected) in [
            ("memory.record.create", "ADD"),
            ("memory.candidate.promoted", "ADD"),
            ("memory.record.update", "UPDATE"),
            ("memory.record.supersede", "UPDATE"),
            ("memory.record.delete", "DELETE"),
        ] {
            assert_eq!(
                event_from_audit_action(canonical),
                Some(expected),
                "{canonical} must map to {expected}"
            );
        }
        // The verb-only spellings the journal does NOT use must not silently
        // match: a mismatch here would mean the real vocabulary is unmapped.
        assert_eq!(event_from_audit_action("memory.record.created"), None);
        assert_eq!(event_from_audit_action("memory.record.updated"), None);
        assert_eq!(event_from_audit_action("memory.record.deleted"), None);
    }

    #[test]
    fn unmapped_actions_have_no_event() {
        assert_eq!(event_from_audit_action("memory.record.expired"), None);
        assert_eq!(event_from_audit_action(""), None);
    }

    #[test]
    fn history_entry_is_content_free_but_carries_the_record_scope() {
        let entry = history_entry_from_audit(
            "9001",
            "7001",
            "memory.record.create",
            "2026-09-28T10:00:00Z",
            Some("alice"),
        )
        .expect("creation maps to ADD");
        assert_eq!(entry.event, "ADD");
        assert_eq!(entry.user_id.as_deref(), Some("alice"));
        assert_eq!(entry.created_at, "2026-09-28T10:00:00Z");
        assert_eq!(entry.updated_at, entry.created_at);
        assert!(entry.input.is_none());
        assert!(entry.old_memory.is_none());
        assert!(entry.new_memory.is_none());
        assert!(entry.categories.is_none());
    }
}
