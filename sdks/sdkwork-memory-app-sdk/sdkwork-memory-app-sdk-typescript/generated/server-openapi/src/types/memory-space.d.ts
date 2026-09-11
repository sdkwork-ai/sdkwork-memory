export interface MemorySpace {
    spaceId: string;
    uuid?: string;
    tenantId: string;
    organizationId?: string | null;
    ownerSubjectType: string;
    ownerSubjectId: string;
    spaceType: string;
    displayName: string;
    defaultScope?: string;
    lifecycleStatus: string;
    metadata?: Record<string, unknown> | null;
    policy?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-space.d.ts.map