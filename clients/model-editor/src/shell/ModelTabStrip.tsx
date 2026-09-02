/**
 * The document tabs: one per open model, under the top bar. Each tab names
 * its model (ui/modelLabel.ts) and carries the state of its own poll: a
 * pulsing dot while it opens, a danger dot when it failed or the binding
 * check refused it. Close is a sibling of the tab's button rather than a
 * child, since a button may not contain one.
 *
 * Not ui/Tabs.tsx: that strip's `TabSpec` has no close and no status, and a
 * document tab is a different thing from a panel tab.
 */

import type { ModelId } from '../api/types';
import { isRefusal } from '../model/binding';
import type { ModelTab } from '../state/store';
import { X } from '../ui/icons';
import { ICON } from '../ui/iconProps';

interface ModelTabStripProps {
  tabs: readonly ModelTab[];
  selected: ModelId | null;
  labelOf: (tab: ModelTab) => string;
  onSelect: (id: ModelId) => void;
  onClose: (id: ModelId) => void;
  /** Bring the model list into view. */
  onShowModels: () => void;
  connected: boolean;
}

function tone(tab: ModelTab): 'accent' | 'danger' | 'warn' | null {
  if (tab.status === 'opening') return 'accent';
  if (tab.status === 'failed') return 'danger';
  if (tab.binding !== null && isRefusal(tab.binding)) return 'danger';
  if (tab.binding?.kind === 'unbound' || tab.binding?.kind === 'no-descriptor') return 'warn';
  return null;
}

function statusTitle(tab: ModelTab): string {
  if (tab.status === 'opening') return 'opening';
  if (tab.status === 'failed') return `could not open: ${tab.error ?? 'unknown error'}`;
  return `model ${tab.id} on ${tab.nodeUrl}`;
}

export function ModelTabStrip({
  tabs,
  selected,
  labelOf,
  onSelect,
  onClose,
  onShowModels,
  connected,
}: ModelTabStripProps) {
  const onKeyDown = (event: React.KeyboardEvent) => {
    if (tabs.length === 0 || selected === null) return;
    const index = tabs.findIndex((tab) => tab.id === selected);
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      onSelect(tabs[(index + 1) % tabs.length].id);
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      onSelect(tabs[(index - 1 + tabs.length) % tabs.length].id);
    }
  };

  return (
    <div className="me-doctabs" role="tablist" aria-label="Open models" onKeyDown={onKeyDown}>
      {tabs.map((tab) => {
        const label = labelOf(tab);
        const active = tab.id === selected;
        const dot = tone(tab);
        return (
          <div
            key={tab.id}
            className={active ? 'me-doctabs__tab me-doctabs__tab--active' : 'me-doctabs__tab'}
            data-model-id={tab.id}
          >
            <button
              type="button"
              role="tab"
              id={`doctab-${tab.id}`}
              aria-selected={active}
              tabIndex={active ? 0 : -1}
              className="me-doctabs__name"
              title={statusTitle(tab)}
              onClick={() => onSelect(tab.id)}
            >
              {dot !== null && (
                <span
                  className={`me-dot me-dot--${dot}${tab.status === 'opening' ? ' me-dot--pulse' : ''}`}
                  aria-hidden="true"
                />
              )}
              {label}
            </button>
            <button
              type="button"
              tabIndex={-1}
              className="me-iconbtn me-doctabs__close me-noprint"
              aria-label={`Close ${label}`}
              title="Close tab"
              onClick={() => onClose(tab.id)}
            >
              <X {...ICON} size={13} aria-hidden="true" />
            </button>
          </div>
        );
      })}
      {tabs.length === 0 && (
        <span className="me-doctabs__empty me-subtle">
          {connected ? 'No model open — open one from the Models tab.' : 'No model open.'}
        </span>
      )}
      <span className="me-doctabs__spacer" />
      <button
        type="button"
        className="me-btn me-btn--sm me-noprint"
        aria-label="Show the model list"
        onClick={onShowModels}
        disabled={!connected}
      >
        Models…
      </button>
    </div>
  );
}
