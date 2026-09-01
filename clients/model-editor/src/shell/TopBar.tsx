/**
 * App identity on the left, live truth on the right.
 *
 * The document-context chip names the metamodel PACKAGE read from the
 * descriptor at runtime, with its provenance (node or file) — the editor never
 * hard-codes what it is editing.
 */

import type { AppState } from '../state/store';
import type { FlushProgress } from '../ui/editGate';
import { FlushBar } from '../ui/FlushBar';
import { Cpu, Keyboard } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import type { SyncView } from '../ui/syncState';
import { ConnectPopover } from './ConnectPopover';
import { SyncChip } from './SyncChip';

interface TopBarProps {
  state: AppState;
  pollMs: number;
  setPollMs: (ms: number) => void;
  setUrl: (url: string) => void;
  connect: () => void;
  disconnect: () => void;
  view: SyncView;
  connectOpen: boolean;
  setConnectOpen: (open: boolean) => void;
  /** Non-null while a structural batch is being applied. */
  progress: FlushProgress | null;
  onShowHelp: () => void;
}

export function TopBar({
  state,
  pollMs,
  setPollMs,
  setUrl,
  connect,
  disconnect,
  view,
  connectOpen,
  setConnectOpen,
  progress,
  onShowHelp,
}: TopBarProps) {
  const { connection, metamodel, metamodelSource, pendingOps } = state;
  const connected = connection.status === 'connected';

  return (
    <header className="me-topbar">
      <div className="me-topbar__identity">
        <span className="me-topbar__mark" aria-hidden="true" />
        <span className="me-topbar__wordmark">Model Editor</span>
      </div>
      <span className="me-topbar__divider" aria-hidden="true" />
      <div className="me-topbar__context">
        {metamodel === null ? (
          <span className="me-muted">no metamodel</span>
        ) : (
          <>
            <span className="me-topbar__package">{metamodel.package}</span>
            <span className="me-subtle">
              {metamodelSource === 'node' ? 'from node' : 'from file'}
            </span>
          </>
        )}
      </div>

      <div className="me-topbar__spacer" />

      {/* A batch whose size we know gets a measured bar; a stray queued op
          (a character being flushed) gets the count it deserves and no more. */}
      {progress !== null ? (
        <FlushBar progress={progress} compact />
      ) : (
        pendingOps > 0 && (
          <span className="me-chip me-chip--accent me-num">
            {pendingOps} op{pendingOps === 1 ? '' : 's'} queued
          </span>
        )
      )}
      <SyncChip view={view} onRetry={connect} />
      {connected && connection.replicaId !== null && (
        <span className="me-chip me-topbar__replica" title="Replica identity from /api/health">
          <Cpu {...ICON} size={13} aria-hidden="true" />
          <span className="me-mono">{connection.replicaId}</span>
        </span>
      )}
      <button
        type="button"
        className="me-iconbtn me-iconbtn--lg me-noprint"
        aria-label="Keyboard shortcuts"
        title="Keyboard shortcuts (?)"
        onClick={onShowHelp}
      >
        <Keyboard {...ICON} aria-hidden="true" />
      </button>
      <div className="me-anchor me-noprint">
        <button
          type="button"
          className={connected ? 'me-btn' : 'me-btn me-btn--primary'}
          aria-expanded={connectOpen}
          aria-haspopup="dialog"
          onClick={() => setConnectOpen(!connectOpen)}
        >
          {connected ? 'Disconnect' : 'Connect'}
        </button>
        <ConnectPopover
          open={connectOpen}
          onClose={() => setConnectOpen(false)}
          connection={connection}
          pollMs={pollMs}
          setPollMs={setPollMs}
          setUrl={setUrl}
          connect={connect}
          disconnect={disconnect}
        />
      </div>
    </header>
  );
}
