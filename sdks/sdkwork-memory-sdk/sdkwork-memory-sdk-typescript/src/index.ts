import {
  createClient as createGeneratedOpenClient,
  SdkworkMemoryOpenClient,
} from '../generated/server-openapi/src/index';
import type { SdkworkCustomConfig as SdkworkOpenConfig } from '../generated/server-openapi/src/types/common';

export { SdkworkMemoryOpenClient, createGeneratedOpenClient };
export type { SdkworkOpenConfig };
export * from '../generated/server-openapi/src/types';
export * from '../generated/server-openapi/src/api';
export * from '../generated/server-openapi/src/http';
export * from '../generated/server-openapi/src/auth';

export function createClient(config: SdkworkOpenConfig): SdkworkMemoryOpenClient {
  return createGeneratedOpenClient(config);
}
