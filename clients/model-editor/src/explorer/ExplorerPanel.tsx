/**
 * The left panel: Models / Model / Metamodel tabs, a filter toolbar, and the
 * tree. The Models tab is the node's hosted list with create and join; the
 * other two are the selected document tab's.
 *
 * Every state this panel can be in instructs — not connected, no model open,
 * no metamodel, empty document, no filter match — so the left column is never
 * simply blank.
 */

import { useMemo, type RefObject } from 'react';
import type {
  ContainmentDesc,
  Descriptor,
  HostedModel,
  MetamodelId,
  MetamodelListing,
  ModelId,
  Path,
  PlainJson,
} from '../api/types';
import { EmptyState } from '../common/EmptyState';
import { pathText, type Diagnostic } from '../model/conformance';
import { buildTree, rootCandidates, type ModelNode } from '../model/instance';
import { AddControl } from '../properties/AddControl';
import { countElements, flattenTree } from '../ui/flattenTree';
import { Box, FileWarning, Layers, ListCollapse, Search, X } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { Tabs, type TabSpec } from '../ui/Tabs';
import { MetamodelBrowser } from './MetamodelBrowser';
import { ModelsPanel } from './ModelsPanel';
import { ModelTree } from './ModelTree';

export type ExplorerTab = 'models' | 'model' | 'metamodel';

const TABS: readonly TabSpec<ExplorerTab>[] = [
  { id: 'models', label: 'Models' },
  { id: 'model', label: 'Model' },
  { id: 'metamodel', label: 'Metamodel' },
];

interface ExplorerPanelProps {
  tab: ExplorerTab;
  setTab: (tab: ExplorerTab) => void;
  /** The node's hosted list and descriptors, and the actions of the Models tab. */
  hosted: HostedModel[] | null;
  seen: readonly ModelId[];
  metamodels: MetamodelListing[];
  openIds: readonly ModelId[];
  selectedModel: ModelId | null;
  onRefreshModels: () => void;
  onCreateModel: (metamodelId: MetamodelId) => void;
  onJoinModel: (id: ModelId, metamodelId: MetamodelId) => void;
  onOpenModel: (id: ModelId) => void;
  descriptor: Descriptor | null;
  metamodelSource: 'node' | 'file' | null;
  loadDescriptorFile: (descriptor: Descriptor) => void;
  doc: PlainJson;
  connected: boolean;
  /** The binding check's refusal when the document was not applied (model/binding.ts); null otherwise. */
  refusal: string | null;
  /** The invariants the applied document violates (model/conformance.ts), listed under the tree. */
  diagnostics: Diagnostic[];
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
  /** False while the edit gate holds structural edits (ui/editGate.ts). */
  structureEnabled: boolean;
  structureReason: string | null;
  onActivate: () => void;
  onRename: () => void;
  onRowCount: (count: number) => void;
  visibleRows: number;
}

export function ExplorerPanel({
  tab,
  setTab,
  hosted,
  seen,
  metamodels,
  openIds,
  selectedModel,
  onRefreshModels,
  onCreateModel,
  onJoinModel,
  onOpenModel,
  descriptor,
  metamodelSource,
  loadDescriptorFile,
  doc,
  connected,
  refusal,
  diagnostics,
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
  structureEnabled,
  structureReason,
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

      {tab === 'models' ? (
        <div className="me-panel__body" id="panel-models" role="tabpanel" aria-labelledby="tab-models">
          <ModelsPanel
            connected={connected}
            hosted={hosted}
            seen={seen}
            metamodels={metamodels}
            openIds={openIds}
            selected={selectedModel}
            onRefresh={onRefreshModels}
            onCreate={onCreateModel}
            onJoin={onJoinModel}
            onOpen={onOpenModel}
          />
        </div>
      ) : tab === 'metamodel' ? (
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
          {/* A toolbar of disabled controls is the first thing a cold start
              should NOT show: there is nothing to filter or collapse yet. */}
          {tree !== null && (
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
              onClick={collapseAll}
            >
              <ListCollapse {...ICON} aria-hidden="true" />
            </button>
            <span className="me-subtle me-num me-panel__count">
              {countElements(tree)} element{countElements(tree) === 1 ? '' : 's'}
            </span>
          </div>
          )}

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
            ) : selectedModel === null ? (
              <EmptyState
                icon={Layers}
                title="No model open"
                body="Open one of the models this node hosts, create a new one, or join one by id."
              >
                <button type="button" className="me-btn me-btn--primary" onClick={() => setTab('models')}>
                  Open Models tab
                </button>
              </EmptyState>
            ) : refusal !== null ? (
              <EmptyState
                icon={FileWarning}
                title="Model not applied"
                body={refusal}
                tone="warn"
              >
                <button type="button" className="me-btn" onClick={() => setTab('metamodel')}>
                  Open Metamodel tab
                </button>
              </EmptyState>
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
                  disabled={!structureEnabled}
                  disabledReason={structureReason}
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

            {connected && selectedModel !== null && descriptor !== null && tree !== null && (
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
                structureEnabled={structureEnabled}
                structureReason={structureReason}
                onActivate={onActivate}
                onRename={onRename}
                onRowCount={onRowCount}
              />
            )}

            {connected && selectedModel !== null && diagnostics.length > 0 && (
              <ul className="me-diagnostics" aria-label="Conformance diagnostics" data-testid="diagnostics-list">
                {diagnostics.map((diagnostic, index) => (
                  <li key={index} className="me-diagnostics__item">
                    <button
                      type="button"
                      className="me-diagnostics__path me-mono"
                      title="Select the element"
                      onClick={() => onSelectPath(diagnostic.path)}
                    >
                      {pathText(diagnostic.path)}
                    </button>
                    <span className="me-chip me-chip--warn">{diagnostic.rule}</span>
                    <span className="me-diagnostics__message">{diagnostic.message}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}

    </section>
  );
}
