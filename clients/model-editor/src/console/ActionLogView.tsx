/**
 * The append-only evaluation log: every edit intent, the exact op payloads
 * sent, and the replica's verdict.
 *
 * There is deliberately NO "clear log" — this is evidence for the case study,
 * and it is append-only up to MAX_LOG_ENTRIES. Filters are view-only.
 */

import { useMemo, useState } from 'react';
import { EmptyState } from '../common/EmptyState';
import type { LogEntry } from '../state/store';
import { ChevronDown, ChevronRight, Download, Search, Terminal, X } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { exportLog } from './exportLog';

type Outcome = LogEntry['outcome'];

const OUTCOMES: readonly Outcome[] = ['ok', 'refused', 'error'];

/** A filter chip carries its outcome's own colour once it has entries to show. */
const TONE: Record<Outcome, string> = {
  ok: 'me-chip--ok',
  refused: 'me-chip--warn',
  error: 'me-chip--danger',
};

interface ActionLogViewProps {
  log: LogEntry[];
  /** Set by "Details" on an alert: pre-filter to the failures. */
  failuresOnly: boolean;
  setFailuresOnly: (value: boolean) => void;
}

export function ActionLogView({ log, failuresOnly, setFailuresOnly }: ActionLogViewProps) {
  const [query, setQuery] = useState('');
  const [hidden, setHidden] = useState<ReadonlySet<Outcome>>(new Set());
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(new Set());

  const counts = useMemo(() => {
    const result: Record<Outcome, number> = { ok: 0, refused: 0, error: 0 };
    for (const entry of log) result[entry.outcome]++;
    return result;
  }, [log]);

  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return [...log]
      .reverse()
      .filter((entry) => {
        if (failuresOnly && entry.outcome === 'ok') return false;
        if (hidden.has(entry.outcome)) return false;
        if (needle.length === 0) return true;
        return (
          entry.description.toLowerCase().includes(needle) ||
          (entry.detail ?? '').toLowerCase().includes(needle)
        );
      });
  }, [log, query, hidden, failuresOnly]);

  const filtered = query.trim().length > 0 || hidden.size > 0 || failuresOnly;

  return (
    <div className="me-log">
      <div className="me-panel__toolbar me-noprint">
        <span className="me-panel__search">
          <Search {...ICON} size={14} className="me-panel__search-icon" aria-hidden="true" />
          <input
            className="me-input me-panel__search-input"
            type="search"
            value={query}
            placeholder="Filter log…"
            aria-label="Filter log"
            onChange={(e) => setQuery(e.target.value)}
          />
        </span>
        {OUTCOMES.map((outcome) => (
          <button
            key={outcome}
            type="button"
            aria-pressed={!hidden.has(outcome)}
            title={`${counts[outcome]} ${outcome} — click to hide`}
            className={
              hidden.has(outcome)
                ? 'me-chip me-chip--button me-chip--off'
                : `me-chip me-chip--button ${counts[outcome] > 0 ? TONE[outcome] : ''}`
            }
            onClick={() =>
              setHidden((prev) => {
                const next = new Set(prev);
                if (next.has(outcome)) next.delete(outcome);
                else next.add(outcome);
                return next;
              })
            }
          >
            {outcome}
            <span className="me-num">{counts[outcome]}</span>
          </button>
        ))}
        <span className="me-log__spacer" />
        {filtered && (
          <button
            type="button"
            className="me-btn me-btn--sm"
            onClick={() => {
              setQuery('');
              setHidden(new Set());
              setFailuresOnly(false);
            }}
          >
            <X {...ICON} size={13} aria-hidden="true" />
            Clear filters
          </button>
        )}
        <button
          type="button"
          className="me-btn me-btn--sm"
          disabled={log.length === 0}
          title="Export the whole log as JSON (⌘⇧E)"
          onClick={() => exportLog(log)}
        >
          <Download {...ICON} size={13} aria-hidden="true" />
          Export JSON
        </button>
      </div>

      {log.length === 0 ? (
        <EmptyState
          icon={Terminal}
          title="No operations yet"
          body="Every edit appears here with the exact CRDT operations sent and the replica's verdict. The log is append-only — there is no clear button, because this is the evidence."
        />
      ) : rows.length === 0 ? (
        <EmptyState
          icon={Search}
          title="No matching entries"
          body="No log entry matches the current filters."
        >
          <button
            type="button"
            className="me-btn"
            onClick={() => {
              setQuery('');
              setHidden(new Set());
              setFailuresOnly(false);
            }}
          >
            <X {...ICON} size={14} aria-hidden="true" />
            Clear filters
          </button>
        </EmptyState>
      ) : (
        <ol className="me-log__list">
          {rows.map((entry) => {
            const open = expanded.has(entry.id);
            return (
              <li key={entry.id} className={`me-log__row me-log__row--${entry.outcome}`}>
                <button
                  type="button"
                  className="me-log__disclose"
                  aria-expanded={open}
                  aria-label={open ? 'Hide operations' : 'Show operations'}
                  onClick={() =>
                    setExpanded((prev) => {
                      const next = new Set(prev);
                      if (next.has(entry.id)) next.delete(entry.id);
                      else next.add(entry.id);
                      return next;
                    })
                  }
                >
                  {open ? (
                    <ChevronDown {...ICON} size={13} aria-hidden="true" />
                  ) : (
                    <ChevronRight {...ICON} size={13} aria-hidden="true" />
                  )}
                </button>
                <span className="me-log__time me-mono me-num">
                  {new Date(entry.ts).toLocaleTimeString()}
                </span>
                <span className={`me-log__pill me-log__pill--${entry.outcome}`}>
                  {entry.outcome}
                </span>
                <span className="me-log__desc">{entry.description}</span>
                <span className="me-log__ops me-num">
                  {entry.ops.length} op{entry.ops.length === 1 ? '' : 's'}
                </span>
                {entry.detail !== undefined && entry.detail !== '' && (
                  <span className="me-log__detail">{entry.detail}</span>
                )}
                {open && (
                  <pre className="me-log__payload me-well">{JSON.stringify(entry.ops, null, 1)}</pre>
                )}
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
}
