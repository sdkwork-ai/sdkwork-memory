import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/learning/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/learning/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/learning/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/learning/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/learning/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/learning/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/learning/module.ts";

export const adminLearningModule = {
  id: "admin-learning",
  surface: "backend-admin",
  route: "learning",
  titleKey: "memory.admin-learning.title",
  descriptionKey: "memory.admin-learning.description",
  permission: "memory.backend.candidates.read",
  resources: ["candidates","extractionJobs","consolidationJobs"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminLearningModule;
