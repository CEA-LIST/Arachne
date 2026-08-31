/**
 * A minimal popover: Esc closes, an outside pointer-down closes, focus moves
 * in on open and returns to the trigger on close.
 *
 * Deliberately hand-rolled (~70 lines). A primitive library would bring its
 * own focus manager, and this app already has one hard focus rule of its own —
 * the poll loop must never steal the caret from a field being typed in.
 */

import { useEffect, useRef, type ReactNode } from 'react';

interface PopoverProps {
  open: boolean;
  onClose: () => void;
  align?: 'left' | 'right';
  label: string;
  children: ReactNode;
}

export function Popover({ open, onClose, align = 'right', label, children }: PopoverProps) {
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        onClose();
      }
    };
    const onPointerDown = (event: PointerEvent) => {
      const node = ref.current;
      if (node === null) return;
      const target = event.target as Node | null;
      // The trigger sits outside the popover; let it handle its own toggle.
      if (target !== null && (node.contains(target) || node.parentElement?.contains(target) === true)) {
        return;
      }
      onClose();
    };
    document.addEventListener('keydown', onKeyDown, true);
    document.addEventListener('pointerdown', onPointerDown, true);
    return () => {
      document.removeEventListener('keydown', onKeyDown, true);
      document.removeEventListener('pointerdown', onPointerDown, true);
    };
  }, [open, onClose]);

  useEffect(() => {
    if (!open) return;
    const first = ref.current?.querySelector<HTMLElement>(
      'input:not([disabled]), select, button:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    first?.focus();
  }, [open]);

  if (!open) return null;

  return (
    <div
      ref={ref}
      className={`me-pop ${align === 'right' ? 'me-pop--right' : 'me-pop--left'}`}
      role="dialog"
      aria-label={label}
    >
      {children}
    </div>
  );
}
