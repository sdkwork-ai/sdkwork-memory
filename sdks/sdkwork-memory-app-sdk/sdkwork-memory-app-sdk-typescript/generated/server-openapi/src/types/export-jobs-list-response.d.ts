import type { MemoryExportJob } from './memory-export-job';
import type { PageInfo } from './page-info';
export interface ExportJobsListResponse {
    code: 0;
    data: unknown & {
        items: MemoryExportJob[];
        pageInfo: PageInfo;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=export-jobs-list-response.d.ts.map