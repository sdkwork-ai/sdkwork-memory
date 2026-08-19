import type { MemoryCandidate } from './memory-candidate';

export interface CandidatesRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryCandidate; };
  /** Server-owned request correlation id. */
  traceId: string;
}
