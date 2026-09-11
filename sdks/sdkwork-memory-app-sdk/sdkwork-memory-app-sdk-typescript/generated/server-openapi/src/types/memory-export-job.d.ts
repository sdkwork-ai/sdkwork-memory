export interface MemoryExportJob {
    exportJobId: string;
    state: string;
    format: string;
    driveObjectRef?: string | null;
    result?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
}
//# sourceMappingURL=memory-export-job.d.ts.map