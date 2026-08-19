import type { MemoryBinding } from './memory-binding';

export interface BindingsRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryBinding; };
  /** Server-owned request correlation id. */
  traceId: string;
}
