import type { MemoryIndex } from './memory-index';

export interface IndexesRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryIndex; };
  /** Server-owned request correlation id. */
  traceId: string;
}
