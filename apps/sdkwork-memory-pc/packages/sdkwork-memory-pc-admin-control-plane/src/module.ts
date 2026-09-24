import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/control-plane/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/control-plane/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/control-plane/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/control-plane/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/control-plane/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/control-plane/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/control-plane/module.ts";

export const adminControlPlaneModule = {
  id: "admin-control-plane",
  surface: "backend-admin",
  route: "control-plane",
  titleKey: "memory.admin-control-plane.title",
  descriptionKey: "memory.admin-control-plane.description",
  permission: "memory.backend.subjects.read",
  resources: ["subjects","bindings","capabilityBindings","capabilities"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminControlPlaneModule;
