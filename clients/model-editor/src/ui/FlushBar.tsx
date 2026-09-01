/**
 * The determinate progress affordance for a batch in flight.
 *
 * Measured on the rig: one reorder is 133 ops and the replica answers a
 * single-character POST /api/op in ~400 ms, so a reorder takes the better part
 * of a minute. During that minute the old UI showed an element rebuilding
 * character by character and an EMPTY action log, because a log row is only
 * appended when the whole batch resolves — which reads as a broken app rather
 * than a slow one. The wire cost is not ours to fix here; being honest about it
 * is. The same bar appears in the top bar, in the console's collapsed summary,
 * and at the head of the action log.
 */

import { flushLabel, type FlushProgress } from './editGate';

interface FlushBarProps {
  progress: FlushProgress;
  /** Compact: counts only, for the top bar and the console summary. */
  compact?: boolean;
}

export function FlushBar({ progress, compact = false }: FlushBarProps) {
  const label = flushLabel(progress);
  return (
    <span
      className={compact ? 'me-flush me-flush--compact' : 'me-flush'}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={progress.total}
      aria-valuenow={progress.done}
      aria-valuetext={label}
      aria-label={label}
      title={label}
    >
      <span className="me-flush__text">
        {!compact && <span className="me-flush__desc me-truncate">{progress.description}</span>}
        <span className="me-flush__count me-num">
          {progress.done} / {progress.total} ops
        </span>
      </span>
      <span className="me-flush__track" aria-hidden="true">
        <span className="me-flush__fill" style={{ width: `${progress.percent}%` }} />
      </span>
    </span>
  );
}
