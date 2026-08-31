import type { Descriptor, JsonOp, PlainJson } from '../api/types';

/** One row of the action log: what was attempted, what was sent, what came back. */
export interface LogEntry {
  id: number;
  ts: number;
  description: string;
  ops: JsonOp[];
  /** ok = all applied; refused = the node answered success:false; error = HTTP/network failure. */
  outcome: 'ok' | 'refused' | 'error';
  detail?: string;
}

export type ConnectionStatus = 'idle' | 'connecting' | 'connected' | 'error';

export interface AppState {
  connection: {
    url: string;
    status: ConnectionStatus;
    replicaId: string | null;
    error: string | null;
  };
  metamodel: Descriptor | null;
  metamodelSource: 'node' | 'file' | null;
  /** Decoded document; null = "Unset" (fresh replica) or not yet fetched. */
  doc: PlainJson;
  lastSyncAt: number | null;
  pendingOps: number;
  banner: string | null;
  log: LogEntry[];
}

export const MAX_LOG_ENTRIES = 500;

export function initialState(url: string): AppState {
  return {
    connection: { url, status: 'idle', replicaId: null, error: null },
    metamodel: null,
    metamodelSource: null,
    doc: null,
    lastSyncAt: null,
    pendingOps: 0,
    banner: null,
    log: [],
  };
}

export type Action =
  | { type: 'set-url'; url: string }
  | { type: 'connecting' }
  | { type: 'connected'; replicaId: string }
  | { type: 'connect-error'; error: string }
  | { type: 'disconnected' }
  | { type: 'metamodel'; descriptor: Descriptor | null; source: 'node' | 'file' | null }
  | { type: 'state'; doc: PlainJson; ts: number }
  | { type: 'pending'; count: number }
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
      return {
        ...state,
        connection: { ...state.connection, status: 'idle', replicaId: null, error: null },
        doc: null,
        lastSyncAt: null,
        pendingOps: 0,
        metamodel: state.metamodelSource === 'node' ? null : state.metamodel,
        metamodelSource: state.metamodelSource === 'node' ? null : state.metamodelSource,
      };
    case 'metamodel':
      return { ...state, metamodel: action.descriptor, metamodelSource: action.source };
    case 'state':
      return { ...state, doc: action.doc, lastSyncAt: action.ts };
    case 'pending':
      return { ...state, pendingOps: action.count };
    case 'log': {
      const log = [...state.log, action.entry];
      if (log.length > MAX_LOG_ENTRIES) log.splice(0, log.length - MAX_LOG_ENTRIES);
      return { ...state, log };
    }
    case 'banner':
      return { ...state, banner: action.message };
  }
}
