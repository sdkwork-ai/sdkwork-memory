import type { MemoryPolicy } from './memory-policy';

export interface PoliciesCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryPolicy; };
  /** Server-owned request correlation id. */
  traceId: string;
}
