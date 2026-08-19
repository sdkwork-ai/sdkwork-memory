import type { MemoryEvent } from './memory-event';
import type { PageInfo } from './page-info';

export interface EventsListResponse {
  code: 0;
  data: unknown & { items: MemoryEvent[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
