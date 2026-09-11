export interface MemoryHabitRequest {
    description?: string | null;
    stage?: 'observing' | 'emerging' | 'confirmed' | 'decaying' | 'inactive' | 'rejected';
    metadata?: Record<string, unknown> | null;
    version?: string | null;
}
//# sourceMappingURL=memory-habit-request.d.ts.map