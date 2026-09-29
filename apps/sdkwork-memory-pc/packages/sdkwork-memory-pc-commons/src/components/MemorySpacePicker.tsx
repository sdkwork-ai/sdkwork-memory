import { useContext, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Brain, Check, Copy, Loader2, Plus, X } from 'lucide-react';

import {
  MEMORY_COMMONS_CATALOGS,
  MEMORY_FALLBACK_CATALOG,
  MEMORY_SUPPORTED_LOCALES,
  MemoryI18nContext,
} from '../i18n/runtime.tsx';
import type { MemoryMessageCatalog } from '../types.ts';

import '../styles/memory-space-picker.css';

/**
 * One memory the picker can attach, as the host resolved it.
 *
 * The picker deliberately carries no SDK client: it is the presentation half of
 * the capability and any host that can read `GET /memory/spaces` can embed it.
 */
export interface MemoryPickerSpace {
  spaceId: string;
  displayName: string;
  /** `spaceType` of the owning space; the default memory is identified by it. */
  spaceType: string;
  /**
   * Whether this is the memory attached when an agent names none of its own.
   *
   * Resolved by the host, not by the picker: only the host knows whether it is
   * reading the default from a space type, a metadata marker, or a server field.
   */
  isDefault: boolean;
  /** Active records in the space, when the host counted them. */
  recordCount?: number;
}

export interface MemoryPickerCreateInput {
  displayName: string;
}

export interface MemoryPickerCloneInput {
  sourceSpaceId: string;
  displayName: string;
}

export interface MemorySpacePickerProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  /** The user's memories, default first or not — the picker sorts its own bands. */
  spaces: readonly MemoryPickerSpace[];
  /** Space already attached to the owner this picker is editing, if any. */
  attachedSpaceId?: string;
  loading?: boolean;
  /** Human-readable failure from the host's last load or mutation. */
  errorMessage?: string;
  /** `spaceId` of a row whose action is in flight, or `'__create__'` while creating. */
  busyKey?: string;
  onRetry?(): void;
  onAttach(spaceId: string): void | Promise<void>;
  onCreate(input: MemoryPickerCreateInput): void | Promise<void>;
  onClone(input: MemoryPickerCloneInput): void | Promise<void>;
}

const CREATE_BUSY_KEY = '__create__';

/**
 * Modal memory picker for hosts that attach a memory to something they own.
 *
 * Renders the user's memories, marks the default one, and offers both ways of
 * getting a memory onto the owner: attach an existing one, or clone one into an
 * independent copy and attach that. Creating a brand-new memory is offered in
 * the same footer, because "I need a memory that does not exist yet" is the
 * third thing a user reaches for at this point.
 *
 * All data and all mutations belong to the host; this component never calls an
 * API. That is what lets the Memory application and every embedding host share
 * one behaviour instead of each restating it.
 */
export function MemorySpacePicker({
  attachedSpaceId,
  busyKey,
  errorMessage,
  loading = false,
  onAttach,
  onClone,
  onOpenChange,
  onCreate,
  onRetry,
  open,
  spaces,
}: MemorySpacePickerProps) {
  const translate = useMemoryPickerTranslate();
  const [composeMode, setComposeMode] = useState<'create' | 'clone' | null>(null);
  const [composeName, setComposeName] = useState('');
  const [cloneSourceId, setCloneSourceId] = useState<string | null>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);

  const orderedSpaces = useMemo(() => {
    const defaultFirst = [...spaces].sort((left, right) => {
      if (left.isDefault !== right.isDefault) {
        return left.isDefault ? -1 : 1;
      }
      return left.displayName.localeCompare(right.displayName);
    });
    return defaultFirst;
  }, [spaces]);

  const defaultSpace = orderedSpaces.find((space) => space.isDefault);
  const cloneSource = orderedSpaces.find((space) => space.spaceId === cloneSourceId) ?? null;
  const busy = Boolean(busyKey);

  useEffect(() => {
    if (!open) {
      return;
    }
    setComposeMode(null);
    setComposeName('');
    setCloneSourceId(null);
    const frame = window.requestAnimationFrame(() => closeButtonRef.current?.focus());
    return () => window.cancelAnimationFrame(frame);
  }, [open]);

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) {
        event.preventDefault();
        onOpenChange(false);
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [busy, onOpenChange, open]);

  useEffect(() => {
    if (composeMode) {
      nameInputRef.current?.focus();
    }
  }, [composeMode]);

  if (!open || typeof document === 'undefined') {
    return null;
  }

  const startCreate = () => {
    setCloneSourceId(null);
    setComposeMode('create');
    setComposeName('');
  };

  const startClone = (source: MemoryPickerSpace) => {
    setCloneSourceId(source.spaceId);
    setComposeMode('clone');
    setComposeName(`${source.displayName} ${translate('memory.commons.pickerCloneSuffix')}`.trim());
  };

  const submitCompose = () => {
    const displayName = composeName.trim();
    if (!displayName || busy) {
      return;
    }
    if (composeMode === 'clone' && cloneSource) {
      void onClone({ sourceSpaceId: cloneSource.spaceId, displayName });
      return;
    }
    void onCreate({ displayName });
  };

  const composeLabel = composeMode === 'clone'
    ? translate('memory.commons.pickerCloneNameLabel')
    : translate('memory.commons.pickerNewNameLabel');
  const composePlaceholder = composeMode === 'clone'
    ? translate('memory.commons.pickerCloneNamePlaceholder')
    : translate('memory.commons.pickerNewNamePlaceholder');
  const composeSubmit = composeMode === 'clone'
    ? translate('memory.commons.pickerConfirmClone')
    : translate('memory.commons.pickerConfirmCreate');

  return createPortal(
    <div
      className="sdkwork-memory-picker-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !busy) {
          onOpenChange(false);
        }
      }}
    >
      <div
        aria-label={translate('memory.commons.pickerTitle')}
        aria-modal="true"
        className="sdkwork-memory-picker"
        role="dialog"
      >
        <header className="sdkwork-memory-picker-header">
          <div>
            <h2>{translate('memory.commons.pickerTitle')}</h2>
            <p>{translate('memory.commons.pickerDescription')}</p>
          </div>
          <button
            ref={closeButtonRef}
            aria-label={translate('memory.commons.close')}
            className="sdkwork-memory-picker-close"
            disabled={busy}
            onClick={() => onOpenChange(false)}
            title={translate('memory.commons.close')}
            type="button"
          >
            <X aria-hidden="true" size={15} />
          </button>
        </header>

        <div className="sdkwork-memory-picker-body">
          {loading ? (
            <p className="sdkwork-memory-picker-state" role="status">
              {translate('memory.commons.pickerLoading')}
            </p>
          ) : errorMessage ? (
            <div className="sdkwork-memory-picker-state" data-tone="danger" role="alert">
              <p>{errorMessage}</p>
              {onRetry ? (
                <button
                  className="sdkwork-memory-picker-button"
                  onClick={onRetry}
                  type="button"
                >
                  {translate('memory.commons.pickerRetry')}
                </button>
              ) : null}
            </div>
          ) : orderedSpaces.length === 0 ? (
            <p className="sdkwork-memory-picker-state" role="status">
              {translate('memory.commons.pickerEmpty')}
            </p>
          ) : (
            orderedSpaces.map((space) => {
              const attached = space.spaceId === attachedSpaceId;
              const rowBusy = busyKey === space.spaceId;
              return (
                <div
                  className="sdkwork-memory-picker-row"
                  data-attached={attached ? 'true' : 'false'}
                  key={space.spaceId}
                >
                  <span aria-hidden="true" className="sdkwork-memory-picker-row-icon">
                    {rowBusy ? <Loader2 className="sdkwork-memory-picker-spin" size={15} /> : <Brain size={15} />}
                  </span>
                  <div className="sdkwork-memory-picker-row-copy">
                    <div className="sdkwork-memory-picker-row-name">
                      <strong title={space.displayName}>{space.displayName}</strong>
                      {space.isDefault ? (
                        <span className="sdkwork-memory-picker-tag">
                          {translate('memory.commons.pickerDefaultTag')}
                        </span>
                      ) : null}
                      {attached ? (
                        <span className="sdkwork-memory-picker-tag" data-tone="muted">
                          <Check aria-hidden="true" size={10} />
                          {translate('memory.commons.pickerAttachedTag')}
                        </span>
                      ) : null}
                    </div>
                    <div className="sdkwork-memory-picker-row-meta">
                      {typeof space.recordCount === 'number'
                        ? `${space.recordCount} ${translate('memory.commons.pickerRecordSuffix')}`
                        : space.spaceType}
                    </div>
                  </div>
                  <div className="sdkwork-memory-picker-actions">
                    <button
                      className="sdkwork-memory-picker-button"
                      data-variant={attached ? undefined : 'primary'}
                      disabled={busy || attached}
                      onClick={() => void onAttach(space.spaceId)}
                      type="button"
                    >
                      {attached
                        ? translate('memory.commons.pickerAttachedTag')
                        : translate('memory.commons.pickerAttach')}
                    </button>
                    <button
                      className="sdkwork-memory-picker-button"
                      disabled={busy}
                      onClick={() => startClone(space)}
                      type="button"
                    >
                      <Copy aria-hidden="true" size={11} />
                      {translate('memory.commons.pickerCloneAndAttach')}
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>

        {composeMode ? (
          <div className="sdkwork-memory-picker-compose">
            <label htmlFor="sdkwork-memory-picker-name">{composeLabel}</label>
            <input
              ref={nameInputRef}
              disabled={busy}
              id="sdkwork-memory-picker-name"
              onChange={(event) => setComposeName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault();
                  submitCompose();
                }
              }}
              placeholder={composePlaceholder}
              value={composeName}
            />
            <button
              className="sdkwork-memory-picker-button"
              data-variant="primary"
              disabled={busy || composeName.trim().length === 0}
              onClick={submitCompose}
              type="button"
            >
              {busyKey === CREATE_BUSY_KEY ? <Loader2 aria-hidden="true" size={11} /> : null}
              {composeSubmit}
            </button>
            <button
              className="sdkwork-memory-picker-button"
              disabled={busy}
              onClick={() => setComposeMode(null)}
              type="button"
            >
              {translate('memory.commons.cancel')}
            </button>
          </div>
        ) : (
          <div className="sdkwork-memory-picker-compose">
            <button
              className="sdkwork-memory-picker-button"
              disabled={busy}
              onClick={startCreate}
              type="button"
            >
              <Plus aria-hidden="true" size={11} />
              {translate('memory.commons.pickerNew')}
            </button>
          </div>
        )}

        <p className="sdkwork-memory-picker-compose-hint">
          {defaultSpace
            ? translate('memory.commons.pickerDefaultHint')
            : translate('memory.commons.pickerNoDefaultHint')}
        </p>
      </div>
    </div>,
    document.body,
  );
}

/**
 * Copy lookup that works with and without the Memory i18n provider.
 *
 * A Memory console mounts `MemoryI18nProvider`, and then the picker speaks
 * exactly the console's language. A host that only embeds this block (the Agents
 * console today) has its own i18n and will not mount the provider, so the picker
 * falls back to the package's own catalogs resolved from the browser language.
 * Rendering raw keys in a host would be worse than picking a sensible language.
 */
function useMemoryPickerTranslate(): (key: string) => string {
  const context = useContext(MemoryI18nContext);
  const fallback = useMemo(() => resolveFallbackCatalog(), []);
  return (key: string) => context?.translate(key) ?? fallback[key] ?? key;
}

function resolveFallbackCatalog(): MemoryMessageCatalog {
  const languages = typeof navigator === 'undefined'
    ? []
    : [navigator.language, ...(navigator.languages ?? [])].filter(Boolean);
  for (const language of languages) {
    const matched = MEMORY_SUPPORTED_LOCALES.find(
      (locale) => locale.toLowerCase() === language.toLowerCase(),
    );
    if (matched) {
      return MEMORY_COMMONS_CATALOGS[matched];
    }
    const prefix = language.slice(0, 2).toLowerCase();
    const prefixed = MEMORY_SUPPORTED_LOCALES.find(
      (locale) => locale.slice(0, 2).toLowerCase() === prefix,
    );
    if (prefixed) {
      return MEMORY_COMMONS_CATALOGS[prefixed];
    }
  }
  return MEMORY_FALLBACK_CATALOG;
}
