/**
 * The "add an instance here" control.
 *
 * Options always come from concreteSubtypes(descriptor, target) — one button
 * when the slot admits exactly one concrete class, an arrow-navigable (and,
 * past eight entries, searchable) menu otherwise.
 */

import { useState } from 'react';
import { Plus } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { Menu } from '../ui/Menu';

interface AddControlProps {
  options: string[];
  /** "Add" for arrays, "Create" for an empty single containment. */
  verb: string;
  onAdd: (className: string) => void;
  primary?: boolean;
  /** Held by the edit gate: adding computes an index from the live document. */
  disabled?: boolean;
  /** The gate's sentence, as the control's title. */
  disabledReason?: string | null;
}

export function AddControl({
  options,
  verb,
  onAdd,
  primary,
  disabled = false,
  disabledReason = null,
}: AddControlProps) {
  const [open, setOpen] = useState(false);

  if (options.length === 0) {
    return <p className="me-muted">No concrete class can be instantiated here.</p>;
  }

  const base = primary === true ? 'me-btn me-btn--primary' : 'me-btn';
  const className = disabled ? `${base} me-held` : base;
  const title = disabledReason ?? undefined;

  if (options.length === 1) {
    return (
      <button
        type="button"
        className={className}
        disabled={disabled}
        title={title}
        onClick={() => onAdd(options[0])}
      >
        <Plus {...ICON} size={14} aria-hidden="true" />
        {verb} {options[0]}
      </button>
    );
  }

  return (
    <span className="me-anchor">
      <button
        type="button"
        className={className}
        disabled={disabled}
        title={title}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <Plus {...ICON} size={14} aria-hidden="true" />
        {verb}
      </button>
      <Menu
        open={open}
        items={options.map((option) => ({ id: option, label: option }))}
        label={`Classes available to ${verb.toLowerCase()}`}
        onPick={(id) => {
          setOpen(false);
          onAdd(id);
        }}
        onClose={() => setOpen(false)}
      />
    </span>
  );
}
