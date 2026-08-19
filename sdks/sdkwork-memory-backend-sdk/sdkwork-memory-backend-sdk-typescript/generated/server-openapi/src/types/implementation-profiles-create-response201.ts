import type { MemoryImplementationProfile } from './memory-implementation-profile';

export interface ImplementationProfilesCreateResponse201 {
  code: 0;
  data: unknown & { item: MemoryImplementationProfile; };
  /** Server-owned request correlation id. */
  traceId: string;
}
