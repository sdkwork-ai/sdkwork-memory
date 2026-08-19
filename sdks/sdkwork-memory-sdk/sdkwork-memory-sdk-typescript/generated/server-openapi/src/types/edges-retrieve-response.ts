import type { MemoryEdge } from './memory-edge';

export interface EdgesRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryEdge; };
  /** Server-owned request correlation id. */
  traceId: string;
}
