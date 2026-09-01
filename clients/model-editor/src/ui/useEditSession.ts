/**
 * The editing session: which edits may run right now, and how far the batch in
 * flight has got.
 *
 * A thin view-layer wrapper over `sync.sendOps`. It exists because the UI is
 * the only layer that knows an intent is STRUCTURAL — that its ops were
 * computed from the current document — and the only layer that knows how many
 * ops the batch contains before the first one is posted, which is what makes a
 * determinate progress affordance possible at all. Nothing below the UI is
 * touched: the reducer, the queue and the poll loop are unchanged.
 *
 * See ui/editGate.ts for the rule and the defect it guards.
 */

import { useCallback, useState } from 'react';
import type { SyncApi } from '../sync/useSync';
import { editGate, type EditGate, type StructuralBatch } from './editGate';

export interface EditSession extends EditGate {
  /**
   * Post a structural batch (add / remove / reorder / reference write). The
   * batch's size is recorded first, so the wait is a measured bar rather than
   * a silent minute.
   */
  runStructural: SyncApi['sendOps'];
  /** Value edits (attribute fields) — straight through, no bookkeeping. */
  sendOps: SyncApi['sendOps'];
}

export function useEditSession(sync: SyncApi, now: number): EditSession {
  const [batch, setBatch] = useState<StructuralBatch | null>(null);
  const [settledAt, setSettledAt] = useState<number | null>(null);
  const { sendOps } = sync;

  const runStructural = useCallback<SyncApi['sendOps']>(
    async (description, ops, optimistic) => {
      setBatch({ description, total: ops.length });
      try {
        return await sendOps(description, ops, optimistic);
      } finally {
        // Both, always: the batch is over however it ended, and the poll still
        // has to read the replica back before anything may be computed again.
        setBatch(null);
        setSettledAt(Date.now());
      }
    },
    [sendOps],
  );

  const gate = editGate({
    pendingOps: sync.state.pendingOps,
    batch,
    settledAt,
    lastSyncAt: sync.state.lastSyncAt,
    now,
  });

  return { ...gate, runStructural, sendOps };
}
