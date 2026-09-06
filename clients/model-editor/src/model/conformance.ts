/**
 * State-dependent conformance: what only the converged document can violate,
 * evaluated after an apply and reported beside the binding verdict, never
 * refused and never a hold on the edit gate.
 *
 * The node refuses at its intake what one operation and the descriptor alone
 * rule out (an unknown feature, a value of the wrong kind, a list where one
 * object belongs, a class-tag character that fits no allowed class). Whatever
 * needs the document is here, because two operations each valid alone can
 * jointly violate it, and a replica that refused one of them on receive would
 * never agree with the replica that applied it. So editor-a's `Sequence`
 * named `patrol` and editor-b's `Fallback` named `patrol` are both applied
 * everywhere, and this reports the collision to both.
 *
 * Five rules, each from the descriptor's own fields and nothing else (no OCL
 * is evaluated, in this file or anywhere):
 * - `duplicate-id`: two objects carry one value of the same identifying
 *   attribute, the one `idAttributeOf` resolves references by, keyed by the
 *   class that declares it, so a `Sequence` and a `Fallback` collide on
 *   `TreeNode.ID`;
 * - `missing-required`: a `required` feature absent on an object, the header
 *   excluded; a string counts as absent when empty, since the CRDT
 *   materializes an unset string as `''`;
 * - `dangling-reference`: a reference names an identifying value that no
 *   object assignable to its target carries;
 * - `unknown-class`: an `eClass` that names no concrete class of the
 *   descriptor, which is what a class name refused mid-word at the node's
 *   intake leaves behind (`Cl`), and what a name every character of which
 *   fits some allowed class gets past that intake;
 * - `invalid-literal`: an enum attribute holding a string that is not one of
 *   the enum's literals, the same story over the literals.
 *
 * Pure: no React, no I/O.
 */

import type { AttributeDesc, ContainmentDesc, Descriptor, Path, PlainJson, ReferenceDesc } from '../api/types';
import { pathKey } from '../crdt/path';
import { flattenFeatures, idAttributeOf, isPresent, isReservedKey, isSubtypeOf } from './instance';

export type Rule = 'duplicate-id' | 'missing-required' | 'dangling-reference' | 'unknown-class' | 'invalid-literal';

export interface Diagnostic {
  rule: Rule;
  /** The object the diagnostic is about. */
  path: Path;
  /** One sentence naming the feature, the path and the value. */
  message: string;
}

/** One present object of the document, with the value it is identified by. */
interface Instance {
  path: Path;
  eClass: string;
  /** `Owner.attribute` of the identifying attribute, or null when the class has none. */
  idFeature: string | null;
  id: string;
}

/** `path` as the messages and the panel show it: keys and indices joined, `/` for the root. */
export function pathText(path: Path): string {
  return path.length === 0 ? '/' : path.join('/');
}

/**
 * The class that declares `feature` for an instance of `className`: the
 * class itself, else the nearest supertype in declaration order. Null when
 * no class of the closure declares it.
 */
export function declaringClass(descriptor: Descriptor, className: string, feature: string): string | null {
  const visited = new Set<string>();
  const visit = (name: string): string | null => {
    if (visited.has(name)) return null;
    visited.add(name);
    const cls = descriptor.classes[name];
    if (cls === undefined) return null;
    const declared = [...cls.attributes, ...cls.containments, ...cls.references].some((f) => f.name === feature);
    if (declared) return name;
    for (const sup of cls.superTypes) {
      const found = visit(sup);
      if (found !== null) return found;
    }
    return null;
  };
  return visit(className);
}

function qualified(descriptor: Descriptor, className: string, feature: string): string {
  return `${declaringClass(descriptor, className, feature) ?? className}.${feature}`;
}

function nonEmptyStrings(value: PlainJson | undefined): string[] {
  if (typeof value === 'string') return value.length > 0 ? [value] : [];
  if (Array.isArray(value)) return value.filter((v): v is string => typeof v === 'string' && v.length > 0);
  return [];
}

function attributePresent(attr: AttributeDesc, value: PlainJson | undefined): boolean {
  if (value === undefined || value === null) return false;
  if (attr.many) return Array.isArray(value) && value.length > 0;
  if (attr.kind === 'string' || attr.kind === 'enum') return typeof value === 'string' && value.length > 0;
  return true;
}

function containmentPresent(desc: ContainmentDesc, value: PlainJson | undefined): boolean {
  if (desc.many) return Array.isArray(value) && value.some(isPresent);
  return isPresent(value);
}

function referencePresent(value: PlainJson | undefined): boolean {
  return nonEmptyStrings(value).length > 0;
}

/**
 * Every diagnostic the descriptor's own fields imply for `doc`: an empty
 * list for a document that holds every invariant, a document with no present
 * root included.
 */
export function checkInvariants(descriptor: Descriptor, doc: PlainJson): Diagnostic[] {
  const diagnostics: Diagnostic[] = [];
  const instances: Instance[] = [];
  const references: { path: Path; eClass: string; desc: ReferenceDesc; value: string }[] = [];

  const visit = (value: PlainJson, path: Path) => {
    if (!isPresent(value)) return;
    const eClass = value['eClass'] as string;
    const cls = descriptor.classes[eClass];
    if (cls === undefined || cls.abstract) {
      diagnostics.push({
        rule: 'unknown-class',
        path,
        message: `eClass \`${eClass}\` at ${pathText(path)} is not a concrete class of the descriptor`,
      });
      // Its features are unknown, so nothing below it can be typed.
      return;
    }
    const idAttr = idAttributeOf(descriptor, eClass);
    const idValue = idAttr === null ? '' : value[idAttr.name];
    instances.push({
      path,
      eClass,
      idFeature: idAttr === null ? null : qualified(descriptor, eClass, idAttr.name),
      id: typeof idValue === 'string' ? idValue : '',
    });
    const flat = flattenFeatures(descriptor, eClass);
    for (const attr of flat.attributes) {
      if (isReservedKey(attr.name)) continue;
      const raw = value[attr.name];
      if (attr.required && !attributePresent(attr, raw)) {
        diagnostics.push({
          rule: 'missing-required',
          path,
          message: `\`${qualified(descriptor, eClass, attr.name)}\` is required and absent at ${pathText(path)}`,
        });
      }
      if (attr.kind === 'enum' && attr.enum !== undefined) {
        const literals = descriptor.enums[attr.enum] ?? [];
        for (const literal of nonEmptyStrings(raw)) {
          if (!literals.includes(literal)) {
            diagnostics.push({
              rule: 'invalid-literal',
              path,
              message: `\`${qualified(descriptor, eClass, attr.name)}\` at ${pathText(path)} holds \`${literal}\`, not a literal of \`${attr.enum}\` (${literals.join(', ')})`,
            });
          }
        }
      }
    }
    for (const desc of flat.references) {
      if (isReservedKey(desc.name)) continue;
      const raw = value[desc.name];
      if (desc.required && !referencePresent(raw)) {
        diagnostics.push({
          rule: 'missing-required',
          path,
          message: `\`${qualified(descriptor, eClass, desc.name)}\` is required and absent at ${pathText(path)}`,
        });
      }
      for (const target of nonEmptyStrings(raw)) {
        references.push({ path, eClass, desc, value: target });
      }
    }
    for (const desc of flat.containments) {
      if (isReservedKey(desc.name)) continue;
      const raw = value[desc.name];
      if (desc.required && !containmentPresent(desc, raw)) {
        diagnostics.push({
          rule: 'missing-required',
          path,
          message: `\`${qualified(descriptor, eClass, desc.name)}\` is required and absent at ${pathText(path)}`,
        });
      }
      if (desc.many) {
        if (Array.isArray(raw)) raw.forEach((entry, index) => visit(entry, [...path, desc.name, index]));
      } else {
        visit(raw ?? null, [...path, desc.name]);
      }
    }
  };
  visit(doc, []);

  // Uniqueness of the identifying value, per declaring attribute.
  const byId = new Map<string, Instance[]>();
  for (const instance of instances) {
    if (instance.idFeature === null || instance.id.length === 0) continue;
    const key = `${instance.idFeature}=${instance.id}`;
    byId.set(key, [...(byId.get(key) ?? []), instance]);
  }
  for (const group of byId.values()) {
    if (group.length < 2) continue;
    for (const instance of group) {
      const others = group
        .filter((other) => other !== instance)
        .map((other) => `${pathText(other.path)} (${other.eClass})`)
        .join(', ');
      diagnostics.push({
        rule: 'duplicate-id',
        path: instance.path,
        message: `\`${instance.idFeature}\` \`${instance.id}\` at ${pathText(instance.path)} (${instance.eClass}) is also carried by ${others}`,
      });
    }
  }

  // Referential integrity: a reference names an object assignable to its target.
  for (const reference of references) {
    const resolves = instances.some(
      (instance) => instance.id === reference.value && isSubtypeOf(descriptor, instance.eClass, reference.desc.target),
    );
    if (!resolves) {
      diagnostics.push({
        rule: 'dangling-reference',
        path: reference.path,
        message: `\`${qualified(descriptor, reference.eClass, reference.desc.name)}\` at ${pathText(reference.path)} names \`${reference.value}\`, which no \`${reference.desc.target}\` in the document carries`,
      });
    }
  }

  // Document order, so the list is stable across polls and across replicas.
  return diagnostics.sort((a, b) => {
    const byPath = pathKey(a.path).localeCompare(pathKey(b.path));
    return byPath !== 0 ? byPath : a.message.localeCompare(b.message);
  });
}

/** Whether two reports say the same thing, so a change is reported once and not on every poll. */
export function sameDiagnostics(a: Diagnostic[], b: Diagnostic[]): boolean {
  return (
    a.length === b.length &&
    a.every((d, i) => d.rule === b[i].rule && pathKey(d.path) === pathKey(b[i].path) && d.message === b[i].message)
  );
}

/** The report as one text: a count, then one line per diagnostic. */
export function describeDiagnostics(diagnostics: Diagnostic[]): string {
  if (diagnostics.length === 0) return 'every invariant of the descriptor holds';
  const head = `${diagnostics.length} conformance issue${diagnostics.length === 1 ? '' : 's'}`;
  return `${head}:\n${diagnostics.map((d) => `${d.rule}: ${d.message}`).join('\n')}`;
}
