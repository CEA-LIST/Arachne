/**
 * App identity on the left, live truth on the right.
 *
 * The document-context chips describe the SELECTED model tab: its id, the
 * metamodel PACKAGE read from its descriptor at runtime with its provenance
 * (node or file) — the editor never hard-codes what it is editing — and, once
 * a state has been fetched, the binding check's verdict on that document
 * (model/binding.ts): unbound, bound, or not applied; then the model store's
 * word (model/projection.ts): stored, with the file and its digest in the
 * title, or why not. With no tab open the context says so.
 */

import { bindingLabel, describeBinding, isRefusal } from '../model/binding';
import { describeProjection, projectionLabel } from '../model/projection';
import type { AppState, ModelTab } from '../state/store';
import type { FlushProgress } from '../ui/editGate';
import { FlushBar } from '../ui/FlushBar';
import { Cpu, Keyboard } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { shortId } from '../ui/modelLabel';
import type { SyncView } from '../ui/syncState';
import { ConnectPopover } from './ConnectPopover';
import { SyncChip } from './SyncChip';

interface TopBarProps {
  state: AppState;
  /** The tab the panels show; null when none is open. */
  tab: ModelTab | null;
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
  tab,
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
  const { connection } = state;
  const connected = connection.status === 'connected';
  const pendingOps = tab?.pendingOps ?? 0;

  return (
    <header className="me-topbar">
      <div className="me-topbar__identity">
        <span className="me-topbar__mark" aria-hidden="true" />
        <span className="me-topbar__wordmark">Model Editor</span>
      </div>
      <span className="me-topbar__divider" aria-hidden="true" />
      <div className="me-topbar__context" data-testid="context">
        {tab === null ? (
          <span className="me-muted">{connected ? 'no model open' : 'no model'}</span>
        ) : (
          <>
            <span className="me-chip me-topbar__model" title={`model ${tab.id} on ${tab.nodeUrl}`}>
              <span className="me-mono" data-testid="model-id">
                {shortId(tab.id)}…
              </span>
            </span>
            {tab.metamodel === null ? (
              <span className="me-muted">no metamodel</span>
            ) : (
              <>
                <span className="me-topbar__package" data-testid="package">
                  {tab.metamodel.package}
                </span>
                <span className="me-subtle">{tab.metamodelSource === 'node' ? 'from node' : 'from file'}</span>
              </>
            )}
            {tab.binding !== null && (
              <span
                className={isRefusal(tab.binding) ? 'me-chip me-chip--danger' : 'me-subtle'}
                title={describeBinding(tab.binding)}
                data-testid="binding"
              >
                {bindingLabel(tab.binding)}
              </span>
            )}
            {tab.projection !== null && (
              <span
                className={tab.projection.kind === 'failed' ? 'me-chip me-chip--danger' : 'me-subtle'}
                title={describeProjection(tab.projection)}
                data-testid="projection"
              >
                {projectionLabel(tab.projection)}
              </span>
            )}
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
