import type { MemoryEntity } from './memory-entity';

export interface EntitiesCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryEntity; };
  /** Server-owned request correlation id. */
  traceId: string;
}
