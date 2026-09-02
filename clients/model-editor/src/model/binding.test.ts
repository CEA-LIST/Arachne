/// <reference types="node" />
// Over the repository's descriptors and the digests Rust recorded for them
// (model/testFixtures.ts): the check is over real descriptor bytes.
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';
import type { Descriptor, ModelHeader } from '../api/types';
import {
  applyModel,
  bindingLabel,
  checkBinding,
  describeBinding,
  isRefusal,
  refusalOf,
  sameBinding,
  type ApplyResult,
  type Refused,
} from './binding';
import { fsModelStore } from './fsStore';
import { projectModel } from './projection';
import { bt, btDocument, btHeader, btId, encodeWire as encode, MODEL_ID, uml, umlHeader, umlId } from './testFixtures';

function refused(result: ApplyResult): Refused {
  if (result.applied) throw new Error('expected the apply to be refused');
  return result;
}

describe('checkBinding', () => {
  it('a document with no header is unbound, whatever descriptor is loaded', () => {
    expect(checkBinding(null, btId, null)).toEqual({ kind: 'unbound' });
    expect(checkBinding(null, null, null)).toEqual({ kind: 'unbound' });
  });

  it('a header whose digest equals the served digest is bound', () => {
    expect(checkBinding(btHeader, btId, null)).toEqual({ kind: 'bound', header: btHeader, served: btId });
  });

  it('a header with nothing to check it against is bound, no descriptor', () => {
    expect(checkBinding(btHeader, null, null)).toEqual({ kind: 'no-descriptor', header: btHeader });
  });

  it('a header whose digest differs from the served digest is a mismatch', () => {
    expect(checkBinding(btHeader, umlId, null)).toEqual({ kind: 'mismatch', header: btHeader, served: umlId });
  });

  it('compares digests, so a reformatted descriptor with the same digest is still bound', () => {
    // model/digest.ts guarantees the digest is over the value; here only the
    // comparison is under test, and it must not look at anything else.
    expect(checkBinding(btHeader, { nsURI: btId.nsURI, digest: btId.digest }, null).kind).toBe('bound');
  });

  it('a header equal to the recorded one is checked as usual', () => {
    expect(checkBinding(btHeader, btId, btHeader).kind).toBe('bound');
    expect(checkBinding(btHeader, umlId, btHeader).kind).toBe('mismatch');
  });

  it('a header that differs from the recorded one is refused before anything else', () => {
    expect(checkBinding(umlHeader, umlId, btHeader)).toEqual({
      kind: 'header-rewritten',
      recorded: btHeader,
      header: umlHeader,
    });
    expect(checkBinding(null, btId, btHeader)).toEqual({
      kind: 'header-rewritten',
      recorded: btHeader,
      header: null,
    });
    const otherId: ModelHeader = { ...btHeader, modelId: 'ffffffffffffffffffffffffffffffff' };
    expect(checkBinding(otherId, btId, btHeader).kind).toBe('header-rewritten');
  });

  it('names the two refusing verdicts and no other', () => {
    expect(isRefusal({ kind: 'unbound' })).toBe(false);
    expect(isRefusal({ kind: 'bound', header: btHeader, served: btId })).toBe(false);
    expect(isRefusal({ kind: 'no-descriptor', header: btHeader })).toBe(false);
    expect(isRefusal({ kind: 'mismatch', header: btHeader, served: umlId })).toBe(true);
    expect(isRefusal({ kind: 'header-rewritten', recorded: btHeader, header: umlHeader })).toBe(true);
  });
});

describe('mp9 the binding check at apply', () => {
  const storeDir = mkdtempSync(join(tmpdir(), 'model-binding-'));
  afterAll(() => rmSync(storeDir, { recursive: true, force: true }));

  it('mp9_apply_refuses_a_descriptor_whose_digest_differs_from_the_header', async () => {
    // A behaviour-tree state whose header names the bt digest, served the uml
    // descriptor: the bad day of M-A5, in which Sequence matches no class and
    // the tree used to render as blank rows.
    const result = refused(await applyModel(encode(btDocument(btHeader)), uml, null));
    expect(result.binding.kind).toBe('mismatch');

    // The report names both pairs, nsURI and digest each.
    expect(result.message).toContain(btId.nsURI);
    expect(result.message).toContain(btId.digest);
    expect(result.message).toContain(umlId.nsURI);
    expect(result.message).toContain(umlId.digest);
    expect(result.message).toContain('editing is disabled');

    // The document is untouched: a refusal carries nothing to render.
    expect('doc' in result).toBe(false);

    // The store backend recorded no write: the projection follows Applied
    // and never Refused (model/projection.ts), so the directory stays empty.
    const store = fsModelStore(storeDir);
    expect(await projectModel(store, result)).toEqual({ kind: 'not-written', why: 'refused' });
    expect(await store.list()).toEqual([]);
    expect(readdirSync(storeDir)).toEqual([]);

    // Control: the same state under the descriptor its header names applies,
    // and that apply is what the store records.
    const control = await applyModel(encode(btDocument(btHeader)), bt, null);
    expect(control.applied).toBe(true);
    if (control.applied) {
      expect(control.binding.kind).toBe('bound');
      expect(control.doc).toEqual(btDocument(btHeader));
    }
    expect((await projectModel(store, control)).kind).toBe('written');
    expect(await store.list()).toEqual([MODEL_ID]);
  });

  it('hashes the descriptor it is given rather than trusting a label on it', async () => {
    // A descriptor that carries bt's nsURI and package but uml's classes: the
    // node claims bt, the bytes say otherwise. Comparing labels would apply it.
    const relabelled: Descriptor = { ...uml, nsURI: bt.nsURI, package: bt.package };
    const result = refused(await applyModel(encode(btDocument(btHeader)), relabelled, null));
    expect(result.binding.kind).toBe('mismatch');
    if (result.binding.kind === 'mismatch') {
      expect(result.binding.served.digest).not.toBe(btId.digest);
    }
  });

  it('a state with no header is applied under whatever descriptor is loaded, as before', async () => {
    const result = await applyModel(encode(btDocument(null)), uml, null);
    expect(result.applied).toBe(true);
    if (result.applied) {
      expect(result.binding).toEqual({ kind: 'unbound' });
      expect(result.doc).toEqual(btDocument(null));
    }
    expect((await applyModel('Unset', null, null)).applied).toBe(true);
  });

  it('a state with a header and no descriptor is applied and flagged', async () => {
    const result = await applyModel(encode(btDocument(btHeader)), null, null);
    expect(result.applied).toBe(true);
    if (result.applied) expect(result.binding.kind).toBe('no-descriptor');
  });
});

describe('mp19 the header recorded at the first apply', () => {
  it('mp19_a_remote_op_that_rewrites_the_header_is_detected_at_apply', async () => {
    const first = await applyModel(encode(btDocument(btHeader)), bt, null);
    expect(first.applied).toBe(true);
    const recorded = first.applied && first.binding.kind !== 'unbound' ? first.binding.header : null;
    expect(recorded).toEqual(btHeader);

    // A peer that ignored the rule rewrote the header to the uml pair. The
    // node is made to serve what the header now names, which is the one
    // arrangement the served-versus-header comparison alone would wave
    // through: only the header recorded at the first apply can refuse it.
    const rewritten = btDocument(umlHeader);
    const second = refused(await applyModel(encode(rewritten), uml, recorded));
    expect(second.binding.kind).toBe('header-rewritten');

    // The report names both headers.
    expect(second.message).toContain(btId.nsURI);
    expect(second.message).toContain(btId.digest);
    expect(second.message).toContain(umlId.nsURI);
    expect(second.message).toContain(umlId.digest);
    expect(second.message).toContain(MODEL_ID);

    // With the node still serving bt it is refused as well, for the same reason.
    const third = refused(await applyModel(encode(rewritten), bt, recorded));
    expect(third.binding.kind).toBe('header-rewritten');
  });

  it('a header that vanished after being recorded is refused too', async () => {
    const result = refused(await applyModel(encode(btDocument(null)), bt, btHeader));
    expect(result.binding.kind).toBe('header-rewritten');
    expect(result.message).toContain('no header at all');
  });
});

describe('the verdict as the UI reads it', () => {
  it('describes every verdict, the refusals with both pairs', () => {
    expect(describeBinding({ kind: 'unbound' })).toContain('no __model header');
    expect(describeBinding({ kind: 'bound', header: btHeader, served: btId })).toBe(
      `bound to ${btId.nsURI} (digest ${btId.digest})`,
    );
    expect(describeBinding({ kind: 'no-descriptor', header: btHeader })).toContain('no descriptor is loaded');
    const mismatch = describeBinding({ kind: 'mismatch', header: btHeader, served: umlId });
    expect(mismatch).toBe(
      `apply refused: the header binds this model to ${btId.nsURI} (digest ${btId.digest}) but the descriptor loaded for it is ${umlId.nsURI} (digest ${umlId.digest}); nothing was applied and editing is disabled`,
    );
  });

  it('labels the chip with one word per verdict', () => {
    expect(bindingLabel({ kind: 'unbound' })).toBe('unbound');
    expect(bindingLabel({ kind: 'bound', header: btHeader, served: btId })).toBe('bound');
    expect(bindingLabel({ kind: 'no-descriptor', header: btHeader })).toBe('bound, no descriptor');
    expect(bindingLabel({ kind: 'mismatch', header: btHeader, served: umlId })).toBe('not applied');
    expect(bindingLabel({ kind: 'header-rewritten', recorded: btHeader, header: umlHeader })).toBe('not applied');
  });

  it('refusalOf is the sentence the edit gate holds on, and null otherwise', () => {
    expect(refusalOf(null)).toBeNull();
    expect(refusalOf({ kind: 'unbound' })).toBeNull();
    expect(refusalOf({ kind: 'bound', header: btHeader, served: btId })).toBeNull();
    expect(refusalOf({ kind: 'mismatch', header: btHeader, served: umlId })).toContain('apply refused');
  });

  it('sameBinding tells a repeated verdict from a changed one', () => {
    expect(sameBinding({ kind: 'unbound' }, { kind: 'unbound' })).toBe(true);
    expect(
      sameBinding(
        { kind: 'bound', header: btHeader, served: btId },
        { kind: 'bound', header: { ...btHeader }, served: { ...btId } },
      ),
    ).toBe(true);
    expect(sameBinding({ kind: 'bound', header: btHeader, served: btId }, { kind: 'unbound' })).toBe(false);
    expect(
      sameBinding(
        { kind: 'mismatch', header: btHeader, served: umlId },
        { kind: 'mismatch', header: btHeader, served: btId },
      ),
    ).toBe(false);
    expect(
      sameBinding(
        { kind: 'header-rewritten', recorded: btHeader, header: umlHeader },
        { kind: 'header-rewritten', recorded: btHeader, header: null },
      ),
    ).toBe(false);
    expect(
      sameBinding({ kind: 'no-descriptor', header: btHeader }, { kind: 'no-descriptor', header: btHeader }),
    ).toBe(true);
  });
});

describe('level 4, against live nodes', () => {
  it('mp27_the_editor_refuses_to_open_a_model_under_the_wrong_descriptor', (ctx) => {
    // A node whose METAMODEL_DIR file for the recorded bt digest was edited so
    // the bytes it serves no longer hash to it; open the model there and
    // assert the tab shows both pairs, editing is disabled and the store
    // recorded nothing. Needs the multi-model sync path and its harness.
    ctx.skip('needs the multi-model UI, step 6');
  });
});
