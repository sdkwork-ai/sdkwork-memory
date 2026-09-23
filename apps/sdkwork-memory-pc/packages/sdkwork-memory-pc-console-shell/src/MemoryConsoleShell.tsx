import "@sdkwork/memory-pc-commons/styles.css";

import { MemorySurfaceWorkspace, type MemoryPcModuleDefinition, type MemoryResourceRegistry } from "@sdkwork/memory-pc-commons";

export interface MemoryConsoleShellProps {
  modules: readonly MemoryPcModuleDefinition[];
  onSignOut?(): void;
  permissionScope: readonly string[];
  registry: MemoryResourceRegistry;
  userLabel?: string;
}

export function MemoryConsoleShell(props: MemoryConsoleShellProps) {
  return <MemorySurfaceWorkspace {...props} surface="app-console" />;
}
