export interface MemorySpaceRequest {
    organizationId?: string | null;
    ownerSubjectType: string;
    ownerSubjectId: string;
    spaceType: string;
    displayName: string;
    defaultScope?: string;
    metadata?: Record<string, unknown> | null;
    policy?: Record<string, unknown> | null;
    version?: string | null;
}
//# sourceMappingURL=memory-space-request.d.ts.map