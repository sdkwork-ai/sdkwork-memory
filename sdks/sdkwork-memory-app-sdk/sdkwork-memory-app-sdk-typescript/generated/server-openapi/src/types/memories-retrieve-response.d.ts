import type { MemoryRecord } from './memory-record';
export interface MemoriesRetrieveResponse {
    code: 0;
    data: unknown & {
        item: MemoryRecord;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=memories-retrieve-response.d.ts.map