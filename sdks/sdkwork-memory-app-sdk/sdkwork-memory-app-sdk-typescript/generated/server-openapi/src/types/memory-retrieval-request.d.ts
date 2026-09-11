export interface MemoryRetrievalRequest {
    query: string;
    spaceIds: string[];
    actorId?: string | null;
    retrievalProfileId?: string | null;
    memoryTypes?: ('working' | 'session' | 'semantic' | 'episodic' | 'procedural' | 'habit' | 'relationship' | 'domain_knowledge')[] | null;
    filters?: Record<string, unknown> | null;
    topK: number;
    contextBudgetTokens: number;
    includeTrace?: boolean;
}
//# sourceMappingURL=memory-retrieval-request.d.ts.map