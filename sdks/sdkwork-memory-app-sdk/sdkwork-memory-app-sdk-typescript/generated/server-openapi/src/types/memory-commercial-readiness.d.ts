export interface MemoryCommercialReadiness {
    readinessId: string;
    tenantId: string;
    implementationProfileId?: string | null;
    score: number;
    state: string;
    contractCoverage?: Record<string, unknown> | null;
    managementCoverage?: Record<string, unknown> | null;
    runtimeConformance?: Record<string, unknown> | null;
    privacyCoverage?: Record<string, unknown> | null;
    auditCoverage?: Record<string, unknown> | null;
    sdkCoverage?: Record<string, unknown> | null;
    evaluationCoverage?: Record<string, unknown> | null;
    observabilityCoverage?: Record<string, unknown> | null;
    migrationCoverage?: Record<string, unknown> | null;
    blockingFindings?: string[] | null;
    warningFindings?: string[] | null;
    createdAt: string;
}
//# sourceMappingURL=memory-commercial-readiness.d.ts.map