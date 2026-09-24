import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/learning/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/learning/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/learning/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/learning/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/learning/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/learning/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/learning/module.ts";

export const consoleLearningModule = {
  id: "console-learning",
  surface: "app-console",
  route: "learning",
  titleKey: "memory.console-learning.title",
  descriptionKey: "memory.console-learning.description",
  permission: "memory.candidates.read",
  resources: ["candidates","habits","learningSettings","extractionJobs"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = consoleLearningModule;
