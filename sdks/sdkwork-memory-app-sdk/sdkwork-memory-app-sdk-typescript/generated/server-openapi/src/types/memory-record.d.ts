export interface MemoryRecord {
    memoryId: string;
    uuid?: string;
    spaceId: string;
    userId?: string | null;
    scope: string;
    memoryType: 'working' | 'session' | 'semantic' | 'episodic' | 'procedural' | 'habit' | 'relationship' | 'domain_knowledge';
    subject?: string | null;
    predicate?: string | null;
    objectText?: string;
    canonicalText: string;
    summaryText?: string | null;
    language?: string | null;
    confidence: number;
    evidenceCount?: number;
    contradictionCount?: number;
    importanceScore?: number;
    recencyScore?: number;
    habitStrength?: number | null;
    validFrom?: string | null;
    validTo?: string | null;
    expiresAt?: string | null;
    status: 'candidate' | 'active' | 'inactive' | 'superseded' | 'deleted' | 'rejected';
    sensitivityLevel?: string;
    metadata?: Record<string, unknown> | null;
    tags?: string[] | null;
    supersedesMemoryId?: string | null;
    supersededByMemoryId?: string | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-record.d.ts.map