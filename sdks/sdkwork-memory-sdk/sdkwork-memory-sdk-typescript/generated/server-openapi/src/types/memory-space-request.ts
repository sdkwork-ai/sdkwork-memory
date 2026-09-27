export interface MemorySpaceRequest {
  organizationId?: string | null;
  ownerSubjectType: string;
  ownerSubjectId: string;
  spaceType: string;
  displayName: string;
  defaultScope?: string;
  metadata?: Record<string, unknown> | null;
}
