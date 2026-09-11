export interface MemoryPolicy {
    policyId: string;
    tenantId: string;
    policyType: string;
    scope: string;
    scopeRef?: string | null;
    status: string;
    policy: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-policy.d.ts.map