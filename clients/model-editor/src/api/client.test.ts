import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ModelOp } from './types';
import {
  ApiError,
  getMetamodels,
  getModelMetamodel,
  getModels,
  getModelState,
  postModelOp,
  registerModel,
  stringifyModelOp,
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

  it('getModelState serves the document itself, `null` for a model with no root', async () => {
    const doc = { eClass: 'Root', behaviortrees: [] };
    vi.stubGlobal('fetch', answering(200, null));
    expect(await getModelState('http://node:8081', ID)).toBeNull();
    const fetch = answering(200, doc);
    vi.stubGlobal('fetch', fetch);
    expect(await getModelState('http://node:8081', ID)).toEqual(doc);
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
    const descriptor = { formatVersion: 2, package: 'p', nsURI: 'u', rootClasses: [], classes: {}, enums: {} };
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

/* ---------- the registration and listing routes (mp29's client half) ---------- */

function capturing(status: number, body: unknown) {
  const calls: { url: string; init: RequestInit | undefined }[] = [];
  const fetch = vi.fn(async (url: string, init?: RequestInit) => {
    calls.push({ url, init });
    return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });
  });
  return { fetch, calls };
}

describe('registerModel', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('creates by posting the metamodel alone and takes the id the node minted, never one of its own', async () => {
    const { fetch, calls } = capturing(201, { model_id: ID, metamodel_id: bt, created: true });
    vi.stubGlobal('fetch', fetch);
    const registered = await registerModel('http://node:8081', { metamodelId: { nsURI: bt.nsURI, digest: bt.digest } });
    expect(registered).toEqual({ modelId: ID, metamodelId: { nsURI: bt.nsURI, digest: bt.digest }, created: true });
    expect(calls[0].url).toBe('http://node:8081/api/models');
    expect(calls[0].init?.method).toBe('POST');
    const body = JSON.parse(String(calls[0].init?.body)) as Record<string, unknown>;
    expect(Object.keys(body)).toEqual(['metamodel_id']);
    expect(body['metamodel_id']).toEqual({ nsURI: bt.nsURI, digest: bt.digest });
  });

  it('joins by posting the id beside the metamodel, and reads created:false back', async () => {
    const { fetch, calls } = capturing(200, { model_id: ID, metamodel_id: bt, created: false });
    vi.stubGlobal('fetch', fetch);
    const registered = await registerModel('http://node:8081/', {
      modelId: ID,
      metamodelId: { nsURI: bt.nsURI, digest: bt.digest },
    });
    expect(registered.created).toBe(false);
    expect(registered.modelId).toBe(ID);
    const body = JSON.parse(String(calls[0].init?.body)) as Record<string, unknown>;
    expect(body).toEqual({ metamodel_id: { nsURI: bt.nsURI, digest: bt.digest }, model_id: ID });
  });

  it("carries the node's refusals as ApiErrors with the status and its sentence: 409 hosted, 422 unknown metamodel", async () => {
    vi.stubGlobal('fetch', answering(409, { error: `model ${ID} is already hosted` }));
    const dup = await registerModel('http://node:8081', { modelId: ID, metamodelId: bt }).catch((e: unknown) => e);
    expect(dup).toBeInstanceOf(ApiError);
    expect((dup as ApiError).status).toBe(409);
    expect((dup as ApiError).message).toContain('already hosted');
    vi.stubGlobal('fetch', answering(422, { error: 'no descriptor with that digest' }));
    const unknown = await registerModel('http://node:8081', { metamodelId: bt }).catch((e: unknown) => e);
    expect((unknown as ApiError).status).toBe(422);
    expect((unknown as ApiError).message).toContain('no descriptor');
  });

  it('refuses a reply without a model id', async () => {
    vi.stubGlobal('fetch', answering(201, { created: true }));
    await expect(registerModel('http://node:8081', { metamodelId: bt })).rejects.toBeInstanceOf(ApiError);
  });
});

describe('getModels', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('lists the hosted models, the default log with a null metamodel and a bare digest as a pair with no nsURI', async () => {
    const other = 'c3d4e5f6c3d4e5f6c3d4e5f6c3d4e5f6';
    const dflt = 'c0113c7ed10c0113c7ed10c0113c7ed1';
    const { fetch, calls } = capturing(200, {
      models: [
        { model_id: dflt, metamodel_id: null },
        { model_id: ID, metamodel_id: { nsURI: bt.nsURI, digest: bt.digest } },
        { model_id: other, metamodel_id: bt.digest },
      ],
    });
    vi.stubGlobal('fetch', fetch);
    expect(await getModels('http://node:8081')).toEqual([
      { modelId: dflt, metamodelId: null },
      { modelId: ID, metamodelId: { nsURI: bt.nsURI, digest: bt.digest } },
      { modelId: other, metamodelId: { nsURI: '', digest: bt.digest } },
    ]);
    expect(calls[0].url).toBe('http://node:8081/api/models');
  });

  it('throws on a body without a models array and on a non-OK status', async () => {
    vi.stubGlobal('fetch', answering(200, { hosted: [] }));
    await expect(getModels('http://node:8081')).rejects.toBeInstanceOf(ApiError);
    vi.stubGlobal('fetch', answering(500, { error: 'down' }));
    await expect(getModels('http://node:8081')).rejects.toMatchObject({ status: 500 });
  });
});

describe('postModelOp', () => {
  afterEach(() => vi.unstubAllGlobals());

  it("posts the operation itself to the model's own op route and returns the node's verdict", async () => {
    const { fetch, calls } = capturing(200, { success: false, message: 'operation not enabled' });
    vi.stubGlobal('fetch', fetch);
    // An interpreted node reads the body as a `ModelOp`: `Install` or
    // `Instance`, with nothing wrapping it. A `{"JsonKind": …}` envelope is
    // the 400 this editor was answering before.
    const op: ModelOp = { Instance: { Variant: [17, 'New'] } };
    expect(await postModelOp('http://node:8081', ID, op)).toEqual({ success: false, message: 'operation not enabled' });
    expect(calls[0].url).toBe(`http://node:8081/api/model/${ID}/op`);
    expect(JSON.parse(String(calls[0].init?.body))).toEqual(op);
  });

  it('writes a float scalar as the unquoted integer serde reads back as a u64', () => {
    // `Scalar::Float` carries the bit pattern, which does not fit a
    // JavaScript number: 3.5 is 0x400C000000000000.
    const op: ModelOp = { Instance: { Leaf: { Write: { Float: 0x400c000000000000n } } } };
    expect(stringifyModelOp(op)).toBe('{"Instance":{"Leaf":{"Write":{"Float":4615063718147915776}}}}');
  });

  it('throws with the status on an id the node does not host', async () => {
    vi.stubGlobal('fetch', answering(404, { error: 'not hosted' }));
    await expect(
      postModelOp('http://node:8081', ID, { Instance: 'New' }),
    ).rejects.toMatchObject({ status: 404 });
  });
});
