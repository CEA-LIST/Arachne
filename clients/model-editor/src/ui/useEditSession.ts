/**
 * The editing session: which edits may run right now on the selected model,
 * and how far the batch in flight has got.
 *
 * A thin view-layer wrapper over the sync hook's senders. It exists because
 * the UI is the only layer that knows an intent is STRUCTURAL — that its ops
 * were computed from the current document — and the only layer that knows how
 * many ops the batch contains before the first one is posted, which is what
 * makes a determinate progress affordance possible at all.
 *
 * Every model open in a tab has its own document and its own queue, so the
 * batch in flight and the settle clock are kept per model: a reorder draining
 * in one tab holds that tab's controls and no other's, and a field that
 * flushes as its tab is switched away still posts to the model it belongs to,
 * because the senders handed out here are bound to the tab at render time.
 *
 * See ui/editGate.ts for the rule and the defect it guards.
 */

import { useCallback, useState } from 'react';
import type { ModelId } from '../api/types';
import { refusalOf } from '../model/binding';
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

interface Flight {
  batch: StructuralBatch | null;
  settledAt: number | null;
}

export function useEditSession(sync: SyncApi, now: number): EditSession {
  const [flights, setFlights] = useState<Record<ModelId, Flight>>({});
  const { sendOpsTo, selected } = sync;
  const id = selected?.id ?? null;

  const sendOps = useCallback<SyncApi['sendOps']>(
    (description, ops, optimistic) => sendOpsTo(id, description, ops, optimistic),
    [sendOpsTo, id],
  );

  const runStructural = useCallback<SyncApi['sendOps']>(
    async (description, ops, optimistic) => {
      const target = id;
      if (target !== null) {
        setFlights((prev) => ({
          ...prev,
          [target]: { batch: { description, total: ops.length }, settledAt: prev[target]?.settledAt ?? null },
        }));
      }
      try {
        return await sendOpsTo(target, description, ops, optimistic);
      } finally {
        // Both, always: the batch is over however it ended, and the poll still
        // has to read the replica back before anything may be computed again.
        if (target !== null) setFlights((prev) => ({ ...prev, [target]: { batch: null, settledAt: Date.now() } }));
      }
    },
    [sendOpsTo, id],
  );

  const flight = id === null ? undefined : flights[id];
  const gate = editGate({
    pendingOps: selected?.pendingOps ?? 0,
    batch: flight?.batch ?? null,
    settledAt: flight?.settledAt ?? null,
    lastSyncAt: selected?.lastSyncAt ?? null,
    now,
    refused: refusalOf(selected?.binding ?? null),
  });

  return { ...gate, runStructural, sendOps };
}
