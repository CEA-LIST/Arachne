// A ModelSession against a fake node: mp29's sync half. Each session is bound
// to its own model's descriptor and addresses its own routes; the poll, the
// queue, the funnel guard and the close are per session.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Descriptor, ModelId, PlainJson } from '../api/types';
import { createRootOps, MODEL_HEADER_REFUSAL, setStringOps } from '../crdt/ops';
import type { ModelStore, StoredFile } from '../model/store';
import { bt, btDocument, btHeader, btId, MODEL_ID, uml, umlId } from '../model/testFixtures';
import { recordSession } from '../testing/sessionRecorder';
import { ModelSession } from './modelSession';

const UML_ID = 'c3d4e5f6c3d4e5f6c3d4e5f6c3d4e5f6';
const NODE = 'http://node:8081';

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Poll `predicate` on real time until it holds, or fail after `timeoutMs`. */
async function until(predicate: () => boolean, timeoutMs = 5_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error('condition not met in time');
    await sleep(5);
  }
}

interface FakeModel {
  descriptor: Descriptor | null;
  /** The document the state route serves: an interpreted node serves it as it is. */
  wire: PlainJson;
  /** What the node answers to an op; success by default. */
  verdict?: { success: boolean; message: string };
}

/** A node as `fetch`: the model routes over `models`, every request recorded. */
function fakeNode(models: Record<ModelId, FakeModel>) {
  const urls: string[] = [];
  const posted: { id: ModelId; op: unknown }[] = [];
  const json = (status: number, body: unknown) =>
    new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });
  const fetch = vi.fn(async (url: string, init?: RequestInit) => {
    urls.push(url);
    const match = url.match(/\/api\/model\/([0-9a-f]{32})\/(state|metamodel|op)$/);
    if (match === null) return json(404, { error: `no such route: ${url}` });
    const [, id, leaf] = match;
    const model = models[id];
    if (model === undefined) return json(404, { error: `${id} is not hosted` });
    if (leaf === 'state') return json(200, model.wire);
    if (leaf === 'metamodel') return model.descriptor === null ? json(404, {}) : json(200, model.descriptor);
    posted.push({ id, op: JSON.parse(String(init?.body)) as unknown });
    return json(200, model.verdict ?? { success: true, message: 'ok' });
  });
  return { fetch, urls, posted };
}

const umlDoc: PlainJson = { __model: { modelId: UML_ID, metamodelId: umlId } as unknown as PlainJson, eClass: 'Model', name: 'm' };

function memoryStore(): { store: ModelStore; files: Map<ModelId, PlainJson> } {
  const files = new Map<ModelId, PlainJson>();
  const store: ModelStore = {
    location: 'memory',
    async write(id, doc): Promise<StoredFile> {
      files.set(id, doc);
      return { modelId: id, file: `${id}.json`, bytes: 0, digest: '' };
    },
    async read(id) {
      return files.get(id) ?? null;
    },
    async list() {
      return [...files.keys()].sort();
    },
    async remove(id) {
      files.delete(id);
    },
  };
  return { store, files };
}

describe('mp29 a session per model', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('mp29_a_tab_is_bound_to_its_own_models_descriptor_and_addresses_its_own_routes', async () => {
    const node = fakeNode({
      [MODEL_ID]: { descriptor: bt, wire: btDocument(btHeader) },
      [UML_ID]: { descriptor: uml, wire: umlDoc },
    });
    vi.stubGlobal('fetch', node.fetch);
    const one = recordSession();
    const two = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 60_000, events: one.events });
    const c = new ModelSession({ id: UML_ID, nodeUrl: NODE, store: null, pollMs: 60_000, events: two.events });
    await a.open();
    await c.open();
    try {
      // Each tab holds the descriptor its model's route served, and is bound to it by digest.
      const tabA = one.tab(MODEL_ID, NODE);
      const tabC = two.tab(UML_ID, NODE);
      expect(tabA).toMatchObject({ status: 'open', metamodel: bt, metamodelSource: 'node', doc: btDocument(btHeader) });
      expect(tabA.binding).toMatchObject({ kind: 'bound', served: btId });
      expect(tabC).toMatchObject({ status: 'open', metamodel: uml, metamodelSource: 'node', doc: umlDoc });
      expect(tabC.binding).toMatchObject({ kind: 'bound', served: umlId });
      expect(tabA.metamodel).not.toEqual(tabC.metamodel);
      // Each session read only its own routes.
      expect(node.urls).toEqual([
        `${NODE}/api/model/${MODEL_ID}/metamodel`,
        `${NODE}/api/model/${MODEL_ID}/state`,
        `${NODE}/api/model/${UML_ID}/metamodel`,
        `${NODE}/api/model/${UML_ID}/state`,
      ]);
      // An edit in one tab goes to that model's op route, and only there.
      // One intent, five operations: a text leaf takes `m` out and puts the
      // four characters of `Door` in, one at a time.
      const ops = setStringOps(['name'], 'm', 'Door');
      const outcome = await c.sendOps('set Model.name', ops, { path: ['name'], value: 'Door' });
      expect(outcome).toEqual({ outcome: 'ok', applied: 5 });
      expect(node.posted.map((p) => p.id)).toEqual(Array<string>(5).fill(UML_ID));
      expect(JSON.stringify(node.posted.map((p) => p.op))).toContain('"InsertChar":{"pos":0,"ch":"D"}');
      expect(two.tab(UML_ID, NODE).doc).toMatchObject({ name: 'Door' });
      expect(one.tab(MODEL_ID, NODE).doc).toEqual(btDocument(btHeader));
      expect(two.rows.at(-1)).toMatchObject({ description: 'set Model.name', outcome: 'ok', ops });
      // The fixture document lacks what the descriptor requires, which the
      // conformance report says and this test is not about.
      expect(one.rows.filter((row) => row.description !== 'conformance')).toEqual([]);
    } finally {
      a.close();
      c.close();
    }
  });

  it('every open session polls its own route on its own timer, the background ones included', async () => {
    // Real timers, short: a poll's fetch settles on its own schedule, so the
    // claim is that polls keep coming for an open session and stop for a
    // closed one, not that exactly n fire in n intervals.
    const node = fakeNode({
      [MODEL_ID]: { descriptor: bt, wire: btDocument(btHeader) },
      [UML_ID]: { descriptor: uml, wire: umlDoc },
    });
    vi.stubGlobal('fetch', node.fetch);
    const polls = (id: ModelId) => node.urls.filter((url) => url.endsWith('/state') && url.includes(id)).length;
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 15, events: recordSession().events });
    const c = new ModelSession({ id: UML_ID, nodeUrl: NODE, store: null, pollMs: 15, events: recordSession().events });
    await a.open();
    await c.open();
    try {
      node.urls.length = 0;
      await until(() => polls(MODEL_ID) >= 3 && polls(UML_ID) >= 3);
      expect(node.urls.every((url) => url.endsWith('/state'))).toBe(true);
      // A closed session stops; the other goes on.
      a.close();
      await sleep(60);
      const stopped = polls(MODEL_ID);
      const going = polls(UML_ID);
      await until(() => polls(UML_ID) >= going + 3);
      expect(polls(MODEL_ID)).toBe(stopped);
      c.close();
      await sleep(60);
      const last = node.urls.length;
      await sleep(80);
      expect(node.urls.length).toBe(last);
    } finally {
      a.close();
      c.close();
    }
  });

  it('the projection is written per model, under that model id, from the store the session was given', async () => {
    const node = fakeNode({
      [MODEL_ID]: { descriptor: bt, wire: btDocument(btHeader) },
      [UML_ID]: { descriptor: uml, wire: umlDoc },
    });
    vi.stubGlobal('fetch', node.fetch);
    const { store, files } = memoryStore();
    const one = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store, pollMs: 60_000, events: one.events });
    const c = new ModelSession({ id: UML_ID, nodeUrl: NODE, store, pollMs: 60_000, events: recordSession().events });
    await a.open();
    await c.open();
    a.close();
    c.close();
    expect([...files.keys()].sort()).toEqual([MODEL_ID, UML_ID].sort());
    expect(files.get(MODEL_ID)).toEqual(btDocument(btHeader));
    expect(files.get(UML_ID)).toEqual(umlDoc);
    expect(one.tab(MODEL_ID, NODE).projection).toMatchObject({ kind: 'written', file: { modelId: MODEL_ID } });
  });

  it('a model whose header names another descriptor than the one served is refused in its own tab only', async () => {
    // The bt document served under the uml descriptor: M-A5's bad day, per tab.
    const node = fakeNode({
      [MODEL_ID]: { descriptor: uml, wire: btDocument(btHeader) },
      [UML_ID]: { descriptor: uml, wire: umlDoc },
    });
    vi.stubGlobal('fetch', node.fetch);
    const { store, files } = memoryStore();
    const one = recordSession();
    const two = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store, pollMs: 60_000, events: one.events });
    const c = new ModelSession({ id: UML_ID, nodeUrl: NODE, store, pollMs: 60_000, events: two.events });
    await a.open();
    await c.open();
    try {
      const tabA = one.tab(MODEL_ID, NODE);
      expect(tabA.status).toBe('open');
      expect(tabA.doc).toBeNull();
      expect(tabA.binding?.kind).toBe('mismatch');
      expect(one.banners[0]).toContain(btId.digest);
      expect(one.banners[0]).toContain(umlId.digest);
      expect(one.rows[0]).toMatchObject({ description: 'apply model', outcome: 'refused' });
      // Editing is refused at the wire for that tab, with the same sentence; nothing was posted.
      const refused = await a.sendOps('create root Root', createRootOps('Root'));
      expect(refused.outcome).toBe('refused');
      expect(refused.detail).toBe(one.banners[0]);
      expect(node.posted).toEqual([]);
      // The store holds the other model's file and nothing for the refused one.
      expect([...files.keys()]).toEqual([UML_ID]);
      // The other tab is untouched.
      expect(two.tab(UML_ID, NODE)).toMatchObject({ status: 'open', doc: umlDoc, binding: { kind: 'bound' } });
      expect(two.banners).toEqual([]);
    } finally {
      a.close();
      c.close();
    }
  });

  it('refuses a batch that would write the header before posting anything', async () => {
    const node = fakeNode({ [MODEL_ID]: { descriptor: bt, wire: btDocument(btHeader) } });
    vi.stubGlobal('fetch', node.fetch);
    const one = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 60_000, events: one.events });
    await a.open();
    const outcome = await a.sendOps('rewrite header', [{ kind: 'unset', path: ['__model'] }]);
    a.close();
    expect(outcome).toEqual({ outcome: 'refused', applied: 0, detail: MODEL_HEADER_REFUSAL });
    expect(node.posted).toEqual([]);
    expect(one.rows.at(-1)).toMatchObject({ outcome: 'refused', detail: MODEL_HEADER_REFUSAL });
  });

  it("a refused op is logged and bannered with the node's own message, and the rest of the batch is dropped", async () => {
    const node = fakeNode({
      [MODEL_ID]: {
        descriptor: bt,
        wire: btDocument(btHeader),
        verdict: { success: false, message: 'operation not enabled' },
      },
    });
    vi.stubGlobal('fetch', node.fetch);
    const one = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 60_000, events: one.events });
    await a.open();
    // Two characters, so a refusal on the first has a second to drop.
    const outcome = await a.sendOps('set x', setStringOps(['behaviortrees', 0, 'ID'], 'main', 'mainxy'));
    a.close();
    expect(outcome.outcome).toBe('refused');
    expect(node.posted).toHaveLength(1);
    expect(one.rows.at(-1)).toMatchObject({ description: 'set x', outcome: 'refused', detail: 'operation not enabled' });
    expect(one.banners.at(-1)).toContain('operation not enabled');
    expect(one.tab(MODEL_ID, NODE).pendingOps).toBe(0);
  });

  it('an id the node does not host fails the tab with the 404 and rethrows, and no poll starts', async () => {
    vi.useFakeTimers();
    try {
      const node = fakeNode({});
      vi.stubGlobal('fetch', node.fetch);
      const one = recordSession();
      const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 100, events: one.events });
      await expect(a.open()).rejects.toMatchObject({ status: 404 });
      expect(one.tab(MODEL_ID, NODE)).toMatchObject({ status: 'failed', error: expect.stringContaining('404') });
      node.urls.length = 0;
      await vi.advanceTimersByTimeAsync(500);
      expect(node.urls).toEqual([]);
      a.close();
    } finally {
      vi.useRealTimers();
    }
  });

  it('a node that serves no descriptor for the model is not a failure: the tab opens with no metamodel and a file can be loaded', async () => {
    const node = fakeNode({ [MODEL_ID]: { descriptor: null, wire: btDocument(btHeader) } });
    vi.stubGlobal('fetch', node.fetch);
    const one = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 60_000, events: one.events });
    await a.open();
    expect(one.tab(MODEL_ID, NODE)).toMatchObject({ status: 'open', metamodel: null, binding: { kind: 'no-descriptor' } });
    a.loadDescriptorFile(bt);
    await a.refreshOnce();
    a.close();
    expect(one.tab(MODEL_ID, NODE)).toMatchObject({ metamodel: bt, metamodelSource: 'file', binding: { kind: 'bound' } });
  });

  it('after close no event reaches the tab, even from a refresh that was in flight', async () => {
    let release: (() => void) | null = null;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const node = fakeNode({ [MODEL_ID]: { descriptor: bt, wire: btDocument(btHeader) } });
    const slow = vi.fn(async (url: string, init?: RequestInit) => {
      if (url.endsWith('/state') && node.urls.length > 1) await gate;
      return node.fetch(url, init);
    });
    vi.stubGlobal('fetch', slow);
    const one = recordSession();
    const a = new ModelSession({ id: MODEL_ID, nodeUrl: NODE, store: null, pollMs: 60_000, events: one.events });
    await a.open();
    const before = one.patches.length;
    const inFlight = a.refreshOnce();
    a.close();
    (release as unknown as () => void)();
    await inFlight;
    expect(one.patches.length).toBe(before);
    expect(a.closed).toBe(true);
    expect(await a.sendOps('x', createRootOps('Root'))).toMatchObject({ outcome: 'error' });
  });
});
