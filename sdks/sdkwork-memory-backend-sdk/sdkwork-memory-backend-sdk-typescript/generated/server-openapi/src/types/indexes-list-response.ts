import type { MemoryIndex } from './memory-index';
import type { PageInfo } from './page-info';

export interface IndexesListResponse {
  code: 0;
  data: unknown & { items: MemoryIndex[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
