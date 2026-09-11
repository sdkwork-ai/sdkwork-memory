import type { MemoryCandidate } from './memory-candidate';
export interface CandidatesApproveResponse {
    code: 0;
    data: unknown & {
        item: MemoryCandidate;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=candidates-approve-response.d.ts.map