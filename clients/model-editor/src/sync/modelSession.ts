/**
 * One open model: its poll, its `OpQueue`, its field registry and the
 * adaptation layer's memory, addressed to the model's own routes on the node
 * it was opened against.
 *
 * A session reports to its tab through `SessionEvents` and knows nothing of
 * React: the hook (useSync.ts) turns each event into a reducer action, and the
 * level-4 tests drive a session against live node processes with no DOM. The
 * sync path is the one step 4 and step 5 built, per model now: on open and on
 * every poll, fetch `GET /api/model/{id}/state`, run the binding check against
 * the descriptor in effect (`GET /api/model/{id}/metamodel`, or a loaded
 * file), apply only when the check lets it through, then write the store.
 * After an apply that answered `bound`, the descriptor's invariants are
 * evaluated over the document (model/conformance.ts) and reported to the tab
 * when the report changed; a report never refuses an apply and never reaches
 * the edit gate. Nothing here reads the store.
 */

import { ApiError, getModelMetamodel, getModelState, postModelOp } from '../api/client';
import type { Descriptor, ModelHeader, ModelId, Path, PlainJson } from '../api/types';
import { encodeBatch } from '../crdt/encode';
import { MODEL_HEADER_REFUSAL, opTouchesModelHeader, type EditOp } from '../crdt/ops';
import { setAtPath } from '../crdt/path';
import { slotTable, type SlotTable } from '../crdt/table';
import {
  applyModel,
  describeBinding,
  isRefusal,
  sameBinding,
  type ApplyResult,
  type Binding,
} from '../model/binding';
import { checkInvariants, describeDiagnostics, sameDiagnostics, type Diagnostic } from '../model/conformance';
import { describeProjection, projectModel, sameProjection, type Projection } from '../model/projection';
import type { ModelStore } from '../model/store';
import type { LogEntry, TabPatch } from '../state/store';
import { FieldRegistry } from './fieldRegistry';
import { OpQueue, type BatchOutcome } from './opQueue';

/** What a session tells its tab. The log row's id is the hook's to assign. */
export interface SessionEvents {
  patch(patch: TabPatch): void;
  log(entry: Omit<LogEntry, 'id' | 'modelId'>): void;
  banner(message: string): void;
}

export interface SessionOptions {
  id: ModelId;
  nodeUrl: string;
  /** The model store the projection is written to; null means no store, reported and never in the way. */
  store: ModelStore | null;
  pollMs: number;
  events: SessionEvents;
  /** The tab's field registry (state/store.ts); a fresh one when the caller has no tab. */
  registry?: FieldRegistry;
}

export class ModelSession {
  readonly id: ModelId;
  readonly nodeUrl: string;
  /** The typing overlay for this tab's fields (sync/fieldRegistry.ts): the tab's own, so one tab's caret never writes into another's document. */
  readonly registry: FieldRegistry;

  readonly #store: ModelStore | null;
  readonly #events: SessionEvents;
  readonly #queue: OpQueue;
  #pollMs: number;
  #timer: ReturnType<typeof setInterval> | null = null;
  #polling = false;
  #closed = false;

  #descriptor: Descriptor | null = null;
  #descriptorSource: 'node' | 'file' | null = null;
  // The slot table the operations this session sends are addressed by
  // (crdt/table.ts), derived from the descriptor in effect and recomputed
  // whenever that changes. A descriptor the table cannot be read from is not
  // a failed sync — the document still renders — but no edit can be encoded
  // under it, so the reason is kept and said at the first attempt.
  #table: SlotTable | null = null;
  #tableError: string | null = null;
  #doc: PlainJson = null;
  // The binding check's memory (model/binding.ts): the header recorded at the
  // first apply that carried one, and the last verdict, so a change is
  // reported once rather than on every poll; the store's last outcome for
  // the same reason. A session is one connection to one model, so none of
  // these ever resets: closing the tab and opening it again starts fresh.
  #recorded: ModelHeader | null = null;
  #binding: Binding | null = null;
  #projection: Projection | null = null;
  #diagnostics: Diagnostic[] | null = null;
  #loggedDiagnostics: Diagnostic[] | null = null;

  constructor(options: SessionOptions) {
    this.id = options.id;
    this.nodeUrl = options.nodeUrl;
    this.#store = options.store;
    this.#pollMs = options.pollMs;
    this.#events = options.events;
    this.registry = options.registry ?? new FieldRegistry();
    this.#queue = new OpQueue(
      (op) => postModelOp(this.nodeUrl, this.id, op),
      (count) => this.#patch({ pendingOps: count }),
    );
  }

  get descriptor(): Descriptor | null {
    return this.#descriptor;
  }

  /** The slot table the session encodes with, or null when the descriptor in effect yields none. */
  get table(): SlotTable | null {
    return this.#table;
  }

  /** Read the descriptor in effect into a slot table, keeping the reason when it cannot be read. */
  #retable(): void {
    if (this.#descriptor === null) {
      this.#table = null;
      this.#tableError = 'no metamodel descriptor is in effect, so no operation can be addressed';
      return;
    }
    try {
      this.#table = slotTable(this.#descriptor);
      this.#tableError = null;
    } catch (err) {
      this.#table = null;
      this.#tableError = err instanceof Error ? err.message : String(err);
    }
  }

  get doc(): PlainJson {
    return this.#doc;
  }

  get binding(): Binding | null {
    return this.#binding;
  }

  get diagnostics(): Diagnostic[] {
    return this.#diagnostics ?? [];
  }

  get closed(): boolean {
    return this.#closed;
  }

  #patch(patch: TabPatch): void {
    if (this.#closed) return;
    this.#events.patch(patch);
  }

  #log(entry: Omit<LogEntry, 'id' | 'modelId'>): void {
    if (this.#closed) return;
    this.#events.log(entry);
  }

  #banner(message: string): void {
    if (this.#closed) return;
    this.#events.banner(message);
  }

  /**
   * Fetch the model's descriptor and its first state, then poll. Throws when
   * the state cannot be fetched (an id the node does not host, a network
   * failure), after reporting the tab as failed; a missing descriptor is not
   * a failure, it is the file-load fallback.
   */
  async open(): Promise<void> {
    try {
      const descriptor = await getModelMetamodel(this.nodeUrl, this.id);
      this.#descriptor = descriptor;
      this.#descriptorSource = descriptor === null ? null : 'node';
      this.#retable();
      this.#patch({ metamodel: descriptor, metamodelSource: this.#descriptorSource });
    } catch (err) {
      this.#retable();
      this.#patch({ metamodel: null, metamodelSource: null });
      this.#banner(`metamodel fetch failed: ${err instanceof Error ? err.message : String(err)}`);
    }
    try {
      await this.refreshOnce();
    } catch (err) {
      const error = err instanceof ApiError ? err.message : String(err);
      this.#patch({ status: 'failed', error });
      throw err;
    }
    if (this.#closed) return;
    this.#patch({ status: 'open', error: null });
    this.#arm();
  }

  /** Stop polling and drop the queue; every later event is ignored. */
  close(): void {
    this.#closed = true;
    if (this.#timer !== null) clearInterval(this.#timer);
    this.#timer = null;
  }

  setPollMs(ms: number): void {
    this.#pollMs = ms;
    if (this.#timer !== null) this.#arm();
  }

  /**
   * On a session that has none, ask the model's metamodel route again.
   *
   * The one case this exists for is late binding, and it is the second
   * replica's whole experience of it. A model joined under a digest this
   * replica does not hold is hosted with its binding pending: the node has no
   * descriptor to answer `GET /api/model/{id}/metamodel` with at the moment
   * the tab opens, and gets one only when the language arrives inside the
   * model's own history, seconds later. Asked once at open, the tab would
   * render that model untyped for ever and say so — "no features to edit" —
   * until someone closed it and opened it again.
   *
   * Narrow on purpose. A session that already holds a descriptor never asks,
   * whether it came from the node or from a file, so the typed path is
   * untouched and costs nothing; a route that goes on answering 404 leaves
   * the session exactly as it is today. Nothing here decides what a
   * descriptor means: it is installed through the same two steps `open` uses,
   * and the binding check below is the one that has always run.
   */
  async #adoptDescriptor(): Promise<void> {
    if (this.#descriptor !== null) return;
    // A failure is not reported here. The state fetch on the same tick speaks
    // for the connection, and a banner per poll would bury the one thing a
    // person watching a pending model needs to see.
    const descriptor = await getModelMetamodel(this.nodeUrl, this.id).catch(() => null);
    if (descriptor === null || this.#closed || this.#descriptor !== null) return;
    this.#descriptor = descriptor;
    this.#descriptorSource = 'node';
    this.#retable();
    this.#patch({ metamodel: descriptor, metamodelSource: 'node' });
    this.#log({
      ts: Date.now(),
      description: 'adopt metamodel',
      ops: [],
      outcome: 'ok',
      detail: `the node now serves this model's metamodel (${descriptor.nsURI}); it arrived with the model, and the document is typed from here on`,
    });
  }

  /** Render under a descriptor loaded from a file; the next poll checks the binding against it. */
  loadDescriptorFile(descriptor: Descriptor): void {
    this.#descriptor = descriptor;
    this.#descriptorSource = 'file';
    this.#retable();
    this.#patch({ metamodel: descriptor, metamodelSource: 'file' });
  }

  #arm(): void {
    if (this.#timer !== null) clearInterval(this.#timer);
    // Every open tab polls, the background ones included: a tab that stops
    // reading its log shows a stale document the moment it is switched to.
    this.#timer = setInterval(() => void this.#poll(), this.#pollMs);
  }

  async #poll(): Promise<void> {
    if (this.#polling || this.#closed) return;
    this.#polling = true;
    try {
      await this.refreshOnce();
    } catch (err) {
      this.#banner(`sync failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      this.#polling = false;
    }
  }

  /**
   * The projection after an apply: written on `bound`, nothing otherwise. A
   * change is reported once; a failure reaches the log and the alert dock
   * and leaves the view alone, since the file is a projection of the log
   * and never what the view is built from.
   */
  async #project(result: ApplyResult): Promise<void> {
    const projection = await projectModel(this.#store, result);
    const changed = this.#projection === null || !sameProjection(this.#projection, projection);
    this.#projection = projection;
    if (!changed) return;
    this.#patch({ projection });
    const detail = describeProjection(projection);
    if (projection.kind === 'unavailable') {
      console.warn(detail);
    } else if (projection.kind === 'failed') {
      this.#log({ ts: projection.ts, description: 'write model file', ops: [], outcome: 'error', detail });
      this.#banner(detail);
    }
  }

  /**
   * The invariants after an apply: evaluated on a bound document, empty
   * otherwise. The tab is patched once per change, so the chip follows the
   * document poll by poll; the log gets a row only when the report changed
   * while this session had nothing in flight, because a class name is
   * written one character per operation and a poll in the middle of one
   * sees a half-written tag, which is a report that will be gone at the next
   * poll and not a row anyone wants to read. The row's outcome is
   * `diagnostic`, since nothing sent failed.
   */
  #report(diagnostics: Diagnostic[], ts: number): void {
    if (this.#diagnostics === null || !sameDiagnostics(this.#diagnostics, diagnostics)) {
      this.#diagnostics = diagnostics;
      this.#patch({ diagnostics });
    }
    if (this.#queue.pendingCount > 0) return;
    const logged = this.#loggedDiagnostics;
    if (logged !== null && sameDiagnostics(logged, diagnostics)) return;
    this.#loggedDiagnostics = diagnostics;
    if (diagnostics.length > 0) {
      this.#log({ ts, description: 'conformance', ops: [], outcome: 'diagnostic', detail: describeDiagnostics(diagnostics) });
    } else if (logged !== null && logged.length > 0) {
      this.#log({ ts, description: 'conformance', ops: [], outcome: 'ok', detail: describeDiagnostics(diagnostics) });
    }
  }

  /**
   * One apply: fetch the state, check its binding against the descriptor in
   * effect, the one the document would be rendered under, report the
   * document only when the check lets it through, then write the projection.
   * Runs on open and on every poll, since a header can arrive by transfer
   * after open — and so, on a model joined under a language this replica did
   * not hold, can the descriptor itself. The document always comes from the
   * node.
   */
  async refreshOnce(): Promise<void> {
    await this.#adoptDescriptor();
    const served = await getModelState(this.nodeUrl, this.id);
    if (this.#closed) return;
    const result = await applyModel(served, this.#descriptor, this.#recorded);
    if (this.#closed) return;
    const changed = this.#binding === null || !sameBinding(this.#binding, result.binding);
    this.#binding = result.binding;
    const ts = Date.now();
    if (!result.applied) {
      // Applies nothing. The view is emptied rather than left showing a
      // document the log no longer vouches for, and the poll's own clock
      // still advances so the chip does not call a refusing replica silent.
      this.#doc = null;
      this.#patch({ doc: null, lastSyncAt: ts, ...(changed ? { binding: result.binding } : {}) });
      if (changed) {
        this.#log({ ts, description: 'apply model', ops: [], outcome: 'refused', detail: result.message });
        this.#banner(result.message);
      }
      this.#report([], ts);
      await this.#project(result);
      return;
    }
    if (this.#recorded === null && result.binding.kind !== 'unbound') {
      this.#recorded = result.binding.header;
    }
    this.#doc = result.doc;
    this.#patch({
      doc: this.registry.overlay(result.doc),
      lastSyncAt: ts,
      ...(changed ? { binding: result.binding } : {}),
    });
    // Only a bound document is checked: an unbound log has no descriptor of
    // its own to be held to, and a header with no descriptor has nothing to
    // check against.
    this.#report(
      result.binding.kind === 'bound' && this.#descriptor !== null ? checkInvariants(this.#descriptor, result.doc) : [],
      ts,
    );
    await this.#project(result);
  }

  /**
   * Post an op batch (one edit intent) to this model's op route. Logs the
   * attempt with its outcome; refused/error outcomes also raise the error
   * banner. `optimistic` patches the local doc immediately (the poll
   * reconciles the truth). A batch that would write the model header, or any
   * batch while the binding check refuses the document, is refused here
   * before anything is posted.
   */
  async sendOps(
    description: string,
    ops: EditOp[],
    optimistic?: { path: Path; value: PlainJson },
  ): Promise<BatchOutcome> {
    const refuse = (outcome: BatchOutcome['outcome'], detail: string): BatchOutcome => {
      this.#log({ ts: Date.now(), description, ops, outcome, detail });
      this.#banner(`${description}: ${detail}`);
      return { outcome, applied: 0, detail };
    };
    // The funnel: every op the editor posts passes here, so a write into the
    // model header is refused here too, whatever built it (the builders in
    // crdt/ops.ts already throw; this is the guard behind them).
    if (ops.some(opTouchesModelHeader)) return refuse('refused', MODEL_HEADER_REFUSAL);
    // Nothing may be edited while the binding check refuses the document:
    // the edit gate holds every control, and this is the same rule at the
    // wire, for a caller that did not come through a control.
    if (this.#binding !== null && isRefusal(this.#binding)) return refuse('refused', describeBinding(this.#binding));
    if (this.#closed) return refuse('error', 'the model is closed');
    // The encoding, against the document the intents were computed from and
    // before the optimistic patch moves it: an intent this session cannot
    // address is refused here, naming the feature, rather than sent as a
    // well-formed operation that would land somewhere else.
    if (this.#table === null) {
      return refuse('refused', this.#tableError ?? 'no slot table for this model');
    }
    let encoded;
    try {
      encoded = encodeBatch(this.#table, this.#doc, ops);
    } catch (err) {
      return refuse('refused', err instanceof Error ? err.message : String(err));
    }
    if (optimistic !== undefined) {
      this.#doc = setAtPath(this.#doc, optimistic.path, optimistic.value);
      this.#patch({ doc: this.#doc });
    }
    const result = await this.#queue.enqueue(encoded);
    this.#log({ ts: Date.now(), description, ops, outcome: result.outcome, detail: result.detail });
    if (result.outcome !== 'ok') {
      this.#banner(
        `${description}: ${result.outcome === 'refused' ? 'operation not enabled' : 'failed'}${
          result.detail ? ` (${result.detail})` : ''
        }`,
      );
    }
    return result;
  }
}
