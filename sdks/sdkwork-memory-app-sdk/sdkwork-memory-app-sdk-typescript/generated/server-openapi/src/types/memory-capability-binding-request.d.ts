export interface MemoryCapabilityBindingRequest {
    capabilityCode: string;
    targetType: 'subject' | 'space' | 'binding' | 'memory';
    targetId: string;
    mode: 'allow' | 'deny' | 'conditional';
    priority?: number;
    validFrom?: string | null;
    validTo?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-capability-binding-request.d.ts.map