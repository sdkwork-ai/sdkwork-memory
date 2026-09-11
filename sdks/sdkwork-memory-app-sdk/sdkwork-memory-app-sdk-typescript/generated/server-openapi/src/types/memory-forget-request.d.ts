export interface MemoryForgetRequest {
    scope: 'memory' | 'space' | 'user' | 'query';
    memoryIds?: string[] | null;
    spaceId?: string | null;
    query?: string | null;
    reason: string;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-forget-request.d.ts.map