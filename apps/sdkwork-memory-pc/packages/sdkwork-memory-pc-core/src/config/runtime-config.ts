import { MEMORY_SUPPORTED_LOCALES, type MemoryLocale } from "@sdkwork/memory-pc-commons";

export type MemoryLifecycleEnvironment = "development" | "production" | "staging" | "demo" | "test";
export type MemoryDeploymentProfile = "cloud" | "standalone";
export type { MemoryLocale };

export interface MemoryPcRuntimeConfig {
  appApiBaseUrl: string;
  appbaseAppApiBaseUrl: string;
  backendApiBaseUrl: string;
  defaultLocale: MemoryLocale;
  deploymentProfile: MemoryDeploymentProfile;
  environment: MemoryLifecycleEnvironment;
  fallbackLocale: MemoryLocale;
  supportedLocales: readonly MemoryLocale[];
}

export async function loadMemoryPcRuntimeConfig(fetcher: typeof fetch = fetch): Promise<MemoryPcRuntimeConfig> {
  const response = await fetcher("/runtime-env.json", { cache: "no-store", credentials: "same-origin" });
  if (!response.ok) throw new Error(`Runtime configuration failed with HTTP ${response.status}`);
  return parseMemoryPcRuntimeConfig(await response.json());
}

export function parseMemoryPcRuntimeConfig(value: unknown): MemoryPcRuntimeConfig {
  if (!isRecord(value)) throw new Error("Runtime configuration must be an object");
  const environment = readEnum(value.environment, ["development", "test", "staging", "demo", "production"] as const, "environment");
  const deploymentProfile = readEnum(value.deploymentProfile, ["standalone", "cloud"] as const, "deploymentProfile");
  // Locale acceptance derives from the shared MEMORY_SUPPORTED_LOCALES constant, so a
  // newly shipped language is valid in runtime configuration without a second list here.
  const defaultLocale = readEnum(value.defaultLocale, MEMORY_SUPPORTED_LOCALES, "defaultLocale");
  const fallbackLocale = readEnum(value.fallbackLocale, MEMORY_SUPPORTED_LOCALES, "fallbackLocale");
  const supportedLocales = Array.isArray(value.supportedLocales)
    ? value.supportedLocales.map((locale) => readEnum(locale, MEMORY_SUPPORTED_LOCALES, "supportedLocales"))
    : [];
  if (!supportedLocales.includes(defaultLocale) || !supportedLocales.includes(fallbackLocale)) {
    throw new Error("defaultLocale and fallbackLocale must be supported");
  }
  return {
    environment,
    deploymentProfile,
    defaultLocale,
    fallbackLocale,
    supportedLocales,
    appApiBaseUrl: readPublicUrl(value.appApiBaseUrl, "appApiBaseUrl", environment),
    backendApiBaseUrl: readPublicUrl(value.backendApiBaseUrl, "backendApiBaseUrl", environment),
    appbaseAppApiBaseUrl: readPublicUrl(value.appbaseAppApiBaseUrl, "appbaseAppApiBaseUrl", environment),
  };
}

function readPublicUrl(value: unknown, name: string, environment: MemoryLifecycleEnvironment): string {
  if (typeof value !== "string" || !value.trim()) throw new Error(`${name} is required`);
  const parsed = new URL(value);
  if (!new Set(["http:", "https:"]).has(parsed.protocol)) throw new Error(`${name} must use HTTP or HTTPS`);
  if (environment === "production" && new Set(["localhost", "127.0.0.1", "::1"]).has(parsed.hostname)) {
    throw new Error(`${name} must not use a loopback host in production`);
  }
  return parsed.toString().replace(/\/$/, "");
}

function readEnum<const T extends readonly string[]>(value: unknown, allowed: T, name: string): T[number] {
  if (typeof value !== "string" || !allowed.includes(value)) throw new Error(`${name} is invalid`);
  return value as T[number];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
