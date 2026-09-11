export interface MemoryRetrievalProfile {
    retrievalProfileId: string;
    spaceId?: string | null;
    name: string;
    strategy: string;
    retrievers: Record<string, unknown>;
    fusionPolicy?: Record<string, unknown> | null;
    rerankPolicy?: Record<string, unknown> | null;
    topK: number;
    contextBudgetTokens: number;
    status: string;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-retrieval-profile.d.ts.map