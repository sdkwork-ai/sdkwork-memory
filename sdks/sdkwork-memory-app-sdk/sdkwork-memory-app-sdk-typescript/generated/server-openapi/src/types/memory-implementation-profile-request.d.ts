export interface MemoryImplementationProfileRequest {
    name: string;
    implementationKind: string;
    role: string;
    status?: string;
    capabilities: Record<string, unknown>;
    config?: Record<string, unknown> | null;
    rollout?: Record<string, unknown> | null;
    version?: string | null;
}
//# sourceMappingURL=memory-implementation-profile-request.d.ts.map