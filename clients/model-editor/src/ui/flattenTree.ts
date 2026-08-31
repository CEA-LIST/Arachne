/**
 * The explorer's row model: a ModelNode containment tree plus a collapse set
 * and a filter, flattened into the exact list of VISIBLE rows.
 *
 * Pure and metamodel-agnostic. Two row kinds interleave — elements and
 * containment feature groups — and they must not share ids, because a single
 * containment's child element has the same path as its feature group
 * ([...elementPath, featureName]). Hence the 'e:' / 'f:' key prefixes.
 *
 * Rows are expanded by default and collapse is explicit: a newly created
 * element is therefore visible the moment the poll brings it back, which is
 * the behaviour an editor needs. `flattenTree` also owns the ARIA counters
 * (level / posinset / setsize), so the treeview pattern is testable without a
 * DOM.
 */

import type { ContainmentDesc, Descriptor, Path } from '../api/types';
import { pathKey } from '../crdt/path';
import type { FeatureNode, ModelNode } from '../model/instance';

export type TreeRowKind = 'element' | 'feature';

export interface TreeRow {
  /** Unique across both row kinds: 'e:<pathKey>' or 'f:<pathKey>'. */
  key: string;
  kind: TreeRowKind;
  path: Path;
  /** 1-based, as aria-level wants it. */
  level: number;
  parentKey: string | null;
  posInSet: number;
  setSize: number;
  label: string;
  expandable: boolean;
  expanded: boolean;
  /** True when the row itself matched a non-empty filter. */
  matched: boolean;

  /* element rows */
  eClass: string;
  /** eClass declared by the descriptor; false for unknown classes. */
  known: boolean;
  /** '' eClass — buildTree's degenerate array slot. */
  invalid: boolean;
  /** Index in its parent array, or null when not an array child (not removable). */
  arrayIndex: number | null;
  /** The element declares at least one containment, so "add child" applies. */
  canAddChild: boolean;

  /* feature rows */
  feature: ContainmentDesc | null;
  childCount: number;
  empty: boolean;
}

export function elementKey(path: Path): string {
  return `e:${pathKey(path)}`;
}

export function featureKey(path: Path): string {
  return `f:${pathKey(path)}`;
}

function matches(text: string, needle: string): boolean {
  return needle.length > 0 && text.toLowerCase().includes(needle);
}

/** Internal node of the pre-flatten tree, before collapse and filtering. */
interface Built {
  row: TreeRow;
  children: Built[];
  /** This row or any descendant matched the filter. */
  hit: boolean;
}

function buildElement(
  descriptor: Descriptor,
  node: ModelNode,
  level: number,
  parentKey: string | null,
  needle: string,
): Built {
  const last = node.path[node.path.length - 1];
  const invalid = node.eClass === '';
  const row: TreeRow = {
    key: elementKey(node.path),
    kind: 'element',
    path: node.path,
    level,
    parentKey,
    posInSet: 1,
    setSize: 1,
    label: node.label,
    expandable: node.features.length > 0,
    expanded: true,
    matched: matches(node.label, needle) || matches(node.eClass, needle),
    eClass: node.eClass,
    known: !invalid && descriptor.classes[node.eClass] !== undefined,
    invalid,
    arrayIndex: typeof last === 'number' ? last : null,
    canAddChild: node.features.length > 0,
    feature: null,
    childCount: node.features.length,
    empty: false,
  };
  const children = node.features.map((feature) =>
    buildFeature(descriptor, feature, node.path, level + 1, row.key, needle),
  );
  return { row, children, hit: row.matched || children.some((c) => c.hit) };
}

function buildFeature(
  descriptor: Descriptor,
  feature: FeatureNode,
  elementPath: Path,
  level: number,
  parentKey: string,
  needle: string,
): Built {
  const path = [...elementPath, feature.desc.name];
  const row: TreeRow = {
    key: featureKey(path),
    kind: 'feature',
    path,
    level,
    parentKey,
    posInSet: 1,
    setSize: 1,
    label: feature.desc.name,
    expandable: feature.children.length > 0,
    expanded: true,
    matched: matches(feature.desc.name, needle),
    eClass: '',
    known: true,
    invalid: false,
    arrayIndex: null,
    canAddChild: false,
    feature: feature.desc,
    childCount: feature.children.length,
    empty: feature.children.length === 0,
  };
  const children = feature.children.map((child) =>
    buildElement(descriptor, child, level + 1, row.key, needle),
  );
  return { row, children, hit: row.matched || children.some((c) => c.hit) };
}

export interface FlattenOptions {
  /** Explicitly collapsed row keys; every other expandable row is open. */
  collapsed?: ReadonlySet<string>;
  /** Case-insensitive substring filter over labels, eClasses and feature names. */
  filter?: string;
}

/**
 * The visible rows, in display order.
 *
 * With a filter: rows are kept when they match or have a matching descendant,
 * and the collapse set is IGNORED on the path to a match — a filter that hid
 * its own results behind a collapsed ancestor would be useless.
 */
export function flattenTree(
  descriptor: Descriptor,
  root: ModelNode,
  options: FlattenOptions = {},
): TreeRow[] {
  const collapsed = options.collapsed ?? new Set<string>();
  const needle = (options.filter ?? '').trim().toLowerCase();
  const filtering = needle.length > 0;

  const built = buildElement(descriptor, root, 1, null, needle);
  const rows: TreeRow[] = [];

  const emit = (node: Built, visibleSiblings: Built[]) => {
    const kept = filtering ? node.children.filter((c) => c.hit) : node.children;
    // A filter reveals its matches; otherwise the collapse set decides.
    const expanded = kept.length > 0 && (filtering || !collapsed.has(node.row.key));
    rows.push({
      ...node.row,
      expandable: kept.length > 0,
      expanded,
      posInSet: visibleSiblings.indexOf(node) + 1,
      setSize: visibleSiblings.length,
    });
    if (!expanded) return;
    for (const child of kept) emit(child, kept);
  };

  if (!filtering || built.hit) emit(built, [built]);
  return rows;
}

/** Total element rows in the tree (the explorer's "N elements" count). */
export function countElements(root: ModelNode): number {
  let total = 1;
  for (const feature of root.features) {
    for (const child of feature.children) total += countElements(child);
  }
  return total;
}

export interface HighlightPart {
  text: string;
  hit: boolean;
}

/** Split `text` on every case-insensitive occurrence of `needle`, for <mark>. */
export function highlightParts(text: string, needle: string): HighlightPart[] {
  const trimmed = needle.trim().toLowerCase();
  if (trimmed.length === 0) return [{ text, hit: false }];
  const parts: HighlightPart[] = [];
  const haystack = text.toLowerCase();
  let cursor = 0;
  for (;;) {
    const at = haystack.indexOf(trimmed, cursor);
    if (at === -1) break;
    if (at > cursor) parts.push({ text: text.slice(cursor, at), hit: false });
    parts.push({ text: text.slice(at, at + trimmed.length), hit: true });
    cursor = at + trimmed.length;
  }
  if (cursor < text.length) parts.push({ text: text.slice(cursor), hit: false });
  return parts.length > 0 ? parts : [{ text, hit: false }];
}
