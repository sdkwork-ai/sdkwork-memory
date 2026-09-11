export interface MemoryProviderBindingRequest {
    providerKind: string;
    providerCode: string;
    displayName: string;
    endpointRef?: string | null;
    secretRef?: string | null;
    modelRef?: string | null;
    capabilities: Record<string, unknown>;
    config?: Record<string, unknown> | null;
    healthState?: string;
    version?: string | null;
}
//# sourceMappingURL=memory-provider-binding-request.d.ts.map