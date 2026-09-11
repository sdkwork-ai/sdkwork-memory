import type { MemoryRecord } from './memory-record';
export interface MemoryRetrievalHit {
    hitId: string;
    memory?: MemoryRecord | null;
    memoryId?: string | null;
    retrieverName: string;
    resultRank: number;
    rawScore?: number | null;
    fusedScore?: number | null;
    explanation?: Record<string, unknown> | null;
    status: string;
}
//# sourceMappingURL=memory-retrieval-hit.d.ts.map