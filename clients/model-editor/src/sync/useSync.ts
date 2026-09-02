/**
 * The editor's connection to one node and the models it has open, as a hook.
 *
 * The connection is one URL and one `/api/health`; the models are the node's
 * `GET /api/models` listing, refreshed on connect and after every
 * registration, and the tabs are `ModelSession`s (modelSession.ts), one per
 * open model, each with its own poll, `OpQueue` and field registry addressed
 * to that model's routes. The hook owns the sessions and turns their events
 * into reducer actions; the panels read the selected tab. Creating a model
 * posts `{metamodel_id}` and lets the node mint the id; joining posts the id
 * a person learned out of band beside the metamodel it is bound to, which the
 * node requires and cannot infer.
 */

import { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import { ApiError, getHealth, getMetamodels, getModels, registerModel } from '../api/client';
import type { Descriptor, JsonOp, MetamodelId, ModelId, Path, PlainJson } from '../api/types';
import { openBrowserStore, type ModelStore } from '../model/store';
import { initialState, reducer, selectedTab, type AppState, type LogEntry, type ModelTab } from '../state/store';
import { FieldRegistry } from './fieldRegistry';
import { ModelSession } from './modelSession';
import type { BatchOutcome } from './opQueue';

export const DEFAULT_POLL_MS = 500;
const URL_STORAGE_KEY = 'model-editor.node-url';

function loadStoredUrl(): string {
  try {
    return localStorage.getItem(URL_STORAGE_KEY) ?? 'http://127.0.0.1:3000';
  } catch {
    return 'http://127.0.0.1:3000';
  }
}

export interface SyncApi {
  state: AppState;
  /** The tab the panels show, or null when none is open. */
  selected: ModelTab | null;
  pollMs: number;
  setPollMs: (ms: number) => void;
  /** The selected tab's typing overlay; an idle one when no tab is open. */
  registry: FieldRegistry;
  setUrl: (url: string) => void;
  connect: () => Promise<void>;
  disconnect: () => void;
  /** Re-read GET /api/models and GET /api/metamodels on the connected node. */
  refreshModels: () => Promise<void>;
  /** Create a model bound to `metamodelId` on the node, which mints its id; the new model opens in a tab. */
  createModel: (metamodelId: MetamodelId) => Promise<ModelId | null>;
  /** Join the model `id` on the node under `metamodelId`, then open it in a tab. */
  joinModel: (id: ModelId, metamodelId: MetamodelId) => Promise<boolean>;
  /** Open a hosted model in a tab (or select its tab when it is open already). */
  openModel: (id: ModelId) => void;
  closeModel: (id: ModelId) => void;
  selectModel: (id: ModelId) => void;
  /** Load a descriptor from a file for the selected tab (fallback when the node serves none). */
  loadDescriptorFile: (descriptor: Descriptor) => void;
  /**
   * Post an op batch (one edit intent) to the model `id`. Logs the attempt
   * with its outcome; refused/error outcomes also raise the error banner.
   * `optimistic` patches the local doc immediately (the poll reconciles the
   * truth). A batch that would write the model header, or any batch while
   * the binding check refuses the document, is refused before anything is
   * posted. A null id, or one with no open tab, is an error outcome.
   */
  sendOpsTo: (
    id: ModelId | null,
    description: string,
    ops: JsonOp[],
    optimistic?: { path: Path; value: PlainJson },
  ) => Promise<BatchOutcome>;
  /** `sendOpsTo` for the selected model. */
  sendOps: (
    description: string,
    ops: JsonOp[],
    optimistic?: { path: Path; value: PlainJson },
  ) => Promise<BatchOutcome>;
  clearBanner: () => void;
}

export interface SyncOptions {
  /**
   * The model store the projection is written to (model/store.ts). Absent,
   * the browser's origin-private file system; null, no store at all, which is
   * reported and never blocks an apply.
   */
  store?: ModelStore | null;
}

const IDLE_REGISTRY = new FieldRegistry();

export function useSync(options: SyncOptions = {}): SyncApi {
  const [state, dispatch] = useReducer(reducer, loadStoredUrl(), initialState);
  const [pollMs, setPollMsState] = useState(DEFAULT_POLL_MS);
  const [store] = useState<ModelStore | null>(() =>
    options.store === undefined ? openBrowserStore() : options.store,
  );
  const sessionsRef = useRef(new Map<ModelId, ModelSession>());
  const logIdRef = useRef(0);
  const pollMsRef = useRef(pollMs);

  // Refs mirroring the bits the async callbacks need without re-binding.
  const urlRef = useRef(state.connection.url);
  const selectedRef = useRef(state.selected);
  useEffect(() => {
    urlRef.current = state.connection.url;
    selectedRef.current = state.selected;
  }, [state.connection.url, state.selected]);

  const setUrl = useCallback((url: string) => dispatch({ type: 'set-url', url }), []);

  const log = useCallback((entry: Omit<LogEntry, 'id'>) => {
    dispatch({ type: 'log', entry: { ...entry, id: logIdRef.current++ } });
  }, []);

  const refreshModels = useCallback(async () => {
    const url = urlRef.current;
    try {
      dispatch({ type: 'hosted', models: await getModels(url) });
    } catch (err) {
      dispatch({ type: 'banner', message: `model list failed: ${err instanceof Error ? err.message : String(err)}` });
    }
    try {
      dispatch({ type: 'metamodels', listing: await getMetamodels(url) });
    } catch (err) {
      dispatch({
        type: 'banner',
        message: `metamodel list failed: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }, []);

  const openModel = useCallback(
    (id: ModelId) => {
      const nodeUrl = urlRef.current;
      if (sessionsRef.current.has(id)) {
        dispatch({ type: 'open', id, nodeUrl });
        return;
      }
      const registry = new FieldRegistry();
      dispatch({ type: 'open', id, nodeUrl, registry });
      const session = new ModelSession({
        id,
        nodeUrl,
        store,
        registry,
        pollMs: pollMsRef.current,
        events: {
          patch: (patch) => dispatch({ type: 'tab', id, patch }),
          log: (entry) => log({ ...entry, modelId: id }),
          banner: (message) => dispatch({ type: 'banner', message }),
        },
      });
      sessionsRef.current.set(id, session);
      session.open().catch((err: unknown) => {
        const detail = err instanceof ApiError ? err.message : String(err);
        log({ ts: Date.now(), description: `open model ${id}`, ops: [], outcome: 'error', detail, modelId: id });
        dispatch({ type: 'banner', message: `open model ${id.slice(0, 8)}…: ${detail}` });
      });
    },
    [store, log],
  );

  const closeModel = useCallback((id: ModelId) => {
    sessionsRef.current.get(id)?.close();
    sessionsRef.current.delete(id);
    dispatch({ type: 'close', id });
  }, []);

  const selectModel = useCallback((id: ModelId) => dispatch({ type: 'select', id }), []);

  const closeAll = useCallback(() => {
    for (const session of sessionsRef.current.values()) session.close();
    sessionsRef.current.clear();
  }, []);

  const connect = useCallback(async () => {
    const url = urlRef.current;
    closeAll();
    dispatch({ type: 'connecting' });
    try {
      const health = await getHealth(url);
      dispatch({ type: 'connected', replicaId: health.replicaId });
      try {
        localStorage.setItem(URL_STORAGE_KEY, url);
      } catch {
        // Storage unavailable: connection still works.
      }
      await refreshModels();
    } catch (err) {
      dispatch({
        type: 'connect-error',
        error: err instanceof ApiError ? err.message : String(err),
      });
    }
  }, [closeAll, refreshModels]);

  const disconnect = useCallback(() => {
    closeAll();
    dispatch({ type: 'disconnected' });
  }, [closeAll]);

  const register = useCallback(
    async (
      description: string,
      registration: { modelId?: ModelId; metamodelId: MetamodelId },
    ): Promise<ModelId | null> => {
      const url = urlRef.current;
      try {
        const registered = await registerModel(url, registration);
        log({
          ts: Date.now(),
          description,
          ops: [],
          outcome: 'ok',
          detail: `${registered.created ? 'created' : 'joined'} ${registered.modelId} under ${registered.metamodelId.nsURI} (digest ${registered.metamodelId.digest})`,
          modelId: registered.modelId,
        });
        await refreshModels();
        openModel(registered.modelId);
        return registered.modelId;
      } catch (err) {
        const detail = err instanceof Error ? err.message : String(err);
        log({ ts: Date.now(), description, ops: [], outcome: 'error', detail, modelId: registration.modelId });
        dispatch({ type: 'banner', message: `${description}: ${detail}` });
        return null;
      }
    },
    [log, refreshModels, openModel],
  );

  const createModel = useCallback(
    (metamodelId: MetamodelId) => register(`create model under ${metamodelId.nsURI}`, { metamodelId }),
    [register],
  );

  const joinModel = useCallback(
    async (id: ModelId, metamodelId: MetamodelId) =>
      (await register(`join model ${id}`, { modelId: id, metamodelId })) !== null,
    [register],
  );

  const setPollMs = useCallback((ms: number) => {
    pollMsRef.current = ms;
    setPollMsState(ms);
    for (const session of sessionsRef.current.values()) session.setPollMs(ms);
  }, []);

  const sendOpsTo = useCallback<SyncApi['sendOpsTo']>(async (id, description, ops, optimistic) => {
    const session = id === null ? undefined : sessionsRef.current.get(id);
    if (session === undefined) {
      const detail = id === null ? 'no model is open' : `model ${id} is not open`;
      dispatch({
        type: 'log',
        entry: { id: logIdRef.current++, ts: Date.now(), description, ops, outcome: 'error', detail },
      });
      dispatch({ type: 'banner', message: `${description}: ${detail}` });
      return { outcome: 'error', applied: 0, detail };
    }
    return session.sendOps(description, ops, optimistic);
  }, []);

  const sendOps = useCallback<SyncApi['sendOps']>(
    (description, ops, optimistic) => sendOpsTo(selectedRef.current, description, ops, optimistic),
    [sendOpsTo],
  );

  const loadDescriptorFile = useCallback((descriptor: Descriptor) => {
    const id = selectedRef.current;
    if (id === null) return;
    sessionsRef.current.get(id)?.loadDescriptorFile(descriptor);
  }, []);

  const clearBanner = useCallback(() => dispatch({ type: 'banner', message: null }), []);

  const selected = selectedTab(state);
  const registry = selected?.registry ?? IDLE_REGISTRY;

  return {
    state,
    selected,
    pollMs,
    setPollMs,
    registry,
    setUrl,
    connect,
    disconnect,
    refreshModels,
    createModel,
    joinModel,
    openModel,
    closeModel,
    selectModel,
    loadDescriptorFile,
    sendOpsTo,
    sendOps,
    clearBanner,
  };
}
