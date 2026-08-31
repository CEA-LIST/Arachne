import { useEffect, useState } from 'react';

/**
 * A coarse ticker for relative times and staleness.
 *
 * Staleness is derived, not stored (see ui/syncState.ts), which means the UI
 * needs its own clock to re-render when nothing else changes — a replica that
 * has gone silent produces no state updates by definition.
 */
export function useNow(intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(timer);
  }, [intervalMs]);
  return now;
}
