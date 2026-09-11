import type { MemoryProviderBinding } from './memory-provider-binding';
export interface MemoryProviderHealth {
    status: 'healthy' | 'degraded' | 'unhealthy' | 'unknown';
    checkedAt: string;
    providers: MemoryProviderBinding[];
}
//# sourceMappingURL=memory-provider-health.d.ts.map