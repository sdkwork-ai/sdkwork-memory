import type { MemoryRecordSource } from './memory-record-source';
import type { PageInfo } from './page-info';
export interface MemoriesSourcesListResponse {
    code: 0;
    data: unknown & {
        items: MemoryRecordSource[];
        pageInfo: PageInfo;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=memories-sources-list-response.d.ts.map