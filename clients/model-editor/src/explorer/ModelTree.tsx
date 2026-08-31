/**
 * The containment tree: role="tree" over a roving tabindex, with the keyboard
 * semantics in ui/treeKeyboard.ts and the row model in ui/flattenTree.ts.
 *
 * This component owns only wiring — focus movement, menu payloads, the delete
 * confirmation — so the two hard parts stay pure and tested.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { ContainmentDesc, Descriptor, Path, PlainJson } from '../api/types';
import { getAtPath } from '../crdt/path';
import { concreteSubtypes, flattenFeatures, isPresent, type ModelNode } from '../model/instance';
import { elementKey, flattenTree, type TreeRow as Row } from '../ui/flattenTree';
import type { MenuItem } from '../ui/Menu';
import { emptyTypeahead, treeKeyCommand, type TypeaheadState } from '../ui/treeKeyboard';
import { TreeRow } from './TreeRow';

interface ModelTreeProps {
  descriptor: Descriptor;
  root: ModelNode;
  doc: PlainJson;
  collapsed: ReadonlySet<string>;
  setCollapsed: (update: (prev: ReadonlySet<string>) => ReadonlySet<string>) => void;
  selectedPath: Path;
  onSelectPath: (path: Path) => void;
  filter: string;
  onAddChild: (elementPath: Path, feature: ContainmentDesc, className: string) => void;
  onRemove: (path: Path) => void;
  /** Enter: selection made, move on into the properties form. */
  onActivate: () => void;
  /** F2: jump to the element's id/name field. */
  onRename: () => void;
  onRowCount: (count: number) => void;
}

export function ModelTree({
  descriptor,
  root,
  doc,
  collapsed,
  setCollapsed,
  selectedPath,
  onSelectPath,
  filter,
  onAddChild,
  onRemove,
  onActivate,
  onRename,
  onRowCount,
}: ModelTreeProps) {
  const rows = useMemo(
    () => flattenTree(descriptor, root, { collapsed, filter }),
    [descriptor, root, collapsed, filter],
  );

  const selectedKey = elementKey(selectedPath);
  const [focusKey, setFocusKey] = useState<string | null>(selectedKey);
  const [treeFocused, setTreeFocused] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<Row | null>(null);
  const typeaheadRef = useRef<TypeaheadState>(emptyTypeahead);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  const confirmRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => onRowCount(rows.length), [rows.length, onRowCount]);

  // The roving focus follows selection made elsewhere (breadcrumb, form link).
  const [lastSelected, setLastSelected] = useState(selectedKey);
  if (lastSelected !== selectedKey) {
    setLastSelected(selectedKey);
    setFocusKey(selectedKey);
  }

  useEffect(() => {
    if (confirmDelete !== null) confirmRef.current?.focus();
  }, [confirmDelete]);

  const registerRef = useCallback((key: string, element: HTMLDivElement | null) => {
    if (element === null) rowRefs.current.delete(key);
    else rowRefs.current.set(key, element);
  }, []);

  const moveFocus = useCallback((key: string) => {
    setFocusKey(key);
    rowRefs.current.get(key)?.focus();
  }, []);

  const toggle = useCallback(
    (row: Row) => {
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(row.key)) next.delete(row.key);
        else next.add(row.key);
        return next;
      });
    },
    [setCollapsed],
  );

  const select = useCallback(
    (row: Row) => {
      setFocusKey(row.key);
      // Feature rows are navigational, not editable: selecting one shows its
      // owning element, which is where its add/remove controls live.
      onSelectPath(row.kind === 'element' ? row.path : row.path.slice(0, -1));
    },
    [onSelectPath],
  );

  /** The classes addable at this row, as menu payloads. */
  const addOptionsFor = useCallback(
    (row: Row): MenuItem[] => {
      if (row.kind === 'feature') {
        const feature = row.feature;
        if (feature === null) return [];
        // A single containment that is already filled has no free slot.
        if (!feature.many && isPresent(getAtPath(doc, row.path))) return [];
        return concreteSubtypes(descriptor, feature.target).map((cls) => ({ id: cls, label: cls }));
      }
      if (row.invalid || !row.known) return [];
      const containments = flattenFeatures(descriptor, row.eClass).containments;
      const items: MenuItem[] = [];
      containments.forEach((feature, index) => {
        if (!feature.many && isPresent(getAtPath(doc, [...row.path, feature.name]))) return;
        for (const cls of concreteSubtypes(descriptor, feature.target)) {
          items.push({
            id: `${index}:${cls}`,
            label: containments.length > 1 ? `${feature.name} → ${cls}` : cls,
          });
        }
      });
      return items;
    },
    [descriptor, doc],
  );

  const add = useCallback(
    (row: Row, optionId: string) => {
      if (row.kind === 'feature') {
        if (row.feature === null) return;
        onAddChild(row.path.slice(0, -1), row.feature, optionId);
        return;
      }
      const separator = optionId.indexOf(':');
      const index = Number.parseInt(optionId.slice(0, separator), 10);
      const className = optionId.slice(separator + 1);
      const feature = flattenFeatures(descriptor, row.eClass).containments[index];
      if (feature !== undefined) onAddChild(row.path, feature, className);
    },
    [descriptor, onAddChild],
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (confirmDelete !== null) return;
    const outcome = treeKeyCommand({
      key: event.key,
      rows,
      focusKey,
      typeahead: typeaheadRef.current,
      now: Date.now(),
    });
    typeaheadRef.current = outcome.typeahead;
    if (outcome.handled) event.preventDefault();

    const command = outcome.command;
    switch (command.type) {
      case 'focus':
        moveFocus(command.key);
        break;
      case 'expand':
      case 'collapse': {
        const row = rows.find((r) => r.key === command.key);
        if (row !== undefined) toggle(row);
        break;
      }
      case 'expand-siblings':
        setCollapsed((prev) => {
          const next = new Set(prev);
          for (const key of command.keys) next.delete(key);
          return next;
        });
        break;
      case 'select':
      case 'activate': {
        const row = rows.find((r) => r.key === command.key);
        if (row !== undefined) select(row);
        if (command.type === 'activate') onActivate();
        break;
      }
      case 'delete': {
        const row = rows.find((r) => r.key === command.key);
        if (row !== undefined) setConfirmDelete(row);
        break;
      }
      case 'rename': {
        const row = rows.find((r) => r.key === command.key);
        if (row !== undefined) {
          select(row);
          onRename();
        }
        break;
      }
      case 'none':
        break;
    }
  };

  return (
    <div className="me-tree__wrap">
      <div
        role="tree"
        aria-label="Model containment tree"
        className="me-tree"
        onKeyDown={onKeyDown}
        onFocus={() => setTreeFocused(true)}
        onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
            setTreeFocused(false);
          }
        }}
      >
        {rows.map((row) => (
          <TreeRow
            key={row.key}
            row={row}
            selected={row.key === selectedKey}
            focused={row.key === focusKey}
            treeFocused={treeFocused}
            filter={filter}
            addOptions={addOptionsFor(row)}
            onSelect={select}
            onToggle={toggle}
            onAdd={add}
            onRemove={(target) => setConfirmDelete(target)}
            registerRef={registerRef}
          />
        ))}
      </div>

      {confirmDelete !== null && (
        <div className="me-tree__confirm me-noprint" role="alertdialog" aria-label="Confirm removal">
          <span>
            Remove <strong>{confirmDelete.label}</strong> and everything it contains?
          </span>
          <button
            ref={confirmRef}
            type="button"
            className="me-btn me-btn--sm"
            onClick={() => {
              onRemove(confirmDelete.path);
              setConfirmDelete(null);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Escape') setConfirmDelete(null);
            }}
          >
            Remove
          </button>
          <button
            type="button"
            className="me-btn me-btn--sm"
            onClick={() => setConfirmDelete(null)}
          >
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}
