import type { MemoryHabit } from './memory-habit';
export interface HabitsConfirmResponse {
    code: 0;
    data: unknown & {
        item: MemoryHabit;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=habits-confirm-response.d.ts.map