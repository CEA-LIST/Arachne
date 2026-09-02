import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, getMetamodels, validateMetamodelListing } from './client';

const bt = {
  nsURI: 'http://www.example.org/behaviortree',
  package: 'behaviortree',
  digest: 'f'.repeat(64),
};

/** A fetch that answers every request with one JSON body. */
function answering(status: number, body: unknown) {
  return vi.fn(
    async () =>
      new Response(JSON.stringify(body), {
        status,
        headers: { 'Content-Type': 'application/json' },
      }),
  );
}

describe('getMetamodels', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('reads the digest beside nsURI and package', async () => {
    const fetch = answering(200, { metamodels: [bt] });
    vi.stubGlobal('fetch', fetch);
    expect(await getMetamodels('http://node:8081/')).toEqual([bt]);
    expect(fetch).toHaveBeenCalledWith('http://node:8081/api/metamodels', undefined);
  });

  it('refuses an entry without a digest', async () => {
    vi.stubGlobal('fetch', answering(200, { metamodels: [{ nsURI: bt.nsURI, package: 'x' }] }));
    await expect(getMetamodels('http://node:8081')).rejects.toBeInstanceOf(ApiError);
  });

  it('throws on a non-OK status', async () => {
    vi.stubGlobal('fetch', answering(500, { error: 'down' }));
    await expect(getMetamodels('http://node:8081')).rejects.toBeInstanceOf(ApiError);
  });
});

describe('validateMetamodelListing', () => {
  it('tolerates a missing package', () => {
    expect(validateMetamodelListing({ nsURI: 'u', digest: 'd' })).toEqual({
      nsURI: 'u',
      digest: 'd',
      package: '',
    });
  });

  it('rejects a non-object', () => {
    expect(() => validateMetamodelListing('x')).toThrow(ApiError);
  });
});
