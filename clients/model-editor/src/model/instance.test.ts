import { describe, expect, it } from 'vitest';
import type { Descriptor, PlainJson } from '../api/types';
import {
  attributeValue,
  buildTree,
  collectInstances,
  concreteSubtypes,
  defaultForKind,
  eClassOf,
  flattenFeatures,
  idAttributeOf,
  isPresent,
  isSubtypeOf,
  labelFor,
  rootCandidates,
} from './instance';

/** A compact bt-shaped descriptor exercising every concept the module handles. */
const bt: Descriptor = {
  formatVersion: 1,
  package: 'behaviortree',
  nsURI: 'http://www.example.org/behaviortree',
  rootClasses: ['Root'],
  classes: {
    Root: {
      abstract: false,
      superTypes: [],
      attributes: [],
      containments: [
        { name: 'behaviortrees', target: 'BehaviorTree', many: true, required: false, ordered: true },
        { name: 'main', target: 'BehaviorTree', many: false, required: true, ordered: true },
      ],
      references: [],
    },
    BehaviorTree: {
      abstract: false,
      superTypes: [],
      attributes: [{ name: 'ID', kind: 'string', many: false, required: true, isId: false }],
      containments: [
        { name: 'child', target: 'TreeNode', many: false, required: true, ordered: true },
        { name: 'blackboard', target: 'Blackboard', many: false, required: true, ordered: true },
      ],
      references: [],
    },
    Blackboard: {
      abstract: false,
      superTypes: [],
      attributes: [],
      containments: [
        { name: 'entries', target: 'BlackboardEntry', many: true, required: false, ordered: true },
      ],
      references: [],
    },
    BlackboardEntry: {
      abstract: false,
      superTypes: [],
      attributes: [
        { name: 'key', kind: 'string', many: false, required: true, isId: true },
        { name: 'value', kind: 'string', many: false, required: true, isId: false },
      ],
      containments: [],
      references: [],
    },
    TreeNode: {
      abstract: true,
      superTypes: [],
      attributes: [
        { name: 'ID', kind: 'string', many: false, required: true, isId: false },
        { name: 'name', kind: 'string', many: false, required: false, isId: false },
      ],
      containments: [],
      references: [],
    },
    ControlNode: {
      abstract: true,
      superTypes: ['TreeNode'],
      attributes: [],
      containments: [
        { name: 'children', target: 'TreeNode', many: true, required: false, ordered: true },
      ],
      references: [],
    },
    Sequence: { abstract: false, superTypes: ['ControlNode'], attributes: [], containments: [], references: [] },
    Fallback: { abstract: false, superTypes: ['ControlNode'], attributes: [], containments: [], references: [] },
    ExecutionNode: {
      abstract: true,
      superTypes: ['TreeNode'],
      attributes: [{ name: 'status', kind: 'enum', enum: 'Status', many: false, required: false, isId: false }],
      containments: [
        { name: 'inflowports', target: 'InFlowPort', many: true, required: false, ordered: true },
      ],
      references: [],
    },
    Action: { abstract: true, superTypes: ['ExecutionNode'], attributes: [], containments: [], references: [] },
    OpenDoor: { abstract: false, superTypes: ['Action'], attributes: [], containments: [], references: [] },
    InFlowPort: {
      abstract: false,
      superTypes: [],
      attributes: [],
      containments: [],
      references: [{ name: 'entry', target: 'BlackboardEntry', many: false, required: false }],
    },
  },
  enums: { Status: ['RUNNING', 'SUCCESS', 'FAILURE'] },
};

describe('flattenFeatures', () => {
  it('flattens the inheritance closure, supertype features first', () => {
    const flat = flattenFeatures(bt, 'Sequence');
    expect(flat.attributes.map((a) => a.name)).toEqual(['ID', 'name']);
    expect(flat.containments.map((c) => c.name)).toEqual(['children']);
  });

  it('walks multi-level chains (OpenDoor -> Action -> ExecutionNode -> TreeNode)', () => {
    const flat = flattenFeatures(bt, 'OpenDoor');
    expect(flat.attributes.map((a) => a.name)).toEqual(['ID', 'name', 'status']);
    expect(flat.containments.map((c) => c.name)).toEqual(['inflowports']);
  });

  it('tolerates unknown classes and supertype cycles', () => {
    expect(flattenFeatures(bt, 'Nope')).toEqual({ attributes: [], containments: [], references: [] });
    const cyclic: Descriptor = {
      ...bt,
      classes: {
        A: { abstract: false, superTypes: ['B'], attributes: [], containments: [], references: [] },
        B: { abstract: false, superTypes: ['A'], attributes: [], containments: [], references: [] },
      },
    };
    expect(flattenFeatures(cyclic, 'A').attributes).toEqual([]);
  });
});

describe('subtyping', () => {
  it('isSubtypeOf covers self and transitive supertypes', () => {
    expect(isSubtypeOf(bt, 'OpenDoor', 'TreeNode')).toBe(true);
    expect(isSubtypeOf(bt, 'OpenDoor', 'OpenDoor')).toBe(true);
    expect(isSubtypeOf(bt, 'OpenDoor', 'ControlNode')).toBe(false);
    expect(isSubtypeOf(bt, 'TreeNode', 'OpenDoor')).toBe(false);
  });

  it('concreteSubtypes lists the concrete family of an abstract class, sorted', () => {
    expect(concreteSubtypes(bt, 'TreeNode')).toEqual(['Fallback', 'OpenDoor', 'Sequence']);
    expect(concreteSubtypes(bt, 'ControlNode')).toEqual(['Fallback', 'Sequence']);
    expect(concreteSubtypes(bt, 'Blackboard')).toEqual(['Blackboard']);
  });

  it('rootCandidates expands root classes; falls back to all concrete classes', () => {
    expect(rootCandidates(bt)).toEqual(['Root']);
    expect(rootCandidates({ ...bt, rootClasses: [] })).toEqual([
      'BehaviorTree', 'Blackboard', 'BlackboardEntry', 'Fallback', 'InFlowPort', 'OpenDoor', 'Root', 'Sequence',
    ]);
  });
});

describe('presence rule', () => {
  it('present iff object with non-empty eClass', () => {
    expect(isPresent({ eClass: 'Root' })).toBe(true);
    expect(isPresent({ eClass: '' })).toBe(false); // Object.Remove reset
    expect(isPresent({})).toBe(false);
    expect(isPresent(null)).toBe(false);
    expect(isPresent(undefined)).toBe(false);
    expect(isPresent('Root')).toBe(false);
    expect(isPresent([{ eClass: 'Root' }])).toBe(false);
    expect(eClassOf({ eClass: 'Root' })).toBe('Root');
    expect(eClassOf({ eClass: '' })).toBe(null);
  });
});

describe('labels and ids', () => {
  it('idAttributeOf prefers isId, then ID/name, then any string attribute', () => {
    expect(idAttributeOf(bt, 'BlackboardEntry')?.name).toBe('key'); // isId wins
    expect(idAttributeOf(bt, 'Sequence')?.name).toBe('ID'); // inherited, by-name fallback
    expect(idAttributeOf(bt, 'Root')).toBe(null);
    expect(idAttributeOf(bt, 'Blackboard')).toBe(null);
    // No isId, no ID/name: the first single-valued string attribute serves.
    const noId: Descriptor = {
      ...bt,
      classes: {
        ...bt.classes,
        Entry: {
          abstract: false,
          superTypes: [],
          attributes: [
            { name: 'count', kind: 'int', many: false, required: false, isId: false },
            { name: 'key', kind: 'string', many: false, required: true, isId: false },
          ],
          containments: [],
          references: [],
        },
      },
    };
    expect(idAttributeOf(noId, 'Entry')?.name).toBe('key');
  });

  it('labelFor uses the id value when set, else the eClass', () => {
    expect(labelFor(bt, { eClass: 'Sequence', ID: 'seq-1' })).toBe('seq-1');
    expect(labelFor(bt, { eClass: 'Sequence', ID: '' })).toBe('Sequence');
    expect(labelFor(bt, { eClass: 'Root' })).toBe('Root');
  });

  it('attribute values fall back to kind defaults (the Remove-reset shape)', () => {
    expect(attributeValue({ eClass: 'Sequence' }, { name: 'ID', kind: 'string', many: false, required: true, isId: false })).toBe('');
    expect(attributeValue({ eClass: 'X', n: 3.5 }, { name: 'n', kind: 'float', many: false, required: false, isId: false })).toBe(3.5);
    expect(attributeValue({ eClass: 'X' }, { name: 'n', kind: 'int', many: false, required: false, isId: false })).toBe(0);
    expect(attributeValue({ eClass: 'X' }, { name: 'b', kind: 'bool', many: false, required: false, isId: false })).toBe(false);
    expect(defaultForKind('string')).toBe('');
    expect(defaultForKind('int')).toBe(0);
    expect(defaultForKind('bool')).toBe(false);
  });
});

/** A small but deep instance: Root -> main tree -> Sequence -> [OpenDoor], plus blackboard. */
const doc: PlainJson = {
  eClass: 'Root',
  behaviortrees: [],
  main: {
    eClass: 'BehaviorTree',
    ID: 'main-tree',
    child: {
      eClass: 'Sequence',
      ID: 'seq-1',
      children: [
        { eClass: 'OpenDoor', ID: 'open-1', status: 'RUNNING', inflowports: [{ eClass: 'InFlowPort', entry: 'door' }] },
        { eClass: '' }, // a Remove-reset slot: kept, flagged
      ],
    },
    blackboard: {
      eClass: 'Blackboard',
      entries: [{ eClass: 'BlackboardEntry', key: 'door', value: 'open' }],
    },
  },
};

describe('buildTree', () => {
  it('returns null for absent roots', () => {
    expect(buildTree(bt, null)).toBe(null);
    expect(buildTree(bt, { eClass: '' })).toBe(null);
    expect(buildTree(bt, 'Unexpected')).toBe(null);
  });

  it('builds the containment tree with paths, labels, and feature grouping', () => {
    const tree = buildTree(bt, doc);
    expect(tree).not.toBe(null);
    if (tree === null) return;
    expect(tree.eClass).toBe('Root');
    expect(tree.features.map((f) => f.desc.name)).toEqual(['behaviortrees', 'main']);
    expect(tree.features[0].children).toEqual([]);

    const main = tree.features[1].children[0];
    expect(main.label).toBe('main-tree');
    expect(main.path).toEqual(['main']);

    const seq = main.features[0].children[0];
    expect(seq.eClass).toBe('Sequence');
    expect(seq.label).toBe('seq-1');
    expect(seq.path).toEqual(['main', 'child']);

    const children = seq.features[0].children;
    expect(children).toHaveLength(2); // index preserved for the degenerate slot
    expect(children[0].path).toEqual(['main', 'child', 'children', 0]);
    expect(children[0].label).toBe('open-1');
    expect(children[1].eClass).toBe('');
    expect(children[1].label).toBe('(invalid element)');

    const port = children[0].features[0].children[0];
    expect(port.path).toEqual(['main', 'child', 'children', 0, 'inflowports', 0]);
    expect(port.eClass).toBe('InFlowPort');
  });

  it('treats a missing single containment as absent', () => {
    const tree = buildTree(bt, { eClass: 'Root' });
    expect(tree?.features[1].children).toEqual([]);
  });
});

describe('collectInstances', () => {
  it('collects the concrete family of a target class in tree order, with ids', () => {
    const nodes = collectInstances(bt, doc, 'TreeNode');
    expect(nodes.map((n) => [n.eClass, n.id])).toEqual([
      ['Sequence', 'seq-1'],
      ['OpenDoor', 'open-1'],
    ]);
    const entries = collectInstances(bt, doc, 'BlackboardEntry');
    expect(entries).toHaveLength(1);
    expect(entries[0].id).toBe('door');
    expect(entries[0].path).toEqual(['main', 'blackboard', 'entries', 0]);
  });

  it('returns [] on empty docs and unknown targets', () => {
    expect(collectInstances(bt, null, 'TreeNode')).toEqual([]);
    expect(collectInstances(bt, doc, 'Nope')).toEqual([]);
  });
});
