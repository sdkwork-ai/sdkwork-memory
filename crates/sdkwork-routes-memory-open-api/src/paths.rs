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
///
/// `/v2/` is declared alongside `/v1/` and `/v3/` because the official clients
/// reach it: their entity-deletion and profile calls are built under `/v2/`.
/// Declaring it is what makes those calls *mem0-framed at all* — a path outside
/// every declared prefix is answered by the framework's surface classifier
/// before either mem0 bridge runs, so a caller gets `401` with
/// `application/problem+json`, which the Python client renders as raw text
/// (`mem0/client/utils.py` only unwraps `detail` when the content type starts
/// with `application/json`).
///
/// A declared prefix also has to carry real paths
/// (`mem0_wire_context_selector_contract`), which is why [`MEM0_REFUSED_PATHS`]
/// exists rather than the prefix being declared on its own.
pub const MEM0_PATH_PREFIXES: [&str; 3] = ["/v1/", "/v2/", "/v3/"];
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

// ---------------------------------------------------------------------------
// `/v2/` — the shapes the official clients call and this surface refuses by
// name.
//
// These are *refusals*, not implementations, and the difference is deliberate.
// Each one maps onto something this service either does not own (project-level
// profile settings live on mem0's platform-account plane) or cannot do without
// claiming more than the call means (mem0's entity deletion is a scope erasure;
// mem0's "profile" is an LLM-written summary this service does not store).
// Answering with a narrower result would be indistinguishable from success, so
// each is a `501` carrying its own reason.
//
// Registering them is also what keeps the declared `/v2/` prefix honest: a
// prefix with no path under it is rejected by
// `mem0_wire_context_selector_contract`, and a caller reaching an *unregistered*
// path under a declared prefix still gets the mem0 failure dialect (a mem0-shaped
// `404`) — just without the named reason.
//
// Path shapes are the upstream ones verbatim
// (`external/mem0/docs/openapi.json`), so the clients' own path builds match.
// ---------------------------------------------------------------------------

pub const MEM0_V2_ENTITIES: &str = "/v2/entities/{entity_type}/{entity_id}/";
pub const MEM0_V2_ENTITY_PROFILE: &str = "/v2/entities/{entity_type}/{entity_id}/profile/";
pub const MEM0_V2_PROFILE_JOBS: &str = "/v2/profiles/jobs/";
pub const MEM0_V2_PROFILE_JOB: &str = "/v2/profiles/jobs/{job_id}/";
pub const MEM0_V2_PROFILE_SETTINGS: &str = "/v2/profiles/settings/";

/// Every refusal-only path, as one list.
///
/// Named as identifiers rather than repeated string literals so the constants
/// above stay the single source. The coverage sampler resolves these names
/// against the route constants and fails loudly on a name that has no constant,
/// so a stale entry cannot pass unnoticed.
pub const MEM0_REFUSED_PATHS: [&str; 5] = [
    MEM0_V2_ENTITIES,
    MEM0_V2_ENTITY_PROFILE,
    MEM0_V2_PROFILE_JOBS,
    MEM0_V2_PROFILE_JOB,
    MEM0_V2_PROFILE_SETTINGS,
];

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The declared prefix set is what the framework classifies mem0 traffic
    /// with, and what the materialization gate validates the authority against.
    /// Pinned by value because a prefix added or dropped here silently changes
    /// the framing of every call under it.
    #[test]
    fn the_declared_prefixes_are_the_mem0_versions_the_clients_speak() {
        assert_eq!(MEM0_PATH_PREFIXES, ["/v1/", "/v2/", "/v3/"]);
    }

    /// A refusal path that fell outside the declared prefixes would be answered
    /// by the framework before its handler ran, which is the exact failure the
    /// `/v2/` declaration exists to remove.
    #[test]
    fn every_refusal_path_is_under_a_declared_prefix() {
        assert_eq!(MEM0_REFUSED_PATHS.len(), 5);
        for path in MEM0_REFUSED_PATHS {
            assert!(
                is_mem0_compat_path(path),
                "{path} is refused but sits outside every declared mem0 prefix"
            );
            assert!(
                path.starts_with("/v2/"),
                "{path} is not a `/v2/` path, so its refusal is not the one this list records"
            );
            assert!(
                path.ends_with('/'),
                "{path} must keep the trailing slash the official clients append"
            );
        }
    }
}
