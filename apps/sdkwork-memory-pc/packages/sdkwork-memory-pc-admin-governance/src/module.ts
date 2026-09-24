import type { MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { messages as deDE } from "./i18n/de-DE/memory/governance/module.ts";
import { messages as enUS } from "./i18n/en-US/memory/governance/module.ts";
import { messages as frFR } from "./i18n/fr-FR/memory/governance/module.ts";
import { messages as jaJP } from "./i18n/ja-JP/memory/governance/module.ts";
import { messages as koKR } from "./i18n/ko-KR/memory/governance/module.ts";
import { messages as ruRU } from "./i18n/ru-RU/memory/governance/module.ts";
import { messages as zhCN } from "./i18n/zh-CN/memory/governance/module.ts";

export const adminGovernanceModule = {
  id: "admin-governance",
  surface: "backend-admin",
  route: "governance",
  titleKey: "memory.admin-governance.title",
  descriptionKey: "memory.admin-governance.description",
  permission: "memory.backend.auditLogs.read",
  resources: ["policies","policyAssignments","auditLogs","retentionJobs","migrationJobs"],
  messages: { "de-DE": deDE, "en-US": enUS, "fr-FR": frFR, "ja-JP": jaJP, "ko-KR": koKR, "ru-RU": ruRU, "zh-CN": zhCN },
} as const satisfies MemoryPcModuleDefinition;

export const memoryModule = adminGovernanceModule;
