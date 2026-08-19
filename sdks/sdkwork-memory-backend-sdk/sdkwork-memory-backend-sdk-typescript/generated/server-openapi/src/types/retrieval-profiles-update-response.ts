import type { MemoryRetrievalProfile } from './memory-retrieval-profile';

export interface RetrievalProfilesUpdateResponse {
  code: 0;
  data: unknown & { item: MemoryRetrievalProfile; };
  /** Server-owned request correlation id. */
  traceId: string;
}
