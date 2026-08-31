/**
 * The connection's honest state, in one chip.
 *
 * It renders syncView()'s verdict, not connection.status — a replica that
 * stopped answering reads "Not responding", never a green "connected".
 * aria-live="polite" so a screen reader hears the transition without being
 * interrupted mid-sentence.
 */

import { RefreshCw } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import type { SyncView } from '../ui/syncState';

interface SyncChipProps {
  view: SyncView;
  onRetry: () => void;
}

export function SyncChip({ view, onRetry }: SyncChipProps) {
  const toneClass = view.tone === 'neutral' ? '' : ` me-dot--${view.tone}`;
  return (
    <div className={`me-syncchip me-syncchip--${view.kind}`}>
      <span aria-live="polite" className="me-syncchip__live">
        <span
          className={`me-dot${toneClass}${view.busy ? ' me-dot--pulse' : ''}`}
          aria-hidden="true"
        />
        <span className="me-syncchip__label">{view.label}</span>
      </span>
      {view.kind === 'syncing' && (
        <RefreshCw {...ICON} size={12} className="me-spin" aria-hidden="true" />
      )}
      {view.retryable && (
        <button type="button" className="me-btn me-btn--sm me-noprint" onClick={onRetry}>
          Retry
        </button>
      )}
    </div>
  );
}
