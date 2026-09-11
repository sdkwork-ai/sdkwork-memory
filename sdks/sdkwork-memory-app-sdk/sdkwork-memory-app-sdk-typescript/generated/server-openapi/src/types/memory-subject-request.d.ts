export interface MemorySubjectRequest {
    organizationId?: string | null;
    subjectType: 'tenant' | 'organization' | 'user' | 'application' | 'service';
    subjectRef: string;
    displayName: string;
    defaultSpaceId?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-subject-request.d.ts.map