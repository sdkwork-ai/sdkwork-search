import { appApiPath } from './paths';
import type { ApiRequestOptions, HttpClient } from '../http/client';

import type { PageInfo, SearchIndex, SearchPromotionItem, SearchPromotionRequest, SearchQueryRequest, SearchRecentQuery, SearchRecommendationItem, SearchRecommendationRequest, SearchResult, SearchSemanticQueryRequest, SearchSemanticResult, SearchSuggestion, SearchUserEvent, SearchUserEventResponse } from '../types';


export class SearchSemanticQueriesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create a semantic search query with lexical fallback. */
  async create(body: SearchSemanticQueryRequest, requestOptions?: ApiRequestOptions): Promise<{ items: SearchSemanticResult[]; pageInfo: PageInfo; }> {
    return this.client.request<{ items: SearchSemanticResult[]; pageInfo: PageInfo; }>(appApiPath(`/search/semantic_queries`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'page' });
  }
}

export interface SearchRecentQueriesListParams {
  q?: string;
  page?: number;
  pageSize?: number;
}

export class SearchRecentQueriesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List recent search queries for the current app principal. */
  async list(params?: SearchRecentQueriesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: SearchRecentQuery[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: SearchRecentQuery[]; pageInfo: PageInfo; }>(appendQueryString(appApiPath(`/search/recent_queries`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export class SearchEventsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Record a search or recommendation feedback event. */
  async create(body: SearchUserEvent, requestOptions?: ApiRequestOptions): Promise<SearchUserEventResponse> {
    return this.client.request<SearchUserEventResponse>(appApiPath(`/search/events`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'item' });
  }
}

export class SearchPromotionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create a promotion delivery response for an app placement. */
  async create(body: SearchPromotionRequest, requestOptions?: ApiRequestOptions): Promise<{ items: SearchPromotionItem[]; pageInfo: PageInfo; }> {
    return this.client.request<{ items: SearchPromotionItem[]; pageInfo: PageInfo; }>(appApiPath(`/search/promotions`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'page' });
  }
}

export class SearchRecommendationsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create an explainable recommendation response. */
  async create(body: SearchRecommendationRequest, requestOptions?: ApiRequestOptions): Promise<{ items: SearchRecommendationItem[]; pageInfo: PageInfo; }> {
    return this.client.request<{ items: SearchRecommendationItem[]; pageInfo: PageInfo; }>(appApiPath(`/search/recommendations`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'page' });
  }
}

export interface SearchSuggestionsListParams {
  q?: string;
  pageSize?: number;
  providerId?: string;
  providerKind?: 'algolia' | 'custom' | 'elasticsearch' | 'meilisearch' | 'memory' | 'opensearch' | 'postgresql' | 'typesense' | 'vector';
  capabilityIds?: string[];
  groupIds?: string[];
  scopeIds?: string[];
}

export class SearchSuggestionsApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List search suggestions for app clients. */
  async list(params?: SearchSuggestionsListParams, requestOptions?: ApiRequestOptions): Promise<{ items: SearchSuggestion[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
      { name: 'provider_id', value: params?.providerId, style: 'form', explode: true, allowReserved: false },
      { name: 'provider_kind', value: params?.providerKind, style: 'form', explode: true, allowReserved: false },
      { name: 'capability_ids', value: params?.capabilityIds, style: 'form', explode: true, allowReserved: false },
      { name: 'group_ids', value: params?.groupIds, style: 'form', explode: true, allowReserved: false },
      { name: 'scope_ids', value: params?.scopeIds, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: SearchSuggestion[]; pageInfo: PageInfo; }>(appendQueryString(appApiPath(`/search/suggestions`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export interface SearchIndexesListParams {
  q?: string;
  page?: number;
  pageSize?: number;
}

export class SearchIndexesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** List search indexes visible to the current app principal. */
  async list(params?: SearchIndexesListParams, requestOptions?: ApiRequestOptions): Promise<{ items: SearchIndex[]; pageInfo: PageInfo; }> {
    const query = buildQueryString([
      { name: 'q', value: params?.q, style: 'form', explode: true, allowReserved: false },
      { name: 'page', value: params?.page, style: 'form', explode: true, allowReserved: false },
      { name: 'page_size', value: params?.pageSize, style: 'form', explode: true, allowReserved: false },
    ]);
    return this.client.request<{ items: SearchIndex[]; pageInfo: PageInfo; }>(appendQueryString(appApiPath(`/search/indexes`), query), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'GET' as any, sdkworkUnwrapKind: 'page' });
  }
}

export class SearchQueriesApi {
  private client: HttpClient;

  constructor(client: HttpClient) {
    this.client = client;
  }


/** Create a search query. */
  async create(body: SearchQueryRequest, requestOptions?: ApiRequestOptions): Promise<{ items: SearchResult[]; pageInfo: PageInfo; }> {
    return this.client.request<{ items: SearchResult[]; pageInfo: PageInfo; }>(appApiPath(`/search/queries`), { ...(requestOptions?.signal !== undefined ? { signal: requestOptions.signal } : {}), ...(requestOptions?.timeout !== undefined ? { timeout: requestOptions.timeout } : {}), method: 'POST' as any, body, contentType: 'application/json', sdkworkUnwrapKind: 'page' });
  }
}

export class SearchApi {
  public readonly queries: SearchQueriesApi;
  public readonly indexes: SearchIndexesApi;
  public readonly suggestions: SearchSuggestionsApi;
  public readonly recommendations: SearchRecommendationsApi;
  public readonly promotions: SearchPromotionsApi;
  public readonly events: SearchEventsApi;
  public readonly recentQueries: SearchRecentQueriesApi;
  public readonly semanticQueries: SearchSemanticQueriesApi;

  constructor(client: HttpClient) {
    this.queries = new SearchQueriesApi(client);
    this.indexes = new SearchIndexesApi(client);
    this.suggestions = new SearchSuggestionsApi(client);
    this.recommendations = new SearchRecommendationsApi(client);
    this.promotions = new SearchPromotionsApi(client);
    this.events = new SearchEventsApi(client);
    this.recentQueries = new SearchRecentQueriesApi(client);
    this.semanticQueries = new SearchSemanticQueriesApi(client);
  }

}

export function createSearchApi(client: HttpClient): SearchApi {
  return new SearchApi(client);
}

function appendQueryString(path: string, rawQueryString: string): string {
  const query = rawQueryString.replace(/^\?+/, '');
  if (!query) {
    return path;
  }
  return path.includes('?') ? `${path}&${query}` : `${path}?${query}`;
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
