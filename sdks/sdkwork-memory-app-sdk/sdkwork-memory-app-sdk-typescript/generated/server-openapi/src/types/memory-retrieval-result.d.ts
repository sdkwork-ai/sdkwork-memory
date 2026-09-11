import type { MemoryRetrievalHit } from './memory-retrieval-hit';
import type { MemoryRetrievalTrace } from './memory-retrieval-trace';
export interface MemoryRetrievalResult {
    retrievalId: string;
    trace?: MemoryRetrievalTrace | null;
    hits: MemoryRetrievalHit[];
    degraded: boolean;
}
//# sourceMappingURL=memory-retrieval-result.d.ts.map