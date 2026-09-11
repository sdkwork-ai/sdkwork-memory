export interface MemoryPolicyAssignmentRequest {
    policyId: string;
    targetType: 'subject' | 'space' | 'entity' | 'binding' | 'capability_binding' | 'implementation_profile';
    targetId: string;
    priority?: number;
    inheritanceMode: 'inherit' | 'override' | 'deny' | 'shadow';
    validFrom?: string | null;
    validTo?: string | null;
}
//# sourceMappingURL=memory-policy-assignment-request.d.ts.map