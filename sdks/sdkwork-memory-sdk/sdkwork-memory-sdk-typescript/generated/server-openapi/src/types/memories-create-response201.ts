import type { MemoryRecord } from './memory-record';

export interface MemoriesCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryRecord; };
  /** Server-owned request correlation id. */
  traceId: string;
}
