export interface MemoryPolicyAssignmentPatch {
    priority?: number;
    inheritanceMode?: 'inherit' | 'override' | 'deny' | 'shadow';
    status?: string;
    validFrom?: string | null;
    validTo?: string | null;
}
//# sourceMappingURL=memory-policy-assignment-patch.d.ts.map