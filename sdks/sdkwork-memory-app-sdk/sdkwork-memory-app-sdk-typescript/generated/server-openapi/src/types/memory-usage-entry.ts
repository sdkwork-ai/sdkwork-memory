export interface MemoryUsageEntry {
  /** UTC calendar day (YYYY-MM-DD) the charge belongs to. */
  day: string;
  metric: string;
  /** Cumulative signed charge for the day (record.delete accumulates negative deltas). */
  delta: number;
  updatedAt: string;
}
