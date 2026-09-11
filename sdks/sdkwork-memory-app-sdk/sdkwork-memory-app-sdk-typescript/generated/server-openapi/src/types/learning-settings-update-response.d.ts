import type { MemoryLearningSettings } from './memory-learning-settings';
export interface LearningSettingsUpdateResponse {
    code: 0;
    data: unknown & {
        item: MemoryLearningSettings;
    };
    /** Server-owned request correlation id. */
    traceId: string;
}
//# sourceMappingURL=learning-settings-update-response.d.ts.map