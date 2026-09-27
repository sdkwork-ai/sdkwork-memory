export interface MemoryRecordPatch {
  canonicalText?: string | null;
  subject?: string | null;
  summaryText?: string | null;
  metadata?: Record<string, unknown> | null;
}
