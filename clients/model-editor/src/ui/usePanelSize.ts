import { useCallback, useState } from 'react';

/**
 * A panel dimension that survives a reload, clamped to a sane range.
 *
 * sessionStorage access is wrapped in try/catch on both sides: a private
 * window or a storage-blocked browser must degrade to the default, never
 * throw (useSync already establishes this pattern for localStorage).
 */
export function usePanelSize(
  key: string,
  fallback: number,
  min: number,
  max: number,
): [number, (next: number) => void] {
  const clamp = useCallback(
    (value: number) => Math.min(max, Math.max(min, Math.round(value))),
    [min, max],
  );

  const [size, setSize] = useState(() => {
    try {
      const stored = sessionStorage.getItem(key);
      if (stored !== null) {
        const parsed = Number.parseInt(stored, 10);
        if (Number.isFinite(parsed)) return clamp(parsed);
      }
    } catch {
      // Storage unavailable: the default is correct.
    }
    return fallback;
  });

  const update = useCallback(
    (next: number) => {
      const value = clamp(next);
      setSize(value);
      try {
        sessionStorage.setItem(key, String(value));
      } catch {
        // Storage unavailable: the size still applies for this session.
      }
    },
    [key, clamp],
  );

  return [size, update];
}
