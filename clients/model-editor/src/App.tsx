/**
 * The app shell: top bar, explorer, properties, console — a modelling IDE
 * layout in the EMF/Theia tradition.
 *
 * Everything below the UI is untouched. Every new piece of state here is
 * either view-local (which panel is open, what is collapsed) or DERIVED from
 * what the store already carries, so the reducer, the sync engine and their
 * tests needed no changes at all.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import './App.css';
import type { ContainmentDesc, Path } from './api/types';
import { ConsolePanel, type ConsoleTab } from './console/ConsolePanel';
import { exportLog } from './console/exportLog';
import { addChildOps, createRootOps, createSingleContainmentOps, removeFromArrayOps } from './crdt/ops';
import { getAtPath } from './crdt/path';
import { ExplorerPanel, type ExplorerTab } from './explorer/ExplorerPanel';
import { isPresent } from './model/instance';
import { PropertiesPanel } from './properties/PropertiesPanel';
import { AlertDock } from './shell/AlertDock';
import { TopBar } from './shell/TopBar';
import { useSync } from './sync/useSync';
import { Keyboard, X } from './ui/icons';
import { ICON } from './ui/iconProps';
import { Popover } from './ui/Popover';
import { Resizer } from './ui/Resizer';
import { syncView } from './ui/syncState';
import { useNow } from './ui/useNow';
import { usePanelSize } from './ui/usePanelSize';

const EXPLORER_MIN = 240;
const EXPLORER_MAX = 420;
const CONSOLE_MIN = 140;

const SHORTCUTS: readonly [string, string][] = [
  ['↑ ↓', 'Move through the tree'],
  ['→ ←', 'Expand / collapse, or step in and out'],
  ['Home / End', 'First / last visible row'],
  ['Enter', 'Select and jump into the properties form'],
  ['Space', 'Select without leaving the tree'],
  ['a–z', 'Type-ahead to a row by name'],
  ['*', 'Expand every sibling at this level'],
  ['F2', "Jump to the element's id field"],
  ['Delete', 'Remove the selected element (array children)'],
  ['Esc', 'Revert the focused field to the last synced value'],
  ['⌘/Ctrl + K', 'Focus the tree filter'],
  ['⌘/Ctrl + J', 'Toggle the console'],
  ['⌘/Ctrl + ⇧ + E', 'Export the action log'],
  ['?', 'This help'],
];

export default function App() {
  const sync = useSync();
  const { state } = sync;
  const connected = state.connection.status === 'connected';
  const now = useNow();
  const view = useMemo(
    () => syncView(state, sync.pollMs, now),
    [state, sync.pollMs, now],
  );

  const [selectedPath, setSelectedPath] = useState<Path>([]);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set());
  const [explorerTab, setExplorerTab] = useState<ExplorerTab>('model');
  const [filter, setFilter] = useState('');
  const [visibleRows, setVisibleRows] = useState(0);
  const [consoleOpen, setConsoleOpen] = useState(false);
  const [consoleTab, setConsoleTab] = useState<ConsoleTab>('log');
  const [failuresOnly, setFailuresOnly] = useState(false);
  const [connectOpen, setConnectOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);

  const [explorerWidth, setExplorerWidth] = usePanelSize(
    'model-editor.explorer-w',
    288,
    EXPLORER_MIN,
    EXPLORER_MAX,
  );
  const consoleMax = Math.max(CONSOLE_MIN, Math.round(window.innerHeight * 0.5));
  const [consoleHeight, setConsoleHeight] = usePanelSize(
    'model-editor.console-h',
    260,
    CONSOLE_MIN,
    consoleMax,
  );

  const filterRef = useRef<HTMLInputElement | null>(null);
  const formRef = useRef<HTMLDivElement | null>(null);
  const idInputRef = useRef<HTMLInputElement | null>(null);
  const consoleToggleRef = useRef<HTMLButtonElement | null>(null);

  // A connection failure is the one case where the setup controls should come
  // to the user rather than wait to be found. Derived from the status
  // transition rather than an effect, so it cannot cascade a second render.
  const [lastStatus, setLastStatus] = useState(state.connection.status);
  if (lastStatus !== state.connection.status) {
    setLastStatus(state.connection.status);
    if (state.connection.status === 'error') setConnectOpen(true);
  }

  // If the selected element vanished (removed, reordered away), fall back to
  // the closest present ancestor — derived at render time, no effect needed.
  let effectivePath = selectedPath;
  while (effectivePath.length > 0 && !isPresent(getAtPath(state.doc, effectivePath))) {
    effectivePath = effectivePath.slice(0, -1);
  }

  const onCreateRoot = useCallback(
    (className: string) => {
      void sync.sendOps(`create root ${className}`, createRootOps(className), {
        path: [],
        value: { eClass: className },
      });
    },
    [sync],
  );

  const onAddChild = useCallback(
    (elementPath: Path, feature: ContainmentDesc, className: string) => {
      const label = `${feature.name}`;
      if (feature.many) {
        const arrayPath = [...elementPath, feature.name];
        const raw = getAtPath(state.doc, arrayPath);
        const children = Array.isArray(raw) ? raw : [];
        void sync.sendOps(
          `add ${className} to ${label}[${children.length}]`,
          addChildOps(arrayPath, children.length, className),
          { path: arrayPath, value: [...children, { eClass: className }] },
        );
      } else {
        void sync.sendOps(
          `create ${className} in ${label}`,
          createSingleContainmentOps(elementPath, feature.name, className),
          { path: [...elementPath, feature.name], value: { eClass: className } },
        );
      }
    },
    [sync, state.doc],
  );

  const onRemoveElement = useCallback(
    (path: Path) => {
      const index = path[path.length - 1];
      if (typeof index !== 'number') return;
      const arrayPath = path.slice(0, -1);
      const siblings = getAtPath(state.doc, arrayPath);
      setSelectedPath(arrayPath.slice(0, -1));
      void sync.sendOps(
        `remove element at /${path.join('/')}`,
        removeFromArrayOps(arrayPath, index),
        Array.isArray(siblings)
          ? { path: arrayPath, value: siblings.filter((_, i) => i !== index) }
          : undefined,
      );
    },
    [sync, state.doc],
  );

  const focusForm = useCallback(() => {
    const first = formRef.current?.querySelector<HTMLElement>(
      'input:not([disabled]), select:not([disabled]), button:not([disabled])',
    );
    first?.focus();
  }, []);

  // Global shortcuts. Nothing here overrides a browser default a participant
  // would miss, and none of them fire while a text field has focus except the
  // explicitly modified ones.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey;
      const target = event.target as HTMLElement | null;
      const typing =
        target !== null && (target.tagName === 'INPUT' || target.tagName === 'SELECT');

      if (mod && event.shiftKey && event.key.toLowerCase() === 'e') {
        event.preventDefault();
        exportLog(state.log);
        return;
      }
      if (mod && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setExplorerTab('model');
        filterRef.current?.focus();
        return;
      }
      if (mod && event.key.toLowerCase() === 'j') {
        event.preventDefault();
        setConsoleOpen((open) => !open);
        return;
      }
      if (event.key === '?' && !typing) {
        event.preventDefault();
        setHelpOpen(true);
        return;
      }
      if (event.key === 'Escape') {
        if (helpOpen) setHelpOpen(false);
        else if (state.banner !== null) sync.clearBanner();
      }
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [state.log, state.banner, helpOpen, sync]);

  return (
    <div
      className="me-app"
      style={{ ['--explorer-w' as string]: `${explorerWidth}px` }}
    >
      <TopBar
        state={state}
        pollMs={sync.pollMs}
        setPollMs={sync.setPollMs}
        setUrl={sync.setUrl}
        connect={() => void sync.connect()}
        disconnect={sync.disconnect}
        view={view}
        connectOpen={connectOpen}
        setConnectOpen={setConnectOpen}
        onShowHelp={() => setHelpOpen(true)}
      />

      <div className="me-app__alerts">
        <AlertDock
          message={state.banner}
          onDismiss={sync.clearBanner}
          onShowDetails={() => {
            setConsoleOpen(true);
            setConsoleTab('log');
            setFailuresOnly(true);
            sync.clearBanner();
            consoleToggleRef.current?.focus();
          }}
        />
      </div>

      <ExplorerPanel
        tab={explorerTab}
        setTab={setExplorerTab}
        descriptor={state.metamodel}
        metamodelSource={state.metamodelSource}
        loadDescriptorFile={sync.loadDescriptorFile}
        doc={state.doc}
        connected={connected}
        collapsed={collapsed}
        setCollapsed={setCollapsed}
        selectedPath={effectivePath}
        onSelectPath={setSelectedPath}
        filter={filter}
        setFilter={setFilter}
        filterRef={filterRef}
        onAddChild={onAddChild}
        onRemove={onRemoveElement}
        onCreateRoot={onCreateRoot}
        onActivate={focusForm}
        onRename={() => idInputRef.current?.focus()}
        onRowCount={setVisibleRows}
        visibleRows={visibleRows}
      />

      <Resizer
        orientation="vertical"
        value={explorerWidth}
        min={EXPLORER_MIN}
        max={EXPLORER_MAX}
        onChange={setExplorerWidth}
        label="Resize explorer"
      />

      <PropertiesPanel
        descriptor={state.metamodel}
        doc={state.doc}
        connected={connected}
        path={effectivePath}
        registry={sync.registry}
        sendOps={sync.sendOps}
        onSelectPath={setSelectedPath}
        formRef={formRef}
        idInputRef={idInputRef}
        staleNotice={view.kind === 'offline' ? view.detail : null}
        onOpenConnect={() => setConnectOpen(true)}
      />

      <ConsolePanel
        log={state.log}
        doc={state.doc}
        connected={connected}
        open={consoleOpen}
        setOpen={setConsoleOpen}
        tab={consoleTab}
        setTab={setConsoleTab}
        height={consoleHeight}
        setHeight={setConsoleHeight}
        minHeight={CONSOLE_MIN}
        maxHeight={consoleMax}
        failuresOnly={failuresOnly}
        setFailuresOnly={setFailuresOnly}
        toggleRef={consoleToggleRef}
      />

      {helpOpen && (
        <div className="me-help me-noprint">
          <Popover open={helpOpen} onClose={() => setHelpOpen(false)} align="left" label="Keyboard shortcuts">
            <div className="me-help__head">
              <Keyboard {...ICON} aria-hidden="true" />
              <strong>Keyboard</strong>
              <button
                type="button"
                className="me-iconbtn"
                aria-label="Close"
                onClick={() => setHelpOpen(false)}
              >
                <X {...ICON} size={14} aria-hidden="true" />
              </button>
            </div>
            <dl className="me-help__list">
              {SHORTCUTS.map(([keys, description]) => (
                <div key={keys} className="me-help__row">
                  <dt>
                    <kbd>{keys}</kbd>
                  </dt>
                  <dd>{description}</dd>
                </div>
              ))}
            </dl>
          </Popover>
        </div>
      )}
    </div>
  );
}
