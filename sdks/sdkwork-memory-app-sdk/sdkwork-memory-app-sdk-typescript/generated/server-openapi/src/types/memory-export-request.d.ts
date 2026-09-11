export interface MemoryExportRequest {
    spaceIds: string[];
    format: 'json' | 'jsonl' | 'markdown';
    includeEvents?: boolean;
    driveTargetRef?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-export-request.d.ts.map