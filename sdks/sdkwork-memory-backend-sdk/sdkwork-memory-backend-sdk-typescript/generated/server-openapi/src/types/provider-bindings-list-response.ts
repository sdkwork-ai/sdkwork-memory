import type { MemoryProviderBinding } from './memory-provider-binding';
import type { PageInfo } from './page-info';

export interface ProviderBindingsListResponse {
  code: 0;
  data: unknown & { items: MemoryProviderBinding[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
