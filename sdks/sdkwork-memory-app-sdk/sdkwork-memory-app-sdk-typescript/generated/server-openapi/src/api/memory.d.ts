import type { ApiRequestOptions, HttpClient } from '../http/client';
import type { MemoryCandidate, MemoryContextPack, MemoryContextPackRequest, MemoryEntity, MemoryEntityPatch, MemoryEntityRequest, MemoryEvent, MemoryEventRequest, MemoryExportJob, MemoryExportRequest, MemoryExtractionRequest, MemoryFeedback, MemoryFeedbackRequest, MemoryForgetJob, MemoryForgetRequest, MemoryHabit, MemoryHabitRequest, MemoryLearningJob, MemoryLearningSettings, MemoryLearningSettingsRequest, MemoryPolicyAssignment, MemoryPolicyAssignmentPatch, MemoryPolicyAssignmentRequest, MemoryRecord, MemoryRecordRequest, MemoryRecordSource, MemoryRetrievalRequest, MemoryRetrievalResult, MemoryReviewRequest, MemorySpace, MemorySpaceRequest, PageInfo } from '../types';
export interface MemoryPolicyAssignmentsListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
}
export interface MemoryPolicyAssignmentsCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryPolicyAssignmentsApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemoryPolicyAssignmentsListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryPolicyAssignment[];
        pageInfo: PageInfo;
    }>;
    create(body: MemoryPolicyAssignmentRequest, params?: MemoryPolicyAssignmentsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryPolicyAssignment>;
    update(policyAssignmentId: string, body: MemoryPolicyAssignmentPatch, requestOptions?: ApiRequestOptions): Promise<MemoryPolicyAssignment>;
}
export interface MemoryEntitiesListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
    spaceId?: string;
    entityType?: string;
}
export interface MemoryEntitiesCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryEntitiesApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemoryEntitiesListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryEntity[];
        pageInfo: PageInfo;
    }>;
    create(body: MemoryEntityRequest, params?: MemoryEntitiesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEntity>;
    retrieve(entityId: string, requestOptions?: ApiRequestOptions): Promise<MemoryEntity>;
    update(entityId: string, body: MemoryEntityPatch, requestOptions?: ApiRequestOptions): Promise<MemoryEntity>;
}
export declare class MemoryLearningSettingsApi {
    private client;
    constructor(client: HttpClient);
    retrieve(requestOptions?: ApiRequestOptions): Promise<MemoryLearningSettings>;
    update(body: MemoryLearningSettingsRequest, requestOptions?: ApiRequestOptions): Promise<MemoryLearningSettings>;
}
export interface MemoryExportJobsListParams {
    cursor?: string;
    pageSize?: number;
}
export interface MemoryExportJobsCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryExportJobsApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemoryExportJobsListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryExportJob[];
        pageInfo: PageInfo;
    }>;
    create(body: MemoryExportRequest, params?: MemoryExportJobsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryExportJob>;
    retrieve(exportJobId: string, requestOptions?: ApiRequestOptions): Promise<MemoryExportJob>;
}
export interface MemoryFeedbackCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryFeedbackApi {
    private client;
    constructor(client: HttpClient);
    create(body: MemoryFeedbackRequest, params?: MemoryFeedbackCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryFeedback>;
}
export interface MemoryContextPacksCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryContextPacksApi {
    private client;
    constructor(client: HttpClient);
    create(body: MemoryContextPackRequest, params?: MemoryContextPacksCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryContextPack>;
    retrieve(contextPackId: string, requestOptions?: ApiRequestOptions): Promise<MemoryContextPack>;
}
export interface MemoryRetrievalsCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryRetrievalsApi {
    private client;
    constructor(client: HttpClient);
    create(body: MemoryRetrievalRequest, params?: MemoryRetrievalsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalResult>;
    retrieve(retrievalId: string, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalResult>;
}
export interface MemoryHabitsListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
    stage?: string;
}
export interface MemoryHabitsConfirmParams {
    idempotencyKey?: string;
}
export interface MemoryHabitsRejectParams {
    idempotencyKey?: string;
}
export declare class MemoryHabitsApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemoryHabitsListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryHabit[];
        pageInfo: PageInfo;
    }>;
    retrieve(habitId: string, requestOptions?: ApiRequestOptions): Promise<MemoryHabit>;
    update(habitId: string, body: MemoryHabitRequest, requestOptions?: ApiRequestOptions): Promise<MemoryHabit>;
    confirm(habitId: string, body: MemoryReviewRequest, params?: MemoryHabitsConfirmParams, requestOptions?: ApiRequestOptions): Promise<MemoryHabit>;
    reject(habitId: string, body: MemoryReviewRequest, params?: MemoryHabitsRejectParams, requestOptions?: ApiRequestOptions): Promise<MemoryHabit>;
}
export interface MemoryCandidatesListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
    decisionState?: string;
}
export interface MemoryCandidatesApproveParams {
    idempotencyKey?: string;
}
export interface MemoryCandidatesRejectParams {
    idempotencyKey?: string;
}
export declare class MemoryCandidatesApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemoryCandidatesListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryCandidate[];
        pageInfo: PageInfo;
    }>;
    retrieve(candidateId: string, requestOptions?: ApiRequestOptions): Promise<MemoryCandidate>;
    approve(candidateId: string, body: MemoryReviewRequest, params?: MemoryCandidatesApproveParams, requestOptions?: ApiRequestOptions): Promise<MemoryCandidate>;
    reject(candidateId: string, body: MemoryReviewRequest, params?: MemoryCandidatesRejectParams, requestOptions?: ApiRequestOptions): Promise<MemoryCandidate>;
}
export interface MemoryExtractionsCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryExtractionsApi {
    private client;
    constructor(client: HttpClient);
    create(body: MemoryExtractionRequest, params?: MemoryExtractionsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob>;
}
export interface MemoryForgetRequestsListParams {
    cursor?: string;
    pageSize?: number;
}
export interface MemoryForgetRequestsCreateParams {
    idempotencyKey?: string;
}
export declare class MemoryForgetRequestsApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemoryForgetRequestsListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryForgetJob[];
        pageInfo: PageInfo;
    }>;
    create(body: MemoryForgetRequest, params?: MemoryForgetRequestsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryForgetJob>;
    retrieve(forgetRequestId: string, requestOptions?: ApiRequestOptions): Promise<MemoryForgetJob>;
}
export interface MemorySourcesListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
}
export declare class MemorySourcesApi {
    private client;
    constructor(client: HttpClient);
    list(memoryId: string, params?: MemorySourcesListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryRecordSource[];
        pageInfo: PageInfo;
    }>;
}
export interface MemoryEventsCreateParams {
    idempotencyKey?: string;
}
export interface MemoryEventsRetrieveParams {
    spaceId: string;
}
export declare class MemoryEventsApi {
    private client;
    constructor(client: HttpClient);
    create(body: MemoryEventRequest, params?: MemoryEventsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEvent>;
    retrieve(eventId: string, params: MemoryEventsRetrieveParams, requestOptions?: ApiRequestOptions): Promise<MemoryEvent>;
}
export interface MemorySpacesListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
}
export interface MemorySpacesCreateParams {
    idempotencyKey?: string;
}
export declare class MemorySpacesApi {
    private client;
    constructor(client: HttpClient);
    list(params?: MemorySpacesListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemorySpace[];
        pageInfo: PageInfo;
    }>;
    create(body: MemorySpaceRequest, params?: MemorySpacesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemorySpace>;
    retrieve(spaceId: string, requestOptions?: ApiRequestOptions): Promise<MemorySpace>;
    update(spaceId: string, body: MemorySpaceRequest, requestOptions?: ApiRequestOptions): Promise<MemorySpace>;
}
export interface MemoryListParams {
    q?: string;
    cursor?: string;
    pageSize?: number;
    spaceId: string;
    memoryType?: string;
}
export interface MemoryCreateParams {
    idempotencyKey?: string;
}
export interface MemoryRetrieveParams {
    spaceId: string;
}
export interface MemoryUpdateParams {
    spaceId: string;
}
export interface MemoryDeleteParams {
    spaceId: string;
}
export declare class MemoryApi {
    private client;
    readonly spaces: MemorySpacesApi;
    readonly events: MemoryEventsApi;
    readonly sources: MemorySourcesApi;
    readonly forgetRequests: MemoryForgetRequestsApi;
    readonly extractions: MemoryExtractionsApi;
    readonly candidates: MemoryCandidatesApi;
    readonly habits: MemoryHabitsApi;
    readonly retrievals: MemoryRetrievalsApi;
    readonly contextPacks: MemoryContextPacksApi;
    readonly feedback: MemoryFeedbackApi;
    readonly exportJobs: MemoryExportJobsApi;
    readonly learningSettings: MemoryLearningSettingsApi;
    readonly entities: MemoryEntitiesApi;
    readonly policyAssignments: MemoryPolicyAssignmentsApi;
    constructor(client: HttpClient);
    list(params: MemoryListParams, requestOptions?: ApiRequestOptions): Promise<{
        items: MemoryRecord[];
        pageInfo: PageInfo;
    }>;
    create(body: MemoryRecordRequest, params?: MemoryCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord>;
    retrieve(memoryId: string, params: MemoryRetrieveParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord>;
    update(memoryId: string, body: MemoryRecordRequest, params: MemoryUpdateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord>;
    delete(memoryId: string, params: MemoryDeleteParams, requestOptions?: ApiRequestOptions): Promise<void>;
}
export declare function createMemoryApi(client: HttpClient): MemoryApi;
//# sourceMappingURL=memory.d.ts.map