export interface MemoryProviderBinding {
    providerBindingId: string;
    providerKind: string;
    providerCode: string;
    displayName: string;
    endpointRef?: string | null;
    secretRef?: string | null;
    modelRef?: string | null;
    capabilities: Record<string, unknown>;
    config?: Record<string, unknown> | null;
    healthState: string;
    lastHealthAt?: string | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-provider-binding.d.ts.map