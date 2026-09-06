// The state-dependent invariants, reported and never refused: mp34 pure, and
// mp37 against live nodes, where two replicas each accept one half of a
// duplicate and both report it with editing left enabled.
import { rmSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { Descriptor, JsonOp, PlainJson } from '../api/types';
import { addChildOps, createRootOps, createSingleContainmentOps, setStringOps } from '../crdt/ops';
import { ModelSession } from '../sync/modelSession';
import {
  createModel,
  joinModel,
  modelState,
  scratchDir,
  skipReason,
  startNodes,
  waitFor,
  waitForAgreement,
  writeMetamodelDir,
} from '../testing/liveNodes';
import { recordSession } from '../testing/sessionRecorder';
import { editGate } from '../ui/editGate';
import { applyModel, refusalOf } from './binding';
import { checkInvariants, declaringClass, describeDiagnostics, sameDiagnostics, type Diagnostic } from './conformance';
import { modelHeaderOf } from './instance';
import { bt, btHeader, btId, encodeWire, uml, umlHeader } from './testFixtures';

/** A behaviour tree: its id, its child, and the blackboard the descriptor requires. */
function tree(id: string, child: PlainJson): PlainJson {
  return { eClass: 'BehaviorTree', ID: id, child, blackboard: { eClass: 'Blackboard' } };
}

const withHeader = (doc: Record<string, PlainJson>, header: PlainJson): PlainJson => ({ __model: header, ...doc });

/** A `Sequence` and a `Fallback` both named `patrol`, under two trees. */
const duplicate: PlainJson = withHeader(
  {
    eClass: 'Root',
    main: tree('t1', { eClass: 'Sequence', ID: 'patrol' }),
    behaviortrees: [tree('t2', { eClass: 'Fallback', ID: 'patrol' })],
  },
  btHeader as unknown as PlainJson,
);

/** A tree whose required `child` is absent. */
const missing: PlainJson = withHeader(
  { eClass: 'Root', main: { eClass: 'BehaviorTree', ID: 't', blackboard: { eClass: 'Blackboard' } } },
  btHeader as unknown as PlainJson,
);

/** A `Generalization` whose `general` names a `Class` the document does not hold. */
const dangling: PlainJson = withHeader(
  {
    eClass: 'Class',
    name: 'Window',
    abstract: false,
    stereotype: ['entity'],
    generalizations: [{ eClass: 'Generalization', isSubstitutable: false, general: 'Door' }],
  },
  umlHeader as unknown as PlainJson,
);

const clean: PlainJson = withHeader(
  { eClass: 'Root', main: tree('t', { eClass: 'Sequence', ID: 'patrol' }) },
  btHeader as unknown as PlainJson,
);

const rules = (diagnostics: Diagnostic[]) => diagnostics.map((d) => [d.rule, d.path.join('/')]);

describe('mp34 the invariant diagnostics report and never refuse', () => {
  it('mp34_the_invariant_diagnostics_report_and_never_refuse', async () => {
    const found = checkInvariants(bt, duplicate);
    expect(rules(found)).toEqual([
      ['duplicate-id', 'behaviortrees/0/child'],
      ['duplicate-id', 'main/child'],
    ]);
    for (const diagnostic of found) {
      expect(diagnostic.message).toContain('`TreeNode.ID` `patrol`');
    }
    expect(found[1].message).toContain('behaviortrees/0/child (Fallback)');

    const absent = checkInvariants(bt, missing);
    expect(rules(absent)).toEqual([['missing-required', 'main']]);
    expect(absent[0].message).toBe('`BehaviorTree.child` is required and absent at main');

    const unresolved = checkInvariants(uml, dangling);
    expect(rules(unresolved)).toEqual([['dangling-reference', 'generalizations/0']]);
    expect(unresolved[0].message).toBe(
      '`Generalization.general` at generalizations/0 names `Door`, which no `Class` in the document carries',
    );

    expect(checkInvariants(bt, clean)).toEqual([]);

    // A report is a report: the apply answers bound with the document for
    // all four, so nothing here can reach the edit gate.
    for (const [descriptor, doc] of [
      [bt, duplicate],
      [bt, missing],
      [uml, dangling],
      [bt, clean],
    ] as [Descriptor, PlainJson][]) {
      const result = await applyModel(encodeWire(doc), descriptor, null);
      expect(result.applied).toBe(true);
      if (result.applied) {
        expect(result.binding.kind).toBe('bound');
        expect(result.doc).toEqual(doc);
      }
      expect(refusalOf(result.binding)).toBeNull();
    }
  });

  it('keys uniqueness on the declaring class of the identifying attribute', () => {
    expect(declaringClass(bt, 'Sequence', 'ID')).toBe('TreeNode');
    expect(declaringClass(bt, 'BehaviorTree', 'ID')).toBe('BehaviorTree');
    expect(declaringClass(uml, 'Class', 'name')).toBe('ModelElement');
    // Two trees named alike collide on BehaviorTree.ID, not on TreeNode.ID.
    const trees: PlainJson = { eClass: 'Root', main: tree('same', null), behaviortrees: [tree('same', null)] };
    const found = checkInvariants(bt, trees).filter((d) => d.rule === 'duplicate-id');
    expect(found.map((d) => d.path.join('/'))).toEqual(['behaviortrees/0', 'main']);
    expect(found[0].message).toContain('`BehaviorTree.ID` `same`');
  });

  it('reports a class tag that names no concrete class and an enum value that is no literal', () => {
    const withStatus = structuredClone(bt);
    withStatus.classes['TreeNode'].attributes.push({
      name: 'status',
      kind: 'enum',
      enum: 'Status',
      many: false,
      required: false,
      isId: false,
    });
    const doc: PlainJson = {
      eClass: 'Root',
      main: tree('t', { eClass: 'Cl' }),
      behaviortrees: [tree('u', { eClass: 'Sequence', ID: 's', status: 'PAUSED' })],
    };
    const found = checkInvariants(withStatus, doc);
    expect(rules(found)).toEqual([
      ['invalid-literal', 'behaviortrees/0/child'],
      ['unknown-class', 'main/child'],
    ]);
    expect(found[0].message).toContain('`PAUSED`, not a literal of `Status` (RUNNING, SUCCESS, FAILURE)');
    expect(found[1].message).toBe('eClass `Cl` at main/child is not a concrete class of the descriptor');
  });

  it('tells a repeated report from a changed one, and describes it', () => {
    const a = checkInvariants(bt, duplicate);
    expect(sameDiagnostics(a, checkInvariants(bt, duplicate))).toBe(true);
    expect(sameDiagnostics(a, checkInvariants(bt, missing))).toBe(false);
    expect(sameDiagnostics([], [])).toBe(true);
    expect(describeDiagnostics([])).toBe('every invariant of the descriptor holds');
    expect(describeDiagnostics(a).startsWith('2 conformance issues:')).toBe(true);
    expect(describeDiagnostics(a)).toContain('duplicate-id: ');
  });
});

describe('level 4, against live nodes', () => {
  it('mp37_the_editor_shows_the_duplicate_beside_the_binding_and_keeps_editing_enabled', { timeout: 120_000 }, async (ctx) => {
    const skip = skipReason('mp37');
    if (skip !== null) return ctx.skip(skip);
    const run = scratchDir('mp37');
    const dir = writeMetamodelDir(join(run, 'metamodels'), { 'bt.metamodel.json': bt, 'uml.metamodel.json': uml });
    const [a, b] = await startNodes(
      [
        { name: 'editor-a', metamodelDir: dir },
        { name: 'editor-b', metamodelDir: dir },
      ],
      run,
    );
    try {
      const id = await createModel(a, btId);
      await joinModel(b, id, btId);
      await waitFor('the header to reach editor-b', async () =>
        modelHeaderOf(await modelState(b, id)) === null ? null : true,
      );
      const recA = recordSession();
      const recB = recordSession();
      const onA = new ModelSession({ id, nodeUrl: a.url, store: null, pollMs: 200, events: recA.events });
      const onB = new ModelSession({ id, nodeUrl: b.url, store: null, pollMs: 200, events: recB.events });
      await onA.open();
      await onB.open();
      try {
        // editor-a builds `Root.main`, a tree whose child is a `Sequence`
        // named `patrol`; editor-b, concurrently, `Root.behaviortrees[0]`,
        // a tree whose child is a `Fallback` named `patrol`. Every operation
        // passes its own node's intake: nothing in either is malformed.
        const send = async (session: ModelSession, batches: [string, JsonOp[]][]) => {
          for (const [description, ops] of batches) {
            const outcome = await session.sendOps(description, ops);
            expect(outcome, description).toMatchObject({ outcome: 'ok' });
          }
        };
        await Promise.all([
          send(onA, [
            ['create root Root', createRootOps('Root')],
            ['create main', createSingleContainmentOps([], 'main', 'BehaviorTree')],
            ['name main', setStringOps(['main', 'ID'], '', 't1')],
            ['create child', createSingleContainmentOps(['main'], 'child', 'Sequence')],
            ['name child', setStringOps(['main', 'child', 'ID'], '', 'patrol')],
            ['create blackboard', createSingleContainmentOps(['main'], 'blackboard', 'Blackboard')],
          ]),
          send(onB, [
            ['add tree', addChildOps(['behaviortrees'], 0, 'BehaviorTree')],
            ['name tree', setStringOps(['behaviortrees', 0, 'ID'], '', 't2')],
            ['create child', createSingleContainmentOps(['behaviortrees', 0], 'child', 'Fallback')],
            ['name child', setStringOps(['behaviortrees', 0, 'child', 'ID'], '', 'patrol')],
            ['create blackboard', createSingleContainmentOps(['behaviortrees', 0], 'blackboard', 'Blackboard')],
          ]),
        ]);
        const agreed = await waitForAgreement([a, b], id);
        expect(agreed).toMatchObject({
          main: { child: { eClass: 'Sequence', ID: 'patrol' } },
          behaviortrees: [{ child: { eClass: 'Fallback', ID: 'patrol' } }],
        });

        // Both tabs: bound, the same two diagnostics, both nodes in the
        // document, and nothing held.
        const reported = await waitFor('both tabs to report the duplicate', () => {
          const tabs = [recA.tab(id, a.url), recB.tab(id, b.url)];
          return tabs.every((tab) => tab.diagnostics.length === 2) ? tabs : null;
        });
        for (const tab of reported) {
          expect(tab.binding?.kind).toBe('bound');
          expect(rules(tab.diagnostics)).toEqual([
            ['duplicate-id', 'behaviortrees/0/child'],
            ['duplicate-id', 'main/child'],
          ]);
          expect(tab.doc).toMatchObject({
            main: { child: { eClass: 'Sequence', ID: 'patrol' } },
            behaviortrees: [{ child: { eClass: 'Fallback', ID: 'patrol' } }],
          });
          const gate = editGate({
            pendingOps: 0,
            batch: null,
            settledAt: null,
            lastSyncAt: tab.lastSyncAt,
            now: Date.now(),
            refused: refusalOf(tab.binding),
          });
          expect([gate.canEditStructure, gate.canReorder, gate.canEditValues]).toEqual([true, true, true]);
        }
        expect(sameDiagnostics(reported[0].diagnostics, reported[1].diagnostics)).toBe(true);
        // The log holds the report as it stood once the writes were done;
        // a poll between two batches may have logged the tree without its
        // child first, and that row is earlier.
        for (const rec of [recA, recB]) {
          const row = rec.rows.findLast((entry) => entry.description === 'conformance');
          expect(row).toMatchObject({ outcome: 'diagnostic' });
          expect(row?.detail).toContain('`TreeNode.ID` `patrol`');
        }

        // A structural refusal from the node arrives as any refused
        // operation does: the batch that would put a `Class` under
        // `BehaviorTree.blackboard` is refused at its first character with
        // the node's sentence, logged as a row and a banner, and the
        // document is unchanged.
        const before = await modelState(a, id);
        const refused = await onA.sendOps(
          'create Class under blackboard',
          createSingleContainmentOps(['main'], 'blackboard', 'Class'),
        );
        expect(refused).toMatchObject({ outcome: 'refused', applied: 0 });
        expect(refused.detail).toContain('`eClass` at `BehaviorTree.blackboard` cannot have `C` at position 0');
        expect(refused.detail).toContain('Blackboard');
        expect(recA.rows).toContainEqual(
          expect.objectContaining({ description: 'create Class under blackboard', outcome: 'refused', detail: refused.detail }),
        );
        expect(recA.banners.some((banner) => banner.includes(refused.detail ?? ' '))).toBe(true);
        expect(await modelState(a, id)).toEqual(before);
        expect(await modelState(b, id)).toEqual(before);
      } finally {
        onA.close();
        onB.close();
      }
    } finally {
      await Promise.all([a.stop(), b.stop()]);
      rmSync(run, { recursive: true, force: true });
    }
  });
});
