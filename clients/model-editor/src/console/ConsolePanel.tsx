/**
 * The bottom console: collapsed to a single summary bar, or open on the action
 * log / document JSON. Collapsed it still tells the truth — entry count, the
 * newest line, and a danger dot when the newest entry failed — so a failure is
 * never hidden behind a closed panel.
 */

import type { RefObject } from 'react';
import type { PlainJson } from '../api/types';
import type { LogEntry } from '../state/store';
import { Braces, ChevronDown, ChevronRight, Terminal } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { Resizer } from '../ui/Resizer';
import { Tabs, type TabSpec } from '../ui/Tabs';
import { ActionLogView } from './ActionLogView';
import { DocumentJsonView } from './DocumentJsonView';

export type ConsoleTab = 'log' | 'json';

const TABS: readonly TabSpec<ConsoleTab>[] = [
  { id: 'log', label: 'Action log' },
  { id: 'json', label: 'Document JSON' },
];

interface ConsolePanelProps {
  log: LogEntry[];
  doc: PlainJson;
  connected: boolean;
  open: boolean;
  setOpen: (open: boolean) => void;
  tab: ConsoleTab;
  setTab: (tab: ConsoleTab) => void;
  height: number;
  setHeight: (height: number) => void;
  minHeight: number;
  maxHeight: number;
  failuresOnly: boolean;
  setFailuresOnly: (value: boolean) => void;
  toggleRef: RefObject<HTMLButtonElement | null>;
}

export function ConsolePanel({
  log,
  doc,
  connected,
  open,
  setOpen,
  tab,
  setTab,
  height,
  setHeight,
  minHeight,
  maxHeight,
  failuresOnly,
  setFailuresOnly,
  toggleRef,
}: ConsolePanelProps) {
  const newest = log[log.length - 1];
  const failed = newest !== undefined && newest.outcome !== 'ok';

  const summary = (
    <button
      ref={toggleRef}
      type="button"
      className="me-console__bar"
      aria-expanded={open}
      aria-controls="console-body"
      onClick={() => setOpen(!open)}
    >
      <Terminal {...ICON} size={14} aria-hidden="true" />
      <span className="me-console__label">Console</span>
      <span className="me-badge me-num">{log.length}</span>
      {failed && <span className="me-dot me-dot--danger" aria-label="last operation failed" />}
      <span className="me-console__summary me-truncate">
        {newest === undefined
          ? 'no operations yet'
          : `${newest.description} — ${newest.outcome}${
              newest.detail !== undefined && newest.detail !== '' ? `: ${newest.detail}` : ''
            }`}
      </span>
      <span className="me-console__spacer" />
      <kbd className="me-panel__hint">⌘J</kbd>
      {open ? (
        <ChevronDown {...ICON} size={14} aria-hidden="true" />
      ) : (
        <ChevronRight {...ICON} size={14} aria-hidden="true" />
      )}
    </button>
  );

  if (!open) {
    return (
      <section className="me-console me-console--collapsed me-noprint" aria-label="Console">
        {summary}
      </section>
    );
  }

  return (
    <section
      className="me-console me-noprint"
      aria-label="Console"
      style={{ height: `${height}px` }}
    >
      <Resizer
        orientation="horizontal"
        value={height}
        min={minHeight}
        max={maxHeight}
        invert
        onChange={setHeight}
        label="Resize console"
      />
      {summary}
      <Tabs
        tabs={TABS}
        active={tab}
        onSelect={setTab}
        label="Console views"
        trailing={
          tab === 'json' ? (
            <span className="me-subtle me-console__hint">
              <Braces {...ICON} size={13} aria-hidden="true" />
              decoded from the wire
            </span>
          ) : undefined
        }
      />
      <div className="me-console__body" id="console-body">
        {tab === 'log' ? (
          <ActionLogView
            log={log}
            failuresOnly={failuresOnly}
            setFailuresOnly={setFailuresOnly}
          />
        ) : (
          <DocumentJsonView doc={doc} connected={connected} />
        )}
      </div>
    </section>
  );
}
