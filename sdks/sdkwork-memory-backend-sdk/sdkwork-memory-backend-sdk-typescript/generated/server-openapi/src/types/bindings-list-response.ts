import type { MemoryBinding } from './memory-binding';
import type { PageInfo } from './page-info';

export interface BindingsListResponse {
  code: 0;
  data: unknown & { items: MemoryBinding[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
