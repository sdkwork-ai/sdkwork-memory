pub const PREFIX: &str = "/backend/v3/api";
pub const HEALTHZ: &str = "/healthz";
pub const SPACES: &str = "/backend/v3/api/memory/spaces";
pub const SPACE: &str = "/backend/v3/api/memory/spaces/{spaceId}";
pub const MEMORIES: &str = "/backend/v3/api/memory/memories";
pub const MEMORY: &str = "/backend/v3/api/memory/memories/{memoryId}";
pub const MEMORY_SUPERSEDE: &str = "/backend/v3/api/memory/memories/{memoryId}/supersede";
pub const EVENTS: &str = "/backend/v3/api/memory/events";
pub const EVENT: &str = "/backend/v3/api/memory/events/{eventId}";
pub const CANDIDATES: &str = "/backend/v3/api/memory/candidates";
pub const CANDIDATE_APPROVE: &str = "/backend/v3/api/memory/candidates/{candidateId}/approve";
pub const CANDIDATE_REJECT: &str = "/backend/v3/api/memory/candidates/{candidateId}/reject";
pub const EXTRACTION_JOBS: &str = "/backend/v3/api/memory/extraction_jobs";
pub const EXTRACTION_JOB: &str = "/backend/v3/api/memory/extraction_jobs/{jobId}";
pub const CONSOLIDATION_JOBS: &str = "/backend/v3/api/memory/consolidation_jobs";
pub const CONSOLIDATION_JOB: &str = "/backend/v3/api/memory/consolidation_jobs/{jobId}";
pub const INDEXES: &str = "/backend/v3/api/memory/indexes";
pub const INDEX: &str = "/backend/v3/api/memory/indexes/{indexId}";
pub const INDEX_REBUILD: &str = "/backend/v3/api/memory/indexes/{indexId}/rebuild";
pub const RETRIEVAL_PROFILES: &str = "/backend/v3/api/memory/retrieval_profiles";
pub const RETRIEVAL_PROFILE: &str = "/backend/v3/api/memory/retrieval_profiles/{profileId}";
pub const IMPLEMENTATION_PROFILES: &str = "/backend/v3/api/memory/implementation_profiles";
pub const IMPLEMENTATION_PROFILE: &str =
    "/backend/v3/api/memory/implementation_profiles/{implementationProfileId}";
pub const PROVIDER_BINDINGS: &str = "/backend/v3/api/memory/provider_bindings";
pub const PROVIDER_BINDING: &str = "/backend/v3/api/memory/provider_bindings/{providerBindingId}";
pub const PROVIDER_HEALTH: &str = "/backend/v3/api/memory/provider_health";
pub const EVAL_RUNS: &str = "/backend/v3/api/memory/eval_runs";
pub const EVAL_RUN: &str = "/backend/v3/api/memory/eval_runs/{evalRunId}";
pub const RETRIEVAL_TRACES: &str = "/backend/v3/api/memory/retrieval_traces";
pub const RETRIEVAL_TRACE: &str = "/backend/v3/api/memory/retrieval_traces/{traceId}";
pub const AUDIT_LOGS: &str = "/backend/v3/api/memory/audit_logs";
pub const RETENTION_JOBS: &str = "/backend/v3/api/memory/retention_jobs";
pub const RETENTION_JOB: &str = "/backend/v3/api/memory/retention_jobs/{retentionJobId}";
pub const MIGRATION_JOBS: &str = "/backend/v3/api/memory/migration_jobs";
pub const MIGRATION_JOB: &str = "/backend/v3/api/memory/migration_jobs/{migrationJobId}";

// Commercial subject, binding, and capability-management paths.
pub const SUBJECTS: &str = "/backend/v3/api/memory/subjects";
pub const SUBJECT: &str = "/backend/v3/api/memory/subjects/{subjectId}";
pub const BINDINGS: &str = "/backend/v3/api/memory/bindings";
pub const BINDING: &str = "/backend/v3/api/memory/bindings/{bindingId}";
pub const CAPABILITY_BINDINGS: &str = "/backend/v3/api/memory/capability_bindings";
pub const CAPABILITY_BINDING: &str =
    "/backend/v3/api/memory/capability_bindings/{capabilityBindingId}";
pub const CAPABILITIES_RESOLVE: &str = "/backend/v3/api/memory/capabilities/resolve";

// Commercial graph, policy, and readiness paths.
pub const ENTITIES: &str = "/backend/v3/api/memory/entities";
pub const ENTITY: &str = "/backend/v3/api/memory/entities/{entityId}";
pub const EDGES: &str = "/backend/v3/api/memory/edges";
pub const EDGE: &str = "/backend/v3/api/memory/edges/{edgeId}";
pub const POLICIES: &str = "/backend/v3/api/memory/policies";
pub const POLICY: &str = "/backend/v3/api/memory/policies/{policyId}";
pub const POLICY_ASSIGNMENTS: &str = "/backend/v3/api/memory/policy_assignments";
pub const POLICY_ASSIGNMENT: &str =
    "/backend/v3/api/memory/policy_assignments/{policyAssignmentId}";
pub const COMMERCIAL_READINESS: &str = "/backend/v3/api/memory/commercial_readiness";
pub const COMMERCIAL_READINESS_REBUILD: &str =
    "/backend/v3/api/memory/commercial_readiness/rebuild";

// Usage metering facts.
pub const USAGE: &str = "/backend/v3/api/memory/usage";
