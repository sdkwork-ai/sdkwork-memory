export interface MemoryEvent {
    eventId: string;
    uuid?: string;
    spaceId: string;
    userId?: string | null;
    actorType?: string;
    actorId?: string | null;
    sessionId?: string | null;
    traceId?: string | null;
    eventType: string;
    sourceType: string;
    sourceRef?: string | null;
    eventTime: string;
    payload?: Record<string, unknown>;
    payloadHash: string;
    sensitivityLevel?: string;
    ingestionStatus: string;
    createdAt: string;
}
//# sourceMappingURL=memory-event.d.ts.map