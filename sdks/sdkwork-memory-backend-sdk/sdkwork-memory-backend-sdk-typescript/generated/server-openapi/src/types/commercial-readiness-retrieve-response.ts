import type { MemoryCommercialReadiness } from './memory-commercial-readiness';

export interface CommercialReadinessRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryCommercialReadiness; };
  /** Server-owned request correlation id. */
  traceId: string;
}
