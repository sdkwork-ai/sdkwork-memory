import type { MemoryPolicy } from './memory-policy';
import type { PageInfo } from './page-info';

export interface PoliciesListResponse {
  code: 0;
  data: unknown & { items: MemoryPolicy[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
