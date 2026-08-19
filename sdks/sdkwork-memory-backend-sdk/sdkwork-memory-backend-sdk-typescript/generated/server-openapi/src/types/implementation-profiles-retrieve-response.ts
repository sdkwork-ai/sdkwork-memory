import type { MemoryImplementationProfile } from './memory-implementation-profile';

export interface ImplementationProfilesRetrieveResponse {
  code: 0;
  data: unknown & { item: MemoryImplementationProfile; };
  /** Server-owned request correlation id. */
  traceId: string;
}
