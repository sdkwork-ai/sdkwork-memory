export interface MemoryHabit {
    habitId: string;
    spaceId: string;
    userId: string;
    habitKey: string;
    habitType: string;
    description: string;
    stage: 'observing' | 'emerging' | 'confirmed' | 'decaying' | 'inactive' | 'rejected';
    strength: number;
    confidence: number;
    supportCount: number;
    lastSignalAt?: string | null;
    promotedMemoryId?: string | null;
    decayAfter?: string | null;
    metadata?: Record<string, unknown> | null;
    createdAt: string;
    updatedAt: string;
    version: string;
}
//# sourceMappingURL=memory-habit.d.ts.map