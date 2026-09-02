/**
 * A `SessionEvents` that records what a `ModelSession` told its tab, for the
 * tests that drive a session without the hook: the patches in order, the log
 * rows, the banners, and the tab as the patches leave it. Test-only.
 */

import type { LogEntry, ModelTab, TabPatch } from '../state/store';
import { newTab } from '../state/store';
import type { SessionEvents } from '../sync/modelSession';

export interface SessionRecorder {
  events: SessionEvents;
  patches: TabPatch[];
  rows: Omit<LogEntry, 'id' | 'modelId'>[];
  banners: string[];
  /** The tab after every patch so far, from a fresh one. */
  tab(id: string, nodeUrl: string): ModelTab;
}

export function recordSession(): SessionRecorder {
  const patches: TabPatch[] = [];
  const rows: Omit<LogEntry, 'id' | 'modelId'>[] = [];
  const banners: string[] = [];
  return {
    events: {
      patch: (patch) => {
        patches.push(patch);
      },
      log: (entry) => {
        rows.push(entry);
      },
      banner: (message) => {
        banners.push(message);
      },
    },
    patches,
    rows,
    banners,
    tab: (id, nodeUrl) => patches.reduce<ModelTab>((tab, patch) => ({ ...tab, ...patch }), newTab(id, nodeUrl)),
  };
}
