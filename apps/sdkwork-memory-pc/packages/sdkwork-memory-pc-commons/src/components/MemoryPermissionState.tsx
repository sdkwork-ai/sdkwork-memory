import { ShieldAlert } from "lucide-react";

import { useMemoryI18n } from "../i18n/runtime.tsx";

export interface MemoryPermissionStateProps {
  /** Message key of the module or surface that was denied. */
  titleKey: string;
}

/**
 * Denied state for a Memory console surface.
 *
 * Shared by the standalone workspace and the embeddable block so hosts render
 * exactly the same denial semantics as the Memory application itself: the
 * module stays visible and explains why it is empty instead of hiding silently.
 */
export function MemoryPermissionState({ titleKey }: MemoryPermissionStateProps) {
  const { translate } = useMemoryI18n();
  return (
    <section className="permission-state">
      <ShieldAlert aria-hidden="true" size={28} />
      <h1>{translate(titleKey)}</h1>
      <p>{translate("memory.commons.permissionDenied")}</p>
    </section>
  );
}
