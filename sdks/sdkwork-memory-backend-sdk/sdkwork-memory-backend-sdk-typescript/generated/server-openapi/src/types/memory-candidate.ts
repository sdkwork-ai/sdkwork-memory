export interface MemoryCandidate {
  candidateId: string;
  spaceId: string;
  candidateType: string;
  memoryType: 'working' | 'session' | 'semantic' | 'episodic' | 'procedural' | 'habit' | 'relationship' | 'domain_knowledge';
  proposedText: string;
  confidence: number;
  decisionState: 'pending' | 'auto_approved' | 'approved' | 'rejected' | 'expired' | 'superseded';
  createdAt: string;
  updatedAt: string;
}
