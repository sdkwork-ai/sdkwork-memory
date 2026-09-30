export interface MemoryRetrievalRequest {
  /** Natural-language query text. Requests past 8192 characters are rejected as invalid parameters. */
  query: string;
  spaceIds: string[];
  actorId?: string | null;
  retrievalProfileId?: string | null;
  memoryTypes?: ('working' | 'session' | 'semantic' | 'episodic' | 'procedural' | 'habit' | 'relationship' | 'domain_knowledge')[] | null;
  filters?: Record<string, unknown> | null;
  topK: number;
  contextBudgetTokens: number;
  showExpired?: boolean;
  threshold?: number | null;
  explain?: boolean;
  includeTrace?: boolean;
}
