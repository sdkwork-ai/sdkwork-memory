import type { MemoryUsageEntry } from './memory-usage-entry';
import type { PageInfo } from './page-info';

export interface MemoryUsageList {
  items: MemoryUsageEntry[];
  pageInfo: PageInfo;
}
