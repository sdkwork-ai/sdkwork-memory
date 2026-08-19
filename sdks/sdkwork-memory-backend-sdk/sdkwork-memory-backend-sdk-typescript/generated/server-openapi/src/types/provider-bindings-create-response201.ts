import type { MemoryProviderBinding } from './memory-provider-binding';

export interface ProviderBindingsCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryProviderBinding; };
  /** Server-owned request correlation id. */
  traceId: string;
}
