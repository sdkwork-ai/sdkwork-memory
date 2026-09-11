export interface MemoryEvalRun {
    evalRunId: string;
    evalType: string;
    state: string;
    datasetRef?: string | null;
    profileRef?: string | null;
    metrics?: Record<string, unknown> | null;
    result?: Record<string, unknown> | null;
    startedAt?: string | null;
    finishedAt?: string | null;
    createdAt: string;
    updatedAt: string;
}
//# sourceMappingURL=memory-eval-run.d.ts.map