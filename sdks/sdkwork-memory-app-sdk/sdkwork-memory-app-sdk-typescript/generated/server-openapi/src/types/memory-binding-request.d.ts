export interface MemoryBindingRequest {
    spaceId?: string | null;
    bindingKind: 'ownership' | 'access' | 'share' | 'reference' | 'provision';
    bindingRole: string;
    sourceSubjectId?: string | null;
    targetSubjectId?: string | null;
    targetSpaceId?: string | null;
    capabilityCodes?: string[] | null;
    validFrom?: string | null;
    validTo?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-binding-request.d.ts.map