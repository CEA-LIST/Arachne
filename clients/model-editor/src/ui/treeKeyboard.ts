/**
 * Tree keyboard semantics as a pure reducer.
 *
 * The WAI-ARIA treeview pattern (https://www.w3.org/WAI/ARIA/apg/patterns/treeview/)
 * over a roving tabindex: one tab stop for the whole tree, arrows walk the
 * FLATTENED visible-row list, so navigation and collapse interact correctly
 * without any DOM query. Keeping this a function of (key, rows, focus,
 * typeahead) means every branch below is unit-testable.
 */

import type { TreeRow } from './flattenTree';

/** Type-ahead buffer lifetime, per the APG's recommendation. */
export const TYPEAHEAD_RESET_MS = 500;

export type TreeCommand =
  | { type: 'none' }
  /** Move the roving focus (does not change selection). */
  | { type: 'focus'; key: string }
  /** Select without leaving the tree (Space). */
  | { type: 'select'; key: string }
  /** Select and move focus into the properties form (Enter). */
  | { type: 'activate'; key: string }
  | { type: 'expand'; key: string }
  | { type: 'collapse'; key: string }
  /** '*' — expand every expandable sibling at this level. */
  | { type: 'expand-siblings'; keys: string[] }
  /** Delete — only ever emitted for a row that is an array child. */
  | { type: 'delete'; key: string }
  /** F2 — jump to the element's id/name field. */
  | { type: 'rename'; key: string };

export interface TypeaheadState {
  buffer: string;
  at: number;
}

export const emptyTypeahead: TypeaheadState = { buffer: '', at: 0 };

export interface TreeKeyInput {
  key: string;
  rows: readonly TreeRow[];
  focusKey: string | null;
  typeahead: TypeaheadState;
  now: number;
}

export interface TreeKeyResult {
  command: TreeCommand;
  typeahead: TypeaheadState;
  /** True when the tree consumed the key and the event should be defaulted away. */
  handled: boolean;
}

const NONE: TreeCommand = { type: 'none' };

function result(
  command: TreeCommand,
  typeahead: TypeaheadState,
  handled = command.type !== 'none',
): TreeKeyResult {
  return { command, typeahead, handled };
}

/** Is this a single printable character worth feeding to type-ahead? */
function isTypeaheadKey(key: string): boolean {
  return key.length === 1 && key !== ' ' && key !== '*' && /\S/.test(key);
}

export function treeKeyCommand(input: TreeKeyInput): TreeKeyResult {
  const { key, rows, focusKey, typeahead, now } = input;
  if (rows.length === 0) return result(NONE, typeahead, false);

  const index = focusKey === null ? -1 : rows.findIndex((r) => r.key === focusKey);
  const current = index >= 0 ? rows[index] : null;

  switch (key) {
    case 'ArrowDown': {
      const next = rows[Math.min(index + 1, rows.length - 1)];
      return result(next.key === focusKey ? NONE : { type: 'focus', key: next.key }, typeahead, true);
    }
    case 'ArrowUp': {
      const prev = rows[Math.max(index - 1, 0)];
      return result(prev.key === focusKey ? NONE : { type: 'focus', key: prev.key }, typeahead, true);
    }
    case 'ArrowRight': {
      if (current === null) return result({ type: 'focus', key: rows[0].key }, typeahead, true);
      if (current.expandable && !current.expanded) {
        return result({ type: 'expand', key: current.key }, typeahead);
      }
      // Already open: step into the first child, which is the next visible row.
      const child = rows[index + 1];
      if (current.expanded && child !== undefined && child.parentKey === current.key) {
        return result({ type: 'focus', key: child.key }, typeahead);
      }
      return result(NONE, typeahead, true);
    }
    case 'ArrowLeft': {
      if (current === null) return result({ type: 'focus', key: rows[0].key }, typeahead, true);
      if (current.expandable && current.expanded) {
        return result({ type: 'collapse', key: current.key }, typeahead);
      }
      if (current.parentKey !== null) {
        return result({ type: 'focus', key: current.parentKey }, typeahead);
      }
      return result(NONE, typeahead, true);
    }
    case 'Home':
      return result({ type: 'focus', key: rows[0].key }, typeahead, true);
    case 'End':
      return result({ type: 'focus', key: rows[rows.length - 1].key }, typeahead, true);
    case 'Enter':
      return current === null
        ? result(NONE, typeahead, false)
        : result({ type: 'activate', key: current.key }, typeahead);
    case ' ':
      return current === null
        ? result(NONE, typeahead, false)
        : result({ type: 'select', key: current.key }, typeahead);
    case '*': {
      if (current === null) return result(NONE, typeahead, false);
      const keys = rows
        .filter((r) => r.parentKey === current.parentKey && r.expandable && !r.expanded)
        .map((r) => r.key);
      return result({ type: 'expand-siblings', keys }, typeahead);
    }
    case 'Delete':
      // Only array children can be removed — there is no op that deletes a
      // single containment, and the root has no parent array.
      return current !== null && current.kind === 'element' && current.arrayIndex !== null
        ? result({ type: 'delete', key: current.key }, typeahead)
        : result(NONE, typeahead, false);
    case 'F2':
      return current !== null && current.kind === 'element'
        ? result({ type: 'rename', key: current.key }, typeahead)
        : result(NONE, typeahead, false);
    default:
      break;
  }

  if (!isTypeaheadKey(key)) return result(NONE, typeahead, false);

  const fresh = now - typeahead.at > TYPEAHEAD_RESET_MS;
  const buffer = (fresh ? '' : typeahead.buffer) + key.toLowerCase();
  const next: TypeaheadState = { buffer, at: now };
  // Search forward from the row after the focused one, wrapping once. A
  // single repeated character cycles through the rows starting with it.
  const start = index >= 0 ? index + 1 : 0;
  for (let i = 0; i < rows.length; i++) {
    const row = rows[(start + i) % rows.length];
    if (row.label.toLowerCase().startsWith(buffer)) {
      return result({ type: 'focus', key: row.key }, next);
    }
  }
  return result(NONE, next, true);
}
