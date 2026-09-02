import { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import { ApiError, getHealth, getMetamodel, getState, postOp } from '../api/client';
import type { Descriptor, JsonOp, ModelHeader, Path, PlainJson } from '../api/types';
import { MODEL_HEADER_REFUSAL, opTouchesModelHeader } from '../crdt/ops';
import { setAtPath } from '../crdt/path';
import {
  applyModel,
  describeBinding,
  isRefusal,
  sameBinding,
  type ApplyResult,
  type Binding,
} from '../model/binding';
import { describeProjection, projectModel, sameProjection, type Projection } from '../model/projection';
import { openBrowserStore, type ModelStore } from '../model/store';
import { initialState, reducer, type AppState } from '../state/store';
import { FieldRegistry } from './fieldRegistry';
import { OpQueue, type BatchOutcome } from './opQueue';

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
  pollMs: number;
  setPollMs: (ms: number) => void;
  registry: FieldRegistry;
  setUrl: (url: string) => void;
  connect: () => Promise<void>;
  disconnect: () => void;
  /** Load a descriptor from a file (fallback when the node serves none). */
  loadDescriptorFile: (descriptor: Descriptor) => void;
  /**
   * Post an op batch (one edit intent). Logs the attempt with its outcome;
   * refused/error outcomes also raise the error banner. `optimistic` patches
   * the local doc immediately (the poll reconciles the truth). A batch that
   * would write the model header, or any batch while the binding check
   * refuses the document, is refused here before anything is posted.
   */
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

export function useSync(options: SyncOptions = {}): SyncApi {
  const [state, dispatch] = useReducer(reducer, loadStoredUrl(), initialState);
  const [pollMs, setPollMs] = useState(DEFAULT_POLL_MS);
  const [registry] = useState(() => new FieldRegistry());
  const [store] = useState<ModelStore | null>(() =>
    options.store === undefined ? openBrowserStore() : options.store,
  );
  const queueRef = useRef<OpQueue | null>(null);
  const logIdRef = useRef(0);
  const pollingRef = useRef(false);

  // Refs mirroring the bits the async callbacks need without re-binding.
  const urlRef = useRef(state.connection.url);
  const docRef = useRef(state.doc);
  useEffect(() => {
    urlRef.current = state.connection.url;
    docRef.current = state.doc;
  }, [state.connection.url, state.doc]);

  const setUrl = useCallback((url: string) => dispatch({ type: 'set-url', url }), []);

  // The binding check's memory (model/binding.ts): the header recorded at the
  // first apply that carried one, and the last verdict, so a change is
  // reported once rather than on every poll. Both reset with the connection.
  const recordedRef = useRef<ModelHeader | null>(null);
  const bindingRef = useRef<Binding | null>(null);
  // The store's last outcome (model/projection.ts), for the same reason.
  const projectionRef = useRef<Projection | null>(null);

  /**
   * The projection after an apply: written on `bound`, nothing otherwise. A
   * change is reported once; a failure reaches the log and the alert dock
   * and leaves the view alone, since the file is a projection of the log
   * and never what the view is built from.
   */
  const project = useCallback(
    async (result: ApplyResult) => {
      const projection = await projectModel(store, result);
      const changed =
        projectionRef.current === null || !sameProjection(projectionRef.current, projection);
      projectionRef.current = projection;
      if (!changed) return;
      dispatch({ type: 'projection', projection });
      const detail = describeProjection(projection);
      if (projection.kind === 'unavailable') {
        console.warn(detail);
      } else if (projection.kind === 'failed') {
        dispatch({
          type: 'log',
          entry: {
            id: logIdRef.current++,
            ts: projection.ts,
            description: 'write model file',
            ops: [],
            outcome: 'error',
            detail,
          },
        });
        dispatch({ type: 'banner', message: detail });
      }
    },
    [store],
  );

  /**
   * One apply: fetch the state, check its binding against `descriptor`, the
   * one the document would be rendered under, dispatch the document only
   * when the check lets it through, then write the projection. Runs on
   * connect and on every poll, since a header can arrive by transfer after
   * connect. The document always comes from the node: the store is written
   * after the dispatch and read by nothing here.
   */
  const refreshOnce = useCallback(
    async (url: string, descriptor: Descriptor | null) => {
      const wire = await getState(url);
      const result = await applyModel(wire, descriptor, recordedRef.current);
      const changed =
        bindingRef.current === null || !sameBinding(bindingRef.current, result.binding);
      bindingRef.current = result.binding;
      if (changed) dispatch({ type: 'binding', binding: result.binding });
      const ts = Date.now();
      if (!result.applied) {
        // Applies nothing. The view is emptied rather than left showing a
        // document the log no longer vouches for, and the poll's own clock
        // still advances so the chip does not call a refusing replica silent.
        dispatch({ type: 'state', doc: null, ts });
        if (changed) {
          dispatch({
            type: 'log',
            entry: {
              id: logIdRef.current++,
              ts,
              description: 'apply model',
              ops: [],
              outcome: 'refused',
              detail: result.message,
            },
          });
          dispatch({ type: 'banner', message: result.message });
        }
        await project(result);
        return;
      }
      if (recordedRef.current === null && result.binding.kind !== 'unbound') {
        recordedRef.current = result.binding.header;
      }
      dispatch({ type: 'state', doc: registry.overlay(result.doc), ts });
      await project(result);
    },
    [registry, project],
  );

  const connect = useCallback(async () => {
    const url = urlRef.current;
    recordedRef.current = null;
    bindingRef.current = null;
    projectionRef.current = null;
    dispatch({ type: 'connecting' });
    try {
      const health = await getHealth(url);
      dispatch({ type: 'connected', replicaId: health.replicaId });
      try {
        localStorage.setItem(URL_STORAGE_KEY, url);
      } catch {
        // Storage unavailable: connection still works.
      }
      queueRef.current = new OpQueue(
        (op) => postOp(url, op),
        (count) => dispatch({ type: 'pending', count }),
      );
      // Metamodel discovery: the node serves it, or 404 -> file-load fallback.
      let descriptor: Descriptor | null = null;
      try {
        descriptor = await getMetamodel(url);
        dispatch({ type: 'metamodel', descriptor, source: descriptor ? 'node' : null });
      } catch (err) {
        dispatch({ type: 'metamodel', descriptor: null, source: null });
        dispatch({
          type: 'banner',
          message: `metamodel fetch failed: ${err instanceof Error ? err.message : String(err)}`,
        });
      }
      await refreshOnce(url, descriptor);
    } catch (err) {
      dispatch({
        type: 'connect-error',
        error: err instanceof ApiError ? err.message : String(err),
      });
    }
  }, [refreshOnce]);

  const disconnect = useCallback(() => {
    queueRef.current = null;
    recordedRef.current = null;
    bindingRef.current = null;
    projectionRef.current = null;
    dispatch({ type: 'disconnected' });
  }, []);

  // Poll loop: every pollMs while connected, with a re-entrancy guard.
  useEffect(() => {
    if (state.connection.status !== 'connected') return;
    const url = state.connection.url;
    // The descriptor the document is rendered under: the node's, or a loaded
    // file. A change re-arms the loop, so the next poll checks against it.
    const descriptor = state.metamodel;
    const timer = setInterval(() => {
      if (pollingRef.current) return;
      pollingRef.current = true;
      refreshOnce(url, descriptor)
        .catch((err) => {
          dispatch({
            type: 'banner',
            message: `sync failed: ${err instanceof Error ? err.message : String(err)}`,
          });
        })
        .finally(() => {
          pollingRef.current = false;
        });
    }, pollMs);
    return () => clearInterval(timer);
  }, [state.connection.status, state.connection.url, state.metamodel, pollMs, refreshOnce]);

  const sendOps = useCallback(
    async (
      description: string,
      ops: JsonOp[],
      optimistic?: { path: Path; value: PlainJson },
    ): Promise<BatchOutcome> => {
      const refuse = (outcome: BatchOutcome['outcome'], detail: string): BatchOutcome => {
        dispatch({
          type: 'log',
          entry: { id: logIdRef.current++, ts: Date.now(), description, ops, outcome, detail },
        });
        dispatch({ type: 'banner', message: `${description}: ${detail}` });
        return { outcome, applied: 0, detail };
      };
      // The funnel: every op the editor posts passes here, so a write into the
      // model header is refused here too, whatever built it (the builders in
      // crdt/ops.ts already throw; this is the guard behind them).
      if (ops.some(opTouchesModelHeader)) return refuse('refused', MODEL_HEADER_REFUSAL);
      // Nothing may be edited while the binding check refuses the document:
      // the edit gate holds every control, and this is the same rule at the
      // wire, for a caller that did not come through a control.
      const binding = bindingRef.current;
      if (binding !== null && isRefusal(binding)) return refuse('refused', describeBinding(binding));
      const queue = queueRef.current;
      if (queue === null) return refuse('error', 'not connected');
      if (optimistic !== undefined) {
        dispatch({
          type: 'state',
          doc: setAtPath(docRef.current, optimistic.path, optimistic.value),
          ts: Date.now(),
        });
      }
      const result = await queue.enqueue(ops);
      dispatch({
        type: 'log',
        entry: {
          id: logIdRef.current++,
          ts: Date.now(),
          description,
          ops,
          outcome: result.outcome,
          detail: result.detail,
        },
      });
      if (result.outcome !== 'ok') {
        dispatch({
          type: 'banner',
          message: `${description}: ${result.outcome === 'refused' ? 'operation not enabled' : 'failed'}${
            result.detail ? ` (${result.detail})` : ''
          }`,
        });
      }
      return result;
    },
    [],
  );

  const loadDescriptorFile = useCallback((descriptor: Descriptor) => {
    dispatch({ type: 'metamodel', descriptor, source: 'file' });
  }, []);

  const clearBanner = useCallback(() => dispatch({ type: 'banner', message: null }), []);

  return {
    state,
    pollMs,
    setPollMs,
    registry,
    setUrl,
    connect,
    disconnect,
    loadDescriptorFile,
    sendOps,
    clearBanner,
  };
}
