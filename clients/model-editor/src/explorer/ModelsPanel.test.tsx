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
