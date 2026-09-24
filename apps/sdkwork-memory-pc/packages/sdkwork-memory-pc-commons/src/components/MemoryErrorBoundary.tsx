import { Component, type ErrorInfo, type ReactNode } from "react";

import { MEMORY_FALLBACK_CATALOG, MemoryI18nContext, type MemoryI18nContextValue } from "../i18n/runtime.tsx";

export interface MemoryErrorBoundaryProps {
  children: ReactNode;
}

interface MemoryErrorBoundaryState {
  failed: boolean;
}

/**
 * Route/page-level error boundary for the Memory workspace.
 *
 * A render error must surface a recoverable state instead of a blank screen: the
 * fallback stays generic (no raw error details reach the UI) and the retry button
 * resets the boundary so the view remounts. When no provider is above this
 * boundary — the top-level application shell — fallback copy is read straight from
 * the `en-US` commons catalog, so user-facing text still comes from i18n.
 */
export class MemoryErrorBoundary extends Component<MemoryErrorBoundaryProps, MemoryErrorBoundaryState> {
  static contextType = MemoryI18nContext;
  declare context: MemoryI18nContextValue | null;

  override state: MemoryErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): MemoryErrorBoundaryState {
    return { failed: true };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    // Diagnostics only. The rendered fallback must remain generic so raw error
    // details never leak into the user interface.
    console.error("Memory workspace render error", error, info.componentStack);
  }

  override render(): ReactNode {
    if (!this.state.failed) return this.props.children;
    const translate = this.context?.translate ?? ((key: string) => MEMORY_FALLBACK_CATALOG[key] ?? key);
    return (
      <section className="memory-error-boundary" role="alert">
        <h1>{translate("memory.commons.errorBoundaryTitle")}</h1>
        <p>{translate("memory.commons.errorBoundaryDescription")}</p>
        <button type="button" className="memory-error-retry" onClick={() => this.setState({ failed: false })}>
          {translate("memory.commons.errorBoundaryRetry")}
        </button>
      </section>
    );
  }
}
