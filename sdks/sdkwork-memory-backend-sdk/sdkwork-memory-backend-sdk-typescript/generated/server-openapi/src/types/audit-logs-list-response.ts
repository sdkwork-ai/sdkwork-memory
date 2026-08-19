import type { MemoryAuditLog } from './memory-audit-log';
import type { PageInfo } from './page-info';

export interface AuditLogsListResponse {
  code: 0;
  data: unknown & { items: MemoryAuditLog[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
