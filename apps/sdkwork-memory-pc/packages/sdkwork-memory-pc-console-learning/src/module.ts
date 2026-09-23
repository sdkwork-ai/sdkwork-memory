import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as enUS } from "./i18n/en-US/memory/learning/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/learning/module.ts";

export const consoleLearningModule = {
  id: "console-learning",
  surface: "app-console",
  route: "learning",
  titleKey: "memory.console-learning.title",
  descriptionKey: "memory.console-learning.description",
  permission: "memory.candidates.read",
  resources: ["candidates","habits","learningSettings","extractionJobs"],
  messages: { "en-US": enUS, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = consoleLearningModule;
