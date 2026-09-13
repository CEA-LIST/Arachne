/**
 * The Metamodel tab's two actions, which must stay two.
 *
 * Loading a descriptor shows one in this editor and reaches nothing. Adding
 * one gives it to the replica this window is connected to, which serves it
 * from that moment. Collapsing them — or quietly turning the old picker into
 * the new one — would take away the only way to read a document against a
 * descriptor a node does not hold, so both controls are asserted here.
 *
 * The rest is the behaviour behind the button: the bytes posted are the bytes
 * read, and the line that comes back says how far the metamodel travelled.
 * That sentence is a claim about the architecture and is pinned word for
 * word, because a shorter one ("added") is exactly the sentence that makes a
 * person call the second editor window broken.
 */

import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { Descriptor, MetamodelListing } from '../api/types';
import type { MetamodelAddOutcome } from '../sync/useSync';
import { addDescriptorFile, servedHereLine } from './addMetamodel';
import { MetamodelBrowser } from './MetamodelBrowser';

const cd: MetamodelListing = {
  nsURI: 'http://www.example.org/classdiagram',
  package: 'classdiagram',
  digest: '7'.repeat(64),
};

const descriptor: Descriptor = {
  formatVersion: 2,
  package: 'classdiagram',
  nsURI: cd.nsURI,
  rootClasses: ['Class'],
  classes: {
    Class: { abstract: false, superTypes: [], attributes: [], containments: [], references: [] },
  },
  enums: {},
};

/** A descriptor file as a tool wrote it: pretty, key order its own, newline at the end. */
const FILE_TEXT = `{\n  "formatVersion": 2,\n  "nsURI": "${cd.nsURI}"\n}\n`;

function markup(metamodel: Descriptor | null, connected = true): string {
  return renderToStaticMarkup(
    <MetamodelBrowser
      metamodel={metamodel}
      source={metamodel === null ? null : 'node'}
      connected={connected}
      loadDescriptorFile={() => {}}
      addMetamodel={async () => ({ ok: true, listing: cd, added: true, detail: null })}
    />,
  );
}

describe('the Metamodel tab offers both actions, and says which is which', () => {
  it('offers adding to the replica beside loading into the editor, with a descriptor shown', () => {
    const html = markup(descriptor);
    expect(html).toContain('Add to this replica');
    expect(html).toContain('Load into this editor');
  });

  it('offers both on a replica that serves no descriptor at all, which is where a language is first given', () => {
    const html = markup(null);
    expect(html).toContain('This replica serves no metamodel');
    expect(html).toContain('Add to this replica');
    expect(html).toContain('Load into this editor');
  });

  it('offers no add control when there is no replica to add to', () => {
    const html = markup(null, false);
    expect(html).toContain('Not connected');
    expect(html).not.toContain('Add to this replica');
  });
});

describe('servedHereLine', () => {
  it('names the language, this replica, and the only route another replica has to it', () => {
    const line = servedHereLine(cd, true);
    expect(line).toBe(
      'This replica serves classdiagram now; another replica learns it only when a model written in classdiagram reaches it.',
    );
  });

  it('says a descriptor the node already held changed nothing, rather than reporting a failure', () => {
    expect(servedHereLine(cd, false)).toContain('already served classdiagram');
    expect(servedHereLine(cd, false)).toContain('nothing changed');
  });

  it('falls back to the nsURI for a descriptor that declares no package', () => {
    expect(servedHereLine({ ...cd, package: '' }, true)).toContain(cd.nsURI);
  });
});

describe('addDescriptorFile', () => {
  it("hands the file's text over byte for byte, under its own name", async () => {
    const seen: { text: string; fileName: string }[] = [];
    await addDescriptorFile(new File([FILE_TEXT], 'class_diagram.metamodel.json'), async (text, fileName) => {
      seen.push({ text, fileName });
      return { ok: true, listing: cd, added: true, detail: null };
    });
    expect(seen).toEqual([{ text: FILE_TEXT, fileName: 'class_diagram.metamodel.json' }]);
  });

  it('turns a node that took it into the one-replica line', async () => {
    const state = await addDescriptorFile(new File([FILE_TEXT], 'cd.json'), async () => ({
      ok: true,
      listing: cd,
      added: true,
      detail: null,
    }));
    expect(state).toEqual({ phase: 'served', line: servedHereLine(cd, true) });
  });

  it('turns a node that already held it into the no-op line, not a refusal', async () => {
    const state = await addDescriptorFile(new File([FILE_TEXT], 'cd.json'), async () => ({
      ok: true,
      listing: cd,
      added: false,
      detail: null,
    }));
    expect(state).toEqual({ phase: 'served', line: servedHereLine(cd, false) });
  });

  it("shows the node's own sentence when it refuses, and nothing of this editor's own", async () => {
    const why = 'not a descriptor this node can serve: not JSON: expected value at line 1 column 1';
    const refused: MetamodelAddOutcome = { ok: false, listing: null, added: false, detail: why };
    const state = await addDescriptorFile(new File(['<?xml version="1.0"?>'], 'bt.ecore'), async () => refused);
    expect(state).toEqual({ phase: 'refused', detail: why });
  });

  it('still says something when the node refuses and says nothing at all', async () => {
    const state = await addDescriptorFile(new File([FILE_TEXT], 'cd.json'), async () => ({
      ok: false,
      listing: null,
      added: false,
      detail: null,
    }));
    expect(state).toMatchObject({ phase: 'refused' });
    expect((state as { detail: string }).detail.length).toBeGreaterThan(0);
  });

  it('reports a file it could not read without posting anything', async () => {
    let posted = false;
    const unreadable = {
      name: 'gone.json',
      text: () => Promise.reject(new Error('NotReadableError')),
    } as unknown as File;
    const state = await addDescriptorFile(unreadable, async () => {
      posted = true;
      return { ok: true, listing: cd, added: true, detail: null };
    });
    expect(posted).toBe(false);
    expect(state).toEqual({ phase: 'refused', detail: 'could not read gone.json: NotReadableError' });
  });
});
