import type { MemoryEvalRun } from './memory-eval-run';

export interface EvalRunsRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryEvalRun; };
  /** Server-owned request correlation id. */
  traceId: string;
}
