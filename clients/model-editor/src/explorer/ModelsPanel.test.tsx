/**
 * The hosted list carries the whole model id.
 *
 * Not a cosmetic claim. A model created on one replica is never listed on the
 * other — there is no cluster-wide catalog — so the only way to open it in a
 * second editor is to move its 32 characters into the Join by id form by
 * hand. Truncated to eight and a tooltip, that made the panel's central
 * workflow unusable, and a regression here would look like a styling detail.
 *
 * Rendered to static markup rather than driven: this asserts what the row
 * says, which is the whole of the claim, and needs no DOM to do it.
 */

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { HostedModel, MetamodelListing, ModelId } from '../api/types';
import { joinTarget } from './joinTarget';
import { ModelsPanel } from './ModelsPanel';

const ID = 'a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4';
const OTHER = 'ffffffffffffffffffffffffffffffff';
const btListing: MetamodelListing = {
  nsURI: 'http://www.example.org/behaviortree',
  package: 'behaviortree',
  digest: 'f'.repeat(64),
};

function markup(
  hosted: HostedModel[],
  metamodels: MetamodelListing[] = [btListing],
  seen: ModelId[] = [],
): string {
  return renderToStaticMarkup(
    <ModelsPanel
      connected
      hosted={hosted}
      seen={seen}
      metamodels={metamodels}
      openIds={[]}
      selected={null}
      onRefresh={() => {}}
      onCreate={() => {}}
      onJoin={() => {}}
      onOpen={() => {}}
    />,
  );
}

describe('the hosted model list', () => {
  const hosted: HostedModel[] = [
    { modelId: ID, metamodelId: { nsURI: btListing.nsURI, digest: btListing.digest } },
    { modelId: OTHER, metamodelId: null },
  ];

  it('prints every id in full, so it can be selected and pasted into another editor', () => {
    const html = markup(hosted);
    expect(html).toContain(`>${ID}<`);
    expect(html).toContain(`>${OTHER}<`);
    // And not the truncation that made it unusable.
    expect(html).not.toContain('a1b2c3d4…');
  });

  it('gives the id a class of its own, which is what selects it alone and puts it on its own line', () => {
    expect(markup(hosted)).toContain('me-mono me-models__id');
  });

  it('keeps the row addressable and the open control named by the id it opens', () => {
    const html = markup(hosted);
    expect(html).toContain(`data-model-id="${ID}"`);
    expect(html).toContain(`aria-label="Open ${ID}"`);
  });

  it('says nothing about a `__model` header, which an interpreted node does not write', () => {
    expect(markup(hosted)).not.toContain('__model');
  });

  it('still says the node hosts none when it hosts none', () => {
    expect(markup([])).toContain('This node hosts no model yet.');
  });
});

/**
 * The seen list is traffic, not a catalog, and the panel has to say so. A
 * reader who knows a model exists and does not find it here must be told why
 * before they go looking for a bug, so the wording is part of the contract
 * and is asserted here beside the ids themselves.
 */
describe('the seen-but-not-hosted list', () => {
  const hosted: HostedModel[] = [{ modelId: ID, metamodelId: null }];

  it('lists a seen id in full, with a control named by the id it joins', () => {
    const html = markup(hosted, [btListing], [OTHER]);
    expect(html).toContain(`data-seen-id="${OTHER}"`);
    expect(html).toContain(`aria-label="Join ${OTHER}"`);
    expect(html).toContain(`>${OTHER}<`);
  });

  it('says the list is what was heard since connecting, never that it is the session', () => {
    const html = markup(hosted, [btListing], [OTHER]);
    expect(html).toContain('Seen since connecting');
    expect(html).toContain('Not a list of the session');
    expect(html).not.toContain('all models in the session');
  });

  it('says nothing was seen rather than showing an empty list', () => {
    const html = markup(hosted);
    expect(html).toContain('Nothing seen yet.');
    expect(html).not.toContain('data-seen-id');
  });

  it('keeps the Join by id form, because an idle model never appears in the seen list', () => {
    expect(markup(hosted, [btListing], [OTHER])).toContain('aria-label="Join model by id"');
  });
});

/**
 * The Join by id form names the metamodel two ways, and the second one is the
 * only way a replica can join a model written in a language it has never
 * seen.
 *
 * The dropdown is fed by this window's own `GET /api/metamodels`. A digest
 * the replica does not hold cannot be chosen from it, so before this there
 * was no wording of the join that the form would send — while the node
 * accepts exactly that join, hosts the model with its binding pending, and
 * takes the descriptor out of the model's own history when it arrives. The
 * missing control, not a missing capability, was what blocked it.
 *
 * Both halves are asserted: the markup, because the dropdown path must keep
 * working beside the new field, and `joinTarget`, because what gets sent to
 * the node is decided there and a shape check that hardened into a check
 * against the local listing would refuse the one case this exists for.
 */
describe('the Join by id form, and naming a metamodel this replica does not hold', () => {
  const hosted: HostedModel[] = [{ modelId: ID, metamodelId: null }];

  it('keeps the dropdown, and offers a digest field beside it', () => {
    const html = markup(hosted);
    expect(html).toContain('aria-label="Metamodel of the model to join"');
    expect(html).toContain('aria-label="Metamodel digest"');
  });

  it('lets the two ways be chosen, with the dropdown the one in force to begin with', () => {
    const html = markup(hosted);
    expect(html).toContain('name="join-bound-to"');
    expect(html).toContain('checked="" value="held"');
    expect(html).toContain('value="digest"');
  });

  it('says why a digest would ever be typed, since late binding is not guessable', () => {
    const html = markup(hosted);
    expect(html).toContain('A model can be joined under a language this replica does not hold yet');
    expect(html).toContain('the language arrives with the model');
  });

  it('offers the join on a replica holding no descriptor at all, which the dropdown alone cannot', () => {
    // Create stays disabled — there is nothing to write an opening operation
    // from — and Join must not be, or the digest field is unreachable on the
    // one replica that most needs it.
    const html = markup(hosted, []);
    expect(html).toContain('no descriptor on this node');
    expect(html).toContain('disabled="">Create model</button>');
    expect(html).toContain('class="me-btn">Join model</button>');
  });
});

describe('joinTarget', () => {
  const listing = [btListing];

  it('takes the dropdown’s digest from the listing, nsURI and all', () => {
    expect(joinTarget({ source: 'held', digest: btListing.digest }, listing)).toEqual({
      ok: true,
      metamodelId: { nsURI: btListing.nsURI, digest: btListing.digest },
    });
  });

  it('reads an empty dropdown value as the first entry, which is what the select shows', () => {
    expect(joinTarget({ source: 'held', digest: '' }, listing)).toEqual({
      ok: true,
      metamodelId: { nsURI: btListing.nsURI, digest: btListing.digest },
    });
  });

  it('has nothing to send when the replica lists nothing and the dropdown is the source', () => {
    expect(joinTarget({ source: 'held', digest: '' }, [])).toEqual({
      ok: false,
      error: 'choose the metamodel the model is bound to',
    });
  });

  it('sends a typed digest the replica does not hold, with no nsURI to claim', () => {
    const unheld = '2'.repeat(64);
    expect(joinTarget({ source: 'digest', digest: unheld }, listing)).toEqual({
      ok: true,
      metamodelId: { nsURI: '', digest: unheld },
    });
  });

  it('names the language when the typed digest happens to be one the replica holds', () => {
    expect(joinTarget({ source: 'digest', digest: btListing.digest }, listing)).toEqual({
      ok: true,
      metamodelId: { nsURI: btListing.nsURI, digest: btListing.digest },
    });
  });

  it('trims what was pasted, since a copied digest carries whitespace as often as not', () => {
    expect(joinTarget({ source: 'digest', digest: `  ${btListing.digest}\n` }, listing)).toEqual({
      ok: true,
      metamodelId: { nsURI: btListing.nsURI, digest: btListing.digest },
    });
  });

  it('refuses anything that is not a digest before the node is asked', () => {
    const refusal = { ok: false, error: 'a metamodel digest is 64 lowercase hex characters' };
    expect(joinTarget({ source: 'digest', digest: '' }, listing)).toEqual(refusal);
    expect(joinTarget({ source: 'digest', digest: 'f'.repeat(63) }, listing)).toEqual(refusal);
    expect(joinTarget({ source: 'digest', digest: 'f'.repeat(65) }, listing)).toEqual(refusal);
    // Uppercase is the near miss, and the node lists lowercase only.
    expect(joinTarget({ source: 'digest', digest: 'F'.repeat(64) }, listing)).toEqual(refusal);
    expect(joinTarget({ source: 'digest', digest: 'http://library.org' }, listing)).toEqual(refusal);
  });
});
