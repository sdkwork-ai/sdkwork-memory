export interface MemoryRetrievalProfileRequest {
    spaceId?: string | null;
    name: string;
    strategy: string;
    retrievers: Record<string, unknown>;
    fusionPolicy?: Record<string, unknown> | null;
    rerankPolicy?: Record<string, unknown> | null;
    topK: number;
    contextBudgetTokens: number;
    status?: string;
    version?: string | null;
}
//# sourceMappingURL=memory-retrieval-profile-request.d.ts.map