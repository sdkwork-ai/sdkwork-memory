export interface MemoryCapabilityBindingRequest {
  capabilityCode: string;
  targetType: 'subject' | 'space';
  targetId: string;
  mode: 'allow' | 'deny' | 'conditional';
  priority?: number;
  validFrom?: string | null;
  validTo?: string | null;
  metadata?: Record<string, unknown> | null;
}
