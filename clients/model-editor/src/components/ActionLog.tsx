import type { LogEntry } from '../state/store';

interface ActionLogProps {
  log: LogEntry[];
}

function exportLog(log: LogEntry[]) {
  const blob = new Blob([JSON.stringify(log, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `model-editor-log-${new Date().toISOString().replace(/[:.]/g, '-')}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

/**
 * Append-only evaluation log: every edit intent with the exact op payloads
 * sent and the node's verdict. Exportable as JSON for the case study.
 */
export function ActionLog({ log }: ActionLogProps) {
  return (
    <div className="action-log">
      <div className="action-log-header">
        <h3>Action log ({log.length})</h3>
        <button type="button" disabled={log.length === 0} onClick={() => exportLog(log)}>
          Export JSON
        </button>
      </div>
      <ol className="action-log-list">
        {[...log].reverse().map((entry) => (
          <li key={entry.id} className={`log-${entry.outcome}`}>
            <span className="log-time">{new Date(entry.ts).toLocaleTimeString()}</span>{' '}
            <span className="log-desc">{entry.description}</span>{' '}
            <span className="log-outcome">
              [{entry.outcome === 'ok' ? `ok, ${entry.ops.length} op${entry.ops.length === 1 ? '' : 's'}` : entry.outcome}
              {entry.detail !== undefined ? `: ${entry.detail}` : ''}]
            </span>
            <details>
              <summary>ops</summary>
              <pre>{JSON.stringify(entry.ops, null, 1)}</pre>
            </details>
          </li>
        ))}
      </ol>
      {log.length === 0 && <p className="muted">No operations sent yet.</p>}
    </div>
  );
}
