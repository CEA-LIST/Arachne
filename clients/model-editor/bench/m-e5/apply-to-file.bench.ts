/// <reference types="node" />
/**
 * M-E5 of the model plane's validation plan: apply-to-file latency and store
 * size against model size.
 *
 * Behaviour-tree models of 10, 100, 1,000 and 10,000 `TreeNode` objects,
 * each a `Root` with one `BehaviorTree` whose child is a tree of `Sequence`
 * and `Fallback` control nodes with a fan-out of ten and `Action` and
 * `Condition` leaves, every node carrying an `ID` and a `name`. For each
 * size the whole step the adaptation layer runs after a poll is timed:
 * `applyModel` (decode the wire state, read the header, hash the descriptor,
 * decide) followed by `projectModel` (canonical JSON, the write to the
 * store's file, its digest), on the Node fs backend (`model/fsStore.ts`)
 * in a temporary directory, from the call to the file being closed and
 * renamed into place. p50 and p95 over `iterations` repetitions, the first
 * few discarded as warm-up.
 *
 * Store bytes are the file's length; wire-state bytes are the compact JSON
 * body `GET /api/model/{id}/state` answers for the same state, which the
 * node emits key-sorted, so `canonicalJson({json: wire})`. The wire form is
 * reconstructed with the fixture's `encodeWire`; when a node binary is
 * present the reconstruction is calibrated against a live node at the
 * smallest size, byte for byte, before it is trusted at the others.
 *
 * With Chrome present the same bytes are also written through the
 * origin-private file system in a headless page on a loopback origin, the
 * store's browser backend (`opfsFilesBackend.write` in `model/store.ts`:
 * directory handle, file handle, `createWritable`, `write`, `close`), and
 * that write is timed alone as `opfs_write_*`.
 *
 * Threshold, fixed: store bytes at most the wire-state bytes, asserted.
 * Threshold, FOR-DECISION: p95 apply-to-file at 1,000 objects; the plan's
 * proposal is 100 ms, and this bench records the numbers the decision is
 * taken from. Results land beside this file as results.csv and manifest.txt.
 */

import { execSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import { arch, cpus, loadavg, platform, release, totalmem } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import puppeteer from 'puppeteer-core';
import { afterAll, describe, expect, it } from 'vitest';
import type { ModelHeader, ModelId, PlainJson, WireNode } from '../../src/api/types';
import { buildValueOps } from '../../src/crdt/ops';
import { applyModel } from '../../src/model/binding';
import { canonicalJson } from '../../src/model/digest';
import { fsModelStore } from '../../src/model/fsStore';
import { projectModel } from '../../src/model/projection';
import { bt, btId, encodeWire } from '../../src/model/testFixtures';
import {
  applyOps,
  createModel,
  freePort,
  nodeBinary,
  scratchDir,
  skipReason,
  startNodes,
  writeMetamodelDir,
  type LiveNode,
} from '../../src/testing/liveNodes';

const HERE = fileURLToPath(new URL('./', import.meta.url));
/** The load average when this file was loaded, before any node or browser was started. */
const LOAD_AT_START = loadavg().map((l) => l.toFixed(2)).join(' ');
const CHROME = process.env['CHROME_BIN'] ?? '/usr/bin/google-chrome';
const SIZES = (process.env['M_E5_SIZES'] ?? '10,100,1000,10000').split(',').map(Number);
const WARMUP = 3;
const MODEL_ID: ModelId = 'e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5';
const HEADER: ModelHeader = { modelId: MODEL_ID, metamodelId: btId };

/** How many timed repetitions a size gets: enough for a p95 to mean something, bounded at the top. */
function iterationsFor(objects: number): number {
  return objects >= 10_000 ? 20 : objects >= 1_000 ? 50 : 100;
}

/* ---------- the models ---------- */

const LEAVES = ['OpenDoor', 'CloseDoor', 'EnterRoom', 'IsDoorOpen'];

/**
 * A tree of exactly `objects` TreeNode instances: the root Sequence, then
 * nodes in breadth-first order under the control nodes, ten per parent,
 * every tenth child a control node so the tree gains depth as it grows.
 */
export function behaviourTree(objects: number): PlainJson {
  let made = 0;
  const node = (eClass: string): Record<string, PlainJson> => {
    const i = made++;
    return { eClass, ID: `n${i}`, name: `node ${i}` };
  };
  const root = { ...node('Sequence'), children: [] as PlainJson[] };
  const parents: Record<string, PlainJson>[] = [root];
  while (made < objects) {
    const parent = parents.shift();
    if (parent === undefined) throw new Error('no parent left');
    const children = parent['children'] as PlainJson[];
    for (let k = 0; k < 10 && made < objects; k++) {
      if (k === 9 && objects - made > 1) {
        const control = { ...node(parents.length % 2 === 0 ? 'Fallback' : 'Sequence'), children: [] };
        children.push(control);
        parents.push(control);
      } else {
        children.push(node(LEAVES[made % LEAVES.length]));
      }
    }
  }
  return {
    __model: HEADER as unknown as PlainJson,
    eClass: 'Root',
    behaviortrees: [{ eClass: 'BehaviorTree', ID: 'main', child: root }],
  };
}

function countTreeNodes(value: PlainJson): number {
  if (Array.isArray(value)) return value.reduce((n: number, v) => n + countTreeNodes(v), 0);
  if (value !== null && typeof value === 'object') {
    const eClass = value['eClass'];
    const own = typeof eClass === 'string' && !['Root', 'BehaviorTree'].includes(eClass) ? 1 : 0;
    return (
      own +
      Object.entries(value)
        .filter(([k]) => k !== '__model')
        .reduce((n: number, [, v]) => n + countTreeNodes(v), 0)
    );
  }
  return 0;
}

/** The body `GET /api/model/{id}/state` answers: compact, key-sorted, `{"json": <wire>}`. */
function wireStateBytes(wire: WireNode): number {
  return Buffer.byteLength(canonicalJson({ json: wire }), 'utf8');
}

function quantile(sorted: number[], q: number): number {
  return sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)))];
}

interface Row {
  run: number;
  objects: number;
  apply_to_file_p50_ms: number;
  apply_to_file_p95_ms: number;
  store_bytes: number;
  wire_state_bytes: number;
  iterations: number;
  apply_p50_ms: number;
  project_p50_ms: number;
  bytes_per_object: number;
  opfs_write_p50_ms: number | '';
  opfs_write_p95_ms: number | '';
}

const rows: Row[] = [];
const notes: string[] = [];
const tempRoot = scratchDir('m-e5');
afterAll(() => {
  rmSync(tempRoot, { recursive: true, force: true });
  if (rows.length === 0) return;
  const header = Object.keys(rows[0]).join(',');
  writeFileSync(
    join(HERE, 'results.csv'),
    [header, ...rows.map((row) => Object.values(row).join(','))].join('\n') + '\n',
  );
  const git = (dir: string) => {
    try {
      const head = execSync('git rev-parse --short=7 HEAD', { cwd: dir, encoding: 'utf8' }).trim();
      const dirty = execSync('git status --porcelain', { cwd: dir, encoding: 'utf8' }).trim() === '' ? 'clean' : 'dirty';
      return `${head} (${dirty}) ${dir}`;
    } catch {
      return `unknown ${dir}`;
    }
  };
  const moiraiRoot = fileURLToPath(new URL('../../../../../moirai-model-plane/', import.meta.url));
  const chromeVersion = (() => {
    try {
      return execSync(`"${CHROME}" --version`, { encoding: 'utf8' }).trim();
    } catch {
      return 'absent';
    }
  })();
  const pad = (k: string) => k.padEnd(14);
  const manifest = [
    `${pad('run')}${new Date().toISOString().replace(/[-:]/g, '').replace(/\.\d+/, '')}`,
    `${pad('host')}${platform()} ${release()} ${arch()}`,
    `${pad('cpu')}${cpus()[0]?.model ?? 'unknown'} x${cpus().length}`,
    `${pad('memory')}${(totalmem() / 2 ** 30).toFixed(1)} GiB`,
    `${pad('load_start')}${LOAD_AT_START} (1, 5, 15 min before the run started)`,
    `${pad('load_end')}${loadavg().map((l) => l.toFixed(2)).join(' ')} (1, 5, 15 min when the manifest was written)`,
    `${pad('arachne')}${git(fileURLToPath(new URL('../../../../', import.meta.url)))}`,
    `${pad('moirai')}${git(moiraiRoot)}`,
    `${pad('node')}${process.version}`,
    `${pad('chrome')}${chromeVersion}`,
    `${pad('node_binary')}${nodeBinary() ?? 'absent'}`,
    `${pad('command')}npm run bench:m-e5 (bench/m-e5/run.sh)`,
    `${pad('sizes')}${SIZES.join(',')} TreeNode objects; iterations ${SIZES.map(iterationsFor).join(',')} after ${WARMUP} warm-up`,
    `${pad('measured')}applyModel + projectModel over the fs store, per repetition; store_bytes = file length; wire_state_bytes = canonicalJson({json: encodeWire(doc)})`,
    `${pad('opfs')}the same bytes written through OPFS in headless Chrome on a loopback origin, timed alone`,
    `${pad('threshold')}store_bytes <= wire_state_bytes (fixed); p95 apply-to-file at 1000 objects FOR-DECISION, the plan's proposal 100 ms`,
    `${pad('proposal')}from this run: p95 apply-to-file at 1000 objects <= 25 ms, five times the measured p95 and a quarter of the poll-derived 100 ms; see the step-8 report`,
    ...notes.map((note) => `${pad('note')}${note}`),
  ];
  writeFileSync(join(HERE, 'manifest.txt'), manifest.join('\n') + '\n');
});

/* ---------- the calibration against a live node ---------- */

async function calibrate(objects: number): Promise<void> {
  const skip = skipReason('m-e5 calibration');
  if (skip !== null) {
    notes.push(`calibration skipped: ${skip}`);
    return;
  }
  const run = scratchDir('m-e5-node');
  const dir = writeMetamodelDir(join(run, 'metamodels'), { 'bt.metamodel.json': bt });
  const [node]: LiveNode[] = await startNodes([{ name: 'editor-a', metamodelDir: dir }], run);
  try {
    const id = await createModel(node, btId);
    const doc = behaviourTree(objects) as Record<string, PlainJson>;
    const { __model: _header, ...body } = doc;
    await applyOps(node, id, buildValueOps(body));
    const text = await (await fetch(`${node.url}/api/model/${id}/state`)).text();
    const fromNode = JSON.parse(text) as { json: WireNode };
    const expectedDoc: PlainJson = { ...body, __model: { modelId: id, metamodelId: btId } as unknown as PlainJson };
    const rebuilt = encodeWire(expectedDoc);
    expect(canonicalJson(fromNode.json)).toBe(canonicalJson(rebuilt));
    expect(Buffer.byteLength(text, 'utf8')).toBe(wireStateBytes(rebuilt));
    notes.push(
      `calibration at ${objects} objects: the node's GET /api/model/{id}/state body (${Buffer.byteLength(text, 'utf8')} bytes) equals canonicalJson({json: encodeWire(doc)}) byte for byte`,
    );
  } finally {
    await node.stop();
    rmSync(run, { recursive: true, force: true });
  }
}

/* ---------- OPFS in headless Chrome ---------- */

async function opfsWrite(text: string, iterations: number): Promise<{ p50: number; p95: number } | null> {
  if (!existsSync(CHROME)) {
    notes.push(`opfs skipped: no Chrome at ${CHROME} (set CHROME_BIN)`);
    return null;
  }
  const port = await freePort();
  const server: Server = createServer((_request, response) => {
    response.writeHead(200, { 'Content-Type': 'text/html' });
    response.end('<!doctype html><title>m-e5</title><div id="root"></div>');
  });
  await new Promise<void>((resolve) => server.listen(port, '127.0.0.1', resolve));
  const profile = join(tempRoot, `chrome-${port}`);
  const base = { executablePath: CHROME, headless: true as const, userDataDir: profile };
  const args = ['--no-first-run', '--no-default-browser-check', '--disable-extensions', '--disable-background-networking'];
  const browser = await puppeteer.launch({ ...base, args }).catch(() => puppeteer.launch({ ...base, args: [...args, '--no-sandbox'] }));
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${port}/`);
    const samples = await page.evaluate(
      async (bytes: string, name: string, count: number) => {
        // `opfsFilesBackend.write` of model/store.ts, step for step.
        const root = await navigator.storage.getDirectory();
        const dir = await root.getDirectoryHandle('models', { create: true });
        const out: number[] = [];
        for (let i = 0; i < count; i++) {
          const started = performance.now();
          const handle = await dir.getFileHandle(name, { create: true });
          const writable = await handle.createWritable();
          await writable.write(bytes);
          await writable.close();
          out.push(performance.now() - started);
        }
        const size = (await (await dir.getFileHandle(name)).getFile()).size;
        if (size !== new TextEncoder().encode(bytes).length) throw new Error(`OPFS file is ${size} bytes`);
        return out;
      },
      text,
      `${MODEL_ID}.json`,
      iterations + WARMUP,
    );
    const sorted = samples.slice(WARMUP).sort((a, b) => a - b);
    return { p50: quantile(sorted, 0.5), p95: quantile(sorted, 0.95) };
  } finally {
    await browser.close();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
}

/* ---------- the bench ---------- */

describe('M-E5 apply-to-file latency and store size', () => {
  it('calibrates the wire reconstruction against a live node at the smallest size', async () => {
    await calibrate(SIZES[0]);
  });

  for (const objects of SIZES) {
    it(`${objects} TreeNode objects`, async () => {
      const doc = behaviourTree(objects);
      expect(countTreeNodes(doc)).toBe(objects);
      const wire = encodeWire(doc);
      const wireBytes = wireStateBytes(wire);
      const dir = join(tempRoot, `store-${objects}`);
      const store = fsModelStore(dir);
      const iterations = iterationsFor(objects);
      const whole: number[] = [];
      const applyOnly: number[] = [];
      const projectOnly: number[] = [];
      let storeBytes = 0;
      for (let i = 0; i < iterations + WARMUP; i++) {
        const started = performance.now();
        const result = await applyModel(wire, bt, null);
        const applied = performance.now();
        const projection = await projectModel(store, result);
        const done = performance.now();
        if (!result.applied || result.binding.kind !== 'bound') throw new Error('the apply did not go through as bound');
        if (projection.kind !== 'written') throw new Error(`the projection was ${projection.kind}`);
        storeBytes = projection.file.bytes;
        if (i >= WARMUP) {
          whole.push(done - started);
          applyOnly.push(applied - started);
          projectOnly.push(done - applied);
        }
      }
      const file = join(dir, `${MODEL_ID}.json`);
      expect(statSync(file).size).toBe(storeBytes);
      expect(JSON.parse(readFileSync(file, 'utf8'))).toEqual(doc);
      // The fixed threshold: the projection is never larger than the wire state.
      expect(storeBytes).toBeLessThanOrEqual(wireBytes);

      const opfs = await opfsWrite(readFileSync(file, 'utf8'), iterations);
      whole.sort((a, b) => a - b);
      applyOnly.sort((a, b) => a - b);
      projectOnly.sort((a, b) => a - b);
      const row: Row = {
        run: 1,
        objects,
        apply_to_file_p50_ms: +quantile(whole, 0.5).toFixed(3),
        apply_to_file_p95_ms: +quantile(whole, 0.95).toFixed(3),
        store_bytes: storeBytes,
        wire_state_bytes: wireBytes,
        iterations,
        apply_p50_ms: +quantile(applyOnly, 0.5).toFixed(3),
        project_p50_ms: +quantile(projectOnly, 0.5).toFixed(3),
        bytes_per_object: +(storeBytes / objects).toFixed(1),
        opfs_write_p50_ms: opfs === null ? '' : +opfs.p50.toFixed(3),
        opfs_write_p95_ms: opfs === null ? '' : +opfs.p95.toFixed(3),
      };
      rows.push(row);
      console.log(
        `M-E5 ${objects} objects: apply-to-file p50 ${row.apply_to_file_p50_ms} ms, p95 ${row.apply_to_file_p95_ms} ms ` +
          `(apply ${row.apply_p50_ms}, project ${row.project_p50_ms}); store ${storeBytes} B <= wire ${wireBytes} B` +
          (opfs === null ? '' : `; OPFS write p50 ${row.opfs_write_p50_ms} ms, p95 ${row.opfs_write_p95_ms} ms`),
      );
    });
  }
});
