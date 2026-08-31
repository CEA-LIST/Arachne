import { describe, expect, it } from 'vitest';
import type { JsonOp, OpResult } from '../api/types';
import { FieldRegistry, TYPING_THRESHOLD_MS } from './fieldRegistry';
import { OpQueue } from './opQueue';

const op = (n: number): JsonOp => ({ Number: { Inc: n } });

describe('OpQueue', () => {
  it('posts ops of a batch strictly in order', async () => {
    const sent: JsonOp[] = [];
    const queue = new OpQueue(async (o) => {
      sent.push(o);
      return { success: true, message: 'ok' };
    });
    const outcome = await queue.enqueue([op(1), op(2), op(3)]);
    expect(outcome).toEqual({ outcome: 'ok', applied: 3 });
    expect(sent).toEqual([op(1), op(2), op(3)]);
  });

  it('never interleaves batches, even when posts are slow', async () => {
    const sent: number[] = [];
    const queue = new OpQueue(async (o) => {
      const n = (o as { Number: { Inc: number } }).Number.Inc;
      // First batch's ops are slower than the second batch's.
      await new Promise((r) => setTimeout(r, n < 10 ? 10 : 0));
      sent.push(n);
      return { success: true, message: 'ok' };
    });
    const first = queue.enqueue([op(1), op(2)]);
    const second = queue.enqueue([op(10), op(11)]);
    await Promise.all([first, second]);
    expect(sent).toEqual([1, 2, 10, 11]);
  });

  it('a refused op (200 + success:false) aborts the rest of its batch and is reported', async () => {
    const sent: number[] = [];
    const queue = new OpQueue(async (o) => {
      const n = (o as { Number: { Inc: number } }).Number.Inc;
      sent.push(n);
      const refuse = n === 2;
      return { success: !refuse, message: refuse ? 'Operation not enabled' : 'ok' };
    });
    const outcome = await queue.enqueue([op(1), op(2), op(3)]);
    expect(outcome).toEqual({ outcome: 'refused', applied: 1, detail: 'Operation not enabled' });
    expect(sent).toEqual([1, 2]); // op 3 never sent
    expect(queue.pendingCount).toBe(0);
  });

  it('an HTTP/network error is reported and the queue keeps serving later batches', async () => {
    let calls = 0;
    const queue = new OpQueue(async (): Promise<OpResult> => {
      calls++;
      if (calls === 1) throw new Error('boom');
      return { success: true, message: 'ok' };
    });
    const bad = await queue.enqueue([op(1), op(2)]);
    expect(bad.outcome).toBe('error');
    expect(bad.applied).toBe(0);
    expect(bad.detail).toContain('boom');
    const good = await queue.enqueue([op(3)]);
    expect(good).toEqual({ outcome: 'ok', applied: 1 });
    expect(queue.pendingCount).toBe(0);
  });

  it('tracks the pending-op count across a batch', async () => {
    const counts: number[] = [];
    const queue = new OpQueue(
      async () => ({ success: true, message: 'ok' }),
      (n) => counts.push(n),
    );
    await queue.enqueue([op(1), op(2)]);
    expect(counts).toEqual([2, 1, 0]);
  });

  it('an empty batch resolves ok without posting', async () => {
    const queue = new OpQueue(async () => {
      throw new Error('must not post');
    });
    expect(await queue.enqueue([])).toEqual({ outcome: 'ok', applied: 0 });
  });
});

describe('FieldRegistry (focus preservation)', () => {
  const doc = { name: 'remote', other: 'x' };

  it('writes an actively-typing field back into the refreshed doc', () => {
    const registry = new FieldRegistry();
    const now = 10_000;
    registry.register(JSON.stringify(['name']), () => ({
      value: 'local-typing',
      lastEditAt: now - 100,
      focused: true,
    }));
    expect(registry.overlay(doc, now)).toEqual({ name: 'local-typing', other: 'x' });
  });

  it('does not overlay a focused but idle field (threshold expired)', () => {
    const registry = new FieldRegistry();
    const now = 10_000;
    registry.register(JSON.stringify(['name']), () => ({
      value: 'stale-local',
      lastEditAt: now - TYPING_THRESHOLD_MS - 1,
      focused: true,
    }));
    expect(registry.overlay(doc, now)).toEqual(doc);
  });

  it('does not overlay an unfocused field even if recently edited', () => {
    const registry = new FieldRegistry();
    const now = 10_000;
    registry.register(JSON.stringify(['name']), () => ({
      value: 'local',
      lastEditAt: now - 10,
      focused: false,
    }));
    expect(registry.overlay(doc, now)).toEqual(doc);
  });

  it('unregister removes the field', () => {
    const registry = new FieldRegistry();
    const now = 10_000;
    const unregister = registry.register(JSON.stringify(['name']), () => ({
      value: 'local',
      lastEditAt: now,
      focused: true,
    }));
    unregister();
    expect(registry.overlay(doc, now)).toEqual(doc);
  });
});
