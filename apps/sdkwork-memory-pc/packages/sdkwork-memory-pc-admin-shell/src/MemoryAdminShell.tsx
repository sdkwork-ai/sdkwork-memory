import { MemorySurfaceWorkspace, type MemoryPcModuleDefinition, type MemoryResourceRegistry } from "@sdkwork/memory-pc-commons";

export interface MemoryAdminShellProps {
  /** URL prefix the host mounts this admin surface under; see {@link MemorySurfaceWorkspaceProps.basePath}. */
  basePath?: string;
  modules: readonly MemoryPcModuleDefinition[];
  onSignOut?(): void;
  permissionScope: readonly string[];
  registry: MemoryResourceRegistry;
  userLabel?: string;
}

export function MemoryAdminShell(props: MemoryAdminShellProps) {
  return <MemorySurfaceWorkspace {...props} surface="backend-admin" />;
}
