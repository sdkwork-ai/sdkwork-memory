import type { MemoryCapabilityBinding } from './memory-capability-binding';

export interface CapabilityBindingsRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryCapabilityBinding; };
  /** Server-owned request correlation id. */
  traceId: string;
}
