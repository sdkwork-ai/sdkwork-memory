import { createHttpClient } from './http/client';
import { createMemoryApi } from './api/memory';
export class SdkworkMemoryAppClient {
    httpClient;
    memory;
    constructor(config) {
        this.httpClient = createHttpClient(config);
        this.memory = createMemoryApi(this.httpClient);
    }
    setAuthToken(token) {
        this.httpClient.setAuthToken(token);
        return this;
    }
    setAccessToken(token) {
        this.httpClient.setAccessToken(token);
        return this;
    }
    setTokenManager(manager) {
        this.httpClient.setTokenManager(manager);
        return this;
    }
    get http() {
        return this.httpClient;
    }
}
export function createClient(config) {
    return new SdkworkMemoryAppClient(config);
}
export default SdkworkMemoryAppClient;
//# sourceMappingURL=sdk.js.map