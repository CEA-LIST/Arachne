/**
 * The slot table: the descriptor read the way the interpreted node reads it.
 *
 * An interpreted replica routes an operation by position and never by name.
 * `Variant` carries a class's position among the classes sorted by name;
 * `Field` carries a feature's position among the class's VISIBLE features —
 * every feature an instance can see, own and inherited, each name once,
 * sorted by name. Enums are numbered separately, in their own sorted list.
 *
 * This module computes those positions from the descriptor the model is bound
 * to (`GET /api/model/{id}/metamodel`), matching moirai-semantics/src/parse.rs
 * step for step:
 *
 * - `class_names.sort_unstable()` over the `classes` object's keys, and
 *   `enum_names.sort_unstable()` over the `enums` object's keys, each dense
 *   from 0 (parse.rs, "Slots are dense positions by sorted name"). Sorted
 *   here rather than taken from the JSON object's key order, for the same
 *   reason parse.rs sorts rather than trusting its map.
 * - the supertype closure of every class, itself included, by walking
 *   `superTypes` (a cycle terminates on the visited guard).
 * - `visible`: the closure's features filled supertypes first and the class's
 *   own last, so a redeclared name resolves to the class's own entry, then
 *   read out in ascending name order. That ordering is a `BTreeMap<Arc<str>,
 *   …>` in parse.rs and a sort here; both are byte-wise over the name, which
 *   is what `sort_unstable` on `&str` and JavaScript's default string
 *   comparison agree on for the ASCII identifiers a `.ecore` file carries.
 *
 * Getting this wrong is silent: an operation addressing the wrong slot is a
 * well-formed operation and the node applies it. Every helper here therefore
 * refuses rather than guesses — an unknown class, an unknown feature and a
 * feature with no `merge` rule all throw, naming what was missing.
 *
 * The table is derived, never fetched: the node publishes the descriptor and
 * both sides compute the same positions from it.
 */

import type { ClassDesc, Descriptor, LeafDesc, MergeDesc, ShapeDesc } from '../api/types';

/** Why an operation could not be addressed. Thrown, never returned as a silent default. */
export class SlotError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'SlotError';
  }
}

/** The collection a feature's values sit in, as the node splits it (moirai-interp/src/node.rs `shaped`). */
export type Collection = 'bare' | 'optional' | 'sequence' | 'keyed';

/** What that collection holds. */
export type Site =
  /** An object of one of the target's concrete classes. */
  | { site: 'object'; target: string }
  /** One leaf value: a text, counter, flag, register or enum leaf. */
  | { site: 'scalar'; leaf: LeafDesc }
  /** A set of leaf values, addressed by value and never by position. */
  | { site: 'set'; leaf: LeafDesc }
  /** A bag of leaf values, addressed by value and never by position. */
  | { site: 'bag'; leaf: LeafDesc };

/** One feature as the operation path sees it: where it sits, and what sits in it. */
export interface FeatureSlot {
  /** The name the descriptor spells. */
  name: string;
  /** The position in the class's visible features: what `Field` carries. */
  slot: number;
  /** The class that declares it, for the error messages. */
  owner: string;
  collection: Collection;
  site: Site;
}

/** One class as the operation path sees it. */
export interface ClassSlot {
  name: string;
  /** The position among the classes sorted by name: what `Variant` carries. */
  slot: number;
  instantiable: boolean;
  /** Visible features, sorted by name; the index is the feature's slot. */
  visible: FeatureSlot[];
  byName: Map<string, FeatureSlot>;
  /** The visible slot of the one feature a transparent class is represented by, else null. */
  transparent: number | null;
}

export interface SlotTable {
  /** Every class, sorted by name; the index is the class slot. */
  classes: ClassSlot[];
  byName: Map<string, ClassSlot>;
  /** Every enum name, sorted; the index is the enum slot a `Scalar.Enum` carries. */
  enums: string[];
  enumSlot: Map<string, number>;
  enumLiterals: Map<string, string[]>;
  /** The classes a document root may be declared as. */
  rootClasses: string[];
}

/** The leaf of a non-containment reference: design §8 carries it as a string in a multi-value register. */
const REFERENCE_LEAF: LeafDesc = { kind: 'register', tie: 'mv' };

/**
 * Split one feature's rule into its collection and its site, exactly as
 * moirai-interp/src/node.rs `shaped` does.
 *
 * The two places this is more than a transcription of the shape word:
 * `Shape::orderedSet` degrades to a sequence (the generator cannot compile
 * unique-and-ordered and emits a list, so the interpreter merges as one), and
 * a multi-valued containment is a sequence whatever its facets say, so the
 * set and bag shapes are unreachable for one and a `single` is bare.
 */
function split(where: string, merge: MergeDesc): { collection: Collection; site: Site } {
  const shape: ShapeDesc['kind'] = merge.shape?.kind ?? 'single';
  const effective = shape === 'orderedSet' ? 'sequence' : shape;
  switch (merge.kind) {
    case 'attribute': {
      const leaf = merge.leaf;
      if (leaf === undefined) throw new SlotError(`${where}: an attribute rule with no \`leaf\``);
      switch (effective) {
        case 'single':
          return { collection: 'bare', site: { site: 'scalar', leaf } };
        case 'optional':
          return { collection: 'optional', site: { site: 'scalar', leaf } };
        case 'sequence':
          return { collection: 'sequence', site: { site: 'scalar', leaf } };
        case 'set':
          return { collection: 'bare', site: { site: 'set', leaf } };
        case 'bag':
          return { collection: 'bare', site: { site: 'bag', leaf } };
        case 'keyed':
          return { collection: 'keyed', site: { site: 'scalar', leaf } };
      }
      break;
    }
    case 'containment': {
      const target = merge.target;
      if (target === undefined) throw new SlotError(`${where}: a containment rule with no \`target\``);
      const site: Site = { site: 'object', target };
      if (effective === 'optional') return { collection: 'optional', site };
      if (effective === 'sequence') return { collection: 'sequence', site };
      if (effective === 'keyed') return { collection: 'keyed', site };
      return { collection: 'bare', site };
    }
    case 'reference':
      return merge.many === true
        ? { collection: 'bare', site: { site: 'set', leaf: REFERENCE_LEAF } }
        : { collection: 'bare', site: { site: 'scalar', leaf: REFERENCE_LEAF } };
    case 'unsupported':
      throw new SlotError(
        `${where} is \`${merge.reason ?? 'unsupported'}\`, which the interpreted node has no rule to run: ` +
          `no operation can address it`,
      );
  }
  throw new SlotError(`${where}: a merge rule of no known kind`);
}

/** The declared features of one class, whichever array carries them: one numbering, as parse.rs makes it. */
function declaredOf(cls: ClassDesc): { name: string; merge?: MergeDesc }[] {
  return [...cls.attributes, ...cls.containments, ...cls.references];
}

/**
 * Read a descriptor into a slot table. Throws (`SlotError`) on a descriptor
 * the interpreted node would itself refuse: another format version, a
 * supertype naming a class that is not listed, a feature with no `merge`.
 */
export function slotTable(descriptor: Descriptor): SlotTable {
  if (descriptor.formatVersion !== 2) {
    throw new SlotError(
      `the descriptor declares formatVersion ${String(descriptor.formatVersion)}; the interpreted node ` +
        `reads format version 2 only, because that is the version that carries the merge rule the slots are read from`,
    );
  }
  const classNames = Object.keys(descriptor.classes).sort();
  const enums = Object.keys(descriptor.enums ?? {}).sort();
  const enumSlot = new Map(enums.map((name, index) => [name, index] as const));
  const enumLiterals = new Map(enums.map((name) => [name, descriptor.enums[name]] as const));

  // The supertype closure of every class, itself included; a cycle stops on
  // the visited guard rather than looping, as parse.rs's insert guard does.
  const closure = (name: string): string[] => {
    const seen = new Set<string>();
    const pending = [name];
    while (pending.length > 0) {
      const current = pending.pop() as string;
      if (seen.has(current)) continue;
      seen.add(current);
      const cls = descriptor.classes[current];
      if (cls === undefined) {
        throw new SlotError(`\`${name}\` inherits \`${current}\`, which the descriptor does not list`);
      }
      pending.push(...cls.superTypes);
    }
    return [...seen];
  };

  const classes: ClassSlot[] = classNames.map((name, slot) => {
    const cls = descriptor.classes[name];
    // Supertypes first and the class's own declarations last, so a
    // redeclared name resolves to this class's entry.
    const owners = [...closure(name).filter((other) => other !== name).sort(), name];
    const byOwner = new Map<string, { name: string; merge?: MergeDesc; owner: string }>();
    for (const owner of owners) {
      for (const feature of declaredOf(descriptor.classes[owner])) {
        byOwner.set(feature.name, { ...feature, owner });
      }
    }
    const visible: FeatureSlot[] = [...byOwner.values()]
      .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
      .map((feature, index) => {
        const where = `${feature.owner}.${feature.name}`;
        if (feature.merge === undefined) {
          throw new SlotError(
            `\`${where}\`: no \`merge\` object; the slots are read from the merge rule formatVersion 2 ` +
              `carries on every feature, and this editor reads it rather than deriving it`,
          );
        }
        const { collection, site } = split(where, feature.merge);
        return { name: feature.name, slot: index, owner: feature.owner, collection, site };
      });
    const transparentName = cls.transparent ?? null;
    const transparent =
      transparentName === null ? null : visible.findIndex((f) => f.name === transparentName);
    if (transparent === -1) {
      throw new SlotError(
        `\`${name}\`: \`transparent\` names \`${String(transparentName)}\`, which the class cannot see`,
      );
    }
    return {
      name,
      slot,
      instantiable: !cls.abstract,
      visible,
      byName: new Map(visible.map((feature) => [feature.name, feature] as const)),
      transparent,
    };
  });

  return {
    classes,
    byName: new Map(classes.map((cls) => [cls.name, cls] as const)),
    enums,
    enumSlot,
    enumLiterals,
    // Every instantiable class when the descriptor names none, which is what
    // parse.rs does with an empty `rootClasses`.
    rootClasses:
      descriptor.rootClasses.length > 0
        ? descriptor.rootClasses
        : classes.filter((cls) => cls.instantiable).map((cls) => cls.name),
  };
}

/** The class of that name, or a refusal naming it. */
export function classOf(table: SlotTable, name: string): ClassSlot {
  const cls = table.byName.get(name);
  if (cls === undefined) {
    throw new SlotError(`\`${name}\` is not a class of this metamodel, so no class slot addresses it`);
  }
  return cls;
}

/** The visible feature of that name on that class, or a refusal naming both. */
export function featureOf(cls: ClassSlot, name: string): FeatureSlot {
  const feature = cls.byName.get(name);
  if (feature === undefined) {
    throw new SlotError(
      `\`${name}\` is not a feature \`${cls.name}\` can see, so no feature slot addresses it`,
    );
  }
  return feature;
}

/** The slot of an enum literal: [the enum's slot, the literal's position in declaration order]. */
export function enumLiteral(table: SlotTable, enumName: string, literal: string): [number, number] {
  const slot = table.enumSlot.get(enumName);
  const literals = table.enumLiterals.get(enumName);
  if (slot === undefined || literals === undefined) {
    throw new SlotError(`\`${enumName}\` is not an enum of this metamodel`);
  }
  const index = literals.indexOf(literal);
  if (index < 0) {
    throw new SlotError(`\`${literal}\` is not a literal of \`${enumName}\` (${literals.join(', ')})`);
  }
  return [slot, index];
}
