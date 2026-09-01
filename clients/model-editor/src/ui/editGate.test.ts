import { describe, expect, it } from 'vitest';
import {
  editGate,
  flushLabel,
  flushProgress,
  RECONCILE_MAX_MS,
  type QueueSnapshot,
} from './editGate';

const NOW = 1_000_000;

function quiet(overrides: Partial<QueueSnapshot> = {}): QueueSnapshot {
  return {
    pendingOps: 0,
    batch: null,
    settledAt: null,
    lastSyncAt: NOW - 100,
    now: NOW,
    ...overrides,
  };
}

describe('flushProgress', () => {
  it('counts acknowledged ops as total minus pending', () => {
    const progress = flushProgress({ description: 'move x up', total: 133 }, 49);
    expect(progress.done).toBe(84);
    expect(progress.percent).toBe(63);
  });

  it('never reports more than the batch or less than nothing', () => {
    // A second batch can inflate pendingOps beyond this batch's own total.
    expect(flushProgress({ description: 'd', total: 10 }, 40).done).toBe(0);
    expect(flushProgress({ description: 'd', total: 10 }, 0).done).toBe(10);
    expect(flushProgress({ description: 'd', total: 10 }, -5).done).toBe(10);
  });

  it('treats an empty batch as complete rather than dividing by zero', () => {
    expect(flushProgress({ description: 'd', total: 0 }, 0).percent).toBe(100);
  });

  it('labels the batch with its own description and counts', () => {
    expect(flushLabel(flushProgress({ description: 'move a[0] down', total: 133 }, 133))).toBe(
      'move a[0] down — 0 of 133 ops applied',
    );
    expect(flushLabel(flushProgress({ description: 'set x', total: 1 }, 1))).toBe(
      'set x — 0 of 1 op applied',
    );
  });
});

describe('editGate', () => {
  it('lets everything through on a quiet queue', () => {
    const gate = editGate(quiet());
    expect(gate.canEditStructure).toBe(true);
    expect(gate.canReorder).toBe(true);
    expect(gate.canEditValues).toBe(true);
    expect(gate.structureBusy).toBe(false);
    expect(gate.progress).toBeNull();
    expect(gate.structureHeldReason).toBeNull();
    expect(gate.reorderHeldReason).toBeNull();
    expect(gate.valuesHeldReason).toBeNull();
  });

  it('holds a REORDER behind any pending op, including a text flush', () => {
    // The first reproduction: two reorders queued behind a pending string edit
    // re-created the element from its half-typed state, destroying it, while
    // both log rows said `ok`.
    const gate = editGate(quiet({ pendingOps: 7 }));
    expect(gate.canReorder).toBe(false);
    expect(gate.reorderHeldReason).toContain('7 ops still queued');
    expect(gate.reorderHeldReason).toContain('re-creates the element');
  });

  it('does NOT hold an index edit, or a field, behind the user’s own typing', () => {
    // Typing enqueues one op per character at ~400 ms each; holding Add and the
    // fields on that would lock the form for ten seconds after every name.
    const gate = editGate(quiet({ pendingOps: 7 }));
    expect(gate.canEditStructure).toBe(true);
    expect(gate.structureHeldReason).toBeNull();
    expect(gate.canEditValues).toBe(true);
    expect(gate.valuesHeldReason).toBeNull();
  });

  it('holds EVERYTHING while a structural batch is in flight', () => {
    // The second reproduction: add / remove / reference-pick issued while a
    // reorder's re-create was draining destroyed an element.
    const gate = editGate(quiet({ pendingOps: 49, batch: { description: 'move x up', total: 133 } }));
    expect(gate.canEditStructure).toBe(false);
    expect(gate.canReorder).toBe(false);
    expect(gate.canEditValues).toBe(false);
    expect(gate.structureBusy).toBe(true);
    expect(gate.progress).toEqual({
      description: 'move x up',
      total: 133,
      done: 84,
      percent: 63,
    });
  });

  it('names the batch and its progress in both held reasons', () => {
    const gate = editGate(quiet({ pendingOps: 49, batch: { description: 'move x up', total: 133 } }));
    expect(gate.structureHeldReason).toContain('move x up — 84 of 133 ops applied');
    expect(gate.valuesHeldReason).toContain('move x up — 84 of 133 ops applied');
    expect(gate.valuesHeldReason).toContain('rebuilt on the replica');
  });

  it('keeps holding after the last op lands until a poll reads the result back', () => {
    const settledAt = NOW - 200;
    const gate = editGate(quiet({ settledAt, lastSyncAt: settledAt - 50 }));
    expect(gate.structureBusy).toBe(true);
    expect(gate.canEditStructure).toBe(false);
    expect(gate.canEditValues).toBe(false);
    expect(gate.structureHeldReason).toBe('Reading the replica back after the last change');
    expect(gate.progress).toBeNull();
  });

  it('releases as soon as one poll lands after the batch settled', () => {
    const settledAt = NOW - 200;
    const gate = editGate(quiet({ settledAt, lastSyncAt: settledAt + 10 }));
    expect(gate.structureBusy).toBe(false);
    expect(gate.canEditStructure).toBe(true);
  });

  it('gives up waiting for a replica that never answers, rather than freezing the form', () => {
    const settledAt = NOW - RECONCILE_MAX_MS - 1;
    const gate = editGate(quiet({ settledAt, lastSyncAt: settledAt - 5_000 }));
    expect(gate.structureBusy).toBe(false);
    expect(gate.canEditValues).toBe(true);
  });

  it('still holds reorder when the reconcile window expires with ops outstanding', () => {
    const settledAt = NOW - RECONCILE_MAX_MS - 1;
    const gate = editGate(quiet({ settledAt, lastSyncAt: settledAt - 5_000, pendingOps: 3 }));
    expect(gate.canReorder).toBe(false);
    expect(gate.canEditStructure).toBe(true);
    expect(gate.canEditValues).toBe(true);
  });

  it('holds through the whole life of a batch: enqueue, drain, read-back, free', () => {
    const batch = { description: 'add child', total: 4 };
    const enqueued = editGate(quiet({ pendingOps: 4, batch }));
    const draining = editGate(quiet({ pendingOps: 1, batch }));
    const landed = editGate(quiet({ pendingOps: 0, settledAt: NOW, lastSyncAt: NOW - 10 }));
    const readBack = editGate(quiet({ pendingOps: 0, settledAt: NOW, lastSyncAt: NOW + 10 }));

    expect([enqueued, draining, landed].every((g) => !g.canEditStructure)).toBe(true);
    expect([enqueued, draining, landed].every((g) => !g.canReorder)).toBe(true);
    expect(enqueued.progress?.done).toBe(0);
    expect(draining.progress?.done).toBe(3);
    expect(landed.progress).toBeNull();
    expect(readBack.canEditStructure).toBe(true);
    expect(readBack.canEditValues).toBe(true);
  });
});
