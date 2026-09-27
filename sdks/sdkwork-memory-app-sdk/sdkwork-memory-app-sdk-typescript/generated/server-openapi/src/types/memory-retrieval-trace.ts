export interface MemoryRetrievalTrace {
  traceId: string;
  spaceId?: string | null;
  retrievalProfileId?: string | null;
  actorId?: string | null;
  queryText?: string | null;
  queryHash: string;
  resultCount: number;
  degraded: boolean;
  createdAt: string;
}
