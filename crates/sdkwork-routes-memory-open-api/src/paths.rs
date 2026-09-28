pub const PREFIX: &str = "/mem/v3/api";
pub const HEALTHZ: &str = "/healthz";
pub const CAPABILITIES: &str = "/mem/v3/api/memory/capabilities";
pub const EVENTS: &str = "/mem/v3/api/memory/events";
pub const EVENT: &str = "/mem/v3/api/memory/events/{eventId}";
pub const MEMORIES: &str = "/mem/v3/api/memory/memories";
pub const MEMORIES_DELETE_ALL: &str = "/mem/v3/api/memory/memories/delete_all";
pub const MEMORY: &str = "/mem/v3/api/memory/memories/{memoryId}";
pub const RETRIEVALS: &str = "/mem/v3/api/memory/retrievals";
pub const RETRIEVAL: &str = "/mem/v3/api/memory/retrievals/{retrievalId}";
pub const CONTEXT_PACKS: &str = "/mem/v3/api/memory/context_packs";
pub const CONTEXT_PACK: &str = "/mem/v3/api/memory/context_packs/{contextPackId}";
pub const FEEDBACK: &str = "/mem/v3/api/memory/feedback";
pub const EXTRACTIONS: &str = "/mem/v3/api/memory/extractions";
pub const CANDIDATES: &str = "/mem/v3/api/memory/candidates";
pub const CANDIDATE: &str = "/mem/v3/api/memory/candidates/{candidateId}";
pub const PROVIDER_HEALTH: &str = "/mem/v3/api/memory/provider_health";
pub const ENTITIES: &str = "/mem/v3/api/memory/entities";
pub const ENTITY: &str = "/mem/v3/api/memory/entities/{entityId}";
pub const EDGES: &str = "/mem/v3/api/memory/edges";
pub const EDGE: &str = "/mem/v3/api/memory/edges/{edgeId}";

// ---------------------------------------------------------------------------
// mem0 platform compatibility wire (`mem0-platform`, API_SPEC.md section
// 4.5.2). These mirror the mem0 platform REST paths the official `mem0ai`
// clients call verbatim. The trailing slash is significant: it is part of the
// upstream route, and the official clients append it.
// ---------------------------------------------------------------------------

/// Path prefixes owned by the mem0 compatibility protocol surface.
pub const MEM0_PATH_PREFIXES: [&str; 2] = ["/v1/", "/v3/"];
pub const MEM0_PING: &str = "/v1/ping/";
pub const MEM0_ENTITIES: &str = "/v1/entities/";
pub const MEM0_DELETE_ALL: &str = "/v1/memories/";
pub const MEM0_MEMORY: &str = "/v1/memories/{memory_id}/";
pub const MEM0_HISTORY: &str = "/v1/memories/{memory_id}/history/";
pub const MEM0_ADD: &str = "/v3/memories/add/";
pub const MEM0_SEARCH: &str = "/v3/memories/search/";
pub const MEM0_LIST: &str = "/v3/memories/";
pub const MEM0_FEEDBACK: &str = "/v1/feedback/";
/// One path, two methods: `PUT` updates a batch, `DELETE` removes one. Upstream
/// declares them on the same path (`/v1/batch/`), so the constant is shared.
pub const MEM0_BATCH: &str = "/v1/batch/";

/// Whether `path` belongs to the mem0 compatibility surface.
///
/// The single definition of that question, shared by the credential bridge
/// (which must rewrite only mem0 requests) and by the framework profile (which
/// must classify exactly those prefixes as open-api).
pub fn is_mem0_compat_path(path: &str) -> bool {
    MEM0_PATH_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}
