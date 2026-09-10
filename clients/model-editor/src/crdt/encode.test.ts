/**
 * The slot table and the encoder, against the descriptor the rig serves.
 *
 * The claim these tests carry is narrow and is the one that matters: the
 * positions the editor computes are the positions the node computes. A unit
 * test cannot prove that on its own — an operation addressing the wrong slot
 * is well-formed and applied — so the anchor is an operation verified by hand
 * against a live interpreted replica (`ip-editor`, alice on port 8081): the
 * one that writes `g` into the first behaviour tree's ID, whose slots are
 * Root 17, Root.behaviortrees 0, BehaviorTree 1, BehaviorTree.ID 0. Every
 * other case here is the same computation over the same descriptor.
 */

import { describe, expect, it } from 'vitest';
import type { Descriptor, ModelOp, PlainJson } from '../api/types';
import { encodeBatch, encodeEdit, textDiff } from './encode';
import {
  addChildOps,
  createRootOps,
  createSingleContainmentOps,
  reorderArrayOps,
  removeFromArrayOps,
  setStringOps,
  unsetFeatureOps,
  type EditOp,
} from './ops';
import { classOf, featureOf, slotTable, SlotError } from './table';
import { bt } from '../model/testFixtures';

const table = slotTable(bt);

/** A behaviour-tree document with one tree, as the node renders it. */
const doc: PlainJson = {
  eClass: 'Root',
  behaviortrees: [
    { eClass: 'BehaviorTree', ID: 'main', child: { eClass: 'Sequence', ID: 'root', children: [] } },
  ],
};

const one = (edit: EditOp[], against: PlainJson = doc): ModelOp[] => encodeBatch(table, against, edit);

describe('the slot table is the table the node computes', () => {
  it('numbers classes by sorted name, and the hand-verified pair sits where the node put it', () => {
    expect(classOf(table, 'Root').slot).toBe(17);
    expect(classOf(table, 'BehaviorTree').slot).toBe(1);
    expect(table.classes.map((cls) => cls.name)).toEqual([...table.classes.map((cls) => cls.name)].sort());
    expect(table.classes[0].name).toBe('Action');
    expect(table.classes.length).toBe(21);
  });

  it('numbers enums in a list of their own, so an enum slot is not a class slot', () => {
    expect(table.enums).toEqual(['Status']);
    expect(table.enumSlot.get('Status')).toBe(0);
    // Slot 0 of `classes` is a class, and slot 0 of `enums` is the enum: the
    // two numberings overlap by construction and are never interchangeable.
    expect(table.classes[0].name).not.toBe('Status');
  });

  it('gives a feature its VISIBLE slot: own and inherited in one sorted list', () => {
    // `Sequence` sees `ID` and `name` from TreeNode and `children` from
    // ControlNode. Keyed by the declaring class's own numbering, `children`
    // and `ID` would both be 0 and two features would merge into one.
    const sequence = classOf(table, 'Sequence');
    expect(sequence.visible.map((f) => f.name)).toEqual(['ID', 'children', 'name']);
    expect(featureOf(sequence, 'children').slot).toBe(1);
    expect(featureOf(sequence, 'children').owner).toBe('ControlNode');
    expect(featureOf(sequence, 'ID').owner).toBe('TreeNode');

    const tree = classOf(table, 'BehaviorTree');
    expect(tree.visible.map((f) => f.name)).toEqual(['ID', 'blackboard', 'child']);
    expect(featureOf(tree, 'ID').slot).toBe(0);
  });

  it('splits each rule into the collection the node routes it as', () => {
    const tree = classOf(table, 'BehaviorTree');
    expect(featureOf(tree, 'ID').collection).toBe('bare');
    expect(featureOf(tree, 'ID').site).toEqual({ site: 'scalar', leaf: { kind: 'text' } });
    expect(featureOf(tree, 'child').collection).toBe('bare');
    expect(featureOf(tree, 'child').site).toEqual({ site: 'object', target: 'TreeNode' });
    expect(featureOf(classOf(table, 'Root'), 'behaviortrees').collection).toBe('sequence');
    expect(featureOf(classOf(table, 'Sequence'), 'name').collection).toBe('optional');
  });

  it('refuses a class or a feature it cannot address, rather than defaulting to a slot', () => {
    expect(() => classOf(table, 'Klass')).toThrow(SlotError);
    expect(() => featureOf(classOf(table, 'Root'), 'nodes')).toThrow(/`nodes` is not a feature `Root` can see/);
  });
});

describe('the encoder addresses the path the way the node routes it', () => {
  it('writes one character into a behaviour tree ID exactly as the live replica took it', () => {
    // Verified by hand against alice: this body answered
    // {"success":true,"message":"Applied and broadcasted"} and the state
    // read back as {"behaviortrees":[{"ID":"g","eClass":"BehaviorTree"}],"eClass":"Root"}.
    const empty: PlainJson = { eClass: 'Root', behaviortrees: [{ eClass: 'BehaviorTree', ID: '' }] };
    expect(one(setStringOps(['behaviortrees', 0, 'ID'], '', 'g'), empty)).toEqual([
      {
        Instance: {
          Variant: [
            17,
            {
              Field: [
                0,
                {
                  Seq: {
                    Update: {
                      pos: 0,
                      op: { Variant: [1, { Field: [0, { Leaf: { InsertChar: { pos: 0, ch: 'g' } } }] }] },
                    },
                  },
                },
              ],
            },
          ],
        },
      },
    ]);
  });

  it('names the class of every object on the path, not only of the one it created', () => {
    const ops = one(setStringOps(['behaviortrees', 0, 'child', 'ID'], 'root', 'guard'));
    // Root, then the tree at position 0, then the Sequence in `child`: three
    // `Variant` steps for one write four levels down.
    const text = JSON.stringify(ops);
    expect(text).toContain(`"Variant":[${classOf(table, 'Root').slot}`);
    expect(text).toContain(`"Variant":[${classOf(table, 'BehaviorTree').slot}`);
    expect(text).toContain(`"Variant":[${classOf(table, 'Sequence').slot}`);
  });

  it('writes a text leaf one character at a time, keeping the common prefix', () => {
    expect(textDiff('root', 'roof')).toEqual([
      { DeleteRange: { start: 3, len: 1 } },
      { InsertChar: { pos: 3, ch: 'f' } },
    ]);
    const ops = one(setStringOps(['behaviortrees', 0, 'ID'], 'main', 'mainline'));
    expect(ops).toHaveLength(4);
    expect(JSON.stringify(ops[0])).toContain('"InsertChar":{"pos":4,"ch":"l"}');
  });

  it('wraps an optional attribute in its `Opt` step and a sequence in its `Seq` step', () => {
    const named = one(setStringOps(['behaviortrees', 0, 'child', 'name'], '', 'x'));
    expect(JSON.stringify(named[0])).toContain('"Opt":{"Set":{"Leaf"');
    const inserted = one(addChildOps(['behaviortrees', 0, 'child', 'children'], 0, 'Fallback'));
    expect(inserted).toHaveLength(1);
    expect(JSON.stringify(inserted[0])).toContain(
      `"Seq":{"Insert":{"pos":0,"op":{"Variant":[${classOf(table, 'Fallback').slot},"New"]}}}`,
    );
  });

  it('mints the root, a single containment and a sequence element the same way: Variant then New', () => {
    expect(one(createRootOps('Root'), null)).toEqual([{ Instance: { Variant: [17, 'New'] } }]);
    const main = one(createSingleContainmentOps([], 'main', 'BehaviorTree'));
    expect(main).toEqual([{ Instance: { Variant: [17, { Field: [1, { Variant: [1, 'New'] }] }] } }]);
  });

  it('takes an element out of a sequence by position', () => {
    expect(one(removeFromArrayOps(['behaviortrees'], 0))).toEqual([
      { Instance: { Variant: [17, { Field: [0, { Seq: { Delete: { pos: 0 } } }] }] } },
    ]);
  });

  it('rebuilds a reordered element against the document the delete leaves behind', () => {
    const two: PlainJson = {
      eClass: 'Root',
      behaviortrees: [
        { eClass: 'BehaviorTree', ID: 'a' },
        { eClass: 'BehaviorTree', ID: 'b' },
      ],
    };
    const ops = one(reorderArrayOps(['behaviortrees'], 1, 0, { eClass: 'BehaviorTree', ID: 'b' }), two);
    // Delete, insert the class back at 0, then one character of its ID: the
    // write addresses position 0, which is where the insert of the same batch
    // put it, and not position 1 where the document still had it.
    expect(ops).toHaveLength(3);
    expect(JSON.stringify(ops[0])).toContain('"Delete":{"pos":1}');
    expect(JSON.stringify(ops[1])).toContain('"Insert":{"pos":0');
    expect(JSON.stringify(ops[2])).toContain('"Update":{"pos":0');
    expect(JSON.stringify(ops[2])).toContain('"InsertChar":{"pos":0,"ch":"b"}');
  });
});

describe('the encoder refuses what it cannot address, naming it', () => {
  it('refuses the model header, which is no feature of any class', () => {
    expect(() => one([{ kind: 'set', path: ['__model', 'modelId'], from: '', to: 'x' }])).toThrow(
      /__model is the model header/,
    );
  });

  it('refuses a path through an object the document does not carry a class for', () => {
    expect(() => one(setStringOps(['behaviortrees', 3, 'ID'], '', 'x'))).toThrow(
      /\/behaviortrees\/3 holds no object/,
    );
    expect(() =>
      one(setStringOps(['behaviortrees', 0, 'child', 'ID'], '', 'x'), {
        eClass: 'Root',
        behaviortrees: [{ eClass: 'BehaviorTree', child: { ID: 'x' } }],
      }),
    ).toThrow(/carries no `eClass`/);
    expect(() => one(setStringOps(['behaviortrees', 0, 'ID'], '', 'x'), null)).toThrow(
      /the model root holds no object/,
    );
  });

  it('refuses a feature the class cannot see', () => {
    expect(() => one(setStringOps(['behaviortrees', 0, 'label'], '', 'x'))).toThrow(
      /`label` is not a feature `BehaviorTree` can see/,
    );
  });

  it('refuses to empty a single-valued containment, which no operation does', () => {
    expect(() => one(unsetFeatureOps(['behaviortrees', 0], 'child'))).toThrow(
      /single-valued containment and always present/,
    );
  });

  it('unsets an optional feature, which one does', () => {
    expect(one(unsetFeatureOps(['behaviortrees', 0, 'child'], 'name'))).toEqual([
      {
        Instance: {
          Variant: [
            17,
            {
              Field: [
                0,
                {
                  Seq: {
                    Update: { pos: 0, op: { Variant: [1, { Field: [2, { Variant: [18, { Field: [2, { Opt: 'Unset' }] }] }] }] } },
                  },
                },
              ],
            },
          ],
        },
      },
    ]);
  });
});

/** A descriptor exercising the leaf families the behaviour tree has none of. */
const leaves: Descriptor = {
  formatVersion: 2,
  package: 'leaves',
  nsURI: 'urn:leaves',
  rootClasses: ['Bench'],
  enums: { Status: ['RUNNING', 'SUCCESS', 'FAILURE'] },
  classes: {
    Bench: {
      abstract: false,
      superTypes: [],
      containments: [],
      references: [
        {
          name: 'peers',
          target: 'Bench',
          many: true,
          required: false,
          merge: { kind: 'reference', many: true, target: 'Bench' },
        },
      ],
      attributes: [
        {
          name: 'count',
          kind: 'int',
          many: false,
          required: false,
          isId: false,
          merge: { kind: 'attribute', shape: { kind: 'single' }, leaf: { kind: 'counter', num: 'i32', resettable: true } },
        },
        {
          name: 'depth',
          kind: 'float',
          many: false,
          required: false,
          isId: false,
          merge: { kind: 'attribute', shape: { kind: 'single' }, leaf: { kind: 'counter', num: 'f64', resettable: true } },
        },
        {
          name: 'enabled',
          kind: 'bool',
          many: false,
          required: false,
          isId: false,
          merge: { kind: 'attribute', shape: { kind: 'single' }, leaf: { kind: 'flag', wins: 'enable' } },
        },
        {
          name: 'status',
          kind: 'enum',
          enum: 'Status',
          many: false,
          required: false,
          isId: false,
          merge: { kind: 'attribute', shape: { kind: 'single' }, leaf: { kind: 'enum', class: 'Status', tie: 'mv' } },
        },
        {
          name: 'tags',
          kind: 'string',
          many: true,
          required: false,
          isId: false,
          merge: { kind: 'attribute', shape: { kind: 'set', tie: 'aw' }, leaf: { kind: 'register', tie: 'mv' } },
        },
      ],
    },
  },
};

describe('the leaf families the rule decides', () => {
  const leafTable = slotTable(leaves);
  const bench: PlainJson = { eClass: 'Bench', count: 3, tags: ['a', 'b'], peers: ['p'] };
  const leaf = (edit: EditOp): ModelOp => encodeEdit(leafTable, bench, edit)[0];

  it('writes a counter by how far it moves, at the width the descriptor declares', () => {
    expect(leaf({ kind: 'set', path: ['count'], from: 3, to: 7 })).toEqual({
      Instance: { Variant: [0, { Field: [featureOf(classOf(leafTable, 'Bench'), 'count').slot, { Leaf: { Inc: { Int: 4 } } }] }] },
    });
    expect(JSON.stringify(leaf({ kind: 'set', path: ['count'], from: 3, to: 1 }))).toContain('"Dec":{"Int":2}');
  });

  it('carries a float counter as the bit pattern the Rust scalar holds', () => {
    const op = leaf({ kind: 'set', path: ['depth'], from: 0, to: 3.5 });
    // 3.5 is 0x400C000000000000.
    expect(JSON.stringify(op, (_k, v: unknown) => (typeof v === 'bigint' ? v.toString() : v))).toContain(
      '"Float":"4614838538166547251"'.replace('4614838538166547251', String(0x400c000000000000n)),
    );
  });

  it('writes a flag by which way it goes, and an enum by its literal position', () => {
    expect(JSON.stringify(leaf({ kind: 'set', path: ['enabled'], from: false, to: true }))).toContain('"Leaf":"Enable"');
    expect(JSON.stringify(leaf({ kind: 'set', path: ['status'], from: null, to: 'FAILURE' }))).toContain(
      '"Write":{"Enum":[0,2]}',
    );
    expect(() => leaf({ kind: 'set', path: ['status'], from: null, to: 'GONE' })).toThrow(
      /`GONE` is not a literal of `Status`/,
    );
  });

  it('addresses a set by value and never by position, both ways', () => {
    expect(JSON.stringify(leaf({ kind: 'insert', path: ['tags'], pos: 9, value: 'c' }))).toContain(
      '"Add":{"Str":"c"}',
    );
    // A remove names the value the document holds at that position, because
    // that is the only thing a set understands.
    expect(JSON.stringify(leaf({ kind: 'remove', path: ['tags'], pos: 1 }))).toContain('"Remove":{"Str":"b"}');
  });

  it('carries a many-valued reference as a set of strings, per design §8', () => {
    const feature = featureOf(classOf(leafTable, 'Bench'), 'peers');
    expect(feature.collection).toBe('bare');
    expect(feature.site.site).toBe('set');
    expect(JSON.stringify(leaf({ kind: 'insert', path: ['peers'], pos: 1, value: 'q' }))).toContain(
      '"Add":{"Str":"q"}',
    );
  });
});
