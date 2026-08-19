import type { MemoryRetrievalResult } from './memory-retrieval-result';

export interface RetrievalsRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryRetrievalResult; };
  /** Server-owned request correlation id. */
  traceId: string;
}
