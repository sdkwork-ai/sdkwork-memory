import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/memory/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/memory/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/memory/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/memory/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/memory/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/memory/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/memory/module.ts";

export const consoleMemoryModule = {
  id: "console-memory",
  surface: "app-console",
  route: "memory",
  titleKey: "memory.console-memory.title",
  descriptionKey: "memory.console-memory.description",
  permission: "memory.records.read",
  resources: ["spaces","memories","events"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = consoleMemoryModule;
