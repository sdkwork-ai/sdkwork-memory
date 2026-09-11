export interface MemoryEdge {
    edgeId: string;
    spaceId: string;
    sourceEntityId: string;
    targetEntityId: string;
    relationType: string;
    weight?: number | null;
    status: string;
    validFrom?: string | null;
    validTo?: string | null;
    metadata?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-edge.d.ts.map