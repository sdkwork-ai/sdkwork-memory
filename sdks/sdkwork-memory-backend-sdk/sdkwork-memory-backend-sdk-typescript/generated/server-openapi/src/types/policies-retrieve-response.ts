import type { MemoryPolicy } from './memory-policy';

export interface PoliciesRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryPolicy; };
  /** Server-owned request correlation id. */
  traceId: string;
}
