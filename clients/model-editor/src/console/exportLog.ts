import type { LogEntry } from '../state/store';

/**
 * The action log leaves as a JSON file for the case study — carried over
 * verbatim from the previous build. Lives outside the view module so the log
 * component file exports components only.
 */
export function exportLog(log: LogEntry[]) {
  const blob = new Blob([JSON.stringify(log, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `model-editor-log-${new Date().toISOString().replace(/[:.]/g, '-')}.json`;
  a.click();
  URL.revokeObjectURL(url);
}
