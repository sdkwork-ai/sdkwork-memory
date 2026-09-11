import type { MemoryHabit } from './memory-habit';
export interface HabitsRejectResponse {
    code: 0;
    data: unknown & {
        item: MemoryHabit;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=habits-reject-response.d.ts.map