import { customApiPath } from './paths';
import type { ApiRequestOptions, HttpClient } from '../http/client';

import type { MemoryCandidate, MemoryCapabilities, MemoryContextPack, MemoryContextPackRequest, MemoryEdge, MemoryEdgePatch, MemoryEdgeRequest, MemoryEntity, MemoryEntityPatch, MemoryEntityRequest, MemoryEvent, MemoryEventRequest, MemoryExtractionRequest, MemoryFeedback, MemoryFeedbackRequest, MemoryLearningJob, MemoryProviderHealth, MemoryRecord, MemoryRecordRequest, MemoryRetrievalRequest, MemoryRetrievalResult, PageInfo } from '../types';


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
    return this.client.request<{ items: MemoryEdge[]; pageInfo: PageInfo; }>(appendQueryString(customApiPath(`/memory/edges`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryEdgeRequest, params?: MemoryEdgesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEdge> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryEdge>(customApiPath(`/memory/edges`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(edgeId: string, requestOptions?: ApiRequestOptions): Promise<MemoryEdge> {
    return this.client.request<MemoryEdge>(customApiPath(`/memory/edges/${serializePathParameter(edgeId, { name: 'edgeId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(edgeId: string, body: MemoryEdgePatch, requestOptions?: ApiRequestOptions): Promise<MemoryEdge> {
    return this.client.request<MemoryEdge>(customApiPath(`/memory/edges/${serializePathParameter(edgeId, { name: 'edgeId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async delete(edgeId: string, requestOptions?: ApiRequestOptions): Promise<void> {
    return this.client.request<void>(customApiPath(`/memory/edges/${serializePathParameter(edgeId, { name: 'edgeId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
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
    return this.client.request<{ items: MemoryEntity[]; pageInfo: PageInfo; }>(appendQueryString(customApiPath(`/memory/entities`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryEntityRequest, params?: MemoryEntitiesCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEntity> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryEntity>(customApiPath(`/memory/entities`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(entityId: string, requestOptions?: ApiRequestOptions): Promise<MemoryEntity> {
    return this.client.request<MemoryEntity>(customApiPath(`/memory/entities/${serializePathParameter(entityId, { name: 'entityId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(entityId: string, body: MemoryEntityPatch, requestOptions?: ApiRequestOptions): Promise<MemoryEntity> {
    return this.client.request<MemoryEntity>(customApiPath(`/memory/entities/${serializePathParameter(entityId, { name: 'entityId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export class MemoryProviderHealthApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async retrieve(requestOptions?: ApiRequestOptions): Promise<MemoryProviderHealth> {
    return this.client.request<MemoryProviderHealth>(customApiPath(`/memory/provider_health`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryCandidatesListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
  decisionState?: string;
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
      { name: 'decision_state', value: params?.decisionState, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryCandidate[]; pageInfo: PageInfo; }>(appendQueryString(customApiPath(`/memory/candidates`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async retrieve(candidateId: string, requestOptions?: ApiRequestOptions): Promise<MemoryCandidate> {
    return this.client.request<MemoryCandidate>(customApiPath(`/memory/candidates/${serializePathParameter(candidateId, { name: 'candidateId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryExtractionsCreateParams {
  idempotencyKey?: string;
}

export class MemoryExtractionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async create(body: MemoryExtractionRequest, params?: MemoryExtractionsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryLearningJob> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryLearningJob>(customApiPath(`/memory/extractions`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryFeedbackCreateParams {
  idempotencyKey?: string;
}

export class MemoryFeedbackApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async create(body: MemoryFeedbackRequest, params?: MemoryFeedbackCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryFeedback> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryFeedback>(customApiPath(`/memory/feedback`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryContextPacksCreateParams {
  idempotencyKey?: string;
}

export class MemoryContextPacksApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async create(body: MemoryContextPackRequest, params?: MemoryContextPacksCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryContextPack> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryContextPack>(customApiPath(`/memory/context_packs`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(contextPackId: string, requestOptions?: ApiRequestOptions): Promise<MemoryContextPack> {
    return this.client.request<MemoryContextPack>(customApiPath(`/memory/context_packs/${serializePathParameter(contextPackId, { name: 'contextPackId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryRetrievalsCreateParams {
  idempotencyKey?: string;
}

export class MemoryRetrievalsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async create(body: MemoryRetrievalRequest, params?: MemoryRetrievalsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalResult> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryRetrievalResult>(customApiPath(`/memory/retrievals`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(retrievalId: string, requestOptions?: ApiRequestOptions): Promise<MemoryRetrievalResult> {
    return this.client.request<MemoryRetrievalResult>(customApiPath(`/memory/retrievals/${serializePathParameter(retrievalId, { name: 'retrievalId', style: 'simple', explode: false })}`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryEventsCreateParams {
  idempotencyKey?: string;
}

export interface MemoryEventsRetrieveParams {
  spaceId: string;
}

export class MemoryEventsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async create(body: MemoryEventRequest, params?: MemoryEventsCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryEvent> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryEvent>(customApiPath(`/memory/events`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(eventId: string, params: MemoryEventsRetrieveParams, requestOptions?: ApiRequestOptions): Promise<MemoryEvent> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<MemoryEvent>(appendQueryString(customApiPath(`/memory/events/${serializePathParameter(eventId, { name: 'eventId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export class MemoryCapabilitiesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


async retrieve(requestOptions?: ApiRequestOptions): Promise<MemoryCapabilities> {
    return this.client.request<MemoryCapabilities>(customApiPath(`/memory/capabilities`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }
}

export interface MemoryListParams {
  q?: string;
  cursor?: string;
  pageSize?: number;
  spaceId: string;
  memoryType?: string;
  externalSubjectRef?: string;
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

export class MemoryApi {
  private client: HttpClient;
  public readonly capabilities: MemoryCapabilitiesApi;
  public readonly events: MemoryEventsApi;
  public readonly retrievals: MemoryRetrievalsApi;
  public readonly contextPacks: MemoryContextPacksApi;
  public readonly feedback: MemoryFeedbackApi;
  public readonly extractions: MemoryExtractionsApi;
  public readonly candidates: MemoryCandidatesApi;
  public readonly providerHealth: MemoryProviderHealthApi;
  public readonly entities: MemoryEntitiesApi;
  public readonly edges: MemoryEdgesApi;

  constructor(client: HttpClient) {
    this.client = client;
    this.capabilities = new MemoryCapabilitiesApi(client);
    this.events = new MemoryEventsApi(client);
    this.retrievals = new MemoryRetrievalsApi(client);
    this.contextPacks = new MemoryContextPacksApi(client);
    this.feedback = new MemoryFeedbackApi(client);
    this.extractions = new MemoryExtractionsApi(client);
    this.candidates = new MemoryCandidatesApi(client);
    this.providerHealth = new MemoryProviderHealthApi(client);
    this.entities = new MemoryEntitiesApi(client);
    this.edges = new MemoryEdgesApi(client);
  }


async list(params: MemoryListParams, requestOptions?: ApiRequestOptions): Promise<{ items: MemoryRecord[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params.q, style: 'form', explode: true, allowReserved: false },
      { name: 'cursor', value: params.cursor, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
      { name: 'memory_type', value: params.memoryType, style: 'form', explode: true, allowReserved: false },
      { name: 'external_subject_ref', value: params.externalSubjectRef, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: MemoryRecord[]; pageInfo: PageInfo; }>(appendQueryString(customApiPath(`/memory/memories`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }

async create(body: MemoryRecordRequest, params?: MemoryCreateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord> {
    const requestHeaders = buildRequestHeaders(
      {
        'Idempotency-Key': { value: params?.idempotencyKey, style: 'simple', explode: false },
      },
      {}
    );
    return this.client.request<MemoryRecord>(customApiPath(`/memory/memories`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', ...(requestHeaders !== undefined ? { headers: requestHeaders } : {}), sdkworkUnwrapKind: 'item' });
  }

async retrieve(memoryId: string, params: MemoryRetrieveParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<MemoryRecord>(appendQueryString(customApiPath(`/memory/memories/${serializePathParameter(memoryId, { name: 'memoryId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'item' });
  }

async update(memoryId: string, body: MemoryRecordRequest, params: MemoryUpdateParams, requestOptions?: ApiRequestOptions): Promise<MemoryRecord> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<MemoryRecord>(appendQueryString(customApiPath(`/memory/memories/${serializePathParameter(memoryId, { name: 'memoryId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'PATCH' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }

async delete(memoryId: string, params: MemoryDeleteParams, requestOptions?: ApiRequestOptions): Promise<void> {
    const query = buildQueryString([
      { name: 'space_id', value: params.spaceId, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<void>(appendQueryString(customApiPath(`/memory/memories/${serializePathParameter(memoryId, { name: 'memoryId', style: 'simple', explode: false })}`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'DELETE' as any });
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
