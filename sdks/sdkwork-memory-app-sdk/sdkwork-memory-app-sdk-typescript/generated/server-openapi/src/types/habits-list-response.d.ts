import type { MemoryHabit } from './memory-habit';
import type { PageInfo } from './page-info';
export interface HabitsListResponse {
    code: 0;
    data: unknown & {
        items: MemoryHabit[];
        pageInfo: PageInfo;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=habits-list-response.d.ts.map