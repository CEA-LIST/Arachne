/**
 * The left panel: Model / Metamodel tabs, a filter toolbar, and the tree.
 *
 * Every state this panel can be in instructs — not connected, no metamodel,
 * empty document, no filter match — so the left column is never simply blank.
 */

import { useMemo, type RefObject } from 'react';
import type { ContainmentDesc, Descriptor, Path, PlainJson } from '../api/types';
import { EmptyState } from '../common/EmptyState';
import { buildTree, rootCandidates, type ModelNode } from '../model/instance';
import { AddControl } from '../properties/AddControl';
import { countElements, flattenTree } from '../ui/flattenTree';
import { Box, ChevronsDownUp, FileWarning, Plug, Search, X } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { Tabs, type TabSpec } from '../ui/Tabs';
import { MetamodelBrowser } from './MetamodelBrowser';
import { ModelTree } from './ModelTree';

export type ExplorerTab = 'model' | 'metamodel';

const TABS: readonly TabSpec<ExplorerTab>[] = [
  { id: 'model', label: 'Model' },
  { id: 'metamodel', label: 'Metamodel' },
];

interface ExplorerPanelProps {
  tab: ExplorerTab;
  setTab: (tab: ExplorerTab) => void;
  descriptor: Descriptor | null;
  metamodelSource: 'node' | 'file' | null;
  loadDescriptorFile: (descriptor: Descriptor) => void;
  doc: PlainJson;
  connected: boolean;
  collapsed: ReadonlySet<string>;
  setCollapsed: (update: (prev: ReadonlySet<string>) => ReadonlySet<string>) => void;
  selectedPath: Path;
  onSelectPath: (path: Path) => void;
  filter: string;
  setFilter: (filter: string) => void;
  filterRef: RefObject<HTMLInputElement | null>;
  onAddChild: (elementPath: Path, feature: ContainmentDesc, className: string) => void;
  onRemove: (path: Path) => void;
  onCreateRoot: (className: string) => void;
  onActivate: () => void;
  onRename: () => void;
  onRowCount: (count: number) => void;
  visibleRows: number;
}

export function ExplorerPanel({
  tab,
  setTab,
  descriptor,
  metamodelSource,
  loadDescriptorFile,
  doc,
  connected,
  collapsed,
  setCollapsed,
  selectedPath,
  onSelectPath,
  filter,
  setFilter,
  filterRef,
  onAddChild,
  onRemove,
  onCreateRoot,
  onActivate,
  onRename,
  onRowCount,
  visibleRows,
}: ExplorerPanelProps) {

  const tree: ModelNode | null = useMemo(
    () => (descriptor === null ? null : buildTree(descriptor, doc)),
    [descriptor, doc],
  );

  const collapseAll = () => {
    if (descriptor === null || tree === null) return;
    const keys = flattenTree(descriptor, tree, {})
      .filter((row) => row.expandable && row.level > 1)
      .map((row) => row.key);
    setCollapsed(() => new Set(keys));
  };

  return (
    <section className="me-panel me-explorer" aria-label="Model explorer">
      <Tabs tabs={TABS} active={tab} onSelect={setTab} label="Explorer views" />

      {tab === 'metamodel' ? (
        <div className="me-panel__body" id="panel-metamodel" role="tabpanel" aria-labelledby="tab-metamodel">
          <MetamodelBrowser
            metamodel={descriptor}
            source={metamodelSource}
            connected={connected}
            loadDescriptorFile={loadDescriptorFile}
          />
        </div>
      ) : (
        <>
          <div className="me-panel__toolbar me-noprint">
            <span className="me-panel__search">
              <Search {...ICON} size={14} className="me-panel__search-icon" aria-hidden="true" />
              <input
                ref={filterRef}
                className="me-input me-panel__search-input"
                type="search"
                value={filter}
                placeholder="Filter elements…"
                aria-label="Filter elements"
                disabled={tree === null}
                onChange={(e) => setFilter(e.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Escape' && filter.length > 0) {
                    event.stopPropagation();
                    setFilter('');
                  }
                }}
              />
              <kbd className="me-panel__hint">⌘K</kbd>
            </span>
            <button
              type="button"
              className="me-iconbtn"
              title="Collapse all"
              aria-label="Collapse all"
              disabled={tree === null}
              onClick={collapseAll}
            >
              <ChevronsDownUp {...ICON} size={14} aria-hidden="true" />
            </button>
            {tree !== null && (
              <span className="me-subtle me-num me-panel__count">
                {countElements(tree)} element{countElements(tree) === 1 ? '' : 's'}
              </span>
            )}
          </div>

          <div
            className="me-panel__body"
            id="panel-model"
            role="tabpanel"
            aria-labelledby="tab-model"
          >
            {!connected ? (
              <p className="me-panel__placeholder">
                Not connected — the model tree appears once a replica answers.
              </p>
            ) : descriptor === null ? (
              <EmptyState
                icon={FileWarning}
                title="No metamodel"
                body="This replica serves no descriptor, so the document cannot be typed. Load one from the Metamodel tab."
                tone="warn"
              >
                <button type="button" className="me-btn" onClick={() => setTab('metamodel')}>
                  Open Metamodel tab
                </button>
              </EmptyState>
            ) : tree === null ? (
              <EmptyState
                icon={Box}
                title="The document is empty"
                body={
                  <>
                    This replica has never had an operation applied (state <code>Unset</code>).
                    Create the model root to start.
                  </>
                }
              >
                <AddControl
                  options={rootCandidates(descriptor)}
                  verb="Create root"
                  primary
                  onAdd={onCreateRoot}
                />
              </EmptyState>
            ) : visibleRows === 0 && filter.trim().length > 0 ? (
              <EmptyState
                icon={Search}
                title="No match"
                body={<>No element matches “{filter}”.</>}
              >
                <button type="button" className="me-btn" onClick={() => setFilter('')}>
                  <X {...ICON} size={14} aria-hidden="true" />
                  Clear filter
                </button>
              </EmptyState>
            ) : null}

            {connected && descriptor !== null && tree !== null && (
              <ModelTree
                descriptor={descriptor}
                root={tree}
                doc={doc}
                collapsed={collapsed}
                setCollapsed={setCollapsed}
                selectedPath={selectedPath}
                onSelectPath={onSelectPath}
                filter={filter}
                onAddChild={onAddChild}
                onRemove={onRemove}
                onActivate={onActivate}
                onRename={onRename}
                onRowCount={onRowCount}
              />
            )}
          </div>
        </>
      )}

      {!connected && tab === 'model' && (
        <div className="me-panel__footnote">
          <Plug {...ICON} size={13} aria-hidden="true" />
          Use Connect in the top bar.
        </div>
      )}
    </section>
  );
}
