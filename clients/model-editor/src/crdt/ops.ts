/**
 * The intent layer: pure functions mapping what the user did to what the
 * editor means, in the vocabulary of the document.
 *
 * An intent is not an operation. It says "set /behaviortrees/0/ID to `guard`",
 * and crdt/encode.ts turns that into the `Variant`/`Field`/`Seq` path an
 * interpreted replica routes, against the slot table (crdt/table.ts) and the
 * document the intent was computed from. The split is deliberate: a control
 * knows the path it edits and nothing about slots, and the one place slots
 * are computed is the one place the descriptor is read.
 *
 * Why the encoding is not here. `Variant` names the concrete class of every
 * object on the path, and a text leaf takes one character at a time while a
 * register takes a whole write — both are decided by the merge rule of the
 * feature, which the descriptor carries and a control does not. So a control
 * says what it did, and the session encodes it when it sends it, against the
 * document the node last served.
 *
 * The root key `__model` is the model header: no builder here targets it, and
 * one asked to throws. It is not a feature of any class, so the encoder would
 * refuse it too; the refusal is said in both places because the sentence is
 * what the user sees.
 *
 * Every function returns the intents in the exact order they must be sent.
 */

import { MODEL_HEADER_KEY, type Path, type PlainJson } from '../api/types';
import { MODEL_HEADER_REFUSAL } from './encode';

export { MODEL_HEADER_REFUSAL };

/**
 * One edit intent, at the level of the document.
 *
 * `path` addresses a feature of an element, or one element of a collection
 * for the intents that write into a position. `from` is the value the intent
 * was computed against: a text leaf is written by the difference between two
 * strings and a counter by how far it moves, so the old value is part of the
 * intent and not something the encoder goes looking for.
 */
export type EditOp =
  /** Create an object: the model root (`path: []`), or the object in a single or optional containment. */
  | { kind: 'mint'; path: Path; className: string }
  /** Write one value-holding feature, or one element of a sequence of values. */
  | { kind: 'set'; path: Path; from: PlainJson; to: PlainJson }
  /** Empty one value-holding feature: a text back to nothing, a set to no members. */
  | { kind: 'clear'; path: Path; from: PlainJson }
  /** Take the object or value out of an optional feature. */
  | { kind: 'unset'; path: Path }
  /** Put a value or an object into a collection: at `pos` for a sequence, by value for a set. */
  | { kind: 'insert'; path: Path; pos: number; value: PlainJson }
  /** Take the element at `pos` out of a collection. */
  | { kind: 'remove'; path: Path; pos: number };

/* ---------- the header ---------- */

/** Whether an intent at `path` would land in the model header. */
export function targetsModelHeader(path: Path): boolean {
  return path[0] === MODEL_HEADER_KEY;
}

/** The same for a built intent: the funnel in sync/modelSession.ts sees intents and not paths. */
export function opTouchesModelHeader(op: EditOp): boolean {
  return targetsModelHeader(op.path);
}

function refuseHeaderTarget(path: Path): void {
  if (targetsModelHeader(path)) {
    throw new Error(MODEL_HEADER_REFUSAL);
  }
}

/** Every builder funnels through here, which is where the header is refused. */
function intents(...ops: EditOp[]): EditOp[] {
  for (const op of ops) refuseHeaderTarget(op.path);
  return ops;
}

/* ---------- values ---------- */

/** Edit the value at `path` from `oldValue` to `newValue`. */
export function setStringOps(path: Path, oldValue: string, newValue: string): EditOp[] {
  if (oldValue === newValue) return [];
  return intents({ kind: 'set', path, from: oldValue, to: newValue });
}

/** Empty the value at `path` (the current value is supplied, since a text is emptied by its length). */
export function clearStringOps(path: Path, current: string): EditOp[] {
  if (current.length === 0) return [];
  return intents({ kind: 'clear', path, from: current });
}

/** Set the number at `path` to `target`, given its current value. */
export function setNumberOps(path: Path, current: number, target: number): EditOp[] {
  if (current === target) return [];
  return intents({ kind: 'set', path, from: current, to: target });
}

/** Set the boolean at `path`. */
export function setBooleanOps(path: Path, value: boolean): EditOp[] {
  return intents({ kind: 'set', path, from: !value, to: value });
}

/* ---------- collections ---------- */

/**
 * Put `value` at `pos` of the collection at `arrayPath`: an object when it
 * carries an `eClass`, a value otherwise. A nested value (an object with
 * features already filled in) is built by `buildSubtreeOps`, which mints it
 * first and writes into it afterwards.
 */
export function insertIntoArrayOps(arrayPath: Path, pos: number, value: PlainJson): EditOp[] {
  return intents({ kind: 'insert', path: arrayPath, pos, value });
}

/** Remove the element at `pos` from the collection at `arrayPath`. */
export function removeFromArrayOps(arrayPath: Path, pos: number): EditOp[] {
  return intents({ kind: 'remove', path: arrayPath, pos });
}

/* ---------- model-level intents ---------- */

/** Create the root instance of `className` in a model whose root has not been written. */
export function createRootOps(className: string): EditOp[] {
  return intents({ kind: 'mint', path: [], className });
}

/** Append a new instance of concrete class `className` to the containment at `arrayPath`, which holds `currentLength` elements. */
export function addChildOps(arrayPath: Path, currentLength: number, className: string): EditOp[] {
  return insertIntoArrayOps(arrayPath, currentLength, { eClass: className });
}

/** Create an instance of `className` in the single or optional containment `feature` of the object at `parentPath`. */
export function createSingleContainmentOps(parentPath: Path, feature: string, className: string): EditOp[] {
  return intents({ kind: 'mint', path: [...parentPath, feature], className });
}

/**
 * Unset the feature `feature` of the object at `parentPath`.
 *
 * Only an optional feature can be emptied: the interpreted node has no
 * operation that takes the object out of a single-valued containment, and the
 * encoder refuses one naming the feature rather than sending something that
 * would be applied elsewhere.
 */
export function unsetFeatureOps(parentPath: Path, feature: string): EditOp[] {
  return intents({ kind: 'unset', path: [...parentPath, feature] });
}

/**
 * The intents that build `value` at a position that has just been made: the
 * object itself, then every feature it carries, depth first.
 *
 * `mint` and `insert` carry only the class, so a subtree is rebuilt rather
 * than copied. Used by a reorder, which has no move operation to lean on.
 */
export function buildSubtreeOps(basePath: Path, value: PlainJson): EditOp[] {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return [];
  const ops: EditOp[] = [];
  for (const [key, child] of Object.entries(value)) {
    if (key === 'eClass' || key === MODEL_HEADER_KEY) continue;
    const path = [...basePath, key];
    if (Array.isArray(child)) {
      child.forEach((element, index) => {
        ops.push({ kind: 'insert', path, pos: index, value: element });
        ops.push(...buildSubtreeOps([...path, index], element));
      });
    } else if (child !== null && typeof child === 'object') {
      const className = child['eClass'];
      if (typeof className !== 'string' || className === '') continue;
      ops.push({ kind: 'mint', path, className });
      ops.push(...buildSubtreeOps(path, child));
    } else if (child !== null) {
      ops.push({ kind: 'set', path, from: null, to: child });
    }
  }
  return ops;
}

/**
 * Move the element at `from` to index `to` in the sequence at `arrayPath`.
 *
 * There is no move operation on the wire: this is a delete and a full
 * re-creation of the element's subtree at the target index. Expensive, and
 * the price of a positional sequence. `element` must be the element's value
 * at the time of the move.
 */
export function reorderArrayOps(arrayPath: Path, from: number, to: number, element: PlainJson): EditOp[] {
  if (from === to) return [];
  const className = element !== null && typeof element === 'object' && !Array.isArray(element) ? element['eClass'] : null;
  if (typeof className !== 'string' || className === '') {
    throw new Error(`cannot re-create /${arrayPath.join('/')}[${from}]: the element carries no eClass`);
  }
  return intents(
    { kind: 'remove', path: arrayPath, pos: from },
    { kind: 'insert', path: arrayPath, pos: to, value: { eClass: className } },
    ...buildSubtreeOps([...arrayPath, to], element),
  );
}

/** Add the string `id` to the many-reference at `arrayPath`, which is a set of strings and not a sequence. */
export function addManyReferenceOps(arrayPath: Path, currentLength: number, id: string): EditOp[] {
  return insertIntoArrayOps(arrayPath, currentLength, id);
}
