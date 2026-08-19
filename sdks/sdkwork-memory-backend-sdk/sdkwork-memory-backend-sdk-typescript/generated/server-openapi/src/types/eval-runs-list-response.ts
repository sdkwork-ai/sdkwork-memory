import type { MemoryEvalRun } from './memory-eval-run';
import type { PageInfo } from './page-info';

export interface EvalRunsListResponse {
  code: 0;
  data: unknown & { items: MemoryEvalRun[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
