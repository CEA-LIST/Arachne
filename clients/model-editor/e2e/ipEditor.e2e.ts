/// <reference types="node" />
/**
 * ip-editor: the editor's own client against two live interpreted replicas.
 *
 * The unit tests encode operations and compare them with operations. They
 * cannot tell whether the slots are right, because an operation addressing
 * the wrong feature is well-formed and the node applies it. This scenario
 * closes that gap the only way it can be closed: every edit goes out through
 * `ModelSession` — the same path a control in the browser takes, through the
 * same encoder, the same queue and the same client — and every assertion
 * reads what `GET /api/model/{id}/state` answers on the node afterwards.
 *
 * The replicas are the ones `docker/compose/stack_interpreter.sh` starts:
 *
 *     ./stack_interpreter.sh infra up
 *     ./stack_interpreter.sh up alice --port 8081
 *     ./stack_interpreter.sh up bob   --port 8082
 *
 * `MOIRAI_LIVE_NODES` names them, two comma-separated base URLs, and defaults
 * to those two ports. With nothing listening the scenario follows the suite's
 * skip contract: it prints `E2E-SKIP ip-editor: …` and skips, so a machine
 * with no stack up stays green and a CI that greps the marker fails.
 */

import { describe, expect, it } from 'vitest';
import { getHealth, getMetamodels, registerModel } from '../src/api/client';
import type { MetamodelId, ModelId, PlainJson } from '../src/api/types';
import { addChildOps, createRootOps, setStringOps } from '../src/crdt/ops';
import { ModelSession, type SessionEvents } from '../src/sync/modelSession';

const NODES = (process.env['MOIRAI_LIVE_NODES'] ?? 'http://127.0.0.1:8081,http://127.0.0.1:8082')
  .split(',')
  .map((url) => url.trim());
const BT_NS = 'http://www.example.org/behaviortree';

/** Every node reachable, or the reason none of this can run. */
async function reachable(): Promise<string | null> {
  for (const url of NODES) {
    try {
      await getHealth(url);
    } catch (err) {
      const reason = `E2E-SKIP ip-editor: ${url} does not answer /api/health (${
        err instanceof Error ? err.message : String(err)
      }); start the stack with docker/compose/stack_interpreter.sh, or name other replicas in MOIRAI_LIVE_NODES`;
      console.warn(reason);
      return reason;
    }
  }
  return null;
}

/** A session's events, collected: what a tab would have shown. */
function recorder(): { events: SessionEvents; banners: string[]; rows: { outcome: string; detail?: string }[] } {
  const banners: string[] = [];
  const rows: { outcome: string; detail?: string }[] = [];
  return {
    banners,
    rows,
    events: {
      patch: () => {},
      log: (entry) => rows.push({ outcome: entry.outcome, detail: entry.detail }),
      banner: (message) => banners.push(message),
    },
  };
}

/** `GET /api/model/{id}/state`, read straight from the node and not through a session. */
async function stateOf(url: string, id: ModelId): Promise<PlainJson> {
  const response = await fetch(`${url}/api/model/${id}/state`);
  if (!response.ok) throw new Error(`${url} /api/model/${id}/state: ${response.status}`);
  return (await response.json()) as PlainJson;
}

/** Poll `probe` until it answers true, or fail saying what was last seen. */
async function until(what: string, probe: () => Promise<boolean>, ms = 20_000): Promise<void> {
  const deadline = Date.now() + ms;
  for (;;) {
    if (await probe()) return;
    if (Date.now() >= deadline) throw new Error(`timed out after ${ms} ms waiting for ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

describe('ip-editor, against live interpreted replicas', () => {
  it('ip_editor_drives_an_interpreted_replica_and_two_of_them_converge', { timeout: 300_000 }, async (ctx) => {
    const skip = await reachable();
    if (skip !== null) return ctx.skip(skip);
    const [alice, bob] = NODES;

    // The behaviour-tree descriptor as the node itself identifies it: the
    // digest is the metamodel's identity, and the model is registered under
    // it on both replicas.
    const listed = (await getMetamodels(alice)).find((entry) => entry.nsURI === BT_NS);
    expect(listed, `no ${BT_NS} descriptor on ${alice}`).toBeDefined();
    const btId: MetamodelId = { nsURI: listed!.nsURI, digest: listed!.digest };

    const created = await registerModel(alice, { metamodelId: btId });
    expect(created.created).toBe(true);
    const id = created.modelId;
    await registerModel(bob, { modelId: id, metamodelId: btId });
    console.log(`ip-editor: model ${id} created on ${alice}, joined on ${bob}`);

    const one = recorder();
    const two = recorder();
    const onAlice = new ModelSession({ id, nodeUrl: alice, store: null, pollMs: 500, events: one.events });
    const onBob = new ModelSession({ id, nodeUrl: bob, store: null, pollMs: 500, events: two.events });
    try {
      await onAlice.open();
      await onBob.open();
      expect(onAlice.table, 'the session read a slot table from the descriptor the model is bound to').not.toBeNull();

      // 1. A root and one element, through the editor's own send path.
      expect(await onAlice.sendOps('create root Root', createRootOps('Root'))).toMatchObject({ outcome: 'ok' });
      // Every following intent is encoded against the document the node
      // serves, so the classes on its path are the node's and not a guess.
      await onAlice.refreshOnce();
      expect(
        await onAlice.sendOps('add BehaviorTree', addChildOps(['behaviortrees'], 0, 'BehaviorTree')),
      ).toMatchObject({ outcome: 'ok' });
      await onAlice.refreshOnce();
      console.log(`ip-editor 1: ${JSON.stringify(await stateOf(alice, id))}`);
      expect(await stateOf(alice, id)).toMatchObject({
        eClass: 'Root',
        behaviortrees: [{ eClass: 'BehaviorTree' }],
      });

      // 2. A string attribute, character by character and in the right order.
      expect(
        await onAlice.sendOps('set ID', setStringOps(['behaviortrees', 0, 'ID'], '', 'guard')),
      ).toMatchObject({ outcome: 'ok', applied: 5 });
      await onAlice.refreshOnce();
      const named = (await stateOf(alice, id)) as { behaviortrees: { ID: string }[] };
      console.log(`ip-editor 2: ID = ${JSON.stringify(named.behaviortrees[0].ID)}`);
      expect(named.behaviortrees[0].ID).toBe('guard');
      // And an edit in the middle of it, which is where a reversed or
      // interleaved sequence would show.
      expect(
        await onAlice.sendOps('rename', setStringOps(['behaviortrees', 0, 'ID'], 'guard', 'guarded')),
      ).toMatchObject({ outcome: 'ok' });
      await onAlice.refreshOnce();
      expect(((await stateOf(alice, id)) as { behaviortrees: { ID: string }[] }).behaviortrees[0].ID).toBe('guarded');

      // 3. A child into an ordered containment at a chosen position.
      const children = ['behaviortrees', 0, 'child', 'children'];
      await onAlice.sendOps('create child Sequence', [
        { kind: 'mint', path: ['behaviortrees', 0, 'child'], className: 'Sequence' },
      ]);
      await onAlice.refreshOnce();
      for (const [pos, name] of [
        [0, 'first'],
        [1, 'third'],
        [1, 'second'],
      ] as [number, string][]) {
        expect(await onAlice.sendOps(`add OpenDoor at ${pos}`, addChildOps(children, pos, 'OpenDoor'))).toMatchObject({
          outcome: 'ok',
        });
        await onAlice.refreshOnce();
        expect(await onAlice.sendOps(`name it ${name}`, setStringOps([...children, pos, 'ID'], '', name))).toMatchObject(
          { outcome: 'ok' },
        );
        await onAlice.refreshOnce();
      }
      const ordered = (await stateOf(alice, id)) as PlainJson;
      const ids = (
        (ordered as { behaviortrees: { child: { children: { ID: string }[] } }[] }).behaviortrees[0].child.children
      ).map((child) => child.ID);
      console.log(`ip-editor 3: children in order = ${JSON.stringify(ids)}`);
      expect(ids).toEqual(['first', 'second', 'third']);

      // 4. The other replica, edited from its own session, and both agree.
      await until('bob to see the tree', async () => {
        const doc = (await stateOf(bob, id)) as { behaviortrees?: unknown[] } | null;
        return doc !== null && Array.isArray(doc.behaviortrees) && doc.behaviortrees.length === 1;
      });
      await onBob.refreshOnce();
      expect(
        await onBob.sendOps('name the sequence', setStringOps(['behaviortrees', 0, 'child', 'ID'], '', 'patrol')),
      ).toMatchObject({ outcome: 'ok' });
      await until('the two replicas to agree', async () => {
        const [a, b] = await Promise.all([stateOf(alice, id), stateOf(bob, id)]);
        return JSON.stringify(a) === JSON.stringify(b) && JSON.stringify(a).includes('patrol');
      });
      const converged = await stateOf(alice, id);
      console.log(`ip-editor 4: converged = ${JSON.stringify(converged)}`);
      expect(await stateOf(bob, id)).toEqual(converged);

      // 5. What the editor refuses on its own, before anything is posted.
      const refusedHeader = await onAlice.sendOps('rewrite header', [{ kind: 'unset', path: ['__model'] }]);
      expect(refusedHeader).toMatchObject({ outcome: 'refused', applied: 0 });
      expect(refusedHeader.detail).toContain('__model is the model header');
      const refusedFeature = await onAlice.sendOps('set a feature that is not there', [
        { kind: 'set', path: ['behaviortrees', 0, 'label'], from: '', to: 'x' },
      ]);
      expect(refusedFeature).toMatchObject({ outcome: 'refused', applied: 0 });
      expect(refusedFeature.detail).toContain('`label` is not a feature `BehaviorTree` can see');
      console.log(`ip-editor 5: ${refusedHeader.detail ?? ''} | ${refusedFeature.detail ?? ''}`);
      // Nothing changed on the node: a refusal is a refusal before the wire.
      expect(await stateOf(alice, id)).toEqual(converged);
    } finally {
      onAlice.close();
      onBob.close();
    }
  });
});
