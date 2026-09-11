export interface MemoryContextPackRequest {
    query: string;
    spaceIds: string[];
    actorId?: string | null;
    retrievalProfileId?: string | null;
    contextBudgetTokens: number;
    includeCitations?: boolean;
    filters?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-context-pack-request.d.ts.map