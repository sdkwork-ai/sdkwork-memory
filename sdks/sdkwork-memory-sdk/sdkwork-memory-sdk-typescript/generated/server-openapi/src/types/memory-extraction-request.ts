export interface MemoryExtractionRequest {
  spaceId: string;
  inputEvents: string[];
  extractionMode?: 'deterministic' | 'llm_assisted' | 'hybrid';
  customInstructions?: string | null;
  observationDate?: string | null;
  reviewRequired?: boolean;
  metadata?: Record<string, unknown> | null;
}
