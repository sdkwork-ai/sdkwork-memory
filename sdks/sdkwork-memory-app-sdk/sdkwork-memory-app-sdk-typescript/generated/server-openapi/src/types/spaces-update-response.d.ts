import type { MemorySpace } from './memory-space';
export interface SpacesUpdateResponse {
    code: 0;
    data: unknown & {
        item: MemorySpace;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=spaces-update-response.d.ts.map