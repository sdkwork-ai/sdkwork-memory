export interface MemorySubject {
    subjectId: string;
    tenantId: string;
    organizationId?: string | null;
    subjectType: 'tenant' | 'organization' | 'user' | 'application' | 'service';
    subjectRef: string;
    displayName: string;
    defaultSpaceId?: string | null;
    status: string;
    metadata?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-subject.d.ts.map