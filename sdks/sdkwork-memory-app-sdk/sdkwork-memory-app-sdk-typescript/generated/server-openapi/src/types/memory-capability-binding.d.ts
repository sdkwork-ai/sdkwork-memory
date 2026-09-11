export interface MemoryCapabilityBinding {
    capabilityBindingId: string;
    tenantId: string;
    capabilityCode: string;
    targetType: 'subject' | 'space' | 'binding' | 'memory';
    targetId: string;
    mode: 'allow' | 'deny' | 'conditional';
    priority: number;
    status: string;
    validFrom?: string | null;
    validTo?: string | null;
    metadata?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-capability-binding.d.ts.map