/**
 * The encoder: one edit intent, the document it was computed against, and the
 * slot table, into the operations an interpreted replica routes.
 *
 * An intent (crdt/ops.ts) says what the user did in the vocabulary of the
 * document — "set /behaviortrees/0/ID to `guard`", "insert a `Sequence` at
 * position 2 of /behaviortrees/0/child". An operation says the same thing in
 * the vocabulary of the table: a path of `Variant` and `Field` steps by slot,
 * a collection step, and one write at the end. Everything that turns the
 * first into the second is here, and nothing else in the editor builds an
 * operation.
 *
 * Why the document is an argument. `Variant` names the concrete class of the
 * object in a containment on EVERY operation that reaches through the slot,
 * not only on the one that created it, so encoding a path needs the class of
 * every object along it — and the only place the editor knows that is the
 * document the node served. A path that reaches through an object the
 * document does not carry, or that carries no `eClass`, is refused here
 * rather than encoded against a guess.
 *
 * Why a batch is encoded together. One edit intent can be several operations,
 * and a re-created subtree (a reorder) is a mint followed by writes that
 * address the object the mint made. `encodeBatch` therefore advances a
 * shadow document as it goes, so the second intent of a batch is encoded
 * against what the first one will have built.
 *
 * Every refusal is a throw naming the feature or the class. That is the
 * point: an operation addressing the wrong slot is well-formed and the node
 * applies it, so a case this module cannot encode must stop here loudly
 * rather than reach the wire.
 */

import {
  MODEL_HEADER_KEY,
  type InstanceOp,
  type LeafDesc,
  type LeafOp,
  type ModelOp,
  type Path,
  type PlainJson,
  type Scalar,
} from '../api/types';
import type { EditOp } from './ops';
import { getAtPath, setAtPath } from './path';
import { classOf, enumLiteral, featureOf, SlotError, type ClassSlot, type FeatureSlot, type SlotTable } from './table';

/** Why a header write is refused, said once: by the builders in crdt/ops.ts and by this encoder. */
export const MODEL_HEADER_REFUSAL = `${MODEL_HEADER_KEY} is the model header, written once by the node that created the model; the editor never targets it`;

const wrapVariant = (cls: ClassSlot, op: InstanceOp): InstanceOp => ({ Variant: [cls.slot, op] });
const wrapField = (feature: FeatureSlot, op: InstanceOp): InstanceOp => ({ Field: [feature.slot, op] });

/** The class of the object at `node`, or a refusal saying where the document had none. */
function eClassOf(node: PlainJson, where: string): string {
  if (node === null || typeof node !== 'object' || Array.isArray(node)) {
    throw new SlotError(
      `${where} holds no object in the document the operation was computed against, so there is no class to name it by`,
    );
  }
  const eClass = node['eClass'];
  if (typeof eClass !== 'string' || eClass === '') {
    throw new SlotError(
      `${where} carries no \`eClass\` in the document, and every operation reaching through a containment names the class of the object in it`,
    );
  }
  return eClass;
}

/** The IEEE-754 bit pattern of a double, which is what `Scalar::Float` carries. */
function floatBits(value: number): bigint {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, value);
  return view.getBigUint64(0);
}

/** A JSON value as the scalar the leaf's rule expects. */
function scalarOf(value: PlainJson, leaf: LeafDesc, table: SlotTable, where: string): Scalar {
  if (value === null) return 'Null';
  if (leaf.kind === 'enum') {
    const enumName = leaf.class;
    if (typeof value !== 'string' || enumName === undefined) {
      throw new SlotError(`${where}: \`${JSON.stringify(value)}\` is not a literal of an enum leaf`);
    }
    return { Enum: enumLiteral(table, enumName, value) };
  }
  if (typeof value === 'boolean') return { Bool: value };
  if (typeof value === 'string') return { Str: value };
  if (typeof value === 'number') {
    // A counter's width is declared; a register's is not, so an integral
    // value is an integer and anything else is a double.
    const float = leaf.kind === 'counter' ? leaf.num === 'f32' || leaf.num === 'f64' : !Number.isInteger(value);
    return float ? { Float: floatBits(value) } : { Int: value };
  }
  throw new SlotError(`${where}: \`${JSON.stringify(value)}\` is not a value a leaf can hold`);
}

/**
 * The minimal character-level diff of a text edit: the common prefix and
 * suffix are kept, and the differing middle becomes at most one
 * `DeleteRange` plus one `InsertChar` per inserted character.
 *
 * A text leaf is an `EventGraph<List<char>>` and takes one character at a
 * time, which is why a rename is a sequence of operations and not a write.
 */
export function textDiff(from: string, to: string): LeafOp[] {
  if (from === to) return [];
  let prefix = 0;
  const maxPrefix = Math.min(from.length, to.length);
  while (prefix < maxPrefix && from[prefix] === to[prefix]) prefix++;
  let suffix = 0;
  const maxSuffix = Math.min(from.length, to.length) - prefix;
  while (suffix < maxSuffix && from[from.length - 1 - suffix] === to[to.length - 1 - suffix]) suffix++;

  const ops: LeafOp[] = [];
  const deleteLen = from.length - prefix - suffix;
  if (deleteLen > 0) ops.push({ DeleteRange: { start: prefix, len: deleteLen } });
  const inserted = to.slice(prefix, to.length - suffix);
  for (let i = 0; i < inserted.length; i++) {
    ops.push({ InsertChar: { pos: prefix + i, ch: inserted[i] } });
  }
  return ops;
}

/** The leaf writes that take the leaf at `where` from `from` to `to`. */
function writeOps(
  leaf: LeafDesc,
  from: PlainJson,
  to: PlainJson,
  table: SlotTable,
  where: string,
): LeafOp[] {
  switch (leaf.kind) {
    case 'text':
      return textDiff(typeof from === 'string' ? from : '', typeof to === 'string' ? to : '');
    case 'flag':
      return [to === true ? 'Enable' : 'Disable'];
    case 'counter': {
      // A counter is written by how much it moves, never by what it becomes.
      const before = typeof from === 'number' ? from : 0;
      const after = typeof to === 'number' ? to : 0;
      const delta = after - before;
      if (delta === 0) return [];
      const by = scalarOf(Math.abs(delta), leaf, table, where);
      return [delta > 0 ? { Inc: by } : { Dec: by }];
    }
    case 'register':
    case 'enum':
      return [{ Write: scalarOf(to, leaf, table, where) }];
  }
}

/** The writes that mint a leaf holding `value` in a collection slot that has just been made. */
function mintOps(leaf: LeafDesc, value: PlainJson, table: SlotTable, where: string): LeafOp[] {
  if (leaf.kind === 'text' && (value === '' || value === null)) {
    // A text leaf with nothing in it is still a leaf: a sequence element has
    // to carry an operation, so it is made with a character and emptied.
    return [{ InsertChar: { pos: 0, ch: ' ' } }, { DeleteChar: { pos: 0 } }];
  }
  if (leaf.kind === 'counter' && (value === 0 || value === null)) {
    return [{ Inc: scalarOf(0, leaf, table, where) }];
  }
  return writeOps(leaf, null, value, table, where);
}

/** Where the path being encoded landed: the object holding the last feature, and the wrapper down to it. */
interface Landing {
  /** The steps from the model root down to and including the `Variant` of the object holding `feature`. */
  wrap: (op: InstanceOp) => InstanceOp;
  /** The feature the last named segment of the path resolves to. */
  feature: FeatureSlot;
  /** The trailing index, when the path addresses one element of a collection. */
  index: number | null;
  /** The value of the feature in the document, for the intents that read it. */
  value: PlainJson | undefined;
  /** The path as text, for the refusals. */
  where: string;
}

/**
 * Walk `path` from the model root, naming the class of every object on the
 * way, and stop at the feature the last named segment resolves to.
 */
function land(table: SlotTable, doc: PlainJson, path: Path): Landing {
  const at = (upTo: number) => `/${path.slice(0, upTo).join('/')}`;
  const rootClass = classOf(table, eClassOf(doc, 'the model root'));
  let cls = rootClass;
  let node: PlainJson = doc;
  let wrap = (op: InstanceOp): InstanceOp => wrapVariant(rootClass, op);
  let i = 0;

  for (;;) {
    const segment = path[i];
    if (typeof segment !== 'string') {
      throw new SlotError(`${at(i + 1)}: a position with no feature before it is not a path an operation can take`);
    }
    const feature = featureOf(cls, segment);
    const where = `${cls.name}.${feature.name}`;
    const value = node === null || typeof node !== 'object' || Array.isArray(node) ? undefined : node[segment];
    const next = path[i + 1];
    const remaining = path.length - i - 1;
    if (remaining === 0) return { wrap, feature, index: null, value, where: at(i + 1) };
    if (remaining === 1 && typeof next === 'number') {
      return { wrap, feature, index: next, value, where: at(i + 2) };
    }

    // Another object step: the feature must hold objects, and the document
    // must say which class the one being reached through is.
    if (feature.site.site !== 'object') {
      throw new SlotError(
        `\`${where}\` holds values and not objects, so \`${at(i + 2)}\` is not a path an operation can take`,
      );
    }
    const outer = wrap;
    let child: PlainJson | undefined;
    let step: (op: InstanceOp) => InstanceOp;
    switch (feature.collection) {
      case 'bare': {
        child = value;
        step = (op) => outer(wrapField(feature, op));
        i += 1;
        break;
      }
      case 'optional': {
        child = value;
        step = (op) => outer(wrapField(feature, { Opt: { Set: op } }));
        i += 1;
        break;
      }
      case 'sequence': {
        if (typeof next !== 'number') {
          throw new SlotError(
            `\`${where}\` is a sequence and \`${at(i + 2)}\` addresses it by name; a sequence is addressed by position`,
          );
        }
        child = Array.isArray(value) ? value[next] : undefined;
        step = (op) => outer(wrapField(feature, { Seq: { Update: { pos: next, op } } }));
        i += 2;
        break;
      }
      case 'keyed':
        throw new SlotError(
          `\`${where}\` is a keyed collection, and this editor does not yet encode operations that address one`,
        );
    }
    const childClass = classOf(table, eClassOf(child ?? null, at(i)));
    if (childClass.transparent !== null) {
      throw new SlotError(
        `\`${where}\` holds a \`${childClass.name}\`, a transparent class the read-out renders as one of its ` +
          `features; this editor does not yet encode operations that reach through one`,
      );
    }
    cls = childClass;
    node = child ?? null;
    wrap = (op) => step(wrapVariant(childClass, op));
  }
}

/** `Variant(class, New)`: the shape every object mint has, wherever it sits. */
function mintObject(table: SlotTable, className: string): InstanceOp {
  return wrapVariant(classOf(table, className), 'New');
}

/** The class name an intent's value carries, or a refusal: an object is minted under a class and never anonymously. */
function classNameOf(value: PlainJson, where: string): string {
  if (value === null || typeof value !== 'object' || Array.isArray(value) || typeof value['eClass'] !== 'string') {
    throw new SlotError(`${where}: an object is inserted under a class, and \`${JSON.stringify(value)}\` names none`);
  }
  return value['eClass'];
}

/** One intent, encoded against `doc`. Throws (`SlotError`) on anything it cannot address. */
export function encodeEdit(table: SlotTable, doc: PlainJson, edit: EditOp): ModelOp[] {
  if (edit.path[0] === MODEL_HEADER_KEY) throw new SlotError(MODEL_HEADER_REFUSAL);

  // The root: the one place an object is minted with no feature step before it.
  if (edit.path.length === 0) {
    if (edit.kind !== 'mint') {
      throw new SlotError(`the model root takes no \`${edit.kind}\`; it is created, and then its features are edited`);
    }
    return [{ Instance: mintObject(table, edit.className) }];
  }

  const { wrap, feature, index, value, where } = land(table, doc, edit.path);
  const site = feature.site;
  const instance = (op: InstanceOp): ModelOp => ({ Instance: wrap(wrapField(feature, op)) });
  const leafOf = (): LeafDesc => {
    if (site.site === 'object') {
      throw new SlotError(`\`${where}\` holds objects, and this intent writes a value`);
    }
    return site.leaf;
  };

  switch (edit.kind) {
    case 'mint': {
      if (site.site !== 'object') {
        throw new SlotError(`\`${where}\` holds values and not objects, so nothing is minted in it`);
      }
      const made = mintObject(table, edit.className);
      switch (feature.collection) {
        case 'bare':
          return [instance(made)];
        case 'optional':
          return [instance({ Opt: { Set: made } })];
        case 'sequence':
          throw new SlotError(`\`${where}\` is a sequence: an object goes into it at a position, not on its own`);
        case 'keyed':
          throw new SlotError(`\`${where}\` is a keyed collection, which this editor does not yet address`);
      }
      break;
    }
    case 'set': {
      const leaf = leafOf();
      const writes = writeOps(leaf, edit.from, edit.to, table, where);
      if (site.site !== 'scalar') {
        throw new SlotError(
          `\`${where}\` is a ${site.site} of values, addressed by value: it takes an insert or a remove, not a write`,
        );
      }
      switch (feature.collection) {
        case 'bare':
          if (index !== null) {
            throw new SlotError(`\`${where}\` holds one value, so \`${where}/${index}\` addresses nothing`);
          }
          return writes.map((op) => instance({ Leaf: op }));
        case 'optional':
          return writes.map((op) => instance({ Opt: { Set: { Leaf: op } } }));
        case 'sequence':
          if (index === null) {
            throw new SlotError(`\`${where}\` is a sequence, and a write into it names the position it goes to`);
          }
          return writes.map((op) => instance({ Seq: { Update: { pos: index, op: { Leaf: op } } } }));
        case 'keyed':
          throw new SlotError(`\`${where}\` is a keyed collection, which this editor does not yet address`);
      }
      break;
    }
    case 'clear': {
      const leaf = leafOf();
      if (site.site === 'set' || site.site === 'bag') return [instance({ Leaf: 'Clear' })];
      if (leaf.kind === 'text') {
        const current = typeof edit.from === 'string' ? edit.from : '';
        if (current.length === 0) return [];
        return [instance({ Leaf: { DeleteRange: { start: 0, len: current.length } } })];
      }
      return [instance({ Leaf: { Write: 'Null' } })];
    }
    case 'unset': {
      if (feature.collection === 'optional') return [instance({ Opt: 'Unset' })];
      throw new SlotError(
        `\`${where}\` is ${site.site === 'object' ? 'a single-valued containment' : 'single-valued'} and always ` +
          `present: the interpreted node has no operation that empties one, so the editor does not offer it`,
      );
    }
    case 'insert': {
      if (site.site === 'set' || site.site === 'bag') {
        return [instance({ Leaf: { Add: scalarOf(edit.value, site.leaf, table, where) } })];
      }
      if (feature.collection !== 'sequence') {
        throw new SlotError(`\`${where}\` is not a collection an insert addresses`);
      }
      if (site.site === 'object') {
        return [instance({ Seq: { Insert: { pos: edit.pos, op: mintObject(table, classNameOf(edit.value, where)) } } })];
      }
      // A sequence of values: the element is made by the operation the
      // insert carries, and any further write lands on it in place.
      const writes = mintOps(site.leaf, edit.value, table, where);
      if (writes.length === 0) {
        throw new SlotError(`\`${where}\`: \`${JSON.stringify(edit.value)}\` builds no operation to make an element with`);
      }
      return writes.map((op, at) =>
        at === 0
          ? instance({ Seq: { Insert: { pos: edit.pos, op: { Leaf: op } } } })
          : instance({ Seq: { Update: { pos: edit.pos, op: { Leaf: op } } } }),
      );
    }
    case 'remove': {
      if (site.site === 'set' || site.site === 'bag') {
        const held = Array.isArray(value) ? value[edit.pos] : undefined;
        if (held === undefined) {
          throw new SlotError(
            `\`${where}\` is a ${site.site}, removed from by value, and the document holds no value at position ${edit.pos}`,
          );
        }
        return [instance({ Leaf: { Remove: scalarOf(held, site.leaf, table, where) } })];
      }
      if (feature.collection !== 'sequence') {
        throw new SlotError(`\`${where}\` is not a collection a remove by position addresses`);
      }
      return [instance({ Seq: { Delete: { pos: edit.pos } } })];
    }
  }
  throw new SlotError(`\`${where}\`: no operation for this intent`);
}

/* ---------- the shadow document a batch is encoded against ---------- */

/** The array at `path`, as the document has it. */
function arrayAt(doc: PlainJson, path: Path): PlainJson[] {
  const held = getAtPath(doc, path);
  return Array.isArray(held) ? held : [];
}

/**
 * What `doc` looks like once `edit` has been applied, to the extent the
 * encoder reads it: the classes of the objects on a path, and the contents of
 * the collections a position addresses. Not a merge and not a prediction of
 * what the node will hold — only enough for the next intent of the same batch
 * to be encoded against what this one builds.
 */
export function previewEdit(doc: PlainJson, edit: EditOp): PlainJson {
  switch (edit.kind) {
    case 'mint':
      return edit.path.length === 0
        ? { eClass: edit.className }
        : setAtPath(doc, edit.path, { eClass: edit.className });
    case 'set':
      return setAtPath(doc, edit.path, edit.to);
    case 'clear':
      return setAtPath(doc, edit.path, typeof edit.from === 'string' ? '' : null);
    case 'unset':
      return setAtPath(doc, edit.path, null);
    case 'insert': {
      const held = arrayAt(doc, edit.path);
      return setAtPath(doc, edit.path, [...held.slice(0, edit.pos), edit.value, ...held.slice(edit.pos)]);
    }
    case 'remove': {
      const held = arrayAt(doc, edit.path);
      return setAtPath(doc, edit.path, held.filter((_, at) => at !== edit.pos));
    }
  }
}

/**
 * A whole batch: every intent encoded against the document the one before it
 * leaves, so a mint and the writes into what it made travel together.
 */
export function encodeBatch(table: SlotTable, doc: PlainJson, edits: EditOp[]): ModelOp[] {
  let current = doc;
  const ops: ModelOp[] = [];
  for (const edit of edits) {
    ops.push(...encodeEdit(table, current, edit));
    current = previewEdit(current, edit);
  }
  return ops;
}
