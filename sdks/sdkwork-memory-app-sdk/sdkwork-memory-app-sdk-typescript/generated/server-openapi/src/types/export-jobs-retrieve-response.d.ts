import type { MemoryExportJob } from './memory-export-job';
export interface ExportJobsRetrieveResponse {
    code: 0;
    data: unknown & {
        item: MemoryExportJob;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=export-jobs-retrieve-response.d.ts.map