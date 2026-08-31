/**
 * The op layer: pure functions mapping edit intents to JsonKind op sequences.
 *
 * Wire constraints this module encodes (all verified against a live replica):
 * - String Insert carries EXACTLY one character; multi-char content is a 400.
 *   Every string mutation is therefore a sequence of single-char ops.
 * - Creating an element in an array uses the insert-then-update idiom: the
 *   Array.Insert must carry a real payload (the first constructive op), and
 *   every following constructive op is delivered via Array.Update at the same
 *   position.
 * - An empty string can only be materialized in a fresh slot via the
 *   placeholder trick: Insert " " at 0, then Delete at 0.
 * - Object.Remove resets a key to its type default; it does not delete the key.
 * - Numbers only support relative Inc; setting a value means Inc by the delta.
 * - There is no move op: reorder = Delete + full re-create at the target index.
 *
 * Every function returns the ops in the exact order they must be POSTed.
 */

import type { JsonOp, Path, PlainJson } from '../api/types';

/* ---------- path wrapping ---------- */

/**
 * Wrap an op targeting a nested location so it can be posted at the root.
 * Object keys wrap as Object.Update, array indices as Array.Update.
 */
export function wrapPath(path: Path, op: JsonOp): JsonOp {
  let wrapped = op;
  for (let i = path.length - 1; i >= 0; i--) {
    const seg = path[i];
    if (typeof seg === 'number') {
      wrapped = { Array: { Update: { pos: seg, op: wrapped } } };
    } else {
      wrapped = { Object: { Update: [seg, wrapped] } };
    }
  }
  return wrapped;
}

/* ---------- strings ---------- */

/**
 * Minimal diff of a string edit into raw String ops (unwrapped):
 * common prefix/suffix are kept; the differing middle becomes at most one
 * DeleteRange plus one single-char Insert per inserted character.
 */
export function stringDiffOps(oldValue: string, newValue: string): JsonOp[] {
  if (oldValue === newValue) return [];

  let prefix = 0;
  const maxPrefix = Math.min(oldValue.length, newValue.length);
  while (prefix < maxPrefix && oldValue[prefix] === newValue[prefix]) prefix++;

  let suffix = 0;
  const maxSuffix = Math.min(oldValue.length, newValue.length) - prefix;
  while (
    suffix < maxSuffix &&
    oldValue[oldValue.length - 1 - suffix] === newValue[newValue.length - 1 - suffix]
  ) {
    suffix++;
  }

  const ops: JsonOp[] = [];
  const deleteLen = oldValue.length - prefix - suffix;
  if (deleteLen > 0) {
    ops.push({ String: { DeleteRange: { start: prefix, len: deleteLen } } });
  }
  const inserted = newValue.slice(prefix, newValue.length - suffix);
  for (let i = 0; i < inserted.length; i++) {
    ops.push({ String: { Insert: { content: inserted[i], pos: prefix + i } } });
  }
  return ops;
}

/** Edit the string at `path` from oldValue to newValue. */
export function setStringOps(path: Path, oldValue: string, newValue: string): JsonOp[] {
  return stringDiffOps(oldValue, newValue).map((op) => wrapPath(path, op));
}

/** Clear the string at `path` (current value must be supplied for its length). */
export function clearStringOps(path: Path, current: string): JsonOp[] {
  if (current.length === 0) return [];
  return [wrapPath(path, { String: { DeleteRange: { start: 0, len: current.length } } })];
}

/* ---------- numbers / booleans ---------- */

/** Set the number at `path` to `target`, given its current value. */
export function setNumberOps(path: Path, current: number, target: number): JsonOp[] {
  const delta = target - current;
  if (delta === 0) return [];
  return [wrapPath(path, { Number: { Inc: delta } })];
}

/** Set the boolean at `path`. */
export function setBooleanOps(path: Path, value: boolean): JsonOp[] {
  return [wrapPath(path, { Boolean: value ? 'Enable' : 'Disable' })];
}

/* ---------- constructing values in fresh slots ---------- */

/**
 * The constructive op sequence that builds `value` when applied to a fresh
 * (unset) slot, unwrapped. The first op of the sequence determines the slot's
 * type; the ops must be applied in order at the same location.
 *
 * Empty strings use the placeholder trick. An empty object or empty array
 * produces NO ops (there is nothing to carry the type); callers that need a
 * first payload (Array.Insert) must guarantee non-emptiness — our instance
 * convention does, via the mandatory eClass field.
 */
export function buildValueOps(value: PlainJson): JsonOp[] {
  if (value === null) return [];
  if (typeof value === 'string') {
    if (value.length === 0) {
      // Placeholder trick: a slot only exists once it has content.
      return [
        { String: { Insert: { content: ' ', pos: 0 } } },
        { String: { Delete: { pos: 0 } } },
      ];
    }
    const ops: JsonOp[] = [];
    for (let i = 0; i < value.length; i++) {
      ops.push({ String: { Insert: { content: value[i], pos: i } } });
    }
    return ops;
  }
  if (typeof value === 'number') {
    return [{ Number: { Inc: value } }];
  }
  if (typeof value === 'boolean') {
    return [{ Boolean: value ? 'Enable' : 'Disable' }];
  }
  if (Array.isArray(value)) {
    const ops: JsonOp[] = [];
    value.forEach((element, index) => {
      ops.push(...insertIntoArrayOps([], index, element));
    });
    return ops;
  }
  // Object: eClass first (presence marker and polymorphism tag), then the rest
  // in insertion order, each child sequence wrapped under its key.
  const keys = Object.keys(value);
  keys.sort((a, b) => (a === 'eClass' ? -1 : b === 'eClass' ? 1 : 0));
  const ops: JsonOp[] = [];
  for (const key of keys) {
    for (const op of buildValueOps(value[key])) {
      ops.push({ Object: { Update: [key, op] } });
    }
  }
  return ops;
}

/**
 * Insert `value` at `pos` of the array at `arrayPath` (insert-then-update
 * idiom): the first constructive op rides the Array.Insert, the rest are
 * Array.Updates at the same position. Throws if `value` produces no
 * constructive op (nothing to carry the Insert payload).
 */
export function insertIntoArrayOps(arrayPath: Path, pos: number, value: PlainJson): JsonOp[] {
  const inner = buildValueOps(value);
  if (inner.length === 0) {
    throw new Error(
      'cannot insert a contentless value into an array: the Insert op needs a real payload',
    );
  }
  const ops: JsonOp[] = [wrapPath(arrayPath, { Array: { Insert: { pos, op: inner[0] } } })];
  for (let i = 1; i < inner.length; i++) {
    ops.push(wrapPath(arrayPath, { Array: { Update: { pos, op: inner[i] } } }));
  }
  return ops;
}

/* ---------- model-level intents ---------- */

/**
 * Create the root instance of `className` (fresh node, state "Unset"): the
 * first op makes the union adopt the Object variant.
 */
export function createRootOps(className: string): JsonOp[] {
  return setStringOps(['eClass'], '', className);
}

/**
 * Append a new instance of concrete class `className` to the many-containment
 * array at `arrayPath` holding `currentLength` elements.
 */
export function addChildOps(arrayPath: Path, currentLength: number, className: string): JsonOp[] {
  return insertIntoArrayOps(arrayPath, currentLength, { eClass: className });
}

/**
 * Create an instance of `className` in the single containment `feature` of the
 * object at `parentPath`.
 */
export function createSingleContainmentOps(
  parentPath: Path,
  feature: string,
  className: string,
): JsonOp[] {
  return setStringOps([...parentPath, feature, 'eClass'], '', className);
}

/** Remove the element at `pos` from the array at `arrayPath`. */
export function removeFromArrayOps(arrayPath: Path, pos: number): JsonOp[] {
  return [wrapPath(arrayPath, { Array: { Delete: { pos } } })];
}

/**
 * Unset the key `feature` of the object at `parentPath`.
 * CAVEAT (verified): this resets the value to its type default (empty string /
 * 0 / false / empty array) and the key remains in the serialized state; an
 * object slot counts as absent only when its eClass string is empty.
 */
export function unsetFeatureOps(parentPath: Path, feature: string): JsonOp[] {
  return [wrapPath(parentPath, { Object: { Remove: feature } })];
}

/**
 * Move the element at `from` to index `to` in the array at `arrayPath`.
 * No move op exists on the wire: this is Delete + full re-create of the
 * element's current subtree at the target index. Expensive; acceptable v1.
 * `element` must be the element's decoded value at the time of the move.
 */
export function reorderArrayOps(
  arrayPath: Path,
  from: number,
  to: number,
  element: PlainJson,
): JsonOp[] {
  if (from === to) return [];
  return [
    ...removeFromArrayOps(arrayPath, from),
    ...insertIntoArrayOps(arrayPath, to, element),
  ];
}

/** Add the string `id` to the many-reference array at `arrayPath`. */
export function addManyReferenceOps(arrayPath: Path, currentLength: number, id: string): JsonOp[] {
  return insertIntoArrayOps(arrayPath, currentLength, id);
}
