import type { MemoryEvent } from './memory-event';

export interface EventsRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryEvent; };
  /** Server-owned request correlation id. */
  traceId: string;
}
