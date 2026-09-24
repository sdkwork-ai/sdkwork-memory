import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/knowledge/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/knowledge/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/knowledge/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/knowledge/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/knowledge/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/knowledge/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/knowledge/module.ts";

export const consoleKnowledgeModule = {
  id: "console-knowledge",
  surface: "app-console",
  route: "knowledge",
  titleKey: "memory.console-knowledge.title",
  descriptionKey: "memory.console-knowledge.description",
  permission: "memory.app.entities.read",
  resources: ["entities"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = consoleKnowledgeModule;
