/// <reference types="node" />
/**
 * Live interpreted `model_node` processes for the level-4 tests and the
 * headless-Chrome harness: started the way the e2e process backend starts them
 * (moirai-network/tests/e2e_convergence.rs, `ProcessBackend`), on free
 * loopback ports, peered by `PEERS=id:host:port`, with `METAMODEL_DIR`
 * holding the descriptors each node serves, and killed when the test is done.
 *
 * The binary is `MOIRAI_E2E_NODE_BIN` when set, else the interpreted node
 * example built in the moirai worktree beside this one
 * (`moirai-model-plane/target/debug/examples/model_node`). It is the
 * interpreted replica and not the generated `network_node`, because the
 * editor speaks the interpreted `ModelOp` dialect and a generated node
 * answers every one of its operations with a 400.
 * Without it a test follows the suite's skip contract: it prints
 * `E2E-SKIP <scenario>: <why>` and skips, so the unit run stays green on a
 * machine with no Rust toolchain, and a CI that greps the marker fails.
 *
 * Test-only: imported by *.test.ts files and the harness, never by the app.
 */

import { spawn, type ChildProcess } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { stringifyModelOp } from '../api/client';
import type { Descriptor, MetamodelId, ModelId, ModelOp, PlainJson } from '../api/types';
import { encodeBatch } from '../crdt/encode';
import type { EditOp } from '../crdt/ops';
import { slotTable } from '../crdt/table';

/** The repository's `examples/`, the descriptors the rig image ships under `/metamodels`. */
export const EXAMPLES_DIR = fileURLToPath(new URL('../../../../examples/', import.meta.url));

/** The default log every node hosts, pinned like the e2e suite pins `E2E_LOG_ID`, so the two nodes share it. */
export const HARNESS_LOG_ID = 'e2ee2ee2ee2ee2ee2ee2ee2ee2ee2ee2';

/** Where the node binary is expected, or null when there is none to run. */
export function nodeBinary(): string | null {
  const fromEnv = process.env['MOIRAI_E2E_NODE_BIN'];
  if (fromEnv !== undefined && fromEnv.length > 0) return existsSync(fromEnv) ? fromEnv : null;
  const built = fileURLToPath(
    new URL('../../../../../moirai-model-plane/target/debug/examples/model_node', import.meta.url),
  );
  return existsSync(built) ? built : null;
}

/** The `E2E-SKIP` line for `scenario` when no binary is present, else null. */
export function skipReason(scenario: string): string | null {
  if (nodeBinary() !== null) return null;
  const reason = `E2E-SKIP ${scenario}: no node binary; build the model_node example in moirai-model-plane or set MOIRAI_E2E_NODE_BIN`;
  console.warn(reason);
  return reason;
}

/**
 * The `E2E-SKIP` line for a scenario that needs a `__model` header in the
 * document, which an interpreted replica does not write.
 *
 * A model's identity lives in the `Install` operation that opened its log and
 * in the node's own registration (`GET /api/models` answers the pair), and
 * the state route serves the model document and nothing else. So the binding
 * check (model/binding.ts) never answers `bound` against an interpreted node,
 * and every behaviour keyed on that verdict — the projection file and the
 * conformance report among them — is out of reach until the check is given
 * the registration as its source for the pair.
 *
 * Skipped rather than deleted, and said out loud, because that is a decision
 * about where a model's identity comes from and not a test to quietly drop.
 */
export function headerlessSkip(scenario: string): string {
  const reason =
    `E2E-SKIP ${scenario}: the interpreted node writes no __model header into the document, so the binding ` +
    `check never answers \`bound\`; the scenario needs the model's identity to come from its registration instead`;
  console.warn(reason);
  return reason;
}

/** A free loopback port: bound, read, released. The kernel does not hand the same one out twice in quick succession. */
export function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.unref();
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      const port = typeof address === 'object' && address !== null ? address.port : 0;
      server.close(() => (port > 0 ? resolve(port) : reject(new Error('no port'))));
    });
  });
}

/** Poll `probe` until it answers a non-null value or the deadline passes. */
export async function waitFor<T>(
  what: string,
  probe: () => Promise<T | null> | T | null,
  { timeoutMs = 30_000, intervalMs = 100 }: { timeoutMs?: number; intervalMs?: number } = {},
): Promise<T> {
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown = null;
  for (;;) {
    try {
      const value = await probe();
      if (value !== null) return value;
    } catch (err) {
      lastError = err;
    }
    if (Date.now() >= deadline) {
      throw new Error(
        `timed out after ${timeoutMs} ms waiting for ${what}${
          lastError === null ? '' : ` (last error: ${lastError instanceof Error ? lastError.message : String(lastError)})`
        }`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
}

/** A fresh scratch directory under the OS temporary directory. */
export function scratchDir(prefix: string): string {
  return mkdtempSync(join(tmpdir(), `${prefix}-`));
}

/**
 * A `METAMODEL_DIR` holding the descriptors given, each written pretty under
 * its file name, so the node computes their digests over the parsed value.
 */
export function writeMetamodelDir(dir: string, descriptors: Record<string, unknown>): string {
  mkdirSync(dir, { recursive: true });
  for (const [file, descriptor] of Object.entries(descriptors)) {
    writeFileSync(join(dir, file), JSON.stringify(descriptor, null, 2));
  }
  return dir;
}

/** The checked-in descriptor `file` of `examples/`, parsed. */
export function readExampleDescriptor(file: string): unknown {
  return JSON.parse(readFileSync(join(EXAMPLES_DIR, file), 'utf8'));
}

export interface NodeSpec {
  /** REPLICA_ID, and the name its peers dial it by. */
  name: string;
  /** The directory of descriptors the node serves. */
  metamodelDir: string;
  /** More environment on top of the basics; a later value wins. */
  env?: Record<string, string>;
}

/** One running node process. */
export class LiveNode {
  readonly name: string;
  readonly url: string;
  readonly listenPort: number;
  readonly httpPort: number;
  readonly logPath: string;
  readonly #child: ChildProcess;

  constructor(name: string, listenPort: number, httpPort: number, logPath: string, child: ChildProcess) {
    this.name = name;
    this.url = `http://127.0.0.1:${httpPort}`;
    this.listenPort = listenPort;
    this.httpPort = httpPort;
    this.logPath = logPath;
    this.#child = child;
  }

  get pid(): number | undefined {
    return this.#child.pid;
  }

  get exited(): boolean {
    return this.#child.exitCode !== null || this.#child.signalCode !== null;
  }

  logTail(lines = 40): string {
    let text = '';
    try {
      text = readFileSync(this.logPath, 'utf8');
    } catch (err) {
      return `<${err instanceof Error ? err.message : String(err)}>`;
    }
    const all = text.split('\n');
    return all.slice(Math.max(0, all.length - lines)).join('\n');
  }

  /** SIGKILL and reap: the node has no shutdown route and blocks in its loop. */
  async stop(): Promise<void> {
    if (this.exited) return;
    const exited = new Promise<void>((resolve) => this.#child.once('exit', () => resolve()));
    this.#child.kill('SIGKILL');
    await exited;
  }
}

/**
 * Start `specs` as one peered session: every node gets every other node in
 * its `PEERS`, all share `HARNESS_LOG_ID` as their default log, and the call
 * returns once every node answers `/api/health` and reports every other node
 * as `Connected` on `/api/peers`, which is the readiness gate the e2e suite
 * uses (`await_mesh`): the first node's one dial always fails because its
 * peer is not listening yet, and the mesh forms when the peer dials back.
 */
export async function startNodes(specs: NodeSpec[], runDir: string): Promise<LiveNode[]> {
  const binary = nodeBinary();
  if (binary === null) throw new Error('no node binary');
  mkdirSync(runDir, { recursive: true });
  const ports = await Promise.all(
    specs.map(async () => ({ listen: await freePort(), http: await freePort() })),
  );
  const nodes = specs.map((spec, index) => {
    const peers = specs
      .map((other, j) => `${other.name}:127.0.0.1:${ports[j].listen}`)
      .filter((_, j) => j !== index)
      .join(',');
    const logPath = join(runDir, `${spec.name}.log`);
    const fd = openSync(logPath, 'w');
    const child = spawn(binary, [], {
      cwd: runDir,
      env: {
        ...process.env,
        REPLICA_ID: spec.name,
        LISTEN_PORT: String(ports[index].listen),
        HTTP_PORT: String(ports[index].http),
        PEERS: peers,
        LOG_ID: HARNESS_LOG_ID,
        METAMODEL_DIR: spec.metamodelDir,
        ...(spec.env ?? {}),
      },
      stdio: ['ignore', fd, fd],
    });
    return new LiveNode(spec.name, ports[index].listen, ports[index].http, logPath, child);
  });
  // Belt and braces: a test process that dies without unwinding still kills its children.
  const killAll = () => {
    for (const node of nodes) if (!node.exited) void node.stop();
  };
  process.once('exit', killAll);
  try {
    for (const node of nodes) {
      await waitFor(`${node.name} to answer /api/health`, async () => {
        if (node.exited) throw new Error(`${node.name} exited; its log:\n${node.logTail()}`);
        const response = await fetch(`${node.url}/api/health`).catch(() => null);
        return response !== null && response.ok ? true : null;
      });
    }
    if (nodes.length > 1) {
      await waitFor('the mesh to form', async () => {
        for (const node of nodes) {
          const peers = (await (await fetch(`${node.url}/api/peers`)).json()) as {
            peers?: { id?: string; status?: string }[];
          };
          const connected = new Set(
            (peers.peers ?? []).filter((p) => p.status === 'Connected').map((p) => p.id ?? ''),
          );
          for (const other of nodes) {
            if (other !== node && !connected.has(other.name)) return null;
          }
        }
        return true;
      });
    }
  } catch (err) {
    killAll();
    throw err;
  }
  return nodes;
}

/* ---------- the model routes, raw, for a test's own setup and oracle ---------- */

/** POST /api/models with the body given; the status is part of the answer. */
export async function postModels(
  node: LiveNode,
  body: { model_id?: ModelId; metamodel_id: MetamodelId },
): Promise<{ status: number; body: Record<string, unknown> }> {
  const response = await fetch(`${node.url}/api/models`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return { status: response.status, body: (await response.json()) as Record<string, unknown> };
}

/** Create a model on `node` under `metamodelId`; the id the node minted. */
export async function createModel(node: LiveNode, metamodelId: MetamodelId): Promise<ModelId> {
  const { status, body } = await postModels(node, { metamodel_id: metamodelId });
  if (status !== 201) throw new Error(`${node.name} did not create a model: ${status} ${JSON.stringify(body)}`);
  return body['model_id'] as ModelId;
}

/** Join the model `id` on `node` under `metamodelId`. */
export async function joinModel(node: LiveNode, id: ModelId, metamodelId: MetamodelId): Promise<void> {
  const { status, body } = await postModels(node, { model_id: id, metamodel_id: metamodelId });
  if (status !== 200) throw new Error(`${node.name} did not join ${id}: ${status} ${JSON.stringify(body)}`);
}

/** The ids `GET /api/models` lists on `node`. */
export async function hostedIds(node: LiveNode): Promise<ModelId[]> {
  const body = (await (await fetch(`${node.url}/api/models`)).json()) as { models?: { model_id?: string }[] };
  return (body.models ?? []).map((m) => m.model_id ?? '').sort();
}

/**
 * GET /api/model/{id}/state on `node`: the document itself, which is what an
 * interpreted node serves and what every assertion reads.
 */
export async function modelState(node: LiveNode, id: ModelId): Promise<PlainJson> {
  const response = await fetch(`${node.url}/api/model/${id}/state`);
  if (!response.ok) throw new Error(`${node.name} /api/model/${id}/state: ${response.status}`);
  return (await response.json()) as PlainJson;
}

/** Apply `ops` to one model on `node`, one at a time, failing on the first refusal. */
export async function applyOps(node: LiveNode, id: ModelId, ops: ModelOp[]): Promise<void> {
  for (const op of ops) {
    const response = await fetch(`${node.url}/api/model/${id}/op`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: stringifyModelOp(op),
    });
    const body = (await response.json()) as { success?: boolean; message?: string; error?: string };
    if (!response.ok || body.success !== true) {
      throw new Error(`${node.name} refused ${JSON.stringify(op)} on ${id}: ${response.status} ${JSON.stringify(body)}`);
    }
  }
}

/** Wait until every node's decoded state for `id` equals `expected` (deep), returning it. */
export async function waitForState(
  nodes: LiveNode[],
  id: ModelId,
  expected: PlainJson,
  timeoutMs = 30_000,
): Promise<PlainJson> {
  const want = JSON.stringify(sortKeys(expected));
  return waitFor(
    `${nodes.map((n) => n.name).join(', ')} to agree on ${id.slice(0, 8)}…`,
    async () => {
      for (const node of nodes) {
        const got = JSON.stringify(sortKeys(await modelState(node, id)));
        if (got !== want) return null;
      }
      return expected;
    },
    { timeoutMs, intervalMs: 200 },
  );
}

/** Wait until every node's decoded state for `id` is the same, and return it. */
export async function waitForAgreement(nodes: LiveNode[], id: ModelId, timeoutMs = 30_000): Promise<PlainJson> {
  return waitFor(
    `${nodes.map((n) => n.name).join(', ')} to agree on ${id.slice(0, 8)}…`,
    async () => {
      const states = await Promise.all(nodes.map((node) => modelState(node, id)));
      const texts = states.map((state) => JSON.stringify(sortKeys(state)));
      return texts.every((text) => text === texts[0]) && states[0] !== null ? states[0] : null;
    },
    { timeoutMs, intervalMs: 200 },
  );
}

/** `value` with object keys sorted at every level, for order-blind comparison. */
export function sortKeys(value: PlainJson): PlainJson {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, sortKeys(value[key])]),
    );
  }
  return value;
}

/**
 * Apply edit intents to one model on `node`: the slot table is read from
 * `descriptor` and the intents are encoded against the state the node
 * currently serves, which is what a session does when it sends them.
 */
export async function applyEdits(
  node: LiveNode,
  id: ModelId,
  descriptor: Descriptor,
  edits: EditOp[],
): Promise<void> {
  const doc = await modelState(node, id);
  await applyOps(node, id, encodeBatch(slotTable(descriptor), doc, edits));
}
