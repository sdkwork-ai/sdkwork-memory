export interface MemoryEntityRequest {
    spaceId: string;
    entityType: string;
    canonicalName: string;
    aliases?: string[] | null;
    attributes?: Record<string, unknown> | null;
    sensitivityLevel?: string;
}
//# sourceMappingURL=memory-entity-request.d.ts.map