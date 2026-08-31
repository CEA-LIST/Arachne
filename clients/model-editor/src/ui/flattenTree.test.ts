import { describe, expect, it } from 'vitest';
import type { Descriptor, PlainJson } from '../api/types';
import { buildTree } from '../model/instance';
import { countElements, elementKey, featureKey, flattenTree, highlightParts } from './flattenTree';

/**
 * A deliberately generic descriptor: the UI layer must never know a class name
 * from any sample metamodel, and neither must its tests.
 */
const descriptor: Descriptor = {
  formatVersion: 1,
  package: 'demo',
  nsURI: 'http://example.org/demo',
  rootClasses: ['Container'],
  enums: {},
  classes: {
    Container: {
      abstract: false,
      superTypes: [],
      attributes: [{ name: 'name', kind: 'string', many: false, required: false, isId: true }],
      containments: [
        { name: 'items', target: 'Item', many: true, required: false, ordered: true },
        { name: 'header', target: 'Item', many: false, required: false, ordered: false },
      ],
      references: [],
    },
    Item: {
      abstract: false,
      superTypes: [],
      attributes: [{ name: 'name', kind: 'string', many: false, required: false, isId: true }],
      containments: [{ name: 'items', target: 'Item', many: true, required: false, ordered: true }],
      references: [],
    },
  },
};

const doc: PlainJson = {
  eClass: 'Container',
  name: 'Root',
  items: [
    { eClass: 'Item', name: 'Alpha', items: [{ eClass: 'Item', name: 'Deep' }] },
    { eClass: 'Item', name: 'Beta', items: [] },
  ],
  header: { eClass: '', name: '' },
};

function tree(source: PlainJson = doc) {
  const built = buildTree(descriptor, source);
  if (built === null) throw new Error('fixture has no root');
  return built;
}

describe('flattenTree', () => {
  it('interleaves element and feature rows in display order', () => {
    const rows = flattenTree(descriptor, tree());
    expect(rows.map((r) => `${r.kind}:${r.label}`)).toEqual([
      'element:Root',
      'feature:items',
      'element:Alpha',
      'feature:items',
      'element:Deep',
      'feature:items',
      'element:Beta',
      'feature:items',
      'feature:header',
    ]);
  });

  it('gives element and feature rows distinct keys at the same path', () => {
    const present: PlainJson = { ...(doc as object), header: { eClass: 'Item', name: 'Head' } };
    const rows = flattenTree(descriptor, tree(present));
    const keys = rows.map((r) => r.key);
    expect(keys).toContain(featureKey(['header']));
    expect(keys).toContain(elementKey(['header']));
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('numbers levels and ARIA set counters from the visible rows', () => {
    const rows = flattenTree(descriptor, tree());
    const root = rows[0];
    expect(root.level).toBe(1);
    expect(root.setSize).toBe(1);
    const items = rows.find((r) => r.kind === 'feature' && r.level === 2);
    expect(items?.level).toBe(2);
    const alpha = rows.find((r) => r.label === 'Alpha');
    const beta = rows.find((r) => r.label === 'Beta');
    expect(alpha?.level).toBe(3);
    expect(alpha?.posInSet).toBe(1);
    expect(beta?.posInSet).toBe(2);
    expect(beta?.setSize).toBe(2);
  });

  it('expands by default so a newly created element is visible at once', () => {
    const rows = flattenTree(descriptor, tree());
    expect(rows.find((r) => r.label === 'Deep')).toBeDefined();
  });

  it('hides a collapsed row subtree but keeps the row itself', () => {
    const collapsed = new Set([elementKey(['items', 0])]);
    const rows = flattenTree(descriptor, tree(), { collapsed });
    const alpha = rows.find((r) => r.label === 'Alpha');
    expect(alpha?.expanded).toBe(false);
    expect(alpha?.expandable).toBe(true);
    expect(rows.find((r) => r.label === 'Deep')).toBeUndefined();
  });

  it('marks an empty feature group as empty and not expandable', () => {
    const rows = flattenTree(descriptor, tree());
    const header = rows.find((r) => r.key === featureKey(['header']));
    expect(header?.empty).toBe(true);
    expect(header?.expandable).toBe(false);
    expect(header?.childCount).toBe(0);
  });

  it('keeps the ancestors of a filter match and drops everything else', () => {
    const rows = flattenTree(descriptor, tree(), { filter: 'deep' });
    expect(rows.map((r) => r.label)).toEqual(['Root', 'items', 'Alpha', 'items', 'Deep']);
    expect(rows.find((r) => r.label === 'Deep')?.matched).toBe(true);
    expect(rows.find((r) => r.label === 'Alpha')?.matched).toBe(false);
  });

  it('reveals matches even under a collapsed ancestor', () => {
    const collapsed = new Set([elementKey(['items', 0]), elementKey([])]);
    const rows = flattenTree(descriptor, tree(), { collapsed, filter: 'deep' });
    expect(rows.map((r) => r.label)).toContain('Deep');
  });

  it('matches on the eClass as well as the label', () => {
    const rows = flattenTree(descriptor, tree(), { filter: 'container' });
    expect(rows.map((r) => r.label)).toEqual(['Root']);
  });

  it('returns no rows when nothing matches', () => {
    expect(flattenTree(descriptor, tree(), { filter: 'zzz' })).toEqual([]);
  });

  it('flags a degenerate array slot as invalid and an unknown eClass as unknown', () => {
    const odd: PlainJson = {
      eClass: 'Container',
      name: 'Root',
      items: [{ eClass: '' }, { eClass: 'Ghost', name: 'G' }],
    };
    const rows = flattenTree(descriptor, tree(odd));
    const invalid = rows.find((r) => r.key === elementKey(['items', 0]));
    const unknown = rows.find((r) => r.key === elementKey(['items', 1]));
    expect(invalid?.invalid).toBe(true);
    expect(unknown?.invalid).toBe(false);
    expect(unknown?.known).toBe(false);
  });

  it('exposes the array index of array children only', () => {
    const rows = flattenTree(descriptor, tree());
    expect(rows.find((r) => r.label === 'Beta')?.arrayIndex).toBe(1);
    expect(rows[0].arrayIndex).toBeNull();
  });
});

describe('countElements', () => {
  it('counts every element in the containment tree', () => {
    expect(countElements(tree())).toBe(4);
  });
});

describe('highlightParts', () => {
  it('splits on every case-insensitive occurrence', () => {
    expect(highlightParts('AlphaBetaAlpha', 'alpha')).toEqual([
      { text: 'Alpha', hit: true },
      { text: 'Beta', hit: false },
      { text: 'Alpha', hit: true },
    ]);
  });

  it('returns the whole string unhit for an empty needle', () => {
    expect(highlightParts('Alpha', '')).toEqual([{ text: 'Alpha', hit: false }]);
  });
});
