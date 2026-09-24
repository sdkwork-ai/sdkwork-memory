import { Activity, BrainCircuit, Database, FlaskConical, Gauge, GitBranch, HeartPulse, Network, Search, Settings2, ShieldCheck, Sparkles, Users, type LucideIcon } from "lucide-react";
import { type ReactNode } from "react";
import { Link } from "react-router-dom";

import { MEMORY_LOCALE_LABELS, MEMORY_SUPPORTED_LOCALES, useMemoryI18n, type MemoryLocale } from "../i18n/runtime.tsx";
import type { MemoryPcModuleDefinition, MemoryPcSurface } from "../types.ts";
import { MEMORY_CONSOLE_SURFACE_ATTRIBUTE, memoryConsoleScopeClassName } from "./MemoryConsoleScope.tsx";

const icons: Record<string, LucideIcon> = {
  overview: Gauge,
  memory: BrainCircuit,
  learning: Sparkles,
  retrieval: Search,
  knowledge: Database,
  governance: ShieldCheck,
  providers: Settings2,
  evaluation: FlaskConical,
  "knowledge-graph": GitBranch,
  "control-plane": Network,
};

export interface MemorySurfaceShellProps {
  activeRoute: string;
  children: ReactNode;
  modules: readonly MemoryPcModuleDefinition[];
  onSignOut?(): void;
  surface: MemoryPcSurface;
  userLabel?: string;
}

export function MemorySurfaceShell({ activeRoute, children, modules, onSignOut, surface, userLabel }: MemorySurfaceShellProps) {
  const { locale, setLocale, translate } = useMemoryI18n();
  const basePath = surface === "backend-admin" ? "/admin" : "/console";
  const SurfaceIcon = surface === "backend-admin" ? Activity : HeartPulse;

  return (
    <div
      className={memoryConsoleScopeClassName(surface, `app-frame ${surface === "backend-admin" ? "admin-frame" : "console-frame"}`)}
      {...{ [MEMORY_CONSOLE_SURFACE_ATTRIBUTE]: surface }}
    >
      <aside className="primary-sidebar">
        <Link to={basePath} className="brand-lockup" aria-label={translate("memory.commons.applicationName")}>
          <span className="brand-mark"><BrainCircuit size={21} /></span>
          <span><strong>SDKWork</strong><small>{translate("memory.commons.applicationName")}</small></span>
        </Link>
        <div className="surface-label"><SurfaceIcon size={15} /><span>{surface === "backend-admin" ? translate("memory.commons.admin") : translate("memory.commons.console")}</span></div>
        <nav aria-label={surface === "backend-admin" ? translate("memory.commons.admin") : translate("memory.commons.console")}>
          {modules.map((module) => {
            const Icon = icons[module.route] ?? Database;
            return <Link key={module.id} to={`${basePath}/${module.route}`} className={activeRoute === module.route ? "nav-item active" : "nav-item"}><Icon size={17} /><span>{translate(module.titleKey)}</span></Link>;
          })}
        </nav>
        <footer className="sidebar-footer">
          <label><span>{translate("memory.commons.locale")}</span><select value={locale} onChange={(event) => setLocale(event.target.value as MemoryLocale)}>{MEMORY_SUPPORTED_LOCALES.map((candidate) => <option key={candidate} value={candidate}>{MEMORY_LOCALE_LABELS[candidate]}</option>)}</select></label>
          {userLabel ? <p className="user-label"><Users size={14} />{userLabel}</p> : null}
          {onSignOut ? <button type="button" className="sign-out-button" onClick={onSignOut}><ShieldCheck size={15} />{translate("memory.commons.signOut")}</button> : null}
        </footer>
      </aside>
      <main className="main-workspace">{children}</main>
    </div>
  );
}
