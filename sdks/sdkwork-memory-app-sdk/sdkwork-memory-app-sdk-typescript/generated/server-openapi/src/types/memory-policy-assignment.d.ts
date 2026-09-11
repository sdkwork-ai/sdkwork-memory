export interface MemoryPolicyAssignment {
    policyAssignmentId: string;
    tenantId: string;
    policyId: string;
    targetType: 'subject' | 'space' | 'entity' | 'binding' | 'capability_binding' | 'implementation_profile';
    targetId: string;
    priority: number;
    inheritanceMode: 'inherit' | 'override' | 'deny' | 'shadow';
    status: string;
    validFrom?: string | null;
    validTo?: string | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-policy-assignment.d.ts.map