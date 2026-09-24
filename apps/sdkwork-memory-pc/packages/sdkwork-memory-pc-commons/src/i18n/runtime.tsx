import { createContext, useContext, useMemo, type ReactNode } from "react";

import { messages as deDECommons } from "./de-DE/memory/commons/resource-workspace.ts";
import { messages as enUSCommons } from "./en-US/memory/commons/resource-workspace.ts";
import { messages as frFRCommons } from "./fr-FR/memory/commons/resource-workspace.ts";
import { messages as jaJPCommons } from "./ja-JP/memory/commons/resource-workspace.ts";
import { messages as koKRCommons } from "./ko-KR/memory/commons/resource-workspace.ts";
import { messages as ruRUCommons } from "./ru-RU/memory/commons/resource-workspace.ts";
import { messages as zhCNCommons } from "./zh-CN/memory/commons/resource-workspace.ts";
import type { MemoryMessageCatalog, MemoryPcModuleDefinition } from "../types.ts";

/**
 * The single source of truth for the locales this application ships.
 *
 * Runtime configuration validation, the language switcher, and every per-module
 * catalog wiring derive from this one declaration; adding a language means adding
 * it here plus its translation catalogs, never a second hardcoded list.
 */
export const MEMORY_SUPPORTED_LOCALES = ["en-US", "zh-CN", "de-DE", "fr-FR", "ja-JP", "ko-KR", "ru-RU"] as const;

export type MemoryLocale = (typeof MEMORY_SUPPORTED_LOCALES)[number];

/** Native labels for the language switcher, one per supported locale. */
export const MEMORY_LOCALE_LABELS: Readonly<Record<MemoryLocale, string>> = {
  "de-DE": "Deutsch",
  "en-US": "English",
  "fr-FR": "Français",
  "ja-JP": "日本語",
  "ko-KR": "한국어",
  "ru-RU": "Русский",
  "zh-CN": "简体中文",
};

/**
 * Commons catalogs keyed by locale.
 *
 * Exported so tests can assert every locale carries the full `en-US` key set,
 * and so components above the provider (the top-level error boundary) can read
 * fallback copy from the catalog instead of hardcoding strings.
 */
export const MEMORY_COMMONS_CATALOGS: Readonly<Record<MemoryLocale, MemoryMessageCatalog>> = {
  "de-DE": deDECommons,
  "en-US": enUSCommons,
  "fr-FR": frFRCommons,
  "ja-JP": jaJPCommons,
  "ko-KR": koKRCommons,
  "ru-RU": ruRUCommons,
  "zh-CN": zhCNCommons,
};

export interface MemoryI18nContextValue {
  locale: MemoryLocale;
  setLocale(locale: MemoryLocale): void;
  translate(key: string): string;
}

/**
 * Exported so class components that must render above or below the provider
 * (the error boundary) can consume translations through `contextType`.
 */
export const MemoryI18nContext = createContext<MemoryI18nContextValue | null>(null);

export interface MemoryI18nProviderProps {
  children: ReactNode;
  locale: MemoryLocale;
  modules: readonly MemoryPcModuleDefinition[];
  setLocale(locale: MemoryLocale): void;
}

export function MemoryI18nProvider({ children, locale, modules, setLocale }: MemoryI18nProviderProps) {
  const catalog = useMemo(() => buildCatalog(locale, modules), [locale, modules]);
  const value = useMemo<MemoryI18nContextValue>(() => ({
    locale,
    setLocale,
    translate: (key) => catalog[key] ?? key,
  }), [catalog, locale, setLocale]);

  return <MemoryI18nContext.Provider value={value}>{children}</MemoryI18nContext.Provider>;
}

export function useMemoryI18n(): MemoryI18nContextValue {
  const value = useContext(MemoryI18nContext);
  if (!value) throw new Error("MemoryI18nProvider is required");
  return value;
}

/**
 * Exposed for the error boundary, which may render above the provider and then
 * reads fallback copy straight from the `en-US` catalog.
 */
export const MEMORY_FALLBACK_CATALOG: MemoryMessageCatalog = MEMORY_COMMONS_CATALOGS["en-US"];

function buildCatalog(locale: MemoryLocale, modules: readonly MemoryPcModuleDefinition[]): MemoryMessageCatalog {
  const fallback = MEMORY_FALLBACK_CATALOG;
  const selected = MEMORY_COMMONS_CATALOGS[locale];
  return Object.assign({}, fallback, selected, ...modules.map((module) => module.messages["en-US"] ?? {}), ...modules.map((module) => module.messages[locale] ?? {}));
}
