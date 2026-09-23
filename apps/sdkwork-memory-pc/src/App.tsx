import { useSdkworkAuthControllerState } from "@sdkwork/auth-pc-react";
import { memoryModule as adminControlPlaneModule } from "@sdkwork/memory-pc-admin-control-plane";
import { memoryModule as adminEvaluationModule } from "@sdkwork/memory-pc-admin-evaluation";
import { memoryModule as adminGovernanceModule } from "@sdkwork/memory-pc-admin-governance";
import { memoryModule as adminKnowledgeGraphModule } from "@sdkwork/memory-pc-admin-knowledge-graph";
import { memoryModule as adminLearningModule } from "@sdkwork/memory-pc-admin-learning";
import { memoryModule as adminMemoryModule } from "@sdkwork/memory-pc-admin-memory";
import { memoryModule as adminOverviewModule } from "@sdkwork/memory-pc-admin-overview";
import { memoryModule as adminProvidersModule } from "@sdkwork/memory-pc-admin-providers";
import { memoryModule as adminRetrievalModule } from "@sdkwork/memory-pc-admin-retrieval";
import { MemoryI18nProvider, type MemoryPcModuleDefinition } from "@sdkwork/memory-pc-commons";
import { MemoryConsoleSdkProvider, createMemoryConsoleResourceRegistry } from "@sdkwork/memory-pc-console-core";
import { MemoryConsoleShell, memoryConsoleModules as consoleModules } from "@sdkwork/memory-pc-console-shell";
import { assertUniqueMemoryModules, type MemoryLocale } from "@sdkwork/memory-pc-core";
import { lazy, Suspense, useMemo, useState } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";

import type { BootstrappedMemoryPcRuntime } from "./bootstrap/runtime.ts";
import { MemoryAuthGate } from "./auth/MemoryAuthGate.tsx";

/**
 * The user-console catalog is imported from the console shell package, which owns
 * module membership, ordering, routes, and permission gates. Listing the modules
 * here as well would create a second source of truth that silently drifts the
 * moment a capability package is added.
 */
const adminModules = assertUniqueMemoryModules([
  adminOverviewModule,
  adminMemoryModule,
  adminLearningModule,
  adminRetrievalModule,
  adminProvidersModule,
  adminEvaluationModule,
  adminKnowledgeGraphModule,
  adminControlPlaneModule,
  adminGovernanceModule,
] satisfies readonly MemoryPcModuleDefinition[]);

const allModules = [...consoleModules, ...adminModules];
const LazyMemoryAuthRoutes = lazy(() => import("./auth/MemoryAuthRoutes.tsx").then((module) => ({ default: module.MemoryAuthRoutes })));
const LazyMemoryAdminSurface = lazy(() => import("./surfaces/MemoryAdminSurface.tsx").then((module) => ({ default: module.MemoryAdminSurface })));

export function App({ runtime }: { runtime: BootstrappedMemoryPcRuntime }) {
  const [locale, setLocaleState] = useState<MemoryLocale>(runtime.config.defaultLocale);

  function setLocale(nextLocale: MemoryLocale): void {
    runtime.setLocale(nextLocale);
    setLocaleState(nextLocale);
  }

  return (
    <MemoryI18nProvider locale={locale} modules={allModules} setLocale={setLocale}>
      <BrowserRouter>
        <MemoryAuthenticatedApplication runtime={runtime} />
      </BrowserRouter>
    </MemoryI18nProvider>
  );
}

function MemoryAuthenticatedApplication({ runtime }: { runtime: BootstrappedMemoryPcRuntime }) {
  const authState = useSdkworkAuthControllerState(runtime.authController);
  const consoleRegistry = useMemo(() => createMemoryConsoleResourceRegistry(runtime.appClient), [runtime.appClient]);
  const permissionScope = authState.session?.context?.permissionScope ?? [];
  const userLabel = authState.user?.displayName || authState.user?.email;
  const signOut = () => { void runtime.authController.signOut(); };

  return (
    <MemoryAuthGate
      controller={runtime.authController}
      authRoutes={<Suspense fallback={<div className="bootstrap-state" role="status">SDKWork Memory</div>}><LazyMemoryAuthRoutes controller={runtime.authController} /></Suspense>}
    >
      <MemoryConsoleSdkProvider client={runtime.appClient}>
        <Routes>
          <Route path="/console/*" element={<MemoryConsoleShell modules={consoleModules} permissionScope={permissionScope} registry={consoleRegistry} userLabel={userLabel} onSignOut={signOut} />} />
          <Route path="/admin/*" element={<Suspense fallback={<div className="bootstrap-state" role="status">SDKWork Memory</div>}><LazyMemoryAdminSurface backendApiBaseUrl={runtime.config.backendApiBaseUrl} modules={adminModules} permissionScope={permissionScope} tokenManager={runtime.tokenManager} userLabel={userLabel} onSignOut={signOut} /></Suspense>} />
          <Route path="*" element={<Navigate to="/console" replace />} />
        </Routes>
      </MemoryConsoleSdkProvider>
    </MemoryAuthGate>
  );
}
