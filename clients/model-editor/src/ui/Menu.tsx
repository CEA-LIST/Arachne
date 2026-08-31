/**
 * An arrow-navigable menu of class names, with a filter box once the list is
 * long enough to need one. Used for every "Add <concrete subtype>" choice, so
 * its entries are always descriptor-derived — never a hard-coded class list.
 */

import { useEffect, useMemo, useRef, useState } from 'react';

/** Above this many entries the menu grows a filter box. */
export const SEARCH_THRESHOLD = 8;

export interface MenuItem {
  /** Opaque payload handed back to onPick. */
  id: string;
  label: string;
}

interface MenuProps {
  open: boolean;
  items: MenuItem[];
  label: string;
  onPick: (id: string) => void;
  onClose: () => void;
}

export function Menu({ open, items, label, onPick, onClose }: MenuProps) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const searchable = items.length > SEARCH_THRESHOLD;

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle.length === 0) return items;
    return items.filter((item) => item.label.toLowerCase().includes(needle));
  }, [items, query]);

  // Reset on the closed -> open transition, derived rather than in an effect.
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    if (open) {
      setQuery('');
      setActive(0);
    }
  }

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      const node = ref.current;
      const target = event.target as Node | null;
      if (node === null || target === null) return;
      if (node.contains(target) || node.parentElement?.contains(target) === true) return;
      onClose();
    };
    document.addEventListener('pointerdown', onPointerDown, true);
    return () => document.removeEventListener('pointerdown', onPointerDown, true);
  }, [open, onClose]);

  useEffect(() => {
    if (!open) return;
    const focusTarget = ref.current?.querySelector<HTMLElement>('input, button');
    focusTarget?.focus();
  }, [open]);

  if (!open) return null;

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setActive((i) => Math.min(i + 1, filtered.length - 1));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setActive((i) => Math.max(i - 1, 0));
    } else if (event.key === 'Enter') {
      event.preventDefault();
      const choice = filtered[active];
      if (choice !== undefined) onPick(choice.id);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    }
  };

  return (
    <div className="me-menu" role="menu" aria-label={label} ref={ref} onKeyDown={onKeyDown}>
      {searchable && (
        <input
          className="me-input me-menu__search"
          type="text"
          value={query}
          placeholder="Filter classes…"
          aria-label="Filter classes"
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
        />
      )}
      {filtered.length === 0 ? (
        <p className="me-menu__empty">No class matches “{query}”.</p>
      ) : (
        filtered.map((item, index) => (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            className={index === active ? 'me-menu__item me-menu__item--active' : 'me-menu__item'}
            onMouseEnter={() => setActive(index)}
            onClick={() => onPick(item.id)}
          >
            {item.label}
          </button>
        ))
      )}
    </div>
  );
}
