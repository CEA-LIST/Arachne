/**
 * Pure functions interpreting a decoded document against a metamodel
 * descriptor: inheritance closure, concrete-subtype sets, the eClass presence
 * rule, containment-tree building, and instance collection for reference
 * pickers. No React, no I/O — everything here is unit-testable.
 *
 * Instance-encoding convention (the model <-> JSON contract):
 * - every EObject carries an `eClass` string field; a slot is "present" iff
 *   that string is non-empty (Object.Remove resets to defaults, it never
 *   deletes the key, so emptiness is the only honest absence marker);
 * - many-features are arrays, single containments objects under the feature
 *   key, references strings holding the target's id-attribute value.
 */

import type {
  AttributeDesc,
  ContainmentDesc,
  Descriptor,
  Path,
  PlainJson,
  ReferenceDesc,
} from '../api/types';

/* ---------- inheritance ---------- */

export interface FlatFeatures {
  attributes: AttributeDesc[];
  containments: ContainmentDesc[];
  references: ReferenceDesc[];
}

/**
 * The inheritance closure of `className`, supertype features first, declared
 * order preserved within each class. Unknown classes and supertype cycles are
 * tolerated (each class contributes at most once).
 */
export function flattenFeatures(descriptor: Descriptor, className: string): FlatFeatures {
  const result: FlatFeatures = { attributes: [], containments: [], references: [] };
  const visited = new Set<string>();
  const visit = (name: string) => {
    if (visited.has(name)) return;
    visited.add(name);
    const cls = descriptor.classes[name];
    if (cls === undefined) return;
    for (const sup of cls.superTypes) visit(sup);
    result.attributes.push(...cls.attributes);
    result.containments.push(...cls.containments);
    result.references.push(...cls.references);
  };
  visit(className);
  return result;
}

/** Whether `className` equals `target` or transitively inherits from it. */
export function isSubtypeOf(descriptor: Descriptor, className: string, target: string): boolean {
  const visited = new Set<string>();
  const visit = (name: string): boolean => {
    if (name === target) return true;
    if (visited.has(name)) return false;
    visited.add(name);
    const cls = descriptor.classes[name];
    if (cls === undefined) return false;
    return cls.superTypes.some(visit);
  };
  return visit(className);
}

/**
 * The concrete classes assignable to a slot typed `base` (including `base`
 * itself when concrete), sorted alphabetically. These populate "Add"/"Create"
 * menus for containments typed by an abstract class.
 */
export function concreteSubtypes(descriptor: Descriptor, base: string): string[] {
  return Object.keys(descriptor.classes)
    .filter((name) => !descriptor.classes[name].abstract && isSubtypeOf(descriptor, name, base))
    .sort();
}

/**
 * Concrete classes offered for root creation: the descriptor's root classes
 * expanded to their concrete families; every concrete class if the descriptor
 * names none.
 */
export function rootCandidates(descriptor: Descriptor): string[] {
  if (descriptor.rootClasses.length === 0) {
    return Object.keys(descriptor.classes)
      .filter((name) => !descriptor.classes[name].abstract)
      .sort();
  }
  const set = new Set<string>();
  for (const root of descriptor.rootClasses) {
    for (const cls of concreteSubtypes(descriptor, root)) set.add(cls);
  }
  return [...set].sort();
}

/* ---------- presence and labels ---------- */

/** The presence rule: an object slot exists iff its eClass string is non-empty. */
export function isPresent(value: PlainJson | undefined): value is Record<string, PlainJson> {
  return (
    value !== null &&
    value !== undefined &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    typeof value['eClass'] === 'string' &&
    value['eClass'].length > 0
  );
}

/** The element's eClass, or null when the value is not a present object. */
export function eClassOf(value: PlainJson | undefined): string | null {
  return isPresent(value) ? (value['eClass'] as string) : null;
}

/**
 * The attribute whose value identifies instances of `className`: the first
 * isId attribute, else the first string attribute named ID/id/name/Name,
 * else the first single-valued string attribute at all (a metamodel without
 * declared ids — like bt's BlackboardEntry.key — still needs referenceable
 * instances).
 */
export function idAttributeOf(descriptor: Descriptor, className: string): AttributeDesc | null {
  const attrs = flattenFeatures(descriptor, className).attributes;
  const byId = attrs.find((a) => a.isId);
  if (byId !== undefined) return byId;
  for (const candidate of ['ID', 'id', 'name', 'Name']) {
    const found = attrs.find((a) => a.name === candidate && a.kind === 'string');
    if (found !== undefined) return found;
  }
  return attrs.find((a) => a.kind === 'string' && !a.many) ?? null;
}

/** The id-attribute value of `element` ('' when unset or no id attribute). */
export function idValueOf(descriptor: Descriptor, element: Record<string, PlainJson>): string {
  const attr = idAttributeOf(descriptor, element['eClass'] as string);
  if (attr === null) return '';
  const value = element[attr.name];
  return typeof value === 'string' ? value : '';
}

/** Tree label: the id/name attribute value when non-empty, else the eClass. */
export function labelFor(descriptor: Descriptor, element: Record<string, PlainJson>): string {
  const id = idValueOf(descriptor, element);
  return id.length > 0 ? id : (element['eClass'] as string);
}

/* ---------- attribute defaults ---------- */

/** The CRDT default the wire materializes for an attribute kind. */
export function defaultForKind(kind: AttributeDesc['kind']): string | number | boolean {
  switch (kind) {
    case 'int':
    case 'float':
      return 0;
    case 'bool':
      return false;
    default:
      return '';
  }
}

/** The attribute's current value, falling back to its kind default. */
export function attributeValue(
  element: Record<string, PlainJson>,
  attr: AttributeDesc,
): string | number | boolean {
  const raw = element[attr.name];
  switch (attr.kind) {
    case 'int':
    case 'float':
      return typeof raw === 'number' ? raw : 0;
    case 'bool':
      return typeof raw === 'boolean' ? raw : false;
    default:
      return typeof raw === 'string' ? raw : '';
  }
}

/* ---------- containment tree ---------- */

export interface ModelNode {
  path: Path;
  /** '' marks an array slot that is not a present object (should not happen). */
  eClass: string;
  label: string;
  features: FeatureNode[];
}

export interface FeatureNode {
  desc: ContainmentDesc;
  /** For a single containment: [] when absent, one node when present. */
  children: ModelNode[];
}

/**
 * Build the containment tree of the document. Array indices are preserved
 * even for degenerate elements (non-objects, empty eClass) because the wire
 * ops are positional. Returns null when the document has no present root.
 */
export function buildTree(descriptor: Descriptor, doc: PlainJson): ModelNode | null {
  if (!isPresent(doc)) return null;
  return buildNode(descriptor, doc, []);
}

function buildNode(
  descriptor: Descriptor,
  element: Record<string, PlainJson>,
  path: Path,
): ModelNode {
  const eClass = element['eClass'] as string;
  const flat = flattenFeatures(descriptor, eClass);
  const features: FeatureNode[] = flat.containments.map((desc) => {
    const raw = element[desc.name];
    const children: ModelNode[] = [];
    if (desc.many) {
      if (Array.isArray(raw)) {
        raw.forEach((entry, index) => {
          const childPath = [...path, desc.name, index];
          if (isPresent(entry)) {
            children.push(buildNode(descriptor, entry, childPath));
          } else {
            children.push({ path: childPath, eClass: '', label: '(invalid element)', features: [] });
          }
        });
      }
    } else if (isPresent(raw)) {
      children.push(buildNode(descriptor, raw, [...path, desc.name]));
    }
    return { desc, children };
  });
  return { path, eClass, label: labelFor(descriptor, element), features };
}

/* ---------- reference pickers ---------- */

export interface InstanceRef {
  path: Path;
  eClass: string;
  /** Value of the id attribute; '' when unset (not selectable as a target). */
  id: string;
  label: string;
}

/**
 * Every present instance in the document assignable to a reference typed
 * `target`, in containment-tree order. Reference pickers list these by id.
 */
export function collectInstances(
  descriptor: Descriptor,
  doc: PlainJson,
  target: string,
): InstanceRef[] {
  const result: InstanceRef[] = [];
  const visit = (value: PlainJson, path: Path) => {
    if (!isPresent(value)) return;
    const eClass = value['eClass'] as string;
    if (isSubtypeOf(descriptor, eClass, target)) {
      result.push({ path, eClass, id: idValueOf(descriptor, value), label: labelFor(descriptor, value) });
    }
    for (const desc of flattenFeatures(descriptor, eClass).containments) {
      const raw = value[desc.name];
      if (desc.many) {
        if (Array.isArray(raw)) {
          raw.forEach((entry, index) => visit(entry, [...path, desc.name, index]));
        }
      } else {
        visit(raw ?? null, [...path, desc.name]);
      }
    }
  };
  visit(doc, []);
  return result;
}
