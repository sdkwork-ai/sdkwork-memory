import { Navigate, useLocation } from "react-router-dom";

import type { MemoryPcModuleDefinition, MemoryPcSurface, MemoryResourceRegistry } from "../types.ts";
import { MemoryModulePage } from "./MemoryModulePage.tsx";
import { MemoryPermissionState } from "./MemoryPermissionState.tsx";
import { MemorySurfaceShell } from "./MemorySurfaceShell.tsx";

export interface MemorySurfaceWorkspaceProps {
  modules: readonly MemoryPcModuleDefinition[];
  onSignOut?(): void;
  permissionScope: readonly string[];
  registry: MemoryResourceRegistry;
  surface: MemoryPcSurface;
  userLabel?: string;
}

export function MemorySurfaceWorkspace(props: MemorySurfaceWorkspaceProps) {
  const location = useLocation();
  const basePath = props.surface === "backend-admin" ? "/admin" : "/console";
  const activeRoute = location.pathname.slice(basePath.length).split("/").filter(Boolean)[0];
  const module = props.modules.find((candidate) => candidate.route === activeRoute);
  const first = props.modules[0];

  if (!activeRoute && first) return <Navigate to={`${basePath}/${first.route}`} replace />;
  if (!module && first) return <Navigate to={`${basePath}/${first.route}`} replace />;
  if (!module) return null;

  const allowed = hasPermissionHint(props.permissionScope, module.permission);
  return (
    <MemorySurfaceShell activeRoute={module.route} modules={props.modules} onSignOut={props.onSignOut} surface={props.surface} userLabel={props.userLabel}>
      {allowed ? <MemoryModulePage module={module} registry={props.registry} /> : <MemoryPermissionState titleKey={module.titleKey} />}
    </MemorySurfaceShell>
  );
}

export function hasPermissionHint(permissionScope: readonly string[], required: string): boolean {
  if (permissionScope.includes("*") || permissionScope.includes(required)) return true;
  return permissionScope.some((permission) => permission.endsWith(".*") && required.startsWith(permission.slice(0, -1)));
}
