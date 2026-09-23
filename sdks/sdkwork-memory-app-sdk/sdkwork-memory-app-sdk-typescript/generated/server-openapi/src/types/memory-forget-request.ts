export interface MemoryForgetRequest {
  /** Forget scope. space and user purge the ai_event inputs of the whole footprint; memory and query delete only the named or matched records and report purgedEvents 0 by construction. */
  scope: 'memory' | 'space' | 'user' | 'query';
  memoryIds?: string[] | null;
  spaceId?: string | null;
  query?: string | null;
  reason: string;
  metadata?: Record<string, unknown> | null;
}
