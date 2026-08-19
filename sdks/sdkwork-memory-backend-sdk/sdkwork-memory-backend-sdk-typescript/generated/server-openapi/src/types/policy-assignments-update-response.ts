import type { MemoryPolicyAssignment } from './memory-policy-assignment';

export interface PolicyAssignmentsUpdateResponse {
  code: 0;
  data: unknown & { item: MemoryPolicyAssignment; };
  /** Server-owned request correlation id. */
  traceId: string;
}
