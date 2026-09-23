import { HttpClient, createHttpClient } from './http/client';
import type { SdkworkCustomConfig } from './types/common';

import { MemoryApi, createMemoryApi } from './api/memory';

export class SdkworkCustomClient {
  private httpClient: HttpClient;

  public readonly memory: MemoryApi;

  constructor(config: SdkworkCustomConfig) {
    this.httpClient = createHttpClient(config);
    this.memory = createMemoryApi(this.httpClient);
  }

  setApiKey(apiKey: string): this {
    this.httpClient.setApiKey(apiKey);
    return this;
  }
  get http(): HttpClient {
    return this.httpClient;
  }
}

export function createClient(config: SdkworkCustomConfig): SdkworkCustomClient {
  return new SdkworkCustomClient(config);
}

export default SdkworkCustomClient;
