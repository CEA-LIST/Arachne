import { describe, expect, it } from 'vitest';
import {
  addChildOps,
  addManyReferenceOps,
  buildValueOps,
  clearStringOps,
  createRootOps,
  createSingleContainmentOps,
  insertIntoArrayOps,
  removeFromArrayOps,
  reorderArrayOps,
  setBooleanOps,
  setNumberOps,
  setStringOps,
  stringDiffOps,
  unsetFeatureOps,
  wrapPath,
} from './ops';

describe('wrapPath', () => {
  it('wraps object keys as Object.Update and indices as Array.Update', () => {
    const op = wrapPath(['children', 2, 'name'], { Boolean: 'Enable' });
    expect(op).toEqual({
      Object: {
        Update: [
          'children',
          {
            Array: {
              Update: { pos: 2, op: { Object: { Update: ['name', { Boolean: 'Enable' }] } } },
            },
          },
        ],
      },
    });
  });

  it('returns the op unchanged for the empty path', () => {
    expect(wrapPath([], { Number: { Inc: 1 } })).toEqual({ Number: { Inc: 1 } });
  });
});

describe('stringDiffOps', () => {
  it('returns nothing for equal strings', () => {
    expect(stringDiffOps('abc', 'abc')).toEqual([]);
  });

  it('emits one single-char Insert per appended character', () => {
    expect(stringDiffOps('ab', 'abcd')).toEqual([
      { String: { Insert: { content: 'c', pos: 2 } } },
      { String: { Insert: { content: 'd', pos: 3 } } },
    ]);
  });

  it('emits a DeleteRange for a pure deletion in the middle', () => {
    expect(stringDiffOps('hello world', 'held')).toEqual([
      { String: { DeleteRange: { start: 3, len: 7 } } },
    ]);
  });

  it('combines DeleteRange with Inserts for a replacement', () => {
    expect(stringDiffOps('cat', 'cut')).toEqual([
      { String: { DeleteRange: { start: 1, len: 1 } } },
      { String: { Insert: { content: 'u', pos: 1 } } },
    ]);
  });

  it('builds a string from empty with per-char Inserts', () => {
    expect(stringDiffOps('', 'hi')).toEqual([
      { String: { Insert: { content: 'h', pos: 0 } } },
      { String: { Insert: { content: 'i', pos: 1 } } },
    ]);
  });

  it('deletes everything when the new value is empty', () => {
    expect(stringDiffOps('abc', '')).toEqual([
      { String: { DeleteRange: { start: 0, len: 3 } } },
    ]);
  });

  it('does not let prefix and suffix overlap (aa -> aaa)', () => {
    expect(stringDiffOps('aa', 'aaa')).toEqual([
      { String: { Insert: { content: 'a', pos: 2 } } },
    ]);
  });

  it('every Insert carries exactly one character', () => {
    for (const op of stringDiffOps('x', 'a longer replacement')) {
      if ('String' in op && 'Insert' in op.String) {
        expect(op.String.Insert.content).toHaveLength(1);
      }
    }
  });
});

describe('setStringOps / clearStringOps', () => {
  it('wraps every diff op in the path', () => {
    expect(setStringOps(['name'], 'a', 'ab')).toEqual([
      { Object: { Update: ['name', { String: { Insert: { content: 'b', pos: 1 } } }] } },
    ]);
  });

  it('clears via a single full-length DeleteRange', () => {
    expect(clearStringOps(['name'], 'abc')).toEqual([
      { Object: { Update: ['name', { String: { DeleteRange: { start: 0, len: 3 } } }] } },
    ]);
    expect(clearStringOps(['name'], '')).toEqual([]);
  });
});

describe('setNumberOps', () => {
  it('emits a single relative Inc on commit', () => {
    expect(setNumberOps(['age'], 30, 42)).toEqual([
      { Object: { Update: ['age', { Number: { Inc: 12 } }] } },
    ]);
  });

  it('handles floats and negative deltas', () => {
    expect(setNumberOps(['t'], 30.5, 0)).toEqual([
      { Object: { Update: ['t', { Number: { Inc: -30.5 } }] } },
    ]);
  });

  it('emits nothing when the value is unchanged', () => {
    expect(setNumberOps(['age'], 5, 5)).toEqual([]);
  });
});

describe('setBooleanOps', () => {
  it('maps true/false to Enable/Disable', () => {
    expect(setBooleanOps(['on'], true)).toEqual([
      { Object: { Update: ['on', { Boolean: 'Enable' }] } },
    ]);
    expect(setBooleanOps(['on'], false)).toEqual([
      { Object: { Update: ['on', { Boolean: 'Disable' }] } },
    ]);
  });
});

describe('createRootOps', () => {
  it('creates the root instance one eClass char at a time', () => {
    expect(createRootOps('Root')).toEqual([
      { Object: { Update: ['eClass', { String: { Insert: { content: 'R', pos: 0 } } }] } },
      { Object: { Update: ['eClass', { String: { Insert: { content: 'o', pos: 1 } } }] } },
      { Object: { Update: ['eClass', { String: { Insert: { content: 'o', pos: 2 } } }] } },
      { Object: { Update: ['eClass', { String: { Insert: { content: 't', pos: 3 } } }] } },
    ]);
  });
});

describe('addChildOps (insert-then-update idiom)', () => {
  it('first op is an Array.Insert carrying a real one-char payload, rest are Array.Updates', () => {
    const ops = addChildOps(['children'], 2, 'Seq');
    expect(ops).toEqual([
      {
        Object: {
          Update: [
            'children',
            {
              Array: {
                Insert: {
                  pos: 2,
                  op: { Object: { Update: ['eClass', { String: { Insert: { content: 'S', pos: 0 } } }] } },
                },
              },
            },
          ],
        },
      },
      {
        Object: {
          Update: [
            'children',
            {
              Array: {
                Update: {
                  pos: 2,
                  op: { Object: { Update: ['eClass', { String: { Insert: { content: 'e', pos: 1 } } }] } },
                },
              },
            },
          ],
        },
      },
      {
        Object: {
          Update: [
            'children',
            {
              Array: {
                Update: {
                  pos: 2,
                  op: { Object: { Update: ['eClass', { String: { Insert: { content: 'q', pos: 2 } } }] } },
                },
              },
            },
          ],
        },
      },
    ]);
  });
});

describe('createSingleContainmentOps', () => {
  it('builds the child object under the feature key char by char', () => {
    expect(createSingleContainmentOps([], 'value', 'Ab')).toEqual([
      {
        Object: {
          Update: ['value', { Object: { Update: ['eClass', { String: { Insert: { content: 'A', pos: 0 } } }] } }],
        },
      },
      {
        Object: {
          Update: ['value', { Object: { Update: ['eClass', { String: { Insert: { content: 'b', pos: 1 } } }] } }],
        },
      },
    ]);
  });
});

describe('removeFromArrayOps / unsetFeatureOps', () => {
  it('removes an array element with Array.Delete', () => {
    expect(removeFromArrayOps(['children'], 1)).toEqual([
      { Object: { Update: ['children', { Array: { Delete: { pos: 1 } } }] } },
    ]);
  });

  it('unsets an object feature with Object.Remove', () => {
    expect(unsetFeatureOps(['a', 0], 'entry')).toEqual([
      { Object: { Update: ['a', { Array: { Update: { pos: 0, op: { Object: { Remove: 'entry' } } } } }] } },
    ]);
  });
});

describe('buildValueOps', () => {
  it('materializes an empty string via the space-placeholder trick', () => {
    expect(buildValueOps('')).toEqual([
      { String: { Insert: { content: ' ', pos: 0 } } },
      { String: { Delete: { pos: 0 } } },
    ]);
  });

  it('puts eClass first when building an object', () => {
    const ops = buildValueOps({ name: 'x', eClass: 'A' });
    expect(ops[0]).toEqual({
      Object: { Update: ['eClass', { String: { Insert: { content: 'A', pos: 0 } } }] },
    });
    expect(ops).toHaveLength(2);
    expect(ops[1]).toEqual({
      Object: { Update: ['name', { String: { Insert: { content: 'x', pos: 0 } } }] },
    });
  });

  it('builds numbers and booleans as single ops', () => {
    expect(buildValueOps(2.5)).toEqual([{ Number: { Inc: 2.5 } }]);
    expect(buildValueOps(false)).toEqual([{ Boolean: 'Disable' }]);
  });

  it('builds nested arrays element by element with the insert-then-update idiom', () => {
    expect(buildValueOps(['ab'])).toEqual([
      { Array: { Insert: { pos: 0, op: { String: { Insert: { content: 'a', pos: 0 } } } } } },
      { Array: { Update: { pos: 0, op: { String: { Insert: { content: 'b', pos: 1 } } } } } },
    ]);
  });
});

describe('insertIntoArrayOps', () => {
  it('inserts an empty-string element via the placeholder trick', () => {
    expect(insertIntoArrayOps(['tags'], 0, '')).toEqual([
      { Object: { Update: ['tags', { Array: { Insert: { pos: 0, op: { String: { Insert: { content: ' ', pos: 0 } } } } } }] } },
      { Object: { Update: ['tags', { Array: { Update: { pos: 0, op: { String: { Delete: { pos: 0 } } } } } }] } },
    ]);
  });

  it('refuses a contentless value (nothing to carry the Insert payload)', () => {
    expect(() => insertIntoArrayOps([], 0, {})).toThrow(/payload/);
    expect(() => insertIntoArrayOps([], 0, null)).toThrow(/payload/);
  });
});

describe('reorderArrayOps', () => {
  it('is Delete plus full re-create at the target index', () => {
    const ops = reorderArrayOps(['xs'], 0, 1, { eClass: 'A' });
    expect(ops[0]).toEqual({ Object: { Update: ['xs', { Array: { Delete: { pos: 0 } } }] } });
    expect(ops[1]).toEqual({
      Object: {
        Update: [
          'xs',
          {
            Array: {
              Insert: {
                pos: 1,
                op: { Object: { Update: ['eClass', { String: { Insert: { content: 'A', pos: 0 } } }] } },
              },
            },
          },
        ],
      },
    });
    expect(reorderArrayOps(['xs'], 2, 2, 'x')).toEqual([]);
  });
});

describe('addManyReferenceOps', () => {
  it('adds an id string first char via Insert, rest via Update', () => {
    expect(addManyReferenceOps(['refs'], 1, 'n1')).toEqual([
      { Object: { Update: ['refs', { Array: { Insert: { pos: 1, op: { String: { Insert: { content: 'n', pos: 0 } } } } } }] } },
      { Object: { Update: ['refs', { Array: { Update: { pos: 1, op: { String: { Insert: { content: '1', pos: 1 } } } } } }] } },
    ]);
  });
});
