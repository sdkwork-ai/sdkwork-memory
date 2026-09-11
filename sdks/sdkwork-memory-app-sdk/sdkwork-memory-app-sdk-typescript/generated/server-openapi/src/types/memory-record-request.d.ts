export interface MemoryRecordRequest {
    spaceId: string;
    userId?: string | null;
    scope: string;
    memoryType: 'working' | 'session' | 'semantic' | 'episodic' | 'procedural' | 'habit' | 'relationship' | 'domain_knowledge';
    subject?: string | null;
    predicate?: string | null;
    objectText?: string | null;
    canonicalText: string;
    summaryText?: string | null;
    confidence?: number | null;
    validFrom?: string | null;
    validTo?: string | null;
    expiresAt?: string | null;
    sensitivityLevel?: string;
    metadata?: Record<string, unknown> | null;
    tags?: string[] | null;
    version?: string | null;
}
//# sourceMappingURL=memory-record-request.d.ts.map