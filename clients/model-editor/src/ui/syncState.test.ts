import { describe, expect, it } from 'vitest';
import { initialState, type AppState } from '../state/store';
import { relativeAge, syncView } from './syncState';

const POLL = 500;
const NOW = 1_000_000;

function connected(overrides: Partial<AppState> = {}): AppState {
  const base = initialState('http://127.0.0.1:8081');
  return {
    ...base,
    connection: { ...base.connection, status: 'connected', replicaId: 'editor-a' },
    lastSyncAt: NOW,
    ...overrides,
  };
}

describe('syncView', () => {
  it('reports disconnected for a fresh store', () => {
    const view = syncView(initialState('u'), POLL, NOW);
    expect(view.kind).toBe('disconnected');
    expect(view.label).toBe('Not connected');
    expect(view.retryable).toBe(false);
  });

  it('reports connecting while the health probe is in flight', () => {
    const base = initialState('u');
    const state: AppState = { ...base, connection: { ...base.connection, status: 'connecting' } };
    const view = syncView(state, POLL, NOW);
    expect(view.kind).toBe('connecting');
    expect(view.busy).toBe(true);
  });

  it('surfaces the connection error and offers a retry when connecting failed', () => {
    const base = initialState('u');
    const state: AppState = {
      ...base,
      connection: { ...base.connection, status: 'error', error: '/api/health returned 500' },
    };
    const view = syncView(state, POLL, NOW);
    expect(view.kind).toBe('connect-failed');
    expect(view.detail).toBe('/api/health returned 500');
    expect(view.retryable).toBe(true);
  });

  it('reports live with a relative age when the poll is answering', () => {
    const view = syncView(connected(), POLL, NOW + 1000);
    expect(view.kind).toBe('live');
    expect(view.tone).toBe('ok');
    expect(view.label).toContain('1s ago');
  });

  it('reports syncing while ops are queued', () => {
    const view = syncView(connected({ pendingOps: 3 }), POLL, NOW);
    expect(view.kind).toBe('syncing');
    expect(view.busy).toBe(true);
  });

  it('stops claiming live after three missed polls — the honesty rule', () => {
    const view = syncView(connected(), POLL, NOW + 3 * POLL + 1);
    expect(view.kind).toBe('stale');
    expect(view.tone).toBe('warn');
  });

  it('reports not responding after ten missed polls, with a retry', () => {
    const view = syncView(connected(), POLL, NOW + 10 * POLL + 1);
    expect(view.kind).toBe('offline');
    expect(view.tone).toBe('danger');
    expect(view.retryable).toBe(true);
    expect(view.detail).toContain('has not answered since');
  });

  it('scales staleness with the poll interval, so a slow poll is not misreported', () => {
    const slow = syncView(connected(), 5000, NOW + 4000);
    expect(slow.kind).toBe('live');
    const fast = syncView(connected(), 100, NOW + 4000);
    expect(fast.kind).toBe('offline');
  });

  it('prefers offline over a pending-op count: silence outranks queue depth', () => {
    const view = syncView(connected({ pendingOps: 2 }), POLL, NOW + 20 * POLL);
    expect(view.kind).toBe('offline');
  });

  it('is live-without-age before the first poll answers', () => {
    const view = syncView(connected({ lastSyncAt: null }), POLL, NOW);
    expect(view.kind).toBe('live');
    expect(view.label).toBe('Live');
  });
});

describe('relativeAge', () => {
  it('coarsens as the age grows', () => {
    expect(relativeAge(200)).toBe('just now');
    expect(relativeAge(4000)).toBe('4s ago');
    expect(relativeAge(120_000)).toBe('2m ago');
    expect(relativeAge(7_200_000)).toBe('2h ago');
  });
});
