export interface MemoryEvent {
  eventId: string;
  uuid?: string;
  spaceId: string;
  userId?: string | null;
  actorType?: string;
  actorId?: string | null;
  eventType: string;
  sourceType: string;
  eventTime: string;
  payload?: Record<string, unknown>;
  payloadHash: string;
  sensitivityLevel?: string;
  ingestionStatus: string;
  createdAt: string;
}
