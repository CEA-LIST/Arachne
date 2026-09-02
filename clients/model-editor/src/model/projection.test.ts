/// <reference types="node" />
// The projection: the store is written after an apply the binding check lets
// through, never read for state, and never in the way of an apply. Runs over
// the Node fs backend in a temporary directory, the level-1 form of M-A6;
// mp28 is its level-4 form against live nodes.
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';
import type { ModelId, PlainJson } from '../api/types';
import { decodeState } from '../crdt/decode';
import { applyModel, type ApplyResult } from './binding';
import { canonicalJson, sha256Hex } from './digest';
import { fsModelStore } from './fsStore';
import { describeProjection, projectModel, projectionLabel, sameProjection, type Projection } from './projection';
import { StoreError, type ModelStore } from './store';
import { bt, btDocument, btHeader, encodeWire, MODEL_ID, uml, umlHeader } from './testFixtures';

const tempRoot = mkdtempSync(join(tmpdir(), 'model-projection-'));
afterAll(() => rmSync(tempRoot, { recursive: true, force: true }));

let dirCounter = 0;
function freshDir(): string {
  return join(tempRoot, `store-${dirCounter++}`);
}

function fileOf(dir: string, id: ModelId): string {
  return join(dir, `${id}.json`);
}

function filesIn(dir: string): string[] {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

function written(projection: Projection): Extract<Projection, { kind: 'written' }> {
  if (projection.kind !== 'written') throw new Error(`expected a write, got ${projection.kind}`);
  return projection;
}

function applied(result: ApplyResult): Extract<ApplyResult, { applied: true }> {
  if (!result.applied) throw new Error('expected the apply to go through');
  return result;
}

/** A store that counts what the adaptation layer asks of it. */
function counting(inner: ModelStore): { store: ModelStore; calls: string[] } {
  const calls: string[] = [];
  const store: ModelStore = {
    get location() {
      return inner.location;
    },
    write(id, doc) {
      calls.push('write');
      return inner.write(id, doc);
    },
    read(id) {
      calls.push('read');
      return inner.read(id);
    },
    list() {
      calls.push('list');
      return inner.list();
    },
    remove(id) {
      calls.push('remove');
      return inner.remove(id);
    },
  };
  return { store, calls };
}

describe('mp10 the store after an apply', () => {
  it('mp10_apply_writes_the_store_file_equal_to_the_applied_state', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const doc = btDocument(btHeader);

    // A behaviour-tree state whose header and served descriptor agree.
    const result = applied(await applyModel(encodeWire(doc), bt, null));
    expect(result.binding.kind).toBe('bound');
    const projection = written(await projectModel(store, result));

    // The file named for a1b2… parses to exactly the applied PlainJson.
    expect(filesIn(dir)).toEqual([`${MODEL_ID}.json`]);
    const text = readFileSync(fileOf(dir, MODEL_ID), 'utf8');
    expect(JSON.parse(text)).toEqual(result.doc);
    expect(JSON.parse(text)).toEqual(doc);

    // Its bytes are the canonical JSON the digest rule hashes, so the file
    // digests to what the write reported.
    expect(text).toBe(canonicalJson(result.doc));
    expect(projection.file).toEqual({
      modelId: MODEL_ID,
      file: `${MODEL_ID}.json`,
      bytes: new TextEncoder().encode(text).length,
      digest: await sha256Hex(text),
    });
    expect(await store.list()).toEqual([MODEL_ID]);
  });

  it('writes the decoded state again on every apply, so the file follows the log', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const first = applied(await applyModel(encodeWire(btDocument(btHeader)), bt, null));
    const one = written(await projectModel(store, first));

    const grown: PlainJson = {
      ...(btDocument(btHeader) as { [key: string]: PlainJson }),
      behaviortrees: [
        { eClass: 'BehaviorTree', ID: 'main', child: { eClass: 'Sequence', name: 'root' } },
        { eClass: 'BehaviorTree', ID: 'second', child: { eClass: 'Fallback', name: 'alt' } },
      ],
    };
    const second = applied(await applyModel(encodeWire(grown), bt, btHeader));
    const two = written(await projectModel(store, second));

    expect(JSON.parse(readFileSync(fileOf(dir, MODEL_ID), 'utf8'))).toEqual(grown);
    expect(two.file.digest).not.toBe(one.file.digest);
    expect(sameProjection(one, two)).toBe(false);
  });
});

describe('projection, not truth', () => {
  it('a restart trusts the log: a tampered file is never rendered and is rewritten to match', async () => {
    // Session one: connect, apply, the file lands.
    const dir = freshDir();
    const { store, calls } = counting(fsModelStore(dir));
    const wire = encodeWire(btDocument(btHeader));
    const nodeState = decodeState(wire);
    written(await projectModel(store, applied(await applyModel(wire, bt, null))));
    expect(JSON.parse(readFileSync(fileOf(dir, MODEL_ID), 'utf8'))).toEqual(nodeState);

    // Between sessions the file is replaced by a document holding one empty
    // Root, the tamper of M-A6 and (f). The view it would produce is wrong.
    const tampered: PlainJson = { __model: btHeader as unknown as PlainJson, eClass: 'Root', behaviortrees: [] };
    writeFileSync(fileOf(dir, MODEL_ID), JSON.stringify(tampered));
    expect(JSON.parse(readFileSync(fileOf(dir, MODEL_ID), 'utf8'))).not.toEqual(nodeState);

    // Session two: connect resets the recorded header, the state comes from
    // the node, and the document is the node's, not the file's.
    const result = applied(await applyModel(wire, bt, null));
    expect(result.doc).toEqual(nodeState);
    expect(result.doc).not.toEqual(tampered);

    // The file is rewritten to match, and the store was written twice and
    // read never: the adaptation layer has no read path.
    written(await projectModel(store, result));
    expect(JSON.parse(readFileSync(fileOf(dir, MODEL_ID), 'utf8'))).toEqual(nodeState);
    expect(await store.read(MODEL_ID)).toEqual(nodeState);
    expect(calls.filter((call) => call !== 'read')).toEqual(['write', 'write']);
    expect(calls.slice(0, 2)).toEqual(['write', 'write']);
  });

  it('a file emptied to {} or corrupted to non-JSON is rewritten the same way', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const wire = encodeWire(btDocument(btHeader));
    const nodeState = decodeState(wire);
    for (const tamper of ['{}', '{not json', '']) {
      written(await projectModel(store, applied(await applyModel(wire, bt, null))));
      writeFileSync(fileOf(dir, MODEL_ID), tamper);
      if (tamper !== '{}') await expect(store.read(MODEL_ID)).rejects.toThrow(StoreError);
      const result = applied(await applyModel(wire, bt, null));
      expect(result.doc).toEqual(nodeState);
      written(await projectModel(store, result));
      expect(readFileSync(fileOf(dir, MODEL_ID), 'utf8')).toBe(canonicalJson(nodeState));
    }
  });

  it('applyModel takes no store: the view cannot come from the file by construction', () => {
    expect(applyModel.length).toBe(3);
  });
});

describe('what is not written', () => {
  it('a refused apply writes nothing', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const result = await applyModel(encodeWire(btDocument(btHeader)), uml, null);
    expect(result.applied).toBe(false);
    expect(await projectModel(store, result)).toEqual({ kind: 'not-written', why: 'refused' });
    expect(filesIn(dir)).toEqual([]);
    expect(await store.list()).toEqual([]);
  });

  it('a rewritten header is refused and the file keeps the document from before it', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const first = applied(await applyModel(encodeWire(btDocument(btHeader)), bt, null));
    written(await projectModel(store, first));
    const rewritten = await applyModel(encodeWire(btDocument(umlHeader)), uml, btHeader);
    expect(rewritten.applied).toBe(false);
    expect(await projectModel(store, rewritten)).toEqual({ kind: 'not-written', why: 'refused' });
    expect(JSON.parse(readFileSync(fileOf(dir, MODEL_ID), 'utf8'))).toEqual(btDocument(btHeader));
  });

  it('an unbound document has no id to file under, so nothing is written', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const result = applied(await applyModel(encodeWire(btDocument(null)), bt, null));
    expect(result.binding.kind).toBe('unbound');
    expect(await projectModel(store, result)).toEqual({ kind: 'not-written', why: 'unbound' });
    expect(filesIn(dir)).toEqual([]);
  });

  it('a header with no descriptor to check it against is applied but not yet projected', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const result = applied(await applyModel(encodeWire(btDocument(btHeader)), null, null));
    expect(result.binding.kind).toBe('no-descriptor');
    expect(await projectModel(store, result)).toEqual({ kind: 'not-written', why: 'no-descriptor' });
    expect(filesIn(dir)).toEqual([]);
  });
});

describe('the store never breaks an apply', () => {
  it('no store in this context is reported as unavailable, and the document stands', async () => {
    const result = applied(await applyModel(encodeWire(btDocument(btHeader)), bt, null));
    expect(await projectModel(null, result)).toEqual({ kind: 'unavailable' });
    expect(result.doc).toEqual(btDocument(btHeader));
  });

  it('a failing store is a failed projection naming the model and the cause, never a throw', async () => {
    const failing: ModelStore = {
      location: 'nowhere',
      write: async () => {
        throw new StoreError('cannot write a1b2….json: QuotaExceededError: The quota has been exceeded.');
      },
      read: async () => null,
      list: async () => [],
      remove: async () => {},
    };
    const result = applied(await applyModel(encodeWire(btDocument(btHeader)), bt, null));
    const projection = await projectModel(failing, result);
    expect(projection.kind).toBe('failed');
    if (projection.kind === 'failed') {
      expect(projection.modelId).toBe(MODEL_ID);
      expect(projection.error).toContain('QuotaExceededError');
    }
    expect(result.doc).toEqual(btDocument(btHeader));
  });

  it('a header whose modelId is not a model id cannot name a file outside the store', async () => {
    const dir = freshDir();
    const store = fsModelStore(dir);
    const hostile = { ...btHeader, modelId: '../../escape' };
    const result = applied(await applyModel(encodeWire(btDocument(hostile)), bt, null));
    expect(result.binding.kind).toBe('bound');
    const projection = await projectModel(store, result);
    expect(projection.kind).toBe('failed');
    if (projection.kind === 'failed') expect(projection.error).toContain('not a model id');
    expect(filesIn(dir)).toEqual([]);
    expect(filesIn(tempRoot).some((name) => name.includes('escape'))).toBe(false);
  });
});

describe('the outcome as the UI reads it', () => {
  const file = { modelId: MODEL_ID, file: `${MODEL_ID}.json`, bytes: 12, digest: 'ab'.repeat(32) };
  const writtenAt: Projection = { kind: 'written', file, ts: 1 };

  it('describes every outcome, the write with its file and digest', () => {
    expect(describeProjection({ kind: 'unavailable' })).toContain('no model store');
    expect(describeProjection({ kind: 'not-written', why: 'refused' })).toContain('refused');
    expect(describeProjection({ kind: 'not-written', why: 'unbound' })).toContain('no __model header');
    expect(describeProjection({ kind: 'not-written', why: 'no-descriptor' })).toContain('no descriptor');
    const text = describeProjection(writtenAt);
    expect(text).toContain(`${MODEL_ID}.json`);
    expect(text).toContain('12 bytes');
    expect(text).toContain(file.digest);
    expect(text).toContain('never read for state');
    const failed = describeProjection({ kind: 'failed', modelId: MODEL_ID, error: 'quota', ts: 1 });
    expect(failed).toContain(MODEL_ID);
    expect(failed).toContain('quota');
    expect(failed).toContain('the view is unaffected');
  });

  it('labels each outcome with a word or two', () => {
    expect(projectionLabel({ kind: 'unavailable' })).toBe('no store');
    expect(projectionLabel({ kind: 'not-written', why: 'unbound' })).toBe('not stored');
    expect(projectionLabel(writtenAt)).toBe('stored');
    expect(projectionLabel({ kind: 'failed', modelId: MODEL_ID, error: 'x', ts: 1 })).toBe('store failed');
  });

  it('sameProjection ignores the clock and notices a new digest, a new reason or a new error', () => {
    expect(sameProjection(writtenAt, { kind: 'written', file: { ...file }, ts: 2 })).toBe(true);
    expect(sameProjection(writtenAt, { kind: 'written', file: { ...file, digest: 'cd'.repeat(32) }, ts: 1 })).toBe(
      false,
    );
    expect(sameProjection({ kind: 'unavailable' }, { kind: 'unavailable' })).toBe(true);
    expect(sameProjection({ kind: 'not-written', why: 'unbound' }, { kind: 'not-written', why: 'refused' })).toBe(
      false,
    );
    expect(
      sameProjection(
        { kind: 'failed', modelId: MODEL_ID, error: 'a', ts: 1 },
        { kind: 'failed', modelId: MODEL_ID, error: 'a', ts: 9 },
      ),
    ).toBe(true);
    expect(
      sameProjection(
        { kind: 'failed', modelId: MODEL_ID, error: 'a', ts: 1 },
        { kind: 'failed', modelId: MODEL_ID, error: 'b', ts: 1 },
      ),
    ).toBe(false);
    expect(sameProjection(writtenAt, { kind: 'unavailable' })).toBe(false);
  });
});

describe('level 4, against live nodes', () => {
  it('mp28_the_store_file_matches_the_converged_model_and_a_restart_trusts_the_log', (ctx) => {
    // A converged behaviour-tree model on live nodes: read the store file and
    // compare it with the decoded GET /api/model/a1b2…/state; overwrite the
    // file with a document holding one empty Root and reconnect; the rendered
    // tree is the full one and the file is rewritten to match. The level-1
    // form above runs today; this one runs the real connect path.
    ctx.skip('needs the level-4 harness, step 6');
  });
});
