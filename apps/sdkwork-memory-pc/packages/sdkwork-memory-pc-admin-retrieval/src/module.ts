import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/retrieval/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/retrieval/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/retrieval/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/retrieval/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/retrieval/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/retrieval/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/retrieval/module.ts";

export const adminRetrievalModule = {
  id: "admin-retrieval",
  surface: "backend-admin",
  route: "retrieval",
  titleKey: "memory.admin-retrieval.title",
  descriptionKey: "memory.admin-retrieval.description",
  permission: "memory.backend.indexes.read",
  resources: ["indexes","retrievalProfiles","retrievalTraces"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminRetrievalModule;
