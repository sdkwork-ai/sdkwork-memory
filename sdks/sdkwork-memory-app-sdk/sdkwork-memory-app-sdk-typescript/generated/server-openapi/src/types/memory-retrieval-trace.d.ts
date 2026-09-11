export interface MemoryRetrievalTrace {
    traceId: string;
    spaceId?: string | null;
    retrievalProfileId?: string | null;
    actorId?: string | null;
    queryText?: string | null;
    queryHash: string;
    retrievers?: Record<string, unknown> | null;
    latencyMs?: number | null;
    resultCount: number;
    degraded: boolean;
    metadata?: Record<string, unknown> | null;
    createdAt: string;
}
//# sourceMappingURL=memory-retrieval-trace.d.ts.map