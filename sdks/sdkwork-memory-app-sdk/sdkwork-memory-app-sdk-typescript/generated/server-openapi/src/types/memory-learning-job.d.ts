export interface MemoryLearningJob {
    jobId: string;
    spaceId?: string | null;
    jobType: string;
    state: string;
    priority: number;
    result?: Record<string, unknown> | null;
    error?: Record<string, unknown> | null;
    startedAt?: string | null;
    finishedAt?: string | null;
    createdAt: string;
    updatedAt: string;
    version?: string;
}
//# sourceMappingURL=memory-learning-job.d.ts.map