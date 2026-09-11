export interface MemoryBinding {
    bindingId: string;
    tenantId: string;
    spaceId?: string | null;
    bindingKind: 'ownership' | 'access' | 'share' | 'reference' | 'provision';
    bindingRole: string;
    sourceSubjectId?: string | null;
    targetSubjectId?: string | null;
    targetSpaceId?: string | null;
    capabilityCodes?: string[] | null;
    status: string;
    validFrom?: string | null;
    validTo?: string | null;
    metadata?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-binding.d.ts.map