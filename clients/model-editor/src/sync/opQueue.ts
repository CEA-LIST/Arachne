import type { JsonOp, OpResult } from '../api/types';

/**
 * A single global FIFO for op batches.
 *
 * Every edit intent becomes one batch (an ordered op sequence). Batches are
 * posted strictly one op at a time, one batch at a time, so sequences from
 * different fields can never interleave on the wire — a hard requirement,
 * since per-character string sequences are position-sensitive.
 *
 * A refused op (HTTP 200, success:false) aborts the REST of its batch: the
 * remaining ops were computed against a state the node just refuted, so
 * continuing would corrupt positions. The failure is reported, never
 * swallowed; the poll loop reconciles the local view.
 */

export interface BatchOutcome {
  outcome: 'ok' | 'refused' | 'error';
  /** Ops actually sent (and accepted) before the stop. */
  applied: number;
  detail?: string;
}

export class OpQueue {
  private chain: Promise<void> = Promise.resolve();
  private pending = 0;

  private readonly post: (op: JsonOp) => Promise<OpResult>;
  private readonly onPendingChange: (count: number) => void;

  constructor(
    post: (op: JsonOp) => Promise<OpResult>,
    onPendingChange: (count: number) => void = () => {},
  ) {
    this.post = post;
    this.onPendingChange = onPendingChange;
  }

  get pendingCount(): number {
    return this.pending;
  }

  enqueue(ops: JsonOp[]): Promise<BatchOutcome> {
    if (ops.length === 0) {
      return Promise.resolve({ outcome: 'ok', applied: 0 });
    }
    this.setPending(this.pending + ops.length);
    const run = this.chain.then(async (): Promise<BatchOutcome> => {
      let applied = 0;
      try {
        for (const op of ops) {
          const result = await this.post(op);
          this.setPending(this.pending - 1);
          if (!result.success) {
            // Drop the rest of the batch from the pending count.
            this.setPending(this.pending - (ops.length - applied - 1));
            return { outcome: 'refused', applied, detail: result.message };
          }
          applied++;
        }
        return { outcome: 'ok', applied };
      } catch (err) {
        this.setPending(this.pending - (ops.length - applied));
        return {
          outcome: 'error',
          applied,
          detail: err instanceof Error ? err.message : String(err),
        };
      }
    });
    // The chain must survive any outcome; results are consumed by the caller.
    this.chain = run.then(
      () => {},
      () => {},
    );
    return run;
  }

  private setPending(count: number) {
    this.pending = Math.max(0, count);
    this.onPendingChange(this.pending);
  }
}
