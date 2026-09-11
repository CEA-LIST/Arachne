// The reducer under the many-models state: mp29's state half. A tab per open
// model, each with its own document and verdict; the node's listing beside
// them; nothing minted here, since an id only ever arrives from the node.
import { describe, expect, it } from 'vitest';
import type { HostedModel } from '../api/types';
import { initialState, MAX_LOG_ENTRIES, newTab, reducer, selectedTab, type AppState } from './store';

const A = 'a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4';
const C = 'c3d4e5f6c3d4e5f6c3d4e5f6c3d4e5f6';
const URL = 'http://127.0.0.1:8081';
const bt = { nsURI: 'http://www.example.org/behaviortree', digest: 'f'.repeat(64) };
const uml = { nsURI: 'http:///SimpleUML.ecore', digest: '4'.repeat(64) };

function connected(): AppState {
  return reducer(reducer(initialState(URL), { type: 'connecting' }), { type: 'connected', replicaId: 'editor-a' });
}

describe('mp29 the model list and the tabs', () => {
  it('mp29_the_editor_lists_creates_and_opens_hosted_models', () => {
    // An empty node: the list starts empty, nothing is open.
    let state = connected();
    expect(state.hosted).toBeNull();
    state = reducer(state, { type: 'hosted', models: [], seen: [] });
    expect(state.hosted).toEqual([]);
    expect(state.tabs).toEqual([]);
    expect(selectedTab(state)).toBeNull();
    state = reducer(state, {
      type: 'metamodels',
      listing: [
        { ...bt, package: 'behaviortree' },
        { ...uml, package: 'simpleuml' },
      ],
    });
    expect(state.metamodels.map((m) => m.package)).toEqual(['behaviortree', 'simpleuml']);

    // Creating one against a chosen descriptor: the node answers with the id
    // it minted, the listing is re-read and carries it, and it opens in a tab
    // whose routes are the node's. Nothing here chose the id.
    const listed: HostedModel[] = [{ modelId: A, metamodelId: bt }];
    state = reducer(state, { type: 'hosted', models: listed, seen: [] });
    state = reducer(state, { type: 'open', id: A, nodeUrl: URL });
    expect(state.hosted).toEqual(listed);
    expect(state.tabs).toEqual([A]);
    expect(state.selected).toBe(A);
    const tab = selectedTab(state);
    expect(tab).toMatchObject({ id: A, nodeUrl: URL, status: 'opening', doc: null, metamodel: null, binding: null });

    // Opening it gives a tab bound to THAT model's descriptor, as its session
    // fetched it from /api/model/{id}/metamodel, not whatever was loaded before.
    const descriptor = { formatVersion: 2, package: 'behaviortree', nsURI: bt.nsURI, rootClasses: [], classes: {}, enums: {} };
    state = reducer(state, { type: 'tab', id: A, patch: { metamodel: descriptor, metamodelSource: 'node' } });
    state = reducer(state, {
      type: 'tab',
      id: A,
      patch: {
        status: 'open',
        doc: { eClass: 'Root' },
        binding: { kind: 'bound', header: { modelId: A, metamodelId: bt }, served: bt },
        lastSyncAt: 5,
      },
    });
    expect(selectedTab(state)).toMatchObject({
      status: 'open',
      metamodel: descriptor,
      metamodelSource: 'node',
      doc: { eClass: 'Root' },
      binding: { kind: 'bound' },
      lastSyncAt: 5,
    });

    // A second model, joined by id, gets its own tab with its own document and
    // verdict; the first tab is untouched by the second's patches.
    state = reducer(state, { type: 'hosted', models: [...listed, { modelId: C, metamodelId: uml }], seen: [] });
    state = reducer(state, { type: 'open', id: C, nodeUrl: URL });
    state = reducer(state, { type: 'tab', id: C, patch: { status: 'open', doc: { eClass: 'Model' }, lastSyncAt: 9 } });
    expect(state.tabs).toEqual([A, C]);
    expect(state.selected).toBe(C);
    expect(state.models[A].doc).toEqual({ eClass: 'Root' });
    expect(state.models[C].doc).toEqual({ eClass: 'Model' });
    expect(state.models[A].lastSyncAt).toBe(5);
    expect(state.models[C].binding).toBeNull();
  });

  it('opening an id that is already open selects its tab and duplicates nothing', () => {
    let state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, { type: 'tab', id: A, patch: { status: 'open', doc: { eClass: 'Root' } } });
    state = reducer(state, { type: 'open', id: C, nodeUrl: URL });
    state = reducer(state, { type: 'open', id: A, nodeUrl: 'http://elsewhere' });
    expect(state.tabs).toEqual([A, C]);
    expect(state.selected).toBe(A);
    expect(state.models[A]).toMatchObject({ nodeUrl: URL, status: 'open', doc: { eClass: 'Root' } });
  });

  it('a tab records the node it was opened against, whatever the URL field says later', () => {
    let state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, { type: 'set-url', url: 'http://127.0.0.1:8082' });
    expect(state.models[A].nodeUrl).toBe(URL);
    expect(state.connection.url).toBe('http://127.0.0.1:8082');
  });

  it('select switches the panels between open tabs and ignores an id that is not open', () => {
    let state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, { type: 'open', id: C, nodeUrl: URL });
    state = reducer(state, { type: 'select', id: A });
    expect(state.selected).toBe(A);
    expect(reducer(state, { type: 'select', id: 'd'.repeat(32) })).toBe(state);
  });

  it('close removes the tab and hands the selection to the neighbour on the right, else the left, else nothing', () => {
    const D = 'd'.repeat(32);
    let state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, { type: 'open', id: C, nodeUrl: URL });
    state = reducer(state, { type: 'open', id: D, nodeUrl: URL });
    state = reducer(state, { type: 'select', id: C });
    state = reducer(state, { type: 'close', id: C });
    expect(state.tabs).toEqual([A, D]);
    expect(state.selected).toBe(D);
    expect(C in state.models).toBe(false);
    state = reducer(state, { type: 'close', id: D });
    expect(state.selected).toBe(A);
    // Closing a tab that is not selected leaves the selection alone.
    state = reducer(state, { type: 'open', id: C, nodeUrl: URL });
    state = reducer(state, { type: 'select', id: A });
    state = reducer(state, { type: 'close', id: C });
    expect(state.selected).toBe(A);
    state = reducer(state, { type: 'close', id: A });
    expect(state.tabs).toEqual([]);
    expect(state.selected).toBeNull();
    expect(reducer(state, { type: 'close', id: A })).toBe(state);
  });

  it('a patch for a tab that is not open is dropped, so a late session event cannot resurrect a closed tab', () => {
    const state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    const closed = reducer(state, { type: 'close', id: A });
    expect(reducer(closed, { type: 'tab', id: A, patch: { doc: { eClass: 'Root' } } })).toBe(closed);
    expect(reducer(closed, { type: 'tab', id: C, patch: { pendingOps: 3 } })).toBe(closed);
  });

  it('a patch merges into its tab and never touches another', () => {
    let state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, { type: 'open', id: C, nodeUrl: URL });
    state = reducer(state, { type: 'tab', id: A, patch: { pendingOps: 4 } });
    state = reducer(state, { type: 'tab', id: A, patch: { lastSyncAt: 7 } });
    expect(state.models[A]).toMatchObject({ pendingOps: 4, lastSyncAt: 7, status: 'opening' });
    expect(state.models[C]).toEqual(newTab(C, URL));
  });

  it('a failed open keeps its tab, with the reason, so the user sees why rather than nothing', () => {
    let state = reducer(connected(), { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, {
      type: 'tab',
      id: A,
      patch: { status: 'failed', error: `/api/model/${A}/state returned 404: not hosted` },
    });
    expect(selectedTab(state)).toMatchObject({ status: 'failed', error: expect.stringContaining('404') });
  });

  it('disconnect closes every tab and forgets the listing, since every tab was opened against that connection', () => {
    let state = reducer(connected(), { type: 'hosted', models: [{ modelId: A, metamodelId: bt }], seen: [] });
    state = reducer(state, { type: 'metamodels', listing: [{ ...bt, package: 'behaviortree' }] });
    state = reducer(state, { type: 'open', id: A, nodeUrl: URL });
    state = reducer(state, { type: 'disconnected' });
    expect(state).toMatchObject({
      connection: { url: URL, status: 'idle', replicaId: null },
      hosted: null,
      metamodels: [],
      models: {},
      tabs: [],
      selected: null,
    });
  });

  it('a log row may name its model, and the log stays capped', () => {
    let state = connected();
    for (let i = 0; i < MAX_LOG_ENTRIES + 5; i++) {
      state = reducer(state, {
        type: 'log',
        entry: { id: i, ts: i, description: 'x', ops: [], outcome: 'ok', modelId: i % 2 === 0 ? A : undefined },
      });
    }
    expect(state.log).toHaveLength(MAX_LOG_ENTRIES);
    expect(state.log[state.log.length - 1].id).toBe(MAX_LOG_ENTRIES + 4);
    expect(state.log.some((entry) => entry.modelId === A)).toBe(true);
  });
});
