import type { MemoryEvent } from './memory-event';

export interface EventsCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryEvent; };
  /** Server-owned request correlation id. */
  traceId: string;
}
