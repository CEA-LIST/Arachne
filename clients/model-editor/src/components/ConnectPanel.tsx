import type { AppState } from '../state/store';

interface ConnectPanelProps {
  connection: AppState['connection'];
  pollMs: number;
  setPollMs: (ms: number) => void;
  setUrl: (url: string) => void;
  connect: () => void;
  disconnect: () => void;
}

export function ConnectPanel({
  connection,
  pollMs,
  setPollMs,
  setUrl,
  connect,
  disconnect,
}: ConnectPanelProps) {
  const busy = connection.status === 'connecting';
  const connected = connection.status === 'connected';
  return (
    <form
      className="connect-panel"
      onSubmit={(e) => {
        e.preventDefault();
        if (connected) disconnect();
        else connect();
      }}
    >
      <label>
        Node URL
        <input
          type="text"
          value={connection.url}
          disabled={connected || busy}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="http://127.0.0.1:3000"
          spellCheck={false}
        />
      </label>
      <label>
        Poll (ms)
        <input
          type="number"
          className="poll-input"
          min={100}
          step={100}
          value={pollMs}
          onChange={(e) => {
            const v = parseInt(e.target.value, 10);
            if (!Number.isNaN(v) && v >= 100) setPollMs(v);
          }}
        />
      </label>
      <button type="submit" disabled={busy}>
        {connected ? 'Disconnect' : busy ? 'Connecting…' : 'Connect'}
      </button>
      {connection.status === 'error' && connection.error !== null && (
        <span className="connect-error">{connection.error}</span>
      )}
    </form>
  );
}
