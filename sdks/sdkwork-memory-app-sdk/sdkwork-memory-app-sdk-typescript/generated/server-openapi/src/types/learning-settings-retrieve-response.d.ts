import type { MemoryLearningSettings } from './memory-learning-settings';
export interface LearningSettingsRetrieveResponse {
    code: 0;
    data: unknown & {
        item: MemoryLearningSettings;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=learning-settings-retrieve-response.d.ts.map