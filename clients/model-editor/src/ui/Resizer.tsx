/**
 * A drag handle between panels, with a keyboard path (arrow keys nudge, so a
 * keyboard-only pass can still reach and use it) and an ARIA separator role.
 */

import { useCallback, useEffect, useRef } from 'react';

interface ResizerProps {
  orientation: 'vertical' | 'horizontal';
  value: number;
  min: number;
  max: number;
  onChange: (next: number) => void;
  /** Which direction increases the value as the pointer moves. */
  invert?: boolean;
  label: string;
}

const KEYBOARD_STEP = 16;

export function Resizer({ orientation, value, min, max, onChange, invert, label }: ResizerProps) {
  const dragging = useRef<{ origin: number; start: number } | null>(null);

  const onPointerMove = useCallback(
    (event: PointerEvent) => {
      const drag = dragging.current;
      if (drag === null) return;
      const point = orientation === 'vertical' ? event.clientX : event.clientY;
      const delta = (point - drag.origin) * (invert === true ? -1 : 1);
      onChange(drag.start + delta);
    },
    [orientation, invert, onChange],
  );

  const stop = useCallback(() => {
    dragging.current = null;
    document.body.style.removeProperty('cursor');
    document.body.style.removeProperty('user-select');
  }, []);

  useEffect(() => {
    document.addEventListener('pointermove', onPointerMove);
    document.addEventListener('pointerup', stop);
    return () => {
      document.removeEventListener('pointermove', onPointerMove);
      document.removeEventListener('pointerup', stop);
    };
  }, [onPointerMove, stop]);

  return (
    <div
      className={`me-resizer me-resizer--${orientation} me-noprint`}
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      onPointerDown={(event) => {
        dragging.current = {
          origin: orientation === 'vertical' ? event.clientX : event.clientY,
          start: value,
        };
        document.body.style.cursor = orientation === 'vertical' ? 'col-resize' : 'row-resize';
        document.body.style.userSelect = 'none';
      }}
      onKeyDown={(event) => {
        const forward = orientation === 'vertical' ? 'ArrowRight' : 'ArrowDown';
        const back = orientation === 'vertical' ? 'ArrowLeft' : 'ArrowUp';
        const sign = invert === true ? -1 : 1;
        if (event.key === forward) {
          event.preventDefault();
          onChange(value + KEYBOARD_STEP * sign);
        } else if (event.key === back) {
          event.preventDefault();
          onChange(value - KEYBOARD_STEP * sign);
        }
      }}
    />
  );
}
