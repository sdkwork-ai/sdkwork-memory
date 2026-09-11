export interface MemoryIndex {
    indexId: string;
    spaceId?: string | null;
    indexKind: 'sql' | 'keyword' | 'dictionary' | 'time' | 'event' | 'vector' | 'graph' | 'grep_file' | 'custom';
    implementationProfileId?: string | null;
    providerBindingId?: string | null;
    schemaVersion: string;
    status: string;
    config?: Record<string, unknown> | null;
    lastRebuiltAt?: string | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-index.d.ts.map