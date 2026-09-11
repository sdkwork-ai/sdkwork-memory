import type { MemoryHabit } from './memory-habit';
export interface HabitsUpdateResponse {
    code: 0;
    data: unknown & {
        item: MemoryHabit;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=habits-update-response.d.ts.map