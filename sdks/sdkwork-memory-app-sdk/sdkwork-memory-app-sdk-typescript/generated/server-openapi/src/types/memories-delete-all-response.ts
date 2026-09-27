import type { DeleteAllMemoriesResult } from './delete-all-memories-result';

export interface MemoriesDeleteAllResponse {
  code: 0;
  data: unknown & DeleteAllMemoriesResult;
  /** Server-owned request correlation id. */
  traceId: string;
}
