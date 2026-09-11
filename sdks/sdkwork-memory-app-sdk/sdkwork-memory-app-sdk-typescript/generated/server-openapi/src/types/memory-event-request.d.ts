export interface MemoryEventRequest {
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
    payload: Record<string, unknown>;
    sensitivityLevel?: string;
}
//# sourceMappingURL=memory-event-request.d.ts.map