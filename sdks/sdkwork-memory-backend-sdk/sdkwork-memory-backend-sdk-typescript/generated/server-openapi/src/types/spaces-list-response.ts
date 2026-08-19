import type { MemorySpace } from './memory-space';
import type { PageInfo } from './page-info';

export interface SpacesListResponse {
  code: 0;
  data: unknown & { items: MemorySpace[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
