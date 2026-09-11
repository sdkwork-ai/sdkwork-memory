import { HttpClient } from './http/client';
import type { SdkworkAppConfig } from './types/common';
import type { AuthTokenManager } from '@sdkwork/sdk-common';
import { MemoryApi } from './api/memory';
export declare class SdkworkMemoryAppClient {
    private httpClient;
    readonly memory: MemoryApi;
    constructor(config: SdkworkAppConfig);
    setAuthToken(token: string): this;
    setAccessToken(token: string): this;
    setTokenManager(manager: AuthTokenManager): this;
    get http(): HttpClient;
}
export declare function createClient(config: SdkworkAppConfig): SdkworkMemoryAppClient;
export default SdkworkMemoryAppClient;
//# sourceMappingURL=sdk.d.ts.map