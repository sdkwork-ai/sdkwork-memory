export interface MemoryEdgeRequest {
    spaceId: string;
    sourceEntityId: string;
    targetEntityId: string;
    relationType: string;
    weight?: number | null;
    validFrom?: string | null;
    validTo?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-edge-request.d.ts.map