import type { MemoryCapabilities } from './memory-capabilities';

export interface CapabilitiesRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryCapabilities; };
  /** Server-owned request correlation id. */
  traceId: string;
}
