export interface MemoryExtractionRequest {
    spaceId: string;
    inputEvents: string[];
    extractionMode?: 'deterministic' | 'llm_assisted' | 'hybrid';
    reviewRequired?: boolean;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-extraction-request.d.ts.map