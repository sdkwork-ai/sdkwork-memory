import type { MemoryLearningJob } from './memory-learning-job';
import type { PageInfo } from './page-info';

export interface MigrationJobsListResponse {
  code: 0;
  data: unknown & { items: MemoryLearningJob[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
