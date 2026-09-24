import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/providers/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/providers/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/providers/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/providers/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/providers/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/providers/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/providers/module.ts";

export const adminProvidersModule = {
  id: "admin-providers",
  surface: "backend-admin",
  route: "providers",
  titleKey: "memory.admin-providers.title",
  descriptionKey: "memory.admin-providers.description",
  permission: "memory.backend.providerBindings.read",
  resources: ["implementationProfiles","providerBindings","providerHealth"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminProvidersModule;
