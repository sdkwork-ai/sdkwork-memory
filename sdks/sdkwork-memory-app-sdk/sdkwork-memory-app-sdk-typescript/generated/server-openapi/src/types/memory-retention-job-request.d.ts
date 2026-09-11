export interface MemoryRetentionJobRequest {
    scope: string;
    reason: string;
    spaceId?: string | null;
    dryRun?: boolean;
    policyRef?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-retention-job-request.d.ts.map