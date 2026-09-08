/// <reference types="node" />
// The one test that reads the repository: the app's tsconfig lists no node
// types, and this directive scopes them to this file.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { canonicalJson, metamodelDigest, sha256Hex } from './digest';

/** The repository's `examples/`: the descriptors the rig image ships under `/metamodels`. */
const examples = new URL('../../../../examples/', import.meta.url);

function readExample(file: string): unknown {
  return JSON.parse(readFileSync(new URL(file, examples), 'utf8'));
}

interface DigestFixture {
  [file: string]: { nsURI: string; digest: string };
}

describe('canonicalJson', () => {
  it('sorts keys at every level and emits no whitespace', () => {
    expect(canonicalJson({ b: { z: 1, a: [true, null, 'x'] }, a: 2 })).toBe(
      '{"a":2,"b":{"a":[true,null,"x"],"z":1}}',
    );
  });

  it('escapes strings as JSON does', () => {
    expect(canonicalJson({ k: 'quote " backslash \\ newline \n tab \t é' })).toBe(
      '{"k":"quote \\" backslash \\\\ newline \\n tab \\t é"}',
    );
  });

  it('sorts keys by code point, as the node sorts UTF-8 bytes', () => {
    // U+FF5E is one UTF-16 unit above the surrogates of U+1F600, so a
    // unit-wise sort would put the emoji first; code points and UTF-8 bytes
    // both put U+FF5E first.
    expect(canonicalJson({ '\u{1F600}': 1, '\uFF5E': 2 })).toBe('{"\uFF5E":2,"\u{1F600}":1}');
  });

  it('drops undefined members, as JSON.stringify does', () => {
    expect(canonicalJson({ a: undefined, b: 1 })).toBe('{"b":1}');
  });
});

describe('sha256Hex', () => {
  it('matches the known digest of "{}"', async () => {
    expect(await sha256Hex('{}')).toBe(
      '44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a',
    );
  });
});

describe('mp6 the editor digest equals the node digest', () => {
  const fixture = readExample('fixtures/metamodel-digests.json') as DigestFixture;

  it('covers all three shipped descriptors', () => {
    expect(Object.keys(fixture).sort()).toEqual([
      'bt.metamodel.json',
      'json.metamodel.json',
      'uml.metamodel.json',
    ]);
  });

  for (const [file, expected] of Object.entries(fixture)) {
    it(`reproduces the digest Rust computed for ${file}`, async () => {
      const descriptor = readExample(file) as { nsURI: string };
      expect(descriptor.nsURI).toBe(expected.nsURI);
      expect(await metamodelDigest(descriptor)).toBe(expected.digest);
    });
  }

  it('is over the value, not the file: a reformatted copy hashes the same', async () => {
    const reformatted: unknown = JSON.parse(JSON.stringify(readExample('bt.metamodel.json'), null, 4));
    expect(await metamodelDigest(reformatted)).toBe(fixture['bt.metamodel.json'].digest);
  });

  it('changes when an attribute changes', async () => {
    const descriptor = readExample('bt.metamodel.json') as {
      classes: Record<string, { attributes: { name: string; required: boolean }[] }>;
    };
    const name = descriptor.classes['TreeNode'].attributes.find((a) => a.name === 'name');
    if (name === undefined) throw new Error('bt declares TreeNode.name');
    name.required = true;
    expect(await metamodelDigest(descriptor)).not.toBe(fixture['bt.metamodel.json'].digest);
  });
});
