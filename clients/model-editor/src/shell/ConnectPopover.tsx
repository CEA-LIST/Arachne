/**
 * Node URL and poll interval, moved out of the permanent chrome.
 *
 * These are one-time setup, not something to stare at while modelling — a
 * header row of raw debug inputs is exactly what made the old build read as a
 * test harness. The BEHAVIOUR is unchanged from the old ConnectPanel: the URL
 * is locked while connected or connecting, the poll floor is 100 ms, submit
 * toggles connect/disconnect, and connection.error shows inline.
 */

import { useState } from 'react';
import type { AppState } from '../state/store';
import { Popover } from '../ui/Popover';

interface ConnectPopoverProps {
  open: boolean;
  onClose: () => void;
  connection: AppState['connection'];
  pollMs: number;
  setPollMs: (ms: number) => void;
  setUrl: (url: string) => void;
  connect: () => void;
  disconnect: () => void;
}

export function ConnectPopover({
  open,
  onClose,
  connection,
  pollMs,
  setPollMs,
  setUrl,
  connect,
  disconnect,
}: ConnectPopoverProps) {
  const busy = connection.status === 'connecting';
  const connected = connection.status === 'connected';
  // Poll is typed freely and only committed when it is a legal value, so the
  // field can be cleared mid-edit without snapping back.
  const [pollText, setPollText] = useState(String(pollMs));
  const [lastPoll, setLastPoll] = useState(pollMs);
  if (lastPoll !== pollMs) {
    setLastPoll(pollMs);
    setPollText(String(pollMs));
  }

  return (
    <Popover open={open} onClose={onClose} align="right" label="Connection settings">
      <form
        className="me-connect"
        onSubmit={(event) => {
          event.preventDefault();
          if (connected) disconnect();
          else connect();
        }}
      >
        <label className="me-connect__field">
          <span className="me-connect__label">Node URL</span>
          <input
            className="me-input"
            type="text"
            value={connection.url}
            disabled={connected || busy}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="http://127.0.0.1:8081"
            spellCheck={false}
          />
        </label>
        {connection.status === 'error' && connection.error !== null && (
          <p className="me-connect__error">{connection.error}</p>
        )}
        <label className="me-connect__field">
          <span className="me-connect__label">Poll interval</span>
          <span className="me-connect__poll">
            <input
              className="me-input me-input--num"
              type="number"
              min={100}
              step={100}
              value={pollText}
              onChange={(e) => {
                setPollText(e.target.value);
                const parsed = Number.parseInt(e.target.value, 10);
                if (!Number.isNaN(parsed) && parsed >= 100) setPollMs(parsed);
              }}
            />
            <span className="me-typechip">ms</span>
          </span>
        </label>
        <p className="me-connect__hint">
          The editor re-reads <code>/api/state</code> on this interval and reconciles it with
          whatever you are typing.
        </p>
        <button type="submit" className="me-btn me-btn--primary" disabled={busy}>
          {connected ? 'Disconnect' : busy ? 'Connecting…' : 'Connect'}
        </button>
      </form>
    </Popover>
  );
}
