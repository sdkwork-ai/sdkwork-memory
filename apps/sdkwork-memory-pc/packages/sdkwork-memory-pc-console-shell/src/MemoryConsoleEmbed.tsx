import "@sdkwork/memory-pc-commons/styles.css";

import {
  MemoryConsoleScope,
  MemoryErrorBoundary,
  MemoryI18nProvider,
  MemoryModulePage,
  MemoryPermissionState,
  hasPermissionHint,
  useMemoryI18n,
  type MemoryLocale,
  type MemoryPcModuleDefinition,
  type MemoryResourceRegistry,
} from "@sdkwork/memory-pc-commons";
import {
  MemoryConsoleSdkProvider,
  createMemoryConsoleResourceRegistry,
  type MemoryConsoleSdkClient,
} from "@sdkwork/memory-pc-console-core";
import { useEffect, useMemo, useState } from "react";

export type { MemoryLocale, MemoryPcModuleDefinition, MemoryResourceRegistry } from "@sdkwork/memory-pc-commons";

export interface MemoryConsoleEmbedProps {
  /** Injected Memory app SDK client. The block never constructs its own transport. */
  client: MemoryConsoleSdkClient;
  /** Memory console module catalog. Ownership stays with sdkwork-memory capability packages. */
  modules: readonly MemoryPcModuleDefinition[];
  /** Host-resolved locale. */
  locale: MemoryLocale;
  /**
   * Granted permission codes for the current session.
   * Required (not defaulted) so an embedding host cannot accidentally fail open.
   */
  permissionScope: readonly string[];
  /** Optional locale callback; omit for read-only hosts without a language switcher. */
  onLocaleChange?(locale: MemoryLocale): void;
  /** Optional registry override; defaults to the client-bound Memory console registry. */
  registry?: MemoryResourceRegistry;
  /**
   * Controlled active module id. Pass this together with `onModuleChange` to let the
   * host own module routing (deep links such as `/console/memory/retrieval`).
   * Omit both to let the block own selection with local state.
   */
  moduleId?: string;
  /** Controlled selection callback. Receives the module id the user picked. */
  onModuleChange?(moduleId: string): void;
  className?: string;
}

/**
 * Chrome-less Memory console block.
 *
 * This is the integration seam for host applications: the whole user-facing
 * Memory console (module switcher, tables, drawers, empty/loading/denied states
 * and styles) is owned by sdkwork-memory, while the host only injects its SDK
 * client, locale, and granted permission scope.
 *
 * Router-agnostic on purpose: it never reads `react-router` state itself, so the
 * same block can be mounted by a PC route, an H5 route, or a desktop shell. When
 * the host wants URL-driven modules it passes `moduleId` + `onModuleChange`; when
 * it does not, the block keeps selection in local state.
 *
 * The block imports its own presentation stylesheet, so a host integration is a
 * single package import with nothing to remember separately.
 */
export function MemoryConsoleEmbed(props: MemoryConsoleEmbedProps) {
  const {
    className,
    client,
    locale,
    moduleId,
    modules,
    onLocaleChange,
    onModuleChange,
    permissionScope,
    registry,
  } = props;
  const resolvedRegistry = useMemo(
    () => registry ?? createMemoryConsoleResourceRegistry(client),
    [client, registry],
  );

  return (
    <MemoryI18nProvider locale={locale} modules={modules} setLocale={onLocaleChange ?? ignoreLocale}>
      <MemoryConsoleSdkProvider client={client}>
        <MemoryConsoleScope className={className} surface="app-console">
          <MemoryConsoleEmbedBody
            moduleId={moduleId}
            modules={modules}
            onModuleChange={onModuleChange}
            permissionScope={permissionScope}
            registry={resolvedRegistry}
          />
        </MemoryConsoleScope>
      </MemoryConsoleSdkProvider>
    </MemoryI18nProvider>
  );
}

interface MemoryConsoleEmbedBodyProps {
  moduleId?: string;
  modules: readonly MemoryPcModuleDefinition[];
  onModuleChange?(moduleId: string): void;
  permissionScope: readonly string[];
  registry: MemoryResourceRegistry;
}

function MemoryConsoleEmbedBody({
  moduleId,
  modules,
  onModuleChange,
  permissionScope,
  registry,
}: MemoryConsoleEmbedBodyProps) {
  const { translate } = useMemoryI18n();
  const [uncontrolledModuleId, setUncontrolledModuleId] = useState<string | undefined>(moduleId);

  useEffect(() => {
    // Keep the uncontrolled state in sync when a host hands over an initial module.
    if (moduleId !== undefined) setUncontrolledModuleId(moduleId);
  }, [moduleId]);

  const selectedModuleId = moduleId ?? uncontrolledModuleId;
  const activeModule = modules.find((candidate) => candidate.id === selectedModuleId) ?? modules[0];
  const showModuleSwitcher = modules.length > 1;

  if (!activeModule) return null;

  const allowed = hasPermissionHint(permissionScope, activeModule.permission);

  function selectModule(nextModule: MemoryPcModuleDefinition): void {
    if (onModuleChange) {
      onModuleChange(nextModule.id);
      return;
    }
    setUncontrolledModuleId(nextModule.id);
  }

  return (
    <>
      {showModuleSwitcher ? (
        <nav aria-label={translate("memory.commons.console")} className="module-nav" role="tablist">
          {modules.map((candidate) => (
            <button
              aria-selected={candidate.id === activeModule.id}
              className={candidate.id === activeModule.id ? "module-nav-item active" : "module-nav-item"}
              key={candidate.id}
              onClick={() => selectModule(candidate)}
              role="tab"
              type="button"
            >
              {translate(candidate.titleKey)}
            </button>
          ))}
        </nav>
      ) : null}
      {allowed ? (
        // Keyed by module so switching modules remounts the boundary; a recovered
        // module never inherits a failed boundary's error state.
        <MemoryErrorBoundary key={activeModule.id}>
          <MemoryModulePage module={activeModule} registry={registry} />
        </MemoryErrorBoundary>
      ) : (
        <MemoryPermissionState titleKey={activeModule.titleKey} />
      )}
    </>
  );
}

function ignoreLocale(_locale: MemoryLocale): void {
  // Hosts without a language switcher keep the locale they injected.
}
