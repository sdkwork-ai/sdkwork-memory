import type { ReactNode } from "react";

import type { MemoryPcSurface } from "../types.ts";

/**
 * Root class of the Memory console presentation scope.
 *
 * Every Memory console stylesheet rule is scoped under this class
 * (`src/styles/memory-console.css`), so the block can be rendered inside any
 * host console without leaking styles or colliding with host classes.
 */
export const MEMORY_CONSOLE_SCOPE_CLASS = "sdkwork-memory-console";

/** Data attribute carrying the surface accent (`app-console` vs `backend-admin`). */
export const MEMORY_CONSOLE_SURFACE_ATTRIBUTE = "data-memory-surface";

export function memoryConsoleScopeClassName(
  surface: MemoryPcSurface = "app-console",
  className?: string,
): string {
  return [MEMORY_CONSOLE_SCOPE_CLASS, surface === "backend-admin" ? "memory-surface-admin" : "memory-surface-console", className]
    .filter(Boolean)
    .join(" ");
}

export interface MemoryConsoleScopeProps {
  children: ReactNode;
  className?: string;
  surface?: MemoryPcSurface;
}

/**
 * Scope root for the Memory console block.
 *
 * Standalone hosts mount it around the shell; embedding hosts mount it around
 * the embedded module surface. Both paths therefore share one stylesheet.
 */
export function MemoryConsoleScope({ children, className, surface = "app-console" }: MemoryConsoleScopeProps) {
  return (
    <div
      className={memoryConsoleScopeClassName(surface, className)}
      {...{ [MEMORY_CONSOLE_SURFACE_ATTRIBUTE]: surface }}
    >
      {children}
    </div>
  );
}
