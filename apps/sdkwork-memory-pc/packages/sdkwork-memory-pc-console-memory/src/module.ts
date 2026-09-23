import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as enUS } from "./i18n/en-US/memory/memory/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/memory/module.ts";

export const consoleMemoryModule = {
  id: "console-memory",
  surface: "app-console",
  route: "memory",
  titleKey: "memory.console-memory.title",
  descriptionKey: "memory.console-memory.description",
  permission: "memory.records.read",
  resources: ["spaces","memories","events"],
  messages: { "en-US": enUS, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = consoleMemoryModule;
