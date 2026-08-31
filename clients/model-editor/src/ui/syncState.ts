/**
 * Sync status, derived — not stored.
 *
 * The store carries only raw facts (connection.status, lastSyncAt, pendingOps).
 * The honesty bug this fixes: the old chip printed `connection.status`, so it
 * kept reading "connected" while every request was failing. A connection that
 * has not answered for several poll intervals is NOT live, and this function is
 * the single place that judgement is made.
 *
 * Pure: no React, no clock of its own — `now` is passed in (the UI supplies a
 * 1s ticker), which is what makes every state below unit-testable.
 */

import type { AppState } from '../state/store';

/** How many poll intervals of silence before we stop claiming "live". */
export const STALE_POLLS = 3;
/** How many before we call it not responding. */
export const OFFLINE_POLLS = 10;

export type SyncKind =
  | 'disconnected'
  | 'connecting'
  | 'connect-failed'
  | 'live'
  | 'syncing'
  | 'stale'
  | 'offline';

export type SyncTone = 'neutral' | 'ok' | 'accent' | 'warn' | 'danger';

export interface SyncView {
  kind: SyncKind;
  /** Chip text. */
  label: string;
  tone: SyncTone;
  /** True while a request is expected to be in flight (drives the pulse). */
  busy: boolean;
  /** Offer a manual retry (reconnect) affordance. */
  retryable: boolean;
  /** Longer sentence for the properties-panel strip; null when there is nothing to say. */
  detail: string | null;
}

function clockTime(ts: number): string {
  return new Date(ts).toLocaleTimeString();
}

/** Human relative age, coarse on purpose: a ticking millisecond count is noise. */
export function relativeAge(ms: number): string {
  const seconds = Math.max(0, Math.round(ms / 1000));
  if (seconds < 1) return 'just now';
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.round(minutes / 60)}h ago`;
}

/**
 * The chip's truth. `pollMs` scales the staleness thresholds so a deliberately
 * slow poll is not misreported as a broken one.
 */
export function syncView(state: AppState, pollMs: number, now: number): SyncView {
  const { status, error } = state.connection;
  const { lastSyncAt, pendingOps } = state;

  if (status === 'idle') {
    return {
      kind: 'disconnected',
      label: 'Not connected',
      tone: 'neutral',
      busy: false,
      retryable: false,
      detail: null,
    };
  }
  if (status === 'connecting') {
    return {
      kind: 'connecting',
      label: 'Connecting…',
      tone: 'warn',
      busy: true,
      retryable: false,
      detail: null,
    };
  }
  if (status === 'error') {
    return {
      kind: 'connect-failed',
      label: 'Connection failed',
      tone: 'danger',
      busy: false,
      retryable: true,
      detail: error,
    };
  }

  // status === 'connected': the poll loop's silence decides the rest.
  const age = lastSyncAt === null ? null : now - lastSyncAt;

  if (age !== null && age > OFFLINE_POLLS * pollMs) {
    return {
      kind: 'offline',
      label: 'Not responding',
      tone: 'danger',
      busy: false,
      retryable: true,
      detail: `edits are still being sent, but the replica has not answered since ${clockTime(lastSyncAt as number)}`,
    };
  }
  if (age !== null && age > STALE_POLLS * pollMs) {
    return {
      kind: 'stale',
      label: `Reconnecting… last sync ${clockTime(lastSyncAt as number)}`,
      tone: 'warn',
      busy: true,
      retryable: false,
      detail: null,
    };
  }
  if (pendingOps > 0) {
    return {
      kind: 'syncing',
      label: 'Syncing',
      tone: 'accent',
      busy: true,
      retryable: false,
      detail: null,
    };
  }
  return {
    kind: 'live',
    label: age === null ? 'Live' : `Live · synced ${relativeAge(age)}`,
    tone: 'ok',
    busy: false,
    retryable: false,
    detail: null,
  };
}
