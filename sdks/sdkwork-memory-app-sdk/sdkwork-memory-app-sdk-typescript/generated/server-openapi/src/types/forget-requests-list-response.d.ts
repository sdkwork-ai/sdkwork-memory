import type { MemoryForgetJob } from './memory-forget-job';
import type { PageInfo } from './page-info';
export interface ForgetRequestsListResponse {
    code: 0;
    data: unknown & {
        items: MemoryForgetJob[];
        pageInfo: PageInfo;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=forget-requests-list-response.d.ts.map