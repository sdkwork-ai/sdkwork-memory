import { backendApiPath } from './paths';
import type { ApiRequestOptions, HttpClient } from '../http/client';

import type { MemoryAuditLog, MemoryBinding, MemoryBindingRequest, MemoryCandidate, MemoryCapabilityBinding, MemoryCapabilityBindingRequest, MemoryCommercialReadiness, MemoryCommercialReadinessRequest, MemoryEdge, MemoryEdgePatch, MemoryEdgeRequest, MemoryEntity, MemoryEntityPatch, MemoryEntityRequest, MemoryEvalRun, MemoryEvalRunRequest, MemoryEvent, MemoryExtractionRequest, MemoryImplementationProfile, MemoryImplementationProfileRequest, MemoryIndex, MemoryIndexRequest, MemoryLearningJob, MemoryMigrationJobRequest, MemoryPolicy, MemoryPolicyAssignment, MemoryPolicyAssignmentPatch, MemoryPolicyAssignmentRequest, MemoryPolicyPatch, MemoryPolicyRequest, MemoryProviderBinding, MemoryProviderBindingRequest, MemoryProviderHealth, MemoryRecord, MemoryRecordRequest, MemoryResolveCapabilitiesRequest, MemoryResolvedCapabilityList, MemoryRetentionJobRequest, MemoryRetrievalProfile, MemoryRetrievalProfileRequest, MemoryRetrievalTrace, MemoryReviewRequest, MemorySpace, MemorySpaceRequest, MemorySubject, MemorySubjectPatch, MemorySubjectRequest, PageInfo } from '../types';


export interface MemoryCommercialReadinessRebuildParams {
  idempotencyKey?: string;
}

export class MemoryCommercialReadinessApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async retrieve(requestOptions?: ApiRequestOptions): Promise<MemoryCommercialReadiness> {
    return this.client.request<MemoryCommercialReadiness>(backendApiPath(`/memory/commercial_readiness`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async rebuild(body: MemoryCommercialReadinessRequest, params?: MemoryCommercialReadinessRebuildParams, requestOptions?: ApiRequestOptions): Promise<MemoryCommercialReadiness> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryCommercialReadiness>(backendApiPath(`/memory/commercial_readiness/rebuild`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryPolicyAssignmentsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryPolicyAssignmentsCreateParams {
  idempotencyKey?: string;
}

export class MemoryPolicyAssignmentsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryPolicyAssignmentsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryPolicyAssignment[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryPolicyAssignment[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/policy_assignments`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryPolicyAssignmentRequest, params?: MemoryPolicyAssignmentsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryPolicyAssignment> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryPolicyAssignment>(backendApiPath(`/memory/policy_assignments`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(policyAssignmentId: string, requestOptions?: ApiRequestOptions): Promise<MemoryPolicyAssignment> {
    return this.client.request<MemoryPolicyAssignment>(backendApiPath(`/memory/policy_assignments/${serializePathParameter(policyAssignmentId, { name: 'policyAssignmentId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(policyAssignmentId: string, body: MemoryPolicyAssignmentPatch, requestOptions?: ApiRequestOptions): Promise<MemoryPolicyAssignment> {
    return this.client.request<MemoryPolicyAssignment>(backendApiPath(`/memory/policy_assignments/${serializePathParameter(policyAssignmentId, { name: 'policyAssignmentId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async delete(policyAssignmentId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/memory/policy_assignments/${serializePathParameter(policyAssignmentId, { name: 'policyAssignmentId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface MemoryPoliciesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
  policyType?: string;
  scope?: string;
}

export interface MemoryPoliciesCreateParams {
  idempotencyKey?: string;
}

export class MemoryPoliciesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryPoliciesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryPolicy[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'policyType', value: params?.policyType, style: 'form', explode: true, allowReserved: false },
      { name: 'scope', value: params?.scope, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryPolicy[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/policies`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryPolicyRequest, params?: MemoryPoliciesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryPolicy> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryPolicy>(backendApiPath(`/memory/policies`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(policyId: string, requestOptions?: ApiRequestOptions): Promise<MemoryPolicy> {
    return this.client.request<MemoryPolicy>(backendApiPath(`/memory/policies/${serializePathParameter(policyId, { name: 'policyId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(policyId: string, body: MemoryPolicyPatch, requestOptions?: ApiRequestOptions): Promise<MemoryPolicy> {
    return this.client.request<MemoryPolicy>(backendApiPath(`/memory/policies/${serializePathParameter(policyId, { name: 'policyId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async delete(policyId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/memory/policies/${serializePathParameter(policyId, { name: 'policyId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface MemoryEdgesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
  spaceId?: string;
  sourceEntityId?: string;
  relationType?: string;
}

export interface MemoryEdgesCreateParams {
  idempotencyKey?: string;
}

export class MemoryEdgesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryEdgesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryEdge[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'spaceId', value: params?.spaceId, style: 'form', explode: true, allowReserved: false },
      { name: 'sourceEntityId', value: params?.sourceEntityId, style: 'form', explode: true, allowReserved: false },
      { name: 'relationType', value: params?.relationType, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryEdge[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/edges`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryEdgeRequest, params?: MemoryEdgesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEdge> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryEdge>(backendApiPath(`/memory/edges`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(edgeId: string, requestOptions?: ApiRequestOptions): Promise<MemoryEdge> {
    return this.client.request<MemoryEdge>(backendApiPath(`/memory/edges/${serializePathParameter(edgeId, { name: 'edgeId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(edgeId: string, body: MemoryEdgePatch, requestOptions?: ApiRequestOptions): Promise<MemoryEdge> {
    return this.client.request<MemoryEdge>(backendApiPath(`/memory/edges/${serializePathParameter(edgeId, { name: 'edgeId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async delete(edgeId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/memory/edges/${serializePathParameter(edgeId, { name: 'edgeId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface MemoryEntitiesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
  spaceId?: string;
  entityType?: string;
  status?: string;
}

export interface MemoryEntitiesCreateParams {
  idempotencyKey?: string;
}

export class MemoryEntitiesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryEntitiesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryEntity[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'spaceId', value: params?.spaceId, style: 'form', explode: true, allowReserved: false },
      { name: 'entityType', value: params?.entityType, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryEntity[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/entities`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryEntityRequest, params?: MemoryEntitiesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEntity> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryEntity>(backendApiPath(`/memory/entities`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(entityId: string, requestOptions?: ApiRequestOptions): Promise<MemoryEntity> {
    return this.client.request<MemoryEntity>(backendApiPath(`/memory/entities/${serializePathParameter(entityId, { name: 'entityId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(entityId: string, body: MemoryEntityPatch, requestOptions?: ApiRequestOptions): Promise<MemoryEntity> {
    return this.client.request<MemoryEntity>(backendApiPath(`/memory/entities/${serializePathParameter(entityId, { name: 'entityId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryCapabilitiesResolveParams {
  idempotencyKey?: string;
}

export class MemoryCapabilitiesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async resolve(body: MemoryResolveCapabilitiesRequest, params?: MemoryCapabilitiesResolveParams, requestOptions?: ApiRequestOptions): Promise<MemoryResolvedCapabilityList> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryResolvedCapabilityList>(backendApiPath(`/memory/capabilities/resolve`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'page' });
  }
}

export interface MemoryCapabilityBindingsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryCapabilityBindingsCreateParams {
  idempotencyKey?: string;
}

export class MemoryCapabilityBindingsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryCapabilityBindingsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryCapabilityBinding[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryCapabilityBinding[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/capability_bindings`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryCapabilityBindingRequest, params?: MemoryCapabilityBindingsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryCapabilityBinding> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryCapabilityBinding>(backendApiPath(`/memory/capability_bindings`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(capabilityBindingId: string, requestOptions?: ApiRequestOptions): Promise<MemoryCapabilityBinding> {
    return this.client.request<MemoryCapabilityBinding>(backendApiPath(`/memory/capability_bindings/${serializePathParameter(capabilityBindingId, { name: 'capabilityBindingId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async delete(capabilityBindingId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/memory/capability_bindings/${serializePathParameter(capabilityBindingId, { name: 'capabilityBindingId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface MemoryBindingsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryBindingsCreateParams {
  idempotencyKey?: string;
}

export class MemoryBindingsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryBindingsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryBinding[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryBinding[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/bindings`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryBindingRequest, params?: MemoryBindingsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryBinding> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryBinding>(backendApiPath(`/memory/bindings`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(bindingId: string, requestOptions?: ApiRequestOptions): Promise<MemoryBinding> {
    return this.client.request<MemoryBinding>(backendApiPath(`/memory/bindings/${serializePathParameter(bindingId, { name: 'bindingId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async delete(bindingId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/memory/bindings/${serializePathParameter(bindingId, { name: 'bindingId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface MemorySubjectsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
  subjectType?: string;
  status?: string;
}

export interface MemorySubjectsCreateParams {
  idempotencyKey?: string;
}

export class MemorySubjectsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemorySubjectsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemorySubject[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'subjectType', value: params?.subjectType, style: 'form', explode: true, allowReserved: false },
      { name: 'status', value: params?.status, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemorySubject[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/subjects`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemorySubjectRequest, params?: MemorySubjectsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemorySubject> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemorySubject>(backendApiPath(`/memory/subjects`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(subjectId: string, requestOptions?: ApiRequestOptions): Promise<MemorySubject> {
    return this.client.request<MemorySubject>(backendApiPath(`/memory/subjects/${serializePathParameter(subjectId, { name: 'subjectId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(subjectId: string, body: MemorySubjectPatch, requestOptions?: ApiRequestOptions): Promise<MemorySubject> {
    return this.client.request<MemorySubject>(backendApiPath(`/memory/subjects/${serializePathParameter(subjectId, { name: 'subjectId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async delete(subjectId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(backendApiPath(`/memory/subjects/${serializePathParameter(subjectId, { name: 'subjectId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
  }
}

export interface MemoryMigrationJobsListParams {
  cursor?: string;
  pageSize?: number;
}

export interface MemoryMigrationJobsCreateParams {
  idempotencyKey?: string;
}

export class MemoryMigrationJobsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryMigrationJobsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/migration_jobs`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryMigrationJobRequest, params?: MemoryMigrationJobsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/migration_jobs`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(migrationJobId: string, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/migration_jobs/${serializePathParameter(migrationJobId, { name: 'migrationJobId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryRetentionJobsListParams {
  cursor?: string;
  pageSize?: number;
}

export interface MemoryRetentionJobsCreateParams {
  idempotencyKey?: string;
}

export class MemoryRetentionJobsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryRetentionJobsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/retention_jobs`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryRetentionJobRequest, params?: MemoryRetentionJobsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/retention_jobs`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(retentionJobId: string, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/retention_jobs/${serializePathParameter(retentionJobId, { name: 'retentionJobId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryAuditLogsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export class MemoryAuditLogsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryAuditLogsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryAuditLog[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryAuditLog[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/audit_logs`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface MemoryRetrievalTracesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export class MemoryRetrievalTracesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryRetrievalTracesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryRetrievalTrace[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryRetrievalTrace[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/retrieval_traces`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async retrieve(traceId: string, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalTrace> {
    return this.client.request<MemoryRetrievalTrace>(backendApiPath(`/memory/retrieval_traces/${serializePathParameter(traceId, { name: 'traceId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryEvalRunsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryEvalRunsCreateParams {
  idempotencyKey?: string;
}

export class MemoryEvalRunsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryEvalRunsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryEvalRun[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryEvalRun[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/eval_runs`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryEvalRunRequest, params?: MemoryEvalRunsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEvalRun> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryEvalRun>(backendApiPath(`/memory/eval_runs`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(evalRunId: string, requestOptions?: ApiRequestOptions): Promise<MemoryEvalRun> {
    return this.client.request<MemoryEvalRun>(backendApiPath(`/memory/eval_runs/${serializePathParameter(evalRunId, { name: 'evalRunId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export class MemoryProviderHealthApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async retrieve(requestOptions?: ApiRequestOptions): Promise<MemoryProviderHealth> {
    return this.client.request<MemoryProviderHealth>(backendApiPath(`/memory/provider_health`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryProviderBindingsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryProviderBindingsCreateParams {
  idempotencyKey?: string;
}

export class MemoryProviderBindingsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryProviderBindingsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryProviderBinding[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryProviderBinding[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/provider_bindings`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryProviderBindingRequest, params?: MemoryProviderBindingsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryProviderBinding> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryProviderBinding>(backendApiPath(`/memory/provider_bindings`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async update(providerBindingId: string, body: MemoryProviderBindingRequest, requestOptions?: ApiRequestOptions): Promise<MemoryProviderBinding> {
    return this.client.request<MemoryProviderBinding>(backendApiPath(`/memory/provider_bindings/${serializePathParameter(providerBindingId, { name: 'providerBindingId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryImplementationProfilesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryImplementationProfilesCreateParams {
  idempotencyKey?: string;
}

export class MemoryImplementationProfilesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryImplementationProfilesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryImplementationProfile[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryImplementationProfile[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/implementation_profiles`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryImplementationProfileRequest, params?: MemoryImplementationProfilesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryImplementationProfile> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryImplementationProfile>(backendApiPath(`/memory/implementation_profiles`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(implementationProfileId: string, requestOptions?: ApiRequestOptions): Promise<MemoryImplementationProfile> {
    return this.client.request<MemoryImplementationProfile>(backendApiPath(`/memory/implementation_profiles/${serializePathParameter(implementationProfileId, { name: 'implementationProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(implementationProfileId: string, body: MemoryImplementationProfileRequest, requestOptions?: ApiRequestOptions): Promise<MemoryImplementationProfile> {
    return this.client.request<MemoryImplementationProfile>(backendApiPath(`/memory/implementation_profiles/${serializePathParameter(implementationProfileId, { name: 'implementationProfileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryRetrievalProfilesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryRetrievalProfilesCreateParams {
  idempotencyKey?: string;
}

export class MemoryRetrievalProfilesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryRetrievalProfilesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryRetrievalProfile[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryRetrievalProfile[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/retrieval_profiles`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryRetrievalProfileRequest, params?: MemoryRetrievalProfilesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalProfile> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryRetrievalProfile>(backendApiPath(`/memory/retrieval_profiles`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(profileId: string, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalProfile> {
    return this.client.request<MemoryRetrievalProfile>(backendApiPath(`/memory/retrieval_profiles/${serializePathParameter(profileId, { name: 'profileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(profileId: string, body: MemoryRetrievalProfileRequest, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalProfile> {
    return this.client.request<MemoryRetrievalProfile>(backendApiPath(`/memory/retrieval_profiles/${serializePathParameter(profileId, { name: 'profileId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryIndexesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryIndexesCreateParams {
  idempotencyKey?: string;
}

export interface MemoryIndexesRebuildParams {
  idempotencyKey?: string;
}

export class MemoryIndexesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryIndexesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryIndex[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryIndex[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/indexes`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryIndexRequest, params?: MemoryIndexesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryIndex> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryIndex>(backendApiPath(`/memory/indexes`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(indexId: string, requestOptions?: ApiRequestOptions): Promise<MemoryIndex> {
    return this.client.request<MemoryIndex>(backendApiPath(`/memory/indexes/${serializePathParameter(indexId, { name: 'indexId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(indexId: string, body: MemoryIndexRequest, requestOptions?: ApiRequestOptions): Promise<MemoryIndex> {
    return this.client.request<MemoryIndex>(backendApiPath(`/memory/indexes/${serializePathParameter(indexId, { name: 'indexId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async rebuild(indexId: string, body: MemoryReviewRequest, params?: MemoryIndexesRebuildParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/indexes/${serializePathParameter(indexId, { name: 'indexId', style: 'simple', explode: false })}/rebuild`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryConsolidationJobsListParams {
  cursor?: string;
  pageSize?: number;
}

export interface MemoryConsolidationJobsCreateParams {
  idempotencyKey?: string;
}

export class MemoryConsolidationJobsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryConsolidationJobsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/consolidation_jobs`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryExtractionRequest, params?: MemoryConsolidationJobsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/consolidation_jobs`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(jobId: string, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/consolidation_jobs/${serializePathParameter(jobId, { name: 'jobId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryExtractionJobsListParams {
  cursor?: string;
  pageSize?: number;
  spaceId?: string;
}

export interface MemoryExtractionJobsCreateParams {
  idempotencyKey?: string;
}

export class MemoryExtractionJobsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryExtractionJobsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'space_id', value: params?.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryLearningJob[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/extraction_jobs`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryExtractionRequest, params?: MemoryExtractionJobsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/extraction_jobs`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(jobId: string, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    return this.client.request<MemoryLearningJob>(backendApiPath(`/memory/extraction_jobs/${serializePathParameter(jobId, { name: 'jobId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryCandidatesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryCandidatesApproveParams {
  idempotencyKey?: string;
}

export interface MemoryCandidatesRejectParams {
  idempotencyKey?: string;
}

export class MemoryCandidatesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryCandidatesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryCandidate[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryCandidate[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/candidates`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async approve(candidateId: string, body: MemoryReviewRequest, params?: MemoryCandidatesApproveParams, requestOptions?: ApiRequestOptions): Promise<MemoryCandidate> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryCandidate>(backendApiPath(`/memory/candidates/${serializePathParameter(candidateId, { name: 'candidateId', style: 'simple', explode: false })}/approve`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async reject(candidateId: string, body: MemoryReviewRequest, params?: MemoryCandidatesRejectParams, requestOptions?: ApiRequestOptions): Promise<MemoryCandidate> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryCandidate>(backendApiPath(`/memory/candidates/${serializePathParameter(candidateId, { name: 'candidateId', style: 'simple', explode: false })}/reject`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryEventsListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryEventsRetrieveParams {
  spaceId: string;
}

export class MemoryEventsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemoryEventsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryEvent[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryEvent[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/events`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async retrieve(eventId: string, params: MemoryEventsRetrieveParams, requestOptions?: ApiRequestOptions): Promise<MemoryEvent> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<MemoryEvent>(appendQueryString(backendApiPath(`/memory/events/${serializePathParameter(eventId, { name: 'eventId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemorySpacesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export class MemorySpacesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async list(params?: MemorySpacesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemorySpace[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemorySpace[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/spaces`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async retrieve(spaceId: string, requestOptions?: ApiRequestOptions): Promise<MemorySpace> {
    return this.client.request<MemorySpace>(backendApiPath(`/memory/spaces/${serializePathParameter(spaceId, { name: 'spaceId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(spaceId: string, body: MemorySpaceRequest, requestOptions?: ApiRequestOptions): Promise<MemorySpace> {
    return this.client.request<MemorySpace>(backendApiPath(`/memory/spaces/${serializePathParameter(spaceId, { name: 'spaceId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
}

export interface MemoryRetrieveParams {
  spaceId: string;
}

export interface MemoryUpdateParams {
  spaceId: string;
}

export interface MemorySupersedeParams {
  idempotencyKey?: string;
}

export class MemoryApi {
  private client: HttpClient;
  public readonly spaces: MemorySpacesApi;
  public readonly events: MemoryEventsApi;
  public readonly candidates: MemoryCandidatesApi;
  public readonly extractionJobs: MemoryExtractionJobsApi;
  public readonly consolidationJobs: MemoryConsolidationJobsApi;
  public readonly indexes: MemoryIndexesApi;
  public readonly retrievalProfiles: MemoryRetrievalProfilesApi;
  public readonly implementationProfiles: MemoryImplementationProfilesApi;
  public readonly providerBindings: MemoryProviderBindingsApi;
  public readonly providerHealth: MemoryProviderHealthApi;
  public readonly evalRuns: MemoryEvalRunsApi;
  public readonly retrievalTraces: MemoryRetrievalTracesApi;
  public readonly auditLogs: MemoryAuditLogsApi;
  public readonly retentionJobs: MemoryRetentionJobsApi;
  public readonly migrationJobs: MemoryMigrationJobsApi;
  public readonly subjects: MemorySubjectsApi;
  public readonly bindings: MemoryBindingsApi;
  public readonly capabilityBindings: MemoryCapabilityBindingsApi;
  public readonly capabilities: MemoryCapabilitiesApi;
  public readonly entities: MemoryEntitiesApi;
  public readonly edges: MemoryEdgesApi;
  public readonly policies: MemoryPoliciesApi;
  public readonly policyAssignments: MemoryPolicyAssignmentsApi;
  public readonly commercialReadiness: MemoryCommercialReadinessApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.spaces = new MemorySpacesApi(client);
    this.events = new MemoryEventsApi(client);
    this.candidates = new MemoryCandidatesApi(client);
    this.extractionJobs = new MemoryExtractionJobsApi(client);
    this.consolidationJobs = new MemoryConsolidationJobsApi(client);
    this.indexes = new MemoryIndexesApi(client);
    this.retrievalProfiles = new MemoryRetrievalProfilesApi(client);
    this.implementationProfiles = new MemoryImplementationProfilesApi(client);
    this.providerBindings = new MemoryProviderBindingsApi(client);
    this.providerHealth = new MemoryProviderHealthApi(client);
    this.evalRuns = new MemoryEvalRunsApi(client);
    this.retrievalTraces = new MemoryRetrievalTracesApi(client);
    this.auditLogs = new MemoryAuditLogsApi(client);
    this.retentionJobs = new MemoryRetentionJobsApi(client);
    this.migrationJobs = new MemoryMigrationJobsApi(client);
    this.subjects = new MemorySubjectsApi(client);
    this.bindings = new MemoryBindingsApi(client);
    this.capabilityBindings = new MemoryCapabilityBindingsApi(client);
    this.capabilities = new MemoryCapabilitiesApi(client);
    this.entities = new MemoryEntitiesApi(client);
    this.edges = new MemoryEdgesApi(client);
    this.policies = new MemoryPoliciesApi(client);
    this.policyAssignments = new MemoryPolicyAssignmentsApi(client);
    this.commercialReadiness = new MemoryCommercialReadinessApi(client);
  }


async list(params?: MemoryListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryRecord[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params?.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryRecord[]; pageInfo: PageInfo; }>(appendQueryString(backendApiPath(`/memory/memories`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async retrieve(memoryId: string, params: MemoryRetrieveParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<MemoryRecord>(appendQueryString(backendApiPath(`/memory/memories/${serializePathParameter(memoryId, { name: 'memoryId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(memoryId: string, body: MemoryRecordRequest, params: MemoryUpdateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<MemoryRecord>(appendQueryString(backendApiPath(`/memory/memories/${serializePathParameter(memoryId, { name: 'memoryId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async supersede(memoryId: string, body: MemoryRecordRequest, params?: MemorySupersedeParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryRecord>(backendApiPath(`/memory/memories/${serializePathParameter(memoryId, { name: 'memoryId', style: 'simple', explode: false })}/supersede`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }
}

export function createMemoryApi(client: HttpClient): MemoryApi {
  return new MemoryApi(client);
}

function appendQueryString(path: string, rawQueryString: string): string {
  const query = rawQueryString.replace(/^\?+/, '');
  if (!query) {
    return path;
  }
  return path.includes('?') ? `${path}&${query}` : `${path}?${query}`;
}

interface PathParameterSpec {
  name: string;
  style: string;
  explode: boolean;
}

function serializePathParameter(value: unknown, spec: PathParameterSpec): string {
  if (value === undefined || value === null) {
    return '';
  }

  const style = spec.style || 'simple';
  if (Array.isArray(value)) {
    return serializePathArray(spec.name, value, style, spec.explode);
  }
  if (typeof value === 'object') {
    return serializePathObject(spec.name, value as Record<string, unknown>, style, spec.explode);
  }
  return pathPrefix(spec.name, style, false) + encodePathValue(serializePathPrimitive(value));
}

function serializePathArray(name: string, values: unknown[], style: string, explode: boolean): string {
  const serialized = values
    .filter((item) => item !== undefined && item !== null)
    .map((item) => encodePathValue(serializePathPrimitive(item)));
  if (serialized.length === 0) {
    return pathPrefix(name, style, false);
  }
  if (style === 'matrix') {
    return explode
      ? serialized.map((item) => `;${name}=${item}`).join('')
      : `;${name}=${serialized.join(',')}`;
  }
  return pathPrefix(name, style, false) + serialized.join(explode ? '.' : ',');
}

function serializePathObject(name: string, value: Record<string, unknown>, style: string, explode: boolean): string {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (entries.length === 0) {
    return pathPrefix(name, style, true);
  }
  if (style === 'matrix') {
    return explode
      ? entries.map(([key, entryValue]) => `;${encodePathValue(key)}=${encodePathValue(serializePathPrimitive(entryValue))}`).join('')
      : `;${name}=${entries.flatMap(([key, entryValue]) => [encodePathValue(key), encodePathValue(serializePathPrimitive(entryValue))]).join(',')}`;
  }
  const serialized = explode
    ? entries.map(([key, entryValue]) => `${encodePathValue(key)}=${encodePathValue(serializePathPrimitive(entryValue))}`).join(style === 'label' ? '.' : ',')
    : entries.flatMap(([key, entryValue]) => [encodePathValue(key), encodePathValue(serializePathPrimitive(entryValue))]).join(',');
  return pathPrefix(name, style, true) + serialized;
}

function pathPrefix(name: string, style: string, _objectValue: boolean): string {
  if (style === 'label') return '.';
  if (style === 'matrix') return `;${name}`;
  return '';
}

function encodePathValue(value: string): string {
  return encodeURIComponent(value);
}

function serializePathPrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}
interface QueryParameterSpec {
  name: string;
  value: unknown;
  style: string;
  explode: boolean;
  allowReserved: boolean;
  contentType?: string;
}

function buildQueryString(parameters: QueryParameterSpec[]): string {
  const pairs: string[] = [];
  for (const parameter of parameters) {
    appendSerializedParameter(pairs, parameter);
  }
  return pairs.join('&');
}

function appendSerializedParameter(pairs: string[], parameter: QueryParameterSpec): void {
  if (parameter.value === undefined || parameter.value === null) {
    return;
  }

  if (parameter.contentType) {
    pairs.push(`${encodeQueryComponent(parameter.name)}=${encodeQueryValue(JSON.stringify(parameter.value), parameter.allowReserved)}`);
    return;
  }

  const style = parameter.style || 'form';
  if (style === 'deepObject') {
    appendDeepObjectParameter(pairs, parameter.name, parameter.value, parameter.allowReserved);
    return;
  }

  if (Array.isArray(parameter.value)) {
    appendArrayParameter(pairs, parameter.name, parameter.value, style, parameter.explode, parameter.allowReserved);
    return;
  }

  if (typeof parameter.value === 'object') {
    appendObjectParameter(pairs, parameter.name, parameter.value as Record<string, unknown>, style, parameter.explode, parameter.allowReserved);
    return;
  }

  pairs.push(`${encodeQueryComponent(parameter.name)}=${encodeQueryValue(serializePrimitive(parameter.value), parameter.allowReserved)}`);
}

function appendArrayParameter(
  pairs: string[],
  name: string,
  value: unknown[],
  style: string,
  explode: boolean,
  allowReserved: boolean,
): void {
  const values = value
    .filter((item) => item !== undefined && item !== null)
    .map((item) => serializePrimitive(item));
  if (values.length === 0) {
    return;
  }

  if (style === 'form' && explode) {
    for (const item of values) {
      pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(item, allowReserved)}`);
    }
    return;
  }

  pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(values.join(','), allowReserved)}`);
}

function appendObjectParameter(
  pairs: string[],
  name: string,
  value: Record<string, unknown>,
  style: string,
  explode: boolean,
  allowReserved: boolean,
): void {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (entries.length === 0) {
    return;
  }

  if (style === 'form' && explode) {
    for (const [key, entryValue] of entries) {
      pairs.push(`${encodeQueryComponent(key)}=${encodeQueryValue(serializePrimitive(entryValue), allowReserved)}`);
    }
    return;
  }

  const serialized = entries.flatMap(([key, entryValue]) => [key, serializePrimitive(entryValue)]).join(',');
  pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(serialized, allowReserved)}`);
}

function appendDeepObjectParameter(
  pairs: string[],
  name: string,
  value: unknown,
  allowReserved: boolean,
): void {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    pairs.push(`${encodeQueryComponent(name)}=${encodeQueryValue(serializePrimitive(value), allowReserved)}`);
    return;
  }

  for (const [key, entryValue] of Object.entries(value as Record<string, unknown>)) {
    if (entryValue === undefined || entryValue === null) {
      continue;
    }
    pairs.push(`${encodeQueryComponent(`${name}[${key}]`)}=${encodeQueryValue(serializePrimitive(entryValue), allowReserved)}`);
  }
}

function serializePrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

function encodeQueryComponent(value: string): string {
  return encodeURIComponent(value);
}

function encodeQueryValue(value: string, allowReserved: boolean): string {
  const encoded = encodeURIComponent(value);
  if (!allowReserved) {
    return encoded;
  }
  return encoded.replace(/%3A/gi, ':')
    .replace(/%2F/gi, '/')
    .replace(/%3F/gi, '?')
    .replace(/%23/gi, '#')
    .replace(/%5B/gi, '[')
    .replace(/%5D/gi, ']')
    .replace(/%40/gi, '@')
    .replace(/%21/gi, '!')
    .replace(/%24/gi, '$')
    .replace(/%26/gi, '&')
    .replace(/%27/gi, "'")
    .replace(/%28/gi, '(')
    .replace(/%29/gi, ')')
    .replace(/%2A/gi, '*')
    .replace(/%2B/gi, '+')
    .replace(/%2C/gi, ',')
    .replace(/%3B/gi, ';')
    .replace(/%3D/gi, '=');
}
function buildRequestHeaders(
  headers: Record<string, HeaderParameterSpec | undefined>,
  cookies: Record<string, HeaderParameterSpec | undefined> = {},
): Record<string, string> | undefined {
  const requestHeaders: Record<string, string> = {};

  for (const [name, parameter] of Object.entries(headers)) {
    const serialized = serializeParameterValue(parameter);
    if (serialized !== undefined) {
      requestHeaders[name] = serialized;
    }
  }

  const cookieHeader = buildCookieHeader(cookies);
  if (cookieHeader) {
    requestHeaders.Cookie = requestHeaders.Cookie
      ? `${requestHeaders.Cookie}; ${cookieHeader}`
      : cookieHeader;
  }

  return Object.keys(requestHeaders).length > 0 ? requestHeaders : undefined;
}

interface HeaderParameterSpec {
  value: unknown;
  style: string;
  explode: boolean;
  contentType?: string;
}

function buildCookieHeader(cookies: Record<string, HeaderParameterSpec | undefined>): string | undefined {
  const pairs: string[] = [];
  for (const [name, parameter] of Object.entries(cookies)) {
    const serialized = serializeParameterValue(parameter);
    if (serialized !== undefined) {
      pairs.push(`${encodeURIComponent(name)}=${encodeURIComponent(serialized)}`);
    }
  }
  return pairs.length > 0 ? pairs.join('; ') : undefined;
}

function serializeParameterValue(parameter: HeaderParameterSpec | undefined): string | undefined {
  const value = parameter?.value;
  if (value === undefined || value === null) {
    return undefined;
  }
  if (parameter?.contentType) {
    return JSON.stringify(value);
  }
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (Array.isArray(value)) {
    return value.map((item) => serializeHeaderPrimitive(item)).join(',');
  }
  if (typeof value === 'object' && value !== null) {
    return serializeHeaderObject(value as Record<string, unknown>, parameter?.explode === true);
  }
  return serializeHeaderPrimitive(value);
}

function serializeHeaderObject(value: Record<string, unknown>, explode: boolean): string {
  const entries = Object.entries(value).filter(([, entryValue]) => entryValue !== undefined && entryValue !== null);
  if (explode) {
    return entries.map(([key, entryValue]) => `${key}=${serializeHeaderPrimitive(entryValue)}`).join(',');
  }
  return entries.flatMap(([key, entryValue]) => [key, serializeHeaderPrimitive(entryValue)]).join(',');
}

function serializeHeaderPrimitive(value: unknown): string {
  if (value instanceof Date) {
    return value.toISOString();
  }
  return String(value);
}
