import type { MemoryLearningJob } from './memory-learning-job';

export interface ExtractionsCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryLearningJob; };
  /** Server-owned request correlation id. */
  traceId: string;
}
