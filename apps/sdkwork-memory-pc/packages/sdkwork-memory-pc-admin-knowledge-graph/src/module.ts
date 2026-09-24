import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/knowledge-graph/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/knowledge-graph/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/knowledge-graph/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/knowledge-graph/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/knowledge-graph/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/knowledge-graph/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/knowledge-graph/module.ts";

export const adminKnowledgeGraphModule = {
  id: "admin-knowledge-graph",
  surface: "backend-admin",
  route: "knowledge-graph",
  titleKey: "memory.admin-knowledge-graph.title",
  descriptionKey: "memory.admin-knowledge-graph.description",
  permission: "memory.backend.entities.read",
  resources: ["entities","edges"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminKnowledgeGraphModule;
