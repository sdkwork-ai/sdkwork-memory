export interface MemoryImplementationProfile {
    implementationProfileId: string;
    name: string;
    implementationKind: 'native_sql' | 'event_sourced' | 'graph_temporal' | 'search_first' | 'local_embedded' | 'external_provider_bridge' | 'hybrid_platform';
    role: string;
    status: string;
    capabilities: Record<string, unknown>;
    config?: Record<string, unknown> | null;
    rollout?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-implementation-profile.d.ts.map