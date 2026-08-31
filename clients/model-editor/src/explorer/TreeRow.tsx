/**
 * One tree row: indent guides, twisty, type icon, label, eClass badge, count,
 * and hover/focus actions that never shift the row's layout.
 *
 * Every icon choice below is keyed on STRUCTURE — element vs containment
 * group, empty vs populated, known vs unknown class — so the vocabulary holds
 * for any metamodel the node happens to serve.
 */

import { useState } from 'react';
import { Box, ChevronDown, ChevronRight, CircleHelp, Folder, FolderOpen, Plus, Trash2, TriangleAlert } from '../ui/icons';
import { ICON } from '../ui/iconProps';
import { Menu, type MenuItem } from '../ui/Menu';
import { highlightParts, type TreeRow as Row } from '../ui/flattenTree';

interface TreeRowProps {
  row: Row;
  selected: boolean;
  focused: boolean;
  treeFocused: boolean;
  filter: string;
  addOptions: MenuItem[];
  onSelect: (row: Row) => void;
  onToggle: (row: Row) => void;
  onAdd: (row: Row, optionId: string) => void;
  onRemove: (row: Row) => void;
  registerRef: (key: string, element: HTMLDivElement | null) => void;
}

function RowIcon({ row }: { row: Row }) {
  if (row.kind === 'feature') {
    const Glyph = row.expanded ? FolderOpen : Folder;
    return (
      <Glyph
        {...ICON}
        className={row.empty ? 'me-tree__icon me-tree__icon--ghost' : 'me-tree__icon'}
        aria-hidden="true"
      />
    );
  }
  if (row.invalid) {
    return <TriangleAlert {...ICON} className="me-tree__icon me-tree__icon--warn" aria-hidden="true" />;
  }
  if (!row.known) {
    return (
      <span className="me-tree__icon me-tree__icon--stack">
        <Box {...ICON} aria-hidden="true" />
        <CircleHelp {...ICON} size={9} className="me-tree__icon-overlay" aria-hidden="true" />
      </span>
    );
  }
  return (
    <Box
      {...ICON}
      className={row.level === 1 ? 'me-tree__icon me-tree__icon--root' : 'me-tree__icon'}
      aria-hidden="true"
    />
  );
}

function Label({ text, filter, italic }: { text: string; filter: string; italic: boolean }) {
  const parts = highlightParts(text, filter);
  return (
    <span className={italic ? 'me-tree__label me-tree__label--anon' : 'me-tree__label'}>
      {parts.map((part, index) =>
        part.hit ? <mark key={index}>{part.text}</mark> : <span key={index}>{part.text}</span>,
      )}
    </span>
  );
}

export function TreeRow({
  row,
  selected,
  focused,
  treeFocused,
  filter,
  addOptions,
  onSelect,
  onToggle,
  onAdd,
  onRemove,
  registerRef,
}: TreeRowProps) {
  const [menuOpen, setMenuOpen] = useState(false);

  // A nameless element still has to read as an element, not as blank space.
  const anonymous = row.kind === 'element' && !row.invalid && row.label === row.eClass;
  const showBadge = row.kind === 'element' && row.eClass !== '' && row.label !== row.eClass;

  const classes = ['me-tree__row'];
  if (selected) classes.push(treeFocused ? 'me-tree__row--selected' : 'me-tree__row--selected-blur');
  if (row.kind === 'feature') classes.push('me-tree__row--feature');

  return (
    <div
      ref={(element) => registerRef(row.key, element)}
      role="treeitem"
      aria-level={row.level}
      aria-posinset={row.posInSet}
      aria-setsize={row.setSize}
      aria-selected={selected}
      aria-expanded={row.expandable ? row.expanded : undefined}
      tabIndex={focused ? 0 : -1}
      className={classes.join(' ')}
      style={{ paddingLeft: `${(row.level - 1) * 14 + 4}px` }}
      onClick={() => onSelect(row)}
      onDoubleClick={() => {
        if (row.expandable) onToggle(row);
      }}
    >
      {Array.from({ length: row.level - 1 }, (_, i) => (
        <span key={i} className="me-tree__guide" aria-hidden="true" />
      ))}

      <span className="me-tree__twisty">
        {row.expandable && (
          <button
            type="button"
            tabIndex={-1}
            aria-label={row.expanded ? `Collapse ${row.label}` : `Expand ${row.label}`}
            className="me-tree__twisty-btn"
            onClick={(event) => {
              event.stopPropagation();
              onToggle(row);
            }}
          >
            {row.expanded ? (
              <ChevronDown {...ICON} size={14} aria-hidden="true" />
            ) : (
              <ChevronRight {...ICON} size={14} aria-hidden="true" />
            )}
          </button>
        )}
      </span>

      <RowIcon row={row} />
      <Label text={row.label} filter={filter} italic={anonymous} />

      {showBadge && <span className="me-badge me-tree__eclass">{row.eClass}</span>}
      {row.kind === 'feature' && row.feature?.many === true && row.childCount > 0 && (
        <span className="me-badge me-num">{row.childCount}</span>
      )}
      {row.kind === 'feature' && row.empty && <span className="me-tree__empty">— empty</span>}

      <span className="me-tree__spacer" />

      <span className="me-tree__actions me-noprint">
        {addOptions.length > 0 && (
          <span className="me-anchor">
            <button
              type="button"
              tabIndex={-1}
              className="me-iconbtn"
              aria-label={`Add to ${row.label}`}
              title={`Add to ${row.label}`}
              onClick={(event) => {
                event.stopPropagation();
                if (addOptions.length === 1) onAdd(row, addOptions[0].id);
                else setMenuOpen((open) => !open);
              }}
            >
              <Plus {...ICON} size={14} aria-hidden="true" />
            </button>
            <Menu
              open={menuOpen}
              items={addOptions}
              label={`Classes addable to ${row.label}`}
              onPick={(id) => {
                setMenuOpen(false);
                onAdd(row, id);
              }}
              onClose={() => setMenuOpen(false)}
            />
          </span>
        )}
        {row.kind === 'element' && row.arrayIndex !== null && (
          <button
            type="button"
            tabIndex={-1}
            className="me-iconbtn me-iconbtn--danger"
            aria-label={`Remove ${row.label}`}
            title={`Remove ${row.label}`}
            onClick={(event) => {
              event.stopPropagation();
              onRemove(row);
            }}
          >
            <Trash2 {...ICON} size={14} aria-hidden="true" />
          </button>
        )}
      </span>
    </div>
  );
}
