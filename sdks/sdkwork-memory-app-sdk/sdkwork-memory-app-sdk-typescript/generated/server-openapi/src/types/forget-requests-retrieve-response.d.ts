import type { MemoryForgetJob } from './memory-forget-job';
export interface ForgetRequestsRetrieveResponse {
    code: 0;
    data: unknown & {
        item: MemoryForgetJob;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=forget-requests-retrieve-response.d.ts.map