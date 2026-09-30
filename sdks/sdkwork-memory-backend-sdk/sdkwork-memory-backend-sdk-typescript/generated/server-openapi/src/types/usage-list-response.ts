import type { MemoryUsageEntry } from './memory-usage-entry';
import type { PageInfo } from './page-info';

export interface UsageListResponse {
  code: 0;
  data: unknown & { items: MemoryUsageEntry[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
