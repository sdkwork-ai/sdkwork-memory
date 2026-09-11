export interface MemoryEntity {
    entityId: string;
    spaceId: string;
    entityType: string;
    canonicalName: string;
    aliases?: string[] | null;
    attributes?: Record<string, unknown> | null;
    sensitivityLevel: string;
    status: string;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-entity.d.ts.map