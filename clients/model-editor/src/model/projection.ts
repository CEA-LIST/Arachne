/**
 * The projection step of the adaptation layer: after an apply, write the
 * decoded document to the model store under the model's id.
 *
 * The write follows `Applied` and never `Refused`, and among the applied
 * verdicts only `bound`: an unbound log has no header and so no id to file it
 * under, and a header with no descriptor to check it against is not yet a
 * binding. What is written is the decoded state the node served, the document
 * the log vouches for, not the view's overlay of fields being typed.
 *
 * A store that fails, or a context with no store, never touches the apply:
 * the verdict is reported and the view keeps the document. Nothing in this
 * module reads the store; the projection is one-way by construction.
 */

import type { ModelId } from '../api/types';
import type { ApplyResult } from './binding';
import type { ModelStore, StoredFile } from './store';

export type Projection =
  /** No store in this context: the browser exposes no origin-private file system. */
  | { kind: 'unavailable' }
  /** Nothing to write: the apply was refused, or applied without a checked binding. */
  | { kind: 'not-written'; why: 'refused' | 'unbound' | 'no-descriptor' }
  /** The model's file holds the applied document. */
  | { kind: 'written'; file: StoredFile; ts: number }
  /** The store failed; the view is unaffected and the file, if any, is stale. */
  | { kind: 'failed'; modelId: ModelId; error: string; ts: number };

/** Write the applied document to the store, or say why not. Never throws. */
export async function projectModel(store: ModelStore | null, result: ApplyResult): Promise<Projection> {
  if (!result.applied) return { kind: 'not-written', why: 'refused' };
  const { binding } = result;
  if (binding.kind !== 'bound') return { kind: 'not-written', why: binding.kind };
  if (store === null) return { kind: 'unavailable' };
  const { modelId } = binding.header;
  try {
    return { kind: 'written', file: await store.write(modelId, result.doc), ts: Date.now() };
  } catch (err) {
    return {
      kind: 'failed',
      modelId,
      error: err instanceof Error ? err.message : String(err),
      ts: Date.now(),
    };
  }
}

/** Whether two projections say the same thing, timestamps aside, so a change is reported once and not on every poll. */
export function sameProjection(a: Projection, b: Projection): boolean {
  switch (a.kind) {
    case 'unavailable':
      return b.kind === 'unavailable';
    case 'not-written':
      return b.kind === 'not-written' && a.why === b.why;
    case 'written':
      return b.kind === 'written' && a.file.file === b.file.file && a.file.digest === b.file.digest;
    case 'failed':
      return b.kind === 'failed' && a.modelId === b.modelId && a.error === b.error;
  }
}

function describeNotWritten(why: Extract<Projection, { kind: 'not-written' }>['why']): string {
  switch (why) {
    case 'refused':
      return 'no projection written: the apply was refused';
    case 'unbound':
      return 'no projection written: the log carries no __model header, so there is no model id to file it under';
    case 'no-descriptor':
      return 'no projection written: the binding is unchecked while no descriptor is loaded';
  }
}

/** One sentence per outcome: the chip's title, the log row and the alert say this. */
export function describeProjection(projection: Projection): string {
  switch (projection.kind) {
    case 'unavailable':
      return 'no model store: this context exposes no origin-private file system, so no projection file is written; the view is unaffected';
    case 'not-written':
      return describeNotWritten(projection.why);
    case 'written':
      return `projection written: ${projection.file.file} (${projection.file.bytes} bytes, sha256 ${projection.file.digest}) since ${new Date(projection.ts).toLocaleTimeString()}; the file is regenerated from the log at every apply and never read for state`;
    case 'failed':
      return `model store write failed for ${projection.modelId}: ${projection.error}; the view is unaffected and the file, if any, is stale`;
  }
}

/** The word the top bar shows beside the binding. */
export function projectionLabel(projection: Projection): string {
  switch (projection.kind) {
    case 'unavailable':
      return 'no store';
    case 'not-written':
      return 'not stored';
    case 'written':
      return 'stored';
    case 'failed':
      return 'store failed';
  }
}
