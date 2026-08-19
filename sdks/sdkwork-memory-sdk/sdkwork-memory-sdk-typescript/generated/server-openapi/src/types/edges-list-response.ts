import type { MemoryEdge } from './memory-edge';
import type { PageInfo } from './page-info';

export interface EdgesListResponse {
  code: 0;
  data: unknown & { items: MemoryEdge[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
