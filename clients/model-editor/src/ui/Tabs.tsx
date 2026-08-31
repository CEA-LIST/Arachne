/** The panel tab strip: underline indicator, arrow-key navigation, ARIA tabs. */

import type { ReactNode } from 'react';

export interface TabSpec<T extends string> {
  id: T;
  label: string;
  /** Optional trailing count or badge. */
  suffix?: ReactNode;
}

interface TabsProps<T extends string> {
  tabs: readonly TabSpec<T>[];
  active: T;
  onSelect: (id: T) => void;
  label: string;
  /** Rendered right-aligned inside the strip (toolbar affordances). */
  trailing?: ReactNode;
}

export function Tabs<T extends string>({ tabs, active, onSelect, label, trailing }: TabsProps<T>) {
  const onKeyDown = (event: React.KeyboardEvent) => {
    const index = tabs.findIndex((t) => t.id === active);
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      onSelect(tabs[(index + 1) % tabs.length].id);
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      onSelect(tabs[(index - 1 + tabs.length) % tabs.length].id);
    }
  };

  return (
    <div className="me-tabs" role="tablist" aria-label={label} onKeyDown={onKeyDown}>
      {tabs.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          id={`tab-${tab.id}`}
          aria-selected={tab.id === active}
          aria-controls={`panel-${tab.id}`}
          tabIndex={tab.id === active ? 0 : -1}
          className="me-tabs__tab"
          onClick={() => onSelect(tab.id)}
        >
          {tab.label}
          {tab.suffix}
        </button>
      ))}
      {trailing !== undefined && <div className="me-tabs__trailing">{trailing}</div>}
    </div>
  );
}
