import { describe, expect, it } from 'vitest';
import type { TreeRow } from './flattenTree';
import { emptyTypeahead, treeKeyCommand, TYPEAHEAD_RESET_MS } from './treeKeyboard';

function row(partial: Partial<TreeRow> & { key: string; label: string }): TreeRow {
  return {
    kind: 'element',
    path: [],
    level: 1,
    parentKey: null,
    posInSet: 1,
    setSize: 1,
    expandable: false,
    expanded: false,
    matched: false,
    eClass: 'X',
    known: true,
    invalid: false,
    arrayIndex: null,
    canAddChild: false,
    feature: null,
    childCount: 0,
    empty: false,
    ...partial,
  };
}

/** root > group(open) > alpha, beta(collapsed) ; beta's child is not visible. */
const rows: TreeRow[] = [
  row({ key: 'root', label: 'Root', expandable: true, expanded: true }),
  row({ key: 'group', label: 'items', kind: 'feature', level: 2, parentKey: 'root', expandable: true, expanded: true }),
  row({ key: 'alpha', label: 'Alpha', level: 3, parentKey: 'group', arrayIndex: 0, posInSet: 1, setSize: 2 }),
  row({
    key: 'beta',
    label: 'Beta',
    level: 3,
    parentKey: 'group',
    arrayIndex: 1,
    posInSet: 2,
    setSize: 2,
    expandable: true,
    expanded: false,
  }),
];

function press(key: string, focusKey: string | null, typeahead = emptyTypeahead, now = 0) {
  return treeKeyCommand({ key, rows, focusKey, typeahead, now });
}

describe('treeKeyCommand — navigation', () => {
  it('walks the flattened visible list with the arrow keys', () => {
    expect(press('ArrowDown', 'root').command).toEqual({ type: 'focus', key: 'group' });
    expect(press('ArrowUp', 'alpha').command).toEqual({ type: 'focus', key: 'group' });
  });

  it('clamps at both ends instead of wrapping', () => {
    expect(press('ArrowUp', 'root').command).toEqual({ type: 'none' });
    expect(press('ArrowDown', 'beta').command).toEqual({ type: 'none' });
  });

  it('still consumes a clamped arrow so the panel does not scroll', () => {
    expect(press('ArrowUp', 'root').handled).toBe(true);
  });

  it('jumps to the ends with Home and End', () => {
    expect(press('Home', 'beta').command).toEqual({ type: 'focus', key: 'root' });
    expect(press('End', 'root').command).toEqual({ type: 'focus', key: 'beta' });
  });

  it('enters the tree from nowhere on a first arrow press', () => {
    expect(press('ArrowDown', null).command).toEqual({ type: 'focus', key: 'root' });
  });
});

describe('treeKeyCommand — expansion', () => {
  it('expands a collapsed row with ArrowRight', () => {
    expect(press('ArrowRight', 'beta').command).toEqual({ type: 'expand', key: 'beta' });
  });

  it('steps into the first child when the row is already open', () => {
    expect(press('ArrowRight', 'root').command).toEqual({ type: 'focus', key: 'group' });
  });

  it('does nothing on ArrowRight at a leaf', () => {
    expect(press('ArrowRight', 'alpha').command).toEqual({ type: 'none' });
  });

  it('collapses an open row with ArrowLeft', () => {
    expect(press('ArrowLeft', 'group').command).toEqual({ type: 'collapse', key: 'group' });
  });

  it('moves to the parent from a leaf or a collapsed row', () => {
    expect(press('ArrowLeft', 'alpha').command).toEqual({ type: 'focus', key: 'group' });
    expect(press('ArrowLeft', 'beta').command).toEqual({ type: 'focus', key: 'group' });
  });

  it('collapses the root rather than escaping the tree', () => {
    expect(press('ArrowLeft', 'root').command).toEqual({ type: 'collapse', key: 'root' });
  });

  it('does nothing on ArrowLeft at a row with neither children nor a parent', () => {
    const lone = [row({ key: 'only', label: 'Only' })];
    const out = treeKeyCommand({
      key: 'ArrowLeft',
      rows: lone,
      focusKey: 'only',
      typeahead: emptyTypeahead,
      now: 0,
    });
    expect(out.command).toEqual({ type: 'none' });
  });

  it('expands the collapsed siblings at this level with *', () => {
    expect(press('*', 'alpha').command).toEqual({ type: 'expand-siblings', keys: ['beta'] });
  });
});

describe('treeKeyCommand — activation', () => {
  it('selects without leaving the tree on Space', () => {
    expect(press(' ', 'alpha').command).toEqual({ type: 'select', key: 'alpha' });
  });

  it('selects and moves into the form on Enter', () => {
    expect(press('Enter', 'alpha').command).toEqual({ type: 'activate', key: 'alpha' });
  });

  it('offers Delete only for array children', () => {
    expect(press('Delete', 'alpha').command).toEqual({ type: 'delete', key: 'alpha' });
    expect(press('Delete', 'root').command).toEqual({ type: 'none' });
    expect(press('Delete', 'group').command).toEqual({ type: 'none' });
  });

  it('offers F2 only on element rows', () => {
    expect(press('F2', 'alpha').command).toEqual({ type: 'rename', key: 'alpha' });
    expect(press('F2', 'group').command).toEqual({ type: 'none' });
  });

  it('leaves unhandled keys to the browser', () => {
    const out = press('Tab', 'alpha');
    expect(out.command).toEqual({ type: 'none' });
    expect(out.handled).toBe(false);
  });
});

describe('treeKeyCommand — type-ahead', () => {
  it('jumps to the next row whose label starts with the typed letter', () => {
    const out = press('b', 'root');
    expect(out.command).toEqual({ type: 'focus', key: 'beta' });
    expect(out.typeahead.buffer).toBe('b');
  });

  it('accumulates within the buffer window', () => {
    const first = press('a', 'root', emptyTypeahead, 1000);
    const second = press('l', 'root', first.typeahead, 1000 + TYPEAHEAD_RESET_MS - 1);
    expect(second.typeahead.buffer).toBe('al');
    expect(second.command).toEqual({ type: 'focus', key: 'alpha' });
  });

  it('resets the buffer after the window lapses', () => {
    const first = press('a', 'root', emptyTypeahead, 1000);
    const second = press('b', 'root', first.typeahead, 1000 + TYPEAHEAD_RESET_MS + 1);
    expect(second.typeahead.buffer).toBe('b');
  });

  it('searches from after the focused row and wraps around', () => {
    expect(press('r', 'beta').command).toEqual({ type: 'focus', key: 'root' });
  });

  it('is case-insensitive', () => {
    expect(press('B', 'root').command).toEqual({ type: 'focus', key: 'beta' });
  });

  it('consumes a non-matching letter rather than letting it through', () => {
    const out = press('z', 'root');
    expect(out.command).toEqual({ type: 'none' });
    expect(out.handled).toBe(true);
  });
});
