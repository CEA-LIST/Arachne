import { describe, expect, it } from 'vitest';
import type { WireNode } from '../api/types';
import { decodeState } from './decode';
import { getAtPath, pathKey, setAtPath } from './path';

describe('decodeState', () => {
  it('decodes "Unset" (fresh node) to null', () => {
    expect(decodeState('Unset')).toBeNull();
  });

  it('decodes the wrapped shape the node serves', () => {
    const wire: WireNode = {
      Value: {
        Object: {
          eClass: { Value: { String: ['R', 'o', 'o', 't'] } },
          age: { Value: { Number: 30.5 } },
          on: { Value: { Boolean: true } },
          children: {
            Value: {
              Array: [{ Value: { Object: { eClass: { Value: { String: ['A'] } } } } }],
            },
          },
        },
      },
    };
    expect(decodeState(wire)).toEqual({
      eClass: 'Root',
      age: 30.5,
      on: true,
      children: [{ eClass: 'A' }],
    });
  });

  it('decodes empty strings and empty arrays', () => {
    expect(decodeState({ Value: { String: [] } })).toBe('');
    expect(decodeState({ Value: { Array: [] } })).toEqual([]);
  });

  it('throws on an unrecognized shape instead of guessing', () => {
    expect(() => decodeState({ Value: { Weird: 1 } } as never)).toThrow(/unrecognized/);
  });
});

describe('path utils', () => {
  const doc = { a: [{ b: 'x' }], n: 1 };

  it('getAtPath reads nested values and returns undefined off the map', () => {
    expect(getAtPath(doc, ['a', 0, 'b'])).toBe('x');
    expect(getAtPath(doc, ['a', 1, 'b'])).toBeUndefined();
    expect(getAtPath(doc, ['z'])).toBeUndefined();
  });

  it('setAtPath writes without mutating the original', () => {
    const next = setAtPath(doc, ['a', 0, 'b'], 'y');
    expect(getAtPath(next, ['a', 0, 'b'])).toBe('y');
    expect(getAtPath(doc, ['a', 0, 'b'])).toBe('x');
  });

  it('pathKey is stable and distinguishes indices from keys', () => {
    expect(pathKey(['a', 0])).toBe(pathKey(['a', 0]));
    expect(pathKey(['a', 0])).not.toBe(pathKey(['a', '0']));
  });
});
