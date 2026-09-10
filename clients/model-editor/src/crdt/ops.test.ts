/**
 * The intent layer: what each control says it did.
 *
 * These are claims about the vocabulary, not about the wire — an intent
 * carries no slot and no operation. What the intents become is
 * crdt/encode.test.ts, where the slots are checked against the ones a live
 * interpreted replica took.
 */

import { describe, expect, it } from 'vitest';
import {
  addChildOps,
  addManyReferenceOps,
  buildSubtreeOps,
  clearStringOps,
  createRootOps,
  createSingleContainmentOps,
  insertIntoArrayOps,
  MODEL_HEADER_REFUSAL,
  opTouchesModelHeader,
  removeFromArrayOps,
  reorderArrayOps,
  setBooleanOps,
  setNumberOps,
  setStringOps,
  targetsModelHeader,
  unsetFeatureOps,
} from './ops';

describe('a value intent carries the value it came from', () => {
  it('says nothing when nothing changed', () => {
    expect(setStringOps(['ID'], 'a', 'a')).toEqual([]);
    expect(setNumberOps(['n'], 2, 2)).toEqual([]);
    expect(clearStringOps(['ID'], '')).toEqual([]);
  });

  it('carries both ends of a text edit, because a text leaf is written by the difference', () => {
    expect(setStringOps(['behaviortrees', 0, 'ID'], 'main', 'guard')).toEqual([
      { kind: 'set', path: ['behaviortrees', 0, 'ID'], from: 'main', to: 'guard' },
    ]);
  });

  it('carries both ends of a number, because a counter is written by how far it moves', () => {
    expect(setNumberOps(['n'], 3, 7)).toEqual([{ kind: 'set', path: ['n'], from: 3, to: 7 }]);
  });

  it('says a boolean as the value it becomes', () => {
    expect(setBooleanOps(['on'], true)).toEqual([{ kind: 'set', path: ['on'], from: false, to: true }]);
  });

  it('empties a value with its current length in hand', () => {
    expect(clearStringOps(['ref'], 'Door')).toEqual([{ kind: 'clear', path: ['ref'], from: 'Door' }]);
  });
});

describe('a structural intent names the class, never a subtree', () => {
  it('creates a root, a single containment and a child by class name', () => {
    expect(createRootOps('Root')).toEqual([{ kind: 'mint', path: [], className: 'Root' }]);
    expect(createSingleContainmentOps(['behaviortrees', 0], 'child', 'Sequence')).toEqual([
      { kind: 'mint', path: ['behaviortrees', 0, 'child'], className: 'Sequence' },
    ]);
    expect(addChildOps(['behaviortrees'], 2, 'BehaviorTree')).toEqual([
      { kind: 'insert', path: ['behaviortrees'], pos: 2, value: { eClass: 'BehaviorTree' } },
    ]);
  });

  it('removes by position and unsets by feature', () => {
    expect(removeFromArrayOps(['behaviortrees'], 1)).toEqual([
      { kind: 'remove', path: ['behaviortrees'], pos: 1 },
    ]);
    expect(unsetFeatureOps(['behaviortrees', 0], 'child')).toEqual([
      { kind: 'unset', path: ['behaviortrees', 0, 'child'] },
    ]);
  });

  it('adds a reference and a plain value the same way, since a collection is a collection here', () => {
    expect(addManyReferenceOps(['refs'], 2, 'Door')).toEqual([
      { kind: 'insert', path: ['refs'], pos: 2, value: 'Door' },
    ]);
    expect(insertIntoArrayOps(['tags'], 0, '')).toEqual([{ kind: 'insert', path: ['tags'], pos: 0, value: '' }]);
  });
});

describe('a subtree is rebuilt, class first and depth first', () => {
  it('mints each object before writing into it', () => {
    expect(
      buildSubtreeOps(['behaviortrees', 0], {
        eClass: 'BehaviorTree',
        ID: 'main',
        child: { eClass: 'Sequence', ID: 'root', children: [{ eClass: 'OpenDoor', ID: 'a' }] },
      }),
    ).toEqual([
      { kind: 'set', path: ['behaviortrees', 0, 'ID'], from: null, to: 'main' },
      { kind: 'mint', path: ['behaviortrees', 0, 'child'], className: 'Sequence' },
      { kind: 'set', path: ['behaviortrees', 0, 'child', 'ID'], from: null, to: 'root' },
      { kind: 'insert', path: ['behaviortrees', 0, 'child', 'children'], pos: 0, value: { eClass: 'OpenDoor', ID: 'a' } },
      { kind: 'set', path: ['behaviortrees', 0, 'child', 'children', 0, 'ID'], from: null, to: 'a' },
    ]);
  });

  it('is what a reorder is made of: there is no move operation to lean on', () => {
    const element = { eClass: 'BehaviorTree', ID: 'b' };
    expect(reorderArrayOps(['behaviortrees'], 1, 0, element)).toEqual([
      { kind: 'remove', path: ['behaviortrees'], pos: 1 },
      { kind: 'insert', path: ['behaviortrees'], pos: 0, value: { eClass: 'BehaviorTree' } },
      { kind: 'set', path: ['behaviortrees', 0, 'ID'], from: null, to: 'b' },
    ]);
    expect(reorderArrayOps(['behaviortrees'], 1, 1, element)).toEqual([]);
  });

  it('refuses to re-create an element that names no class', () => {
    expect(() => reorderArrayOps(['tags'], 1, 0, 'a')).toThrow(/carries no eClass/);
  });
});

describe('the model header is never targeted', () => {
  const header = '__model';

  it('every path-taking builder refuses it before producing an intent', () => {
    expect(() => setStringOps([header, 'modelId'], '', 'x')).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => setNumberOps([header, 'n'], 0, 1)).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => setBooleanOps([header, 'b'], true)).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => clearStringOps([header, 'modelId'], 'x')).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => addChildOps([header], 0, 'Root')).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => removeFromArrayOps([header], 0)).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => createSingleContainmentOps([header], 'x', 'Root')).toThrow(MODEL_HEADER_REFUSAL);
    expect(() => unsetFeatureOps([], header)).toThrow(MODEL_HEADER_REFUSAL);
  });

  it('is about the root key only: a nested key of the same name is a feature like any other', () => {
    expect(targetsModelHeader([header])).toBe(true);
    expect(targetsModelHeader([header, 'modelId'])).toBe(true);
    expect(targetsModelHeader(['behaviortrees', 0, header])).toBe(false);
    expect(targetsModelHeader(['__models'])).toBe(false);
    expect(targetsModelHeader([])).toBe(false);
  });

  it('the funnel guard sees intents, and agrees with the builders', () => {
    expect(opTouchesModelHeader({ kind: 'unset', path: [header] })).toBe(true);
    expect(opTouchesModelHeader({ kind: 'mint', path: [], className: 'Root' })).toBe(false);
    for (const op of [
      ...setStringOps(['ID'], '', 'x'),
      ...addChildOps(['behaviortrees'], 0, 'BehaviorTree'),
      ...createRootOps('Root'),
    ]) {
      expect(opTouchesModelHeader(op)).toBe(false);
    }
  });
});
