import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  ApiError,
  getMetamodels,
  getModelMetamodel,
  getModelState,
  validateMetamodelListing,
} from './client';

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

const ID = 'a1b2c3d4a1b2c3d4a1b2c3d4a1b2c3d4';

describe('the model-scoped routes', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('getModelState reads the {"json": ...} envelope from /api/model/{id}/state', async () => {
    const fetch = answering(200, { json: 'Unset' });
    vi.stubGlobal('fetch', fetch);
    expect(await getModelState('http://node:8081', ID)).toBe('Unset');
    expect(fetch).toHaveBeenCalledWith(`http://node:8081/api/model/${ID}/state`, undefined);
  });

  it("getModelState throws with the status and the node's error text on a malformed id", async () => {
    vi.stubGlobal('fetch', answering(400, { error: 'log id must be 32 lowercase hex characters' }));
    const err = await getModelState('http://node:8081', 'zz').catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).status).toBe(400);
    expect((err as ApiError).message).toContain('32 lowercase hex');
  });

  it('getModelState throws on an id the node does not host', async () => {
    vi.stubGlobal('fetch', answering(404, { error: 'not found' }));
    await expect(getModelState('http://node:8081', ID)).rejects.toMatchObject({ status: 404 });
  });

  it('getModelMetamodel returns the validated descriptor from /api/model/{id}/metamodel', async () => {
    const descriptor = { formatVersion: 1, package: 'p', nsURI: 'u', rootClasses: [], classes: {}, enums: {} };
    const fetch = answering(200, descriptor);
    vi.stubGlobal('fetch', fetch);
    expect(await getModelMetamodel('http://node:8081/', ID)).toEqual(descriptor);
    expect(fetch).toHaveBeenCalledWith(`http://node:8081/api/model/${ID}/metamodel`, undefined);
  });

  it('getModelMetamodel is null on 404 and throws on any other failure', async () => {
    vi.stubGlobal('fetch', answering(404, { error: 'not found' }));
    expect(await getModelMetamodel('http://node:8081', ID)).toBeNull();
    vi.stubGlobal('fetch', answering(500, { error: 'down' }));
    await expect(getModelMetamodel('http://node:8081', ID)).rejects.toBeInstanceOf(ApiError);
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
