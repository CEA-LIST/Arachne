/**
 * The editor's state: one connection to a node, the models it hosts, and the
 * models open in tabs.
 *
 * A model is its log, so every open model is one `ModelTab` keyed by its
 * `ModelId`, holding its own document, descriptor, binding verdict, store
 * outcome and poll clock; the node's routes for it are `/api/model/{id}/…`.
 * The default log the unscoped routes serve is not special here: it is
 * listed by `GET /api/models` with no metamodel and opens like any other.
 * The tab's live machinery (its poll, its `OpQueue`, its field registry)
 * lives in `sync/modelSession.ts`; this reducer holds only what the view
 * renders, and a `tab` patch for an id that is not open is dropped, which is
 * how a session's late event after its tab was closed cannot resurrect it.
 */

import type { Descriptor, HostedModel, MetamodelListing, ModelId, PlainJson } from '../api/types';
import type { EditOp } from '../crdt/ops';
import type { Binding } from '../model/binding';
import type { Diagnostic } from '../model/conformance';
import type { Projection } from '../model/projection';
import { FieldRegistry } from '../sync/fieldRegistry';

/** One row of the action log: what was attempted, what was sent, what came back. */
export interface LogEntry {
  id: number;
  ts: number;
  description: string;
  ops: EditOp[];
  /**
   * ok = all applied; refused = the node answered success:false; error =
   * HTTP/network failure; diagnostic = the converged document violates an
   * invariant of its descriptor (model/conformance.ts), which is a report
   * and not a failure of anything sent.
   */
  outcome: 'ok' | 'refused' | 'error' | 'diagnostic';
  detail?: string;
  /** The model the row is about; absent for a row about the connection or the node. */
  modelId?: ModelId;
}

export type ConnectionStatus = 'idle' | 'connecting' | 'connected' | 'error';

/** One open model: what its tab renders. */
export interface ModelTab {
  id: ModelId;
  /** The node the tab was opened against; its routes are addressed there whatever the URL field says later. */
  nodeUrl: string;
  /** opening: no state applied yet; open: the first fetch landed; failed: it could not (an id the node does not host, a network failure). */
  status: 'opening' | 'open' | 'failed';
  /** Why the tab failed to open. */
  error: string | null;
  metamodel: Descriptor | null;
  metamodelSource: 'node' | 'file' | null;
  /** Decoded document; null = "Unset" (fresh model), not yet fetched, or refused by the binding check. */
  doc: PlainJson;
  /** The binding check's last verdict on the document (model/binding.ts); null before the first apply. */
  binding: Binding | null;
  /** The model store's last outcome after an apply (model/projection.ts); null before the first apply. */
  projection: Projection | null;
  /**
   * The invariants the applied document violates (model/conformance.ts),
   * evaluated after every bound apply: shown beside the binding verdict and
   * under the tree, never a hold on the edit gate. Empty until an apply
   * answers `bound`, and after one that refuses.
   */
  diagnostics: Diagnostic[];
  lastSyncAt: number | null;
  pendingOps: number;
  /**
   * The typing overlay for this tab's fields (sync/fieldRegistry.ts), per
   * tab so one tab's caret never writes into another's document. Created
   * with the tab and shared with its session; identity, not data.
   */
  registry: FieldRegistry;
}

/** What a session may change on its tab: everything but its identity. */
export type TabPatch = Partial<Omit<ModelTab, 'id' | 'nodeUrl' | 'registry'>>;

export interface AppState {
  connection: {
    url: string;
    status: ConnectionStatus;
    replicaId: string | null;
    error: string | null;
  };
  /** GET /api/models on the connected node, as last listed; null before the first listing. */
  hosted: HostedModel[] | null;
  /** GET /api/metamodels on the connected node: what a new model can be bound to. */
  metamodels: MetamodelListing[];
  /** The open models, keyed by id. */
  models: Record<ModelId, ModelTab>;
  /** The open models in tab order, oldest first. */
  tabs: ModelId[];
  /** The tab the panels show; null when none is open. */
  selected: ModelId | null;
  banner: string | null;
  log: LogEntry[];
}

export const MAX_LOG_ENTRIES = 500;

export function initialState(url: string): AppState {
  return {
    connection: { url, status: 'idle', replicaId: null, error: null },
    hosted: null,
    metamodels: [],
    models: {},
    tabs: [],
    selected: null,
    banner: null,
    log: [],
  };
}

/** A tab as it is the moment it opens: nothing fetched yet. */
export function newTab(id: ModelId, nodeUrl: string, registry: FieldRegistry = new FieldRegistry()): ModelTab {
  return {
    id,
    nodeUrl,
    registry,
    status: 'opening',
    error: null,
    metamodel: null,
    metamodelSource: null,
    doc: null,
    binding: null,
    projection: null,
    diagnostics: [],
    lastSyncAt: null,
    pendingOps: 0,
  };
}

/** The selected tab, or null when none is open. */
export function selectedTab(state: AppState): ModelTab | null {
  return state.selected === null ? null : (state.models[state.selected] ?? null);
}

export type Action =
  | { type: 'set-url'; url: string }
  | { type: 'connecting' }
  | { type: 'connected'; replicaId: string }
  | { type: 'connect-error'; error: string }
  | { type: 'disconnected' }
  | { type: 'hosted'; models: HostedModel[] }
  | { type: 'metamodels'; listing: MetamodelListing[] }
  /** Open a tab for the model; an id already open is selected and nothing else changes. */
  | { type: 'open'; id: ModelId; nodeUrl: string; registry?: FieldRegistry }
  | { type: 'select'; id: ModelId }
  | { type: 'close'; id: ModelId }
  /** A session's change to its tab; dropped when the tab is not open. */
  | { type: 'tab'; id: ModelId; patch: TabPatch }
  | { type: 'log'; entry: LogEntry }
  | { type: 'banner'; message: string | null };

export function reducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case 'set-url':
      return { ...state, connection: { ...state.connection, url: action.url } };
    case 'connecting':
      return {
        ...state,
        connection: { ...state.connection, status: 'connecting', replicaId: null, error: null },
        banner: null,
      };
    case 'connected':
      return {
        ...state,
        connection: { ...state.connection, status: 'connected', replicaId: action.replicaId, error: null },
      };
    case 'connect-error':
      return {
        ...state,
        connection: { ...state.connection, status: 'error', error: action.error },
      };
    case 'disconnected':
      // Every tab was opened against this connection, so every tab closes with it.
      return {
        ...state,
        connection: { ...state.connection, status: 'idle', replicaId: null, error: null },
        hosted: null,
        metamodels: [],
        models: {},
        tabs: [],
        selected: null,
      };
    case 'hosted':
      return { ...state, hosted: action.models };
    case 'metamodels':
      return { ...state, metamodels: action.listing };
    case 'open': {
      if (action.id in state.models) return { ...state, selected: action.id };
      return {
        ...state,
        models: { ...state.models, [action.id]: newTab(action.id, action.nodeUrl, action.registry) },
        tabs: [...state.tabs, action.id],
        selected: action.id,
      };
    }
    case 'select':
      return action.id in state.models ? { ...state, selected: action.id } : state;
    case 'close': {
      if (!(action.id in state.models)) return state;
      const index = state.tabs.indexOf(action.id);
      const tabs = state.tabs.filter((id) => id !== action.id);
      const models = { ...state.models };
      delete models[action.id];
      // The neighbour to the right takes the selection, else the one to the left, else nothing.
      const selected =
        state.selected !== action.id ? state.selected : (tabs[Math.min(index, tabs.length - 1)] ?? null);
      return { ...state, models, tabs, selected };
    }
    case 'tab': {
      const tab = state.models[action.id];
      if (tab === undefined) return state;
      return { ...state, models: { ...state.models, [action.id]: { ...tab, ...action.patch } } };
    }
    case 'log': {
      const log = [...state.log, action.entry];
      if (log.length > MAX_LOG_ENTRIES) log.splice(0, log.length - MAX_LOG_ENTRIES);
      return { ...state, log };
    }
    case 'banner':
      return { ...state, banner: action.message };
  }
}
