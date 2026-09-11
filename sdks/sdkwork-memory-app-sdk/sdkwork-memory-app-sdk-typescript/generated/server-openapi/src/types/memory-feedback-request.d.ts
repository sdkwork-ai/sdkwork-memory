export interface MemoryFeedbackRequest {
    targetType: 'retrieval' | 'hit' | 'memory' | 'candidate' | 'habit';
    targetId: string;
    feedbackType: string;
    rating?: number | null;
    comment?: string | null;
    metadata?: Record<string, unknown> | null;
}
//# sourceMappingURL=memory-feedback-request.d.ts.map