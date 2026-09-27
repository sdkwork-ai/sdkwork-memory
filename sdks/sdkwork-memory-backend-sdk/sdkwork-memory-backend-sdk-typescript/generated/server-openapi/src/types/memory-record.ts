export interface MemoryRecord {
  memoryId: string;
  uuid?: string;
  spaceId: string;
  userId?: string | null;
  scope: string;
  memoryType: 'working' | 'session' | 'semantic' | 'episodic' | 'procedural' | 'habit' | 'relationship' | 'domain_knowledge';
  subject?: string | null;
  predicate?: string | null;
  objectText?: string;
  canonicalText: string;
  summaryText?: string | null;
  confidence: number;
  evidenceCount?: number;
  contradictionCount?: number;
  expiresAt?: string | null;
  status: 'candidate' | 'active' | 'inactive' | 'superseded' | 'deleted' | 'rejected';
  sensitivityLevel?: string;
  metadata?: Record<string, unknown> | null;
  supersedesMemoryId?: string | null;
  supersededByMemoryId?: string | null;
  createdAt: string;
  updatedAt: string;
  version: string;
}
