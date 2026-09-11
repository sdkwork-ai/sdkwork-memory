import type { MemorySpace } from './memory-space';
export interface SpacesRetrieveResponse {
    code: 0;
    data: unknown & {
        item: MemorySpace;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=spaces-retrieve-response.d.ts.map