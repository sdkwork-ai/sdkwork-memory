import type { MemoryRetrievalProfile } from './memory-retrieval-profile';
import type { PageInfo } from './page-info';

export interface RetrievalProfilesListResponse {
  code: 0;
  data: unknown & { items: MemoryRetrievalProfile[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
