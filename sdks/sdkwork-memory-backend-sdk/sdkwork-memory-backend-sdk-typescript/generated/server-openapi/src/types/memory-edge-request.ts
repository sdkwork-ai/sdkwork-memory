export interface MemoryEdgeRequest {
  spaceId: string;
  sourceEntityId: string;
  targetEntityId: string;
  relationType: string;
  sourceMemoryId?: string | null;
  weight?: number | null;
  validFrom?: string | null;
  validTo?: string | null;
  metadata?: Record<string, unknown> | null;
}
