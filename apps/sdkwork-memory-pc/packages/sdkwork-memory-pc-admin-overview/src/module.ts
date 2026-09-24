import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/overview/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/overview/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/overview/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/overview/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/overview/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/overview/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/overview/module.ts";

export const adminOverviewModule = {
  id: "admin-overview",
  surface: "backend-admin",
  route: "overview",
  titleKey: "memory.admin-overview.title",
  descriptionKey: "memory.admin-overview.description",
  permission: "memory.backend.commercialReadiness.read",
  resources: ["providerHealth","commercialReadiness"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminOverviewModule;
