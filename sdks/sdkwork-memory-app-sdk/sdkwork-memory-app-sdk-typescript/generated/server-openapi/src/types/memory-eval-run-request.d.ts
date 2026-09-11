export interface MemoryEvalRunRequest {
    /** Executable evaluation engine. Additional evaluation types remain unavailable until a production worker implementation exists. */
    evalType: 'retrieval_quality';
    datasetRef?: string | null;
    profileRef?: string | null;
    config?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-eval-run-request.d.ts.map