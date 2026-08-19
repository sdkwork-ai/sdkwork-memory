import type { MemorySubject } from './memory-subject';
import type { PageInfo } from './page-info';

export interface SubjectsListResponse {
  code: 0;
  data: unknown & { items: MemorySubject[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
