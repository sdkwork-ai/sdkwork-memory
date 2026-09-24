import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/evaluation/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/evaluation/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/evaluation/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/evaluation/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/evaluation/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/evaluation/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/evaluation/module.ts";

export const adminEvaluationModule = {
  id: "admin-evaluation",
  surface: "backend-admin",
  route: "evaluation",
  titleKey: "memory.admin-evaluation.title",
  descriptionKey: "memory.admin-evaluation.description",
  permission: "memory.backend.evalRuns.read",
  resources: ["evalRuns"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminEvaluationModule;
