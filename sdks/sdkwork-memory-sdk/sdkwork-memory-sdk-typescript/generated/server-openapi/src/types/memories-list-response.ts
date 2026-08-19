import type { MemoryRecord } from './memory-record';
import type { PageInfo } from './page-info';

export interface MemoriesListResponse {
  code: 0;
  data: unknown & { items: MemoryRecord[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
