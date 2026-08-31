import { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import { ApiError, getHealth, getMetamodel, getState, postOp } from '../api/client';
import type { Descriptor, JsonOp, Path, PlainJson } from '../api/types';
import { decodeState } from '../crdt/decode';
import { setAtPath } from '../crdt/path';
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
   * the local doc immediately (the poll reconciles the truth).
   */
  sendOps: (
    description: string,
    ops: JsonOp[],
    optimistic?: { path: Path; value: PlainJson },
  ) => Promise<BatchOutcome>;
  clearBanner: () => void;
}

export function useSync(): SyncApi {
  const [state, dispatch] = useReducer(reducer, loadStoredUrl(), initialState);
  const [pollMs, setPollMs] = useState(DEFAULT_POLL_MS);
  const [registry] = useState(() => new FieldRegistry());
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

  const refreshOnce = useCallback(async (url: string) => {
    const wire = await getState(url);
    let doc = decodeState(wire);
    doc = registry.overlay(doc);
    dispatch({ type: 'state', doc, ts: Date.now() });
  }, [registry]);

  const connect = useCallback(async () => {
    const url = urlRef.current;
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
      try {
        const descriptor = await getMetamodel(url);
        dispatch({ type: 'metamodel', descriptor, source: descriptor ? 'node' : null });
      } catch (err) {
        dispatch({ type: 'metamodel', descriptor: null, source: null });
        dispatch({
          type: 'banner',
          message: `metamodel fetch failed: ${err instanceof Error ? err.message : String(err)}`,
        });
      }
      await refreshOnce(url);
    } catch (err) {
      dispatch({
        type: 'connect-error',
        error: err instanceof ApiError ? err.message : String(err),
      });
    }
  }, [refreshOnce]);

  const disconnect = useCallback(() => {
    queueRef.current = null;
    dispatch({ type: 'disconnected' });
  }, []);

  // Poll loop: every pollMs while connected, with a re-entrancy guard.
  useEffect(() => {
    if (state.connection.status !== 'connected') return;
    const url = state.connection.url;
    const timer = setInterval(() => {
      if (pollingRef.current) return;
      pollingRef.current = true;
      refreshOnce(url)
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
  }, [state.connection.status, state.connection.url, pollMs, refreshOnce]);

  const sendOps = useCallback(
    async (
      description: string,
      ops: JsonOp[],
      optimistic?: { path: Path; value: PlainJson },
    ): Promise<BatchOutcome> => {
      const queue = queueRef.current;
      if (queue === null) {
        const outcome: BatchOutcome = { outcome: 'error', applied: 0, detail: 'not connected' };
        dispatch({
          type: 'log',
          entry: { id: logIdRef.current++, ts: Date.now(), description, ops, outcome: 'error', detail: 'not connected' },
        });
        dispatch({ type: 'banner', message: `${description}: not connected` });
        return outcome;
      }
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
