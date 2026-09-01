/**
 * When an edit may be computed, and what the UI says while it may not.
 *
 * The defect this exists for, reproduced twice against a live rig: a structural
 * edit (add / remove / reorder / reference write) is computed from the client's
 * CURRENT view of the document — an index, an array length, or, for a reorder,
 * the whole child subtree that gets deleted and re-created, because the wire has
 * no move op. While a batch is draining, that view is the replica's half-applied
 * state (the 500 ms poll faithfully reports it), so an edit computed then is
 * computed from a document that does not exist yet. Measured consequence:
 * elements silently destroyed, with every log row reporting `ok`.
 *
 * The rule has two levels, because the two reproduced failures are not the
 * same failure:
 *
 *   SNAPSHOT edits — today only reorder, which is delete + re-create of the
 *   whole child — copy a subtree out of the local document. ANY op in flight
 *   can be rewriting that subtree, including a per-character string flush. This
 *   is the first reproduction: two reorders queued behind a pending string edit
 *   re-created an element from its half-typed state, dropping its name and its
 *   child. So they need a completely quiet queue.
 *
 *   INDEX edits — add, remove, unset, reference writes — need a position and an
 *   array length, and only a STRUCTURAL batch moves those. This is the second
 *   reproduction: add/remove issued while a reorder's re-create was draining
 *   destroyed an element. So they need no structural batch in flight. Holding
 *   them on any pending op instead would be safe but useless: one typed name is
 *   ~20 ops at ~400 ms each, and a form that locks Add for ten seconds after
 *   every name is not a form anyone can use.
 *
 *   VALUE edits are held exactly while a structural batch is in flight: a
 *   string diff computed against an element being re-created is as corrupt as a
 *   mis-indexed remove. They are NEVER held on the user's own typing, which
 *   would lock a field mid-word, since each character is an op.
 *
 * All three then wait for one poll after the queue empties, so the next edit is
 * computed from what the replica actually says.
 *
 * The real fix is below the UI — a move op on the wire, or re-creates rebased on
 * server state — and is not ours to make here. This is the guard, not the cure,
 * and it says so on every held control.
 *
 * Pure: no React and no clock of its own, so every state below is unit-tested.
 */

/** A structural batch currently being applied. */
export interface StructuralBatch {
  /** The intent, as written to the action log. */
  description: string;
  /** Ops in the batch: the denominator of the progress affordance. */
  total: number;
}

export interface QueueSnapshot {
  /** store.pendingOps: ops enqueued and not yet acknowledged, all batches. */
  pendingOps: number;
  /** The structural batch in flight, or null when none is. */
  batch: StructuralBatch | null;
  /** When the last structural batch resolved; null before the first one. */
  settledAt: number | null;
  /** store.lastSyncAt: the poll's own clock. */
  lastSyncAt: number | null;
  /** The UI ticker, so the reconcile window cannot hang on a dead poll. */
  now: number;
}

export interface FlushProgress {
  description: string;
  total: number;
  /** Ops acknowledged so far. Never above `total`, never below 0. */
  done: number;
  /** 0–100, floored; 0 while nothing has landed yet. */
  percent: number;
}

export interface EditGate {
  /** Non-null exactly while a structural batch is being applied. */
  progress: FlushProgress | null;
  /** A structural batch is in flight, or its result is not yet read back. */
  structureBusy: boolean;
  /** Index edits: add, remove, unset, reference writes. */
  canEditStructure: boolean;
  /** Subtree-snapshot edits: reorder. Stricter — needs a silent queue. */
  canReorder: boolean;
  canEditValues: boolean;
  /** Why index edits are held; null when they are free. */
  structureHeldReason: string | null;
  /** Why reorder is held; null when it is free. */
  reorderHeldReason: string | null;
  /** Why value fields are held; null when they are free. */
  valuesHeldReason: string | null;
}

/**
 * Longest we keep holding edits waiting for the poll to read the result back.
 * Without a cap, a replica that stops answering mid-batch would leave the form
 * held for ever — the offline chip is how that gets reported, not a dead UI.
 */
export const RECONCILE_MAX_MS = 2000;

/** Why an index edit waits for the structural batch, said once. */
export const STRUCTURE_HELD_HINT =
  'this edit is computed from the current document, so it waits until the change being applied has landed';

/** Why a reorder waits for silence, said once. */
export const REORDER_HELD_HINT =
  'a reorder re-creates the element from the current document (the wire has no move op), so it waits until the replica has acknowledged every queued operation';

export function flushProgress(batch: StructuralBatch, pendingOps: number): FlushProgress {
  const done = Math.max(0, Math.min(batch.total, batch.total - pendingOps));
  return {
    description: batch.description,
    total: batch.total,
    done,
    percent: batch.total === 0 ? 100 : Math.floor((done / batch.total) * 100),
  };
}

/** One-line progress text: the same sentence in the top bar and in the log. */
export function flushLabel(progress: FlushProgress): string {
  return `${progress.description} — ${progress.done} of ${progress.total} op${
    progress.total === 1 ? '' : 's'
  } applied`;
}

export function editGate(snapshot: QueueSnapshot): EditGate {
  const { pendingOps, batch, settledAt, lastSyncAt, now } = snapshot;

  const progress = batch === null ? null : flushProgress(batch, pendingOps);

  // After the last op lands, the local document is still the optimistic view;
  // one poll has to bring the replica's own answer back before anything may be
  // computed from it. Capped, so a silent replica cannot hold the UI for ever.
  const reconciling =
    batch === null &&
    settledAt !== null &&
    (lastSyncAt === null || lastSyncAt < settledAt) &&
    now - settledAt < RECONCILE_MAX_MS;

  const structureBusy = batch !== null || reconciling;

  const structureHeldReason =
    progress !== null
      ? `${flushLabel(progress)} — ${STRUCTURE_HELD_HINT}`
      : reconciling
        ? 'Reading the replica back after the last change'
        : null;

  const reorderHeldReason =
    structureHeldReason ??
    (pendingOps > 0
      ? `${pendingOps} op${pendingOps === 1 ? '' : 's'} still queued — ${REORDER_HELD_HINT}`
      : null);

  const valuesHeldReason =
    progress !== null
      ? `${flushLabel(progress)} — fields are held while the element is being rebuilt on the replica`
      : reconciling
        ? 'Reading the replica back after the last change'
        : null;

  return {
    progress,
    structureBusy,
    canEditStructure: !structureBusy,
    canReorder: !structureBusy && pendingOps === 0,
    canEditValues: !structureBusy,
    structureHeldReason,
    reorderHeldReason,
    valuesHeldReason,
  };
}
